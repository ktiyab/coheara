use serde::Serialize;
use tauri::State;
use uuid::Uuid;

use crate::crypto::profile::{self, ProfileInfo};

use super::state::AppState;

/// Serializable result for profile creation (includes recovery phrase).
#[derive(Serialize)]
pub struct ProfileCreateResult {
    pub profile: ProfileInfo,
    pub recovery_phrase: Vec<String>,
}

/// List all available profiles.
#[tauri::command]
pub fn list_profiles(state: State<'_, AppState>) -> Result<Vec<ProfileInfo>, String> {
    profile::list_profiles(&state.profiles_dir).map_err(|e| e.to_string())
}

/// Create a new profile, auto-open session, return info + recovery phrase.
#[tauri::command]
pub fn create_profile(
    name: String,
    password: String,
    managed_by: Option<String>,
    state: State<'_, AppState>,
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

    let mut active = state
        .active_session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;
    *active = Some(session);
    state.update_activity();

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
    state: State<'_, AppState>,
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

    let mut active = state
        .active_session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;
    *active = Some(session);
    state.update_activity();

    Ok(info)
}

/// Lock the current profile (zeroes encryption key).
#[tauri::command]
pub fn lock_profile(state: State<'_, AppState>) {
    state.lock();
}

/// Recover a profile using BIP39 recovery phrase.
#[tauri::command]
pub fn recover_profile(
    profile_id: String,
    recovery_phrase: String,
    _new_password: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let id = Uuid::parse_str(&profile_id).map_err(|e| format!("Invalid profile ID: {e}"))?;

    let session = profile::recover_profile(&state.profiles_dir, &id, &recovery_phrase)
        .map_err(|e| e.to_string())?;

    let mut active = state
        .active_session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;
    *active = Some(session);
    state.update_activity();

    Ok(())
}

/// Check if a profile session is currently active.
#[tauri::command]
pub fn is_profile_active(state: State<'_, AppState>) -> bool {
    !state.is_locked()
}

/// Delete a profile and all its data (cryptographic erasure).
#[tauri::command]
pub fn delete_profile(profile_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let id = Uuid::parse_str(&profile_id).map_err(|e| format!("Invalid profile ID: {e}"))?;

    // Lock if deleting the currently active profile
    let active_id = state
        .active_session
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|s| s.profile_id));
    if active_id == Some(id) {
        state.lock();
    }

    profile::delete_profile(&state.profiles_dir, &id).map_err(|e| e.to_string())
}

/// Check for inactivity timeout — called periodically from frontend.
/// Returns true if the profile was locked due to inactivity.
#[tauri::command]
pub fn check_inactivity(state: State<'_, AppState>) -> bool {
    if state.check_timeout() {
        state.lock();
        true
    } else {
        false
    }
}

/// Update last activity timestamp — called on user interaction.
#[tauri::command]
pub fn update_activity(state: State<'_, AppState>) {
    state.update_activity();
}
