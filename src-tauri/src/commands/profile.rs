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

/// Create a new profile, optionally auto-open session, return info + recovery phrase.
/// Runs on a blocking thread to avoid freezing the UI (Argon2 KDF is CPU-heavy).
/// F7: `auto_open` defaults to true (backward-compatible). Set to false when creating
/// managed profiles from caregiver session to avoid hijacking the active session.
#[tauri::command]
pub async fn create_profile(
    name: String,
    password: String,
    managed_by: Option<String>,
    date_of_birth: Option<String>,
    country: Option<String>,
    address: Option<String>,
    auto_open: Option<bool>,
    state: State<'_, Arc<CoreState>>,
) -> Result<ProfileCreateResult, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        // Parse date_of_birth from ISO 8601 string (YYYY-MM-DD)
        let dob = date_of_birth
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|s| {
                chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                    .map_err(|e| format!("Invalid date of birth format: {e}"))
            })
            .transpose()?;

        // CR-2: Only self-managed profiles can create managed profiles.
        // If managed_by is set, the caller must be logged in as a self-managed profile.
        if managed_by.is_some() {
            if let Ok(guard) = state.read_session() {
                if let Some(session) = guard.as_ref() {
                    let profiles = profile::list_profiles(&state.profiles_dir)
                        .map_err(|e| e.to_string())?;
                    if let Some(caller) = profiles.iter().find(|p| p.id == session.profile_id) {
                        if !caller.is_self_managed() {
                            return Err(
                                "Only self-managed profiles can create managed profiles".into(),
                            );
                        }
                    }
                }
            }
        }

        let (info, phrase) = profile::create_profile(
            &state.profiles_dir,
            &name,
            &password,
            managed_by.as_deref(),
            dob,
            country.as_deref(),
            address.as_deref(),
        )
        .map_err(|e| e.to_string())?;

        // F7: Only auto-open if requested (default: true for backward compatibility)
        if auto_open.unwrap_or(true) {
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
        }

        Ok(ProfileCreateResult {
            profile: info,
            recovery_phrase: phrase.words().iter().map(|w| w.to_string()).collect(),
        })
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

