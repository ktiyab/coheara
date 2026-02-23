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

/// Grant a non-managed profile access to another profile.
///
/// Used when a caregiver wants to share their medical data with a trusted
/// family member's profile. Writes to `profile_access_grants` in `app.db`.
#[tauri::command]
pub fn grant_profile_access(
    granter_profile_id: String,
    grantee_profile_id: String,
    access_level: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
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
#[tauri::command]
pub fn revoke_profile_access(
    granter_profile_id: String,
    grantee_profile_id: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<bool, String> {
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
