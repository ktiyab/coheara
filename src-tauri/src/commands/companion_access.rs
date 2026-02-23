//! E6: Companion access IPC commands.
//!
//! Desktop commands for managing which profiles are unlocked for companion access.
//! Uses SessionCache to hold multiple profile keys in memory.

use std::sync::Arc;

use tauri::State;
use uuid::Uuid;

use crate::core_state::CoreState;
use crate::crypto::profile;
use crate::db::repository::device_registry;
use crate::session_cache::CachedSession;

/// Info about a cached (unlocked) profile for companion access.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CompanionProfileInfo {
    pub profile_id: String,
    pub profile_name: String,
    pub is_active: bool,
}

/// Unlock a managed profile for companion access without switching the desktop UI.
///
/// The caregiver enters the managed profile's password; the key is cached in
/// SessionCache so the companion phone can query that profile's data.
/// Runs on a blocking thread (Argon2 KDF is CPU-heavy).
#[tauri::command]
pub async fn unlock_for_companion(
    profile_id: String,
    password: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<CompanionProfileInfo, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let id = Uuid::parse_str(&profile_id).map_err(|e| format!("Invalid profile ID: {e}"))?;

        // Open the profile (validates password, derives key)
        let session =
            profile::open_profile(&state.profiles_dir, &id, &password).map_err(|e| e.to_string())?;

        // Cache the session without switching the active profile
        let cached = CachedSession::new(
            session.profile_id,
            session.profile_name.clone(),
            *session.key_bytes(),
            session.db_path().to_path_buf(),
        );
        let profile_name = session.profile_name.clone();

        state
            .cache_profile_session(cached)
            .map_err(|e| e.to_string())?;

        state.log_access(
            crate::core_state::AccessSource::DesktopUi,
            "unlock_for_companion",
            &profile_id,
        );

        // Check if this is the active profile
        let is_active = state
            .read_session()
            .ok()
            .and_then(|guard| guard.as_ref().map(|s| s.profile_id == id))
            .unwrap_or(false);

        Ok(CompanionProfileInfo {
            profile_id,
            profile_name,
            is_active,
        })
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

/// Revoke companion access to a specific profile.
///
/// Evicts the profile's key from SessionCache (zeroed via Drop).
/// The companion phone will get 403 on subsequent requests to that profile.
#[tauri::command]
pub fn revoke_companion_access(
    profile_id: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    let id = Uuid::parse_str(&profile_id).map_err(|e| format!("Invalid profile ID: {e}"))?;

    state
        .evict_cached_session(&id)
        .map_err(|e| e.to_string())?;

    state.log_access(
        crate::core_state::AccessSource::DesktopUi,
        "revoke_companion_access",
        &profile_id,
    );

    Ok(())
}

/// List all profiles currently unlocked for companion access.
///
/// Returns cached profile IDs with names and active status.
#[tauri::command]
pub fn list_companion_profiles(
    state: State<'_, Arc<CoreState>>,
) -> Result<Vec<CompanionProfileInfo>, String> {
    let active_id = state
        .read_session()
        .ok()
        .and_then(|guard| guard.as_ref().map(|s| s.profile_id));

    let cached = state.cached_profile_names().map_err(|e| e.to_string())?;

    let result = cached
        .into_iter()
        .map(|(id, name)| CompanionProfileInfo {
            profile_id: id.to_string(),
            profile_name: name,
            is_active: active_id == Some(id),
        })
        .collect();

    Ok(result)
}

/// Verify the active profile matches the claimed granter (prevents impersonation).
fn verify_caller_is_granter(
    state: &CoreState,
    granter_profile_id: &str,
) -> Result<(), String> {
    let guard = state.read_session().map_err(|e| e.to_string())?;
    let session = guard.as_ref().ok_or("No active session")?;
    if session.profile_id.to_string() != granter_profile_id {
        return Err("Can only manage grants for your own profile".into());
    }
    Ok(())
}

/// Grant a non-managed profile access to another profile.
///
/// Used when a caregiver wants to share their medical data with a trusted
/// family member's profile. Writes to `profile_access_grants` in `app.db`.
///
/// **Security**: Validates the caller is the granter (prevents cross-profile impersonation).
#[tauri::command]
pub fn grant_profile_access(
    granter_profile_id: String,
    grantee_profile_id: String,
    access_level: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    // Security: verify caller is the granter
    verify_caller_is_granter(&state, &granter_profile_id)?;

    // Validate access level
    if access_level != "full" && access_level != "read_only" {
        return Err("Invalid access level: must be 'full' or 'read_only'".into());
    }

    let app_conn = state.open_app_db().map_err(|e| e.to_string())?;

    let grant = device_registry::ProfileAccessGrantRow {
        id: Uuid::new_v4().to_string(),
        granter_profile_id: granter_profile_id.clone(),
        grantee_profile_id: grantee_profile_id.clone(),
        access_level,
        granted_at: chrono::Utc::now().to_rfc3339(),
        revoked_at: None,
    };

    device_registry::insert_profile_grant(&app_conn, &grant).map_err(|e| e.to_string())?;

    state.log_access(
        crate::core_state::AccessSource::DesktopUi,
        "grant_profile_access",
        &format!("{granter_profile_id} → {grantee_profile_id}"),
    );

    Ok(())
}

/// Revoke a previously granted profile access.
///
/// Sets `revoked_at` on the grant row in `app.db`.
///
/// **Security**: Validates the caller is the granter (prevents cross-profile impersonation).
#[tauri::command]
pub fn revoke_profile_access(
    granter_profile_id: String,
    grantee_profile_id: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<bool, String> {
    // Security: verify caller is the granter
    verify_caller_is_granter(&state, &granter_profile_id)?;

    let app_conn = state.open_app_db().map_err(|e| e.to_string())?;

    let revoked = device_registry::revoke_profile_grant(
        &app_conn,
        &granter_profile_id,
        &grantee_profile_id,
    )
    .map_err(|e| e.to_string())?;

    if revoked {
        state.log_access(
            crate::core_state::AccessSource::DesktopUi,
            "revoke_profile_access",
            &format!("{granter_profile_id} → {grantee_profile_id}"),
        );
    }

    Ok(revoked)
}

// ═══════════════════════════════════════════════════════════
// MP-02: Enriched grant queries for Privacy UI
// ═══════════════════════════════════════════════════════════

/// A profile access grant enriched with human-readable profile names.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EnrichedGrant {
    pub id: String,
    pub granter_profile_id: String,
    pub grantee_profile_id: String,
    pub granter_name: String,
    pub grantee_name: String,
    pub access_level: String,
    pub granted_at: String,
}

/// Resolve a profile ID to its display name from disk metadata.
fn resolve_profile_name(state: &CoreState, profile_id: &str) -> String {
    profile::list_profiles(&state.profiles_dir)
        .ok()
        .and_then(|profiles| {
            profiles
                .into_iter()
                .find(|p| p.id.to_string() == profile_id)
                .map(|p| p.name)
        })
        .unwrap_or_else(|| profile_id.to_string())
}

/// List all active grants where the active profile is the granter.
///
/// Returns enriched grants with human-readable profile names for the UI.
#[tauri::command]
pub fn list_my_grants(
    state: State<'_, Arc<CoreState>>,
) -> Result<Vec<EnrichedGrant>, String> {
    let active_id = {
        let guard = state.read_session().map_err(|e| e.to_string())?;
        let session = guard.as_ref().ok_or("No active session")?;
        session.profile_id.to_string()
    };

    let app_conn = state.open_app_db().map_err(|e| e.to_string())?;
    let grants = device_registry::list_grants_for_granter(&app_conn, &active_id)
        .map_err(|e| e.to_string())?;

    let enriched = grants
        .into_iter()
        .map(|g| EnrichedGrant {
            id: g.id,
            granter_name: resolve_profile_name(&state, &g.granter_profile_id),
            grantee_name: resolve_profile_name(&state, &g.grantee_profile_id),
            granter_profile_id: g.granter_profile_id,
            grantee_profile_id: g.grantee_profile_id,
            access_level: g.access_level,
            granted_at: g.granted_at,
        })
        .collect();

    Ok(enriched)
}

/// List all active grants where the active profile is the grantee.
///
/// Returns enriched grants with human-readable profile names for the UI.
#[tauri::command]
pub fn list_grants_to_me(
    state: State<'_, Arc<CoreState>>,
) -> Result<Vec<EnrichedGrant>, String> {
    let active_id = {
        let guard = state.read_session().map_err(|e| e.to_string())?;
        let session = guard.as_ref().ok_or("No active session")?;
        session.profile_id.to_string()
    };

    let app_conn = state.open_app_db().map_err(|e| e.to_string())?;
    let grants = device_registry::list_grants_for_grantee(&app_conn, &active_id)
        .map_err(|e| e.to_string())?;

    let enriched = grants
        .into_iter()
        .map(|g| EnrichedGrant {
            id: g.id,
            granter_name: resolve_profile_name(&state, &g.granter_profile_id),
            grantee_name: resolve_profile_name(&state, &g.grantee_profile_id),
            granter_profile_id: g.granter_profile_id,
            grantee_profile_id: g.grantee_profile_id,
            access_level: g.access_level,
            granted_at: g.granted_at,
        })
        .collect();

    Ok(enriched)
}