/// Unlock a profile with password.
/// Runs on a blocking thread to avoid freezing the UI (Argon2 KDF is CPU-heavy).
#[tauri::command]
pub async fn unlock_profile(
    profile_id: String,
    password: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<ProfileInfo, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let id =
            Uuid::parse_str(&profile_id).map_err(|e| format!("Invalid profile ID: {e}"))?;

        let session = profile::open_profile(&state.profiles_dir, &id, &password)
            .map_err(|e| e.to_string())?;

        // Load full ProfileInfo from disk (includes DOB, color_index, managed_by)
        let profiles =
            profile::list_profiles(&state.profiles_dir).map_err(|e| e.to_string())?;
        let info = profiles
            .into_iter()
            .find(|p| p.id == id)
            .unwrap_or_else(|| ProfileInfo {
                id: session.profile_id,
                name: session.profile_name.clone(),
                created_at: chrono::Local::now().naive_local(),
                managed_by: None,
                password_hint: None,
                date_of_birth: None,
                color_index: None,
                country: None,
                address: None,
            });

        state.set_session(session).map_err(|e| e.to_string())?;
        state.update_activity();

        // Hydrate paired devices from DB so they survive app restarts (RS-ME-02-001)
        if let Err(e) = state.hydrate_devices() {
            tracing::warn!("Failed to hydrate devices: {e}");
        }

        // P.6: Startup consistency cleanup (repair stuck pipeline states, trust drift)
        {
            let guard = state.read_session().map_err(|e| e.to_string())?;
            if let Some(session) = guard.as_ref() {
                match crate::db::sqlite::open_database(
                    session.db_path(),
                    Some(session.key_bytes()),
                ) {
                    Ok(conn) => match crate::db::repository::repair_consistency(&conn) {
                        Ok(0) => {}
                        Ok(n) => tracing::info!(
                            repairs = n,
                            "P.6: Startup consistency cleanup applied"
                        ),
                        Err(e) => tracing::warn!(
                            error = %e,
                            "P.6: Startup consistency cleanup failed"
                        ),
                    },
                    Err(e) => tracing::warn!(
                        error = %e,
                        "P.6: Could not open DB for startup cleanup"
                    ),
                }
            }
        }

        // Notify connected phones about profile change (RS-M0-03-003)
        if let Ok(mut devices) = state.write_devices() {
            devices.broadcast(WsOutgoing::ProfileChanged {
                profile_name: info.name.clone(),
            });
        }

        Ok(info)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

/// Change the password for the currently active profile (RS-L3-01-001).
/// Runs on a blocking thread to avoid freezing the UI (Argon2 KDF is CPU-heavy).
#[tauri::command]
pub async fn change_profile_password(
    current_password: String,
    new_password: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
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
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
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
/// Runs on a blocking thread to avoid freezing the UI (crypto operations).
#[tauri::command]
pub async fn recover_profile(
    profile_id: String,
    recovery_phrase: String,
    _new_password: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let id =
            Uuid::parse_str(&profile_id).map_err(|e| format!("Invalid profile ID: {e}"))?;

        let session =
            profile::recover_profile(&state.profiles_dir, &id, &recovery_phrase)
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
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
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

/// Get the full ProfileInfo for the currently active profile (Spec 45 [PU-02]).
/// Returns all fields including managed_by, date_of_birth, color_index.
#[tauri::command]
pub fn get_active_profile_info(
    state: State<'_, Arc<CoreState>>,
) -> Result<ProfileInfo, String> {
    let guard = state.read_session().map_err(|e| e.to_string())?;
    let session = guard.as_ref().ok_or("No active session")?;

    let profiles =
        profile::list_profiles(&state.profiles_dir).map_err(|e| e.to_string())?;
    profiles
        .into_iter()
        .find(|p| p.id == session.profile_id)
        .ok_or_else(|| "Active profile not found in profiles list".to_string())
}

/// Delete a profile and all its data (cryptographic erasure).
/// Runs on a blocking thread to avoid freezing the UI (file I/O operations).
#[tauri::command]
pub async fn delete_profile(
    profile_id: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let id =
            Uuid::parse_str(&profile_id).map_err(|e| format!("Invalid profile ID: {e}"))?;

        // Check if this profile has dependents — block deletion until they are handled.
        // Follows Apple Family pattern: organizer cannot be deleted while children exist.
        let profiles =
            profile::list_profiles(&state.profiles_dir).map_err(|e| e.to_string())?;
        if let Some(target) = profiles.iter().find(|p| p.id == id) {
            let dependents: Vec<&str> = profiles
                .iter()
                .filter(|p| p.managed_by.as_deref() == Some(&target.name))
                .map(|p| p.name.as_str())
                .collect();
            if !dependents.is_empty() {
                return Err(format!(
                    "This profile manages {} dependent(s): {}. Delete or reassign them first.",
                    dependents.len(),
                    dependents.join(", ")
                ));
            }
        }

        // Lock if deleting the currently active profile
        let active_id = state
            .read_session()
            .ok()
            .and_then(|guard| guard.as_ref().map(|s| s.profile_id));
        if active_id == Some(id) {
            state.lock();
        }

        profile::delete_profile(&state.profiles_dir, &id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

/// Check for inactivity timeout — called periodically from frontend.
/// Returns true if the profile was locked due to inactivity.
/// IMP-002: Flushes audit buffer before locking to prevent data loss.
#[tauri::command]
pub fn check_inactivity(state: State<'_, Arc<CoreState>>) -> bool {
    // Already locked — nothing to do. Prevents repeated lock() calls,
    // failed audit flushes, and log noise on already-locked profiles.
    if state.is_locked() {
        return true;
    }
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

/// Spec 46 [CG-02]: Get caregiver summaries for all dependents managed by the current user.
/// Returns summaries from the JSON cache — no profile decryption needed.
#[tauri::command]
pub fn get_caregiver_summaries(
    state: State<'_, Arc<CoreState>>,
) -> Result<Vec<crate::models::caregiver::CaregiverSummary>, String> {
    use crate::models::caregiver::CaregiverSummaries;

    let guard = state.read_session().map_err(|e| e.to_string())?;
    let session = guard.as_ref().ok_or("No active session")?;

    let path = state.profiles_dir.join("caregiver_summaries.json");
    if !path.exists() {
        return Ok(vec![]);
    }

    let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let summaries: CaregiverSummaries =
        serde_json::from_str(&data).map_err(|e| e.to_string())?;

    // Filter to only dependents managed by the current profile
    Ok(summaries
        .summaries
        .into_iter()
        .filter(|s| s.caregiver_profile_id == session.profile_id)
        .collect())
}
