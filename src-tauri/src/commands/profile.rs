use std::sync::Arc;

use serde::Serialize;
use tauri::State;
use uuid::Uuid;

use crate::core_state::CoreState;
use crate::crypto::profile::{self, ProfileInfo};
use crate::device_manager::WsOutgoing;

/// Serializable result for profile creation (includes recovery phrase).
#[derive(Serialize)]
pub struct ProfileCreateResult {
    pub profile: ProfileInfo,
    pub recovery_phrase: Vec<String>,
}

/// List all available profiles.
#[tauri::command]
pub fn list_profiles(state: State<'_, Arc<CoreState>>) -> Result<Vec<ProfileInfo>, String> {
    profile::list_profiles(&state.profiles_dir).map_err(|e| e.to_string())
}

/// Create a new profile, auto-open session, return info + recovery phrase.
#[tauri::command]
pub fn create_profile(
    name: String,
    password: String,
    managed_by: Option<String>,
    state: State<'_, Arc<CoreState>>,
) -> Result<ProfileCreateResult, String> {
    let (info, phrase) = profile::create_profile(
        &state.profiles_dir,
        &name,
        &password,
        managed_by.as_deref(),
    )
    .map_err(|e| e.to_string())?;

    // Auto-open the newly created profile
    let session = profile::open_profile(&state.profiles_dir, &info.id, &password)
        .map_err(|e| e.to_string())?;

    state.set_session(session).map_err(|e| e.to_string())?;
    state.update_activity();

    // Notify connected phones about profile change (RS-M0-03-003)
    if let Ok(mut devices) = state.write_devices() {
        devices.broadcast(WsOutgoing::ProfileChanged {
            profile_name: info.name.clone(),
        });
    }

    Ok(ProfileCreateResult {
        profile: info,
        recovery_phrase: phrase.words().iter().map(|w| w.to_string()).collect(),
    })
}

/// Unlock a profile with password.
#[tauri::command]
pub fn unlock_profile(
    profile_id: String,
    password: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<ProfileInfo, String> {
    let id = Uuid::parse_str(&profile_id).map_err(|e| format!("Invalid profile ID: {e}"))?;

    let session =
        profile::open_profile(&state.profiles_dir, &id, &password).map_err(|e| e.to_string())?;

    let info = ProfileInfo {
        id: session.profile_id,
        name: session.profile_name.clone(),
        created_at: chrono::Local::now().naive_local(),
        managed_by: None,
        password_hint: None,
    };

    state.set_session(session).map_err(|e| e.to_string())?;
    state.update_activity();

    // Hydrate paired devices from DB so they survive app restarts (RS-ME-02-001)
    if let Err(e) = state.hydrate_devices() {
        tracing::warn!("Failed to hydrate devices: {e}");
    }

    // Notify connected phones about profile change (RS-M0-03-003)
    if let Ok(mut devices) = state.write_devices() {
        devices.broadcast(WsOutgoing::ProfileChanged {
            profile_name: info.name.clone(),
        });
    }

    Ok(info)
}

/// Change the password for the currently active profile (RS-L3-01-001).
#[tauri::command]
pub fn change_profile_password(
    current_password: String,
    new_password: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    let guard = state.read_session().map_err(|e| e.to_string())?;
    let session = guard.as_ref().ok_or("No active profile session")?;

    profile::change_password(
        &state.profiles_dir,
        &session.profile_id,
        &current_password,
        &new_password,
        session.key_bytes(),
    )
    .map_err(|e| e.to_string())
}

/// Lock the current profile (zeroes encryption key).
#[tauri::command]
pub fn lock_profile(state: State<'_, Arc<CoreState>>) {
    // Flush audit buffer before clearing session (RS-ME-01-001)
    if let Err(e) = state.flush_and_prune_audit() {
        tracing::warn!("Failed to flush audit log on lock: {e}");
    }
    state.lock();
}

/// Recover a profile using BIP39 recovery phrase.
#[tauri::command]
pub fn recover_profile(
    profile_id: String,
    recovery_phrase: String,
    _new_password: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    let id = Uuid::parse_str(&profile_id).map_err(|e| format!("Invalid profile ID: {e}"))?;

    let session = profile::recover_profile(&state.profiles_dir, &id, &recovery_phrase)
        .map_err(|e| e.to_string())?;

    let profile_name = session.profile_name.clone();
    state.set_session(session).map_err(|e| e.to_string())?;
    state.update_activity();

    // Hydrate paired devices from DB so they survive app restarts (RS-ME-02-001)
    if let Err(e) = state.hydrate_devices() {
        tracing::warn!("Failed to hydrate devices: {e}");
    }

    // Notify connected phones about profile change (RS-M0-03-003)
    if let Ok(mut devices) = state.write_devices() {
        devices.broadcast(WsOutgoing::ProfileChanged { profile_name });
    }

    Ok(())
}

/// Check if a profile session is currently active.
#[tauri::command]
pub fn is_profile_active(state: State<'_, Arc<CoreState>>) -> bool {
    !state.is_locked()
}

/// Get the active profile's display name.
#[tauri::command]
pub fn get_active_profile_name(state: State<'_, Arc<CoreState>>) -> Result<String, String> {
    let guard = state.read_session().map_err(|e| e.to_string())?;
    let session = guard.as_ref().ok_or("No active session")?;
    Ok(session.profile_name.clone())
}

/// Delete a profile and all its data (cryptographic erasure).
#[tauri::command]
pub fn delete_profile(profile_id: String, state: State<'_, Arc<CoreState>>) -> Result<(), String> {
    let id = Uuid::parse_str(&profile_id).map_err(|e| format!("Invalid profile ID: {e}"))?;

    // Lock if deleting the currently active profile
    let active_id = state
        .read_session()
        .ok()
        .and_then(|guard| guard.as_ref().map(|s| s.profile_id));
    if active_id == Some(id) {
        state.lock();
    }

    profile::delete_profile(&state.profiles_dir, &id).map_err(|e| e.to_string())
}

/// Check for inactivity timeout — called periodically from frontend.
/// Returns true if the profile was locked due to inactivity.
/// IMP-002: Flushes audit buffer before locking to prevent data loss.
#[tauri::command]
pub fn check_inactivity(state: State<'_, Arc<CoreState>>) -> bool {
    if state.check_timeout() {
        if let Err(e) = state.flush_and_prune_audit() {
            tracing::warn!("Failed to flush audit log on auto-lock: {e}");
        }
        state.lock();
        true
    } else {
        false
    }
}

/// Update last activity timestamp — called on user interaction.
#[tauri::command]
pub fn update_activity(state: State<'_, Arc<CoreState>>) {
    state.update_activity();
}
