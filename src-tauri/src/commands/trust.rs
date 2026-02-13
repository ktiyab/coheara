//! L5-01: Trust & Safety — Tauri IPC commands.

use std::sync::Arc;

use tauri::State;

use crate::core_state::CoreState;
use crate::db::sqlite::open_database;
use crate::trust;

/// Get all critical lab alerts that haven't been dismissed.
#[tauri::command]
pub fn get_critical_alerts(
    state: State<'_, Arc<CoreState>>,
) -> Result<Vec<trust::CriticalLabAlert>, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;

    state.update_activity();
    trust::fetch_critical_alerts(&conn).map_err(|e| e.to_string())
}

/// Dismiss a critical alert (2-step process).
#[tauri::command]
pub fn dismiss_critical(
    request: trust::CriticalDismissRequest,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;

    state.update_activity();
    trust::dismiss_critical_alert(&conn, &request).map_err(|e| e.to_string())
}

/// Check dose plausibility for a medication.
#[tauri::command]
pub fn check_dose(
    medication_name: String,
    dose_value: f64,
    dose_unit: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<trust::DosePlausibility, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;

    state.update_activity();
    trust::check_dose_plausibility(&conn, &medication_name, dose_value, &dose_unit)
        .map_err(|e| e.to_string())
}

/// Create an encrypted backup of the current profile.
#[tauri::command]
pub fn create_backup(
    output_path: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<trust::BackupResult, String> {
    let guard = state.read_session().map_err(|e| e.to_string())?;
    let session = guard
        .as_ref()
        .ok_or_else(|| "No active profile session".to_string())?;

    let path = std::path::PathBuf::from(&output_path);

    state.update_activity();
    trust::create_backup(session, &path).map_err(|e| e.to_string())
}

/// Preview a backup file (reads metadata without decryption).
#[tauri::command]
pub fn preview_backup_file(
    backup_path: String,
) -> Result<trust::RestorePreview, String> {
    let path = std::path::PathBuf::from(&backup_path);
    trust::preview_backup(&path).map_err(|e| e.to_string())
}

/// Restore from a backup file.
#[tauri::command]
pub fn restore_from_backup(
    backup_path: String,
    password: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<trust::RestoreResult, String> {
    let guard = state.read_session().map_err(|e| e.to_string())?;
    let session = guard
        .as_ref()
        .ok_or_else(|| "No active profile session".to_string())?;

    let profile_dir = session
        .db_path()
        .parent()
        .and_then(|db_dir| db_dir.parent())
        .ok_or_else(|| "Cannot determine profile directory".to_string())?;

    let path = std::path::PathBuf::from(&backup_path);

    state.update_activity();
    trust::restore_backup(&path, &password, profile_dir).map_err(|e| e.to_string())
}

/// Erase a profile (cryptographic erasure).
#[tauri::command]
pub fn erase_profile_data(
    request: trust::ErasureRequest,
    state: State<'_, Arc<CoreState>>,
) -> Result<trust::ErasureResult, String> {
    // Lock the current profile first
    state.lock();

    let profiles_dir = &state.profiles_dir;
    trust::erase_profile_data(profiles_dir, &request).map_err(|e| e.to_string())
}

/// Get privacy verification information.
#[tauri::command]
pub fn get_privacy_info_cmd(
    state: State<'_, Arc<CoreState>>,
) -> Result<trust::PrivacyInfo, String> {
    let guard = state.read_session().map_err(|e| e.to_string())?;
    let session = guard
        .as_ref()
        .ok_or_else(|| "No active profile session".to_string())?;
    let conn = open_database(session.db_path()).map_err(|e| e.to_string())?;

    let profile_dir = session
        .db_path()
        .parent()
        .and_then(|db_dir| db_dir.parent())
        .ok_or_else(|| "Cannot determine profile directory".to_string())?;

    state.update_activity();
    trust::get_privacy_info(&conn, profile_dir).map_err(|e| e.to_string())
}

/// Open the profile data folder in the system file manager.
#[tauri::command]
pub fn open_data_folder(
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    let guard = state.read_session().map_err(|e| e.to_string())?;
    let session = guard
        .as_ref()
        .ok_or_else(|| "No active profile session".to_string())?;

    let profile_dir = session
        .db_path()
        .parent()
        .and_then(|db_dir| db_dir.parent())
        .ok_or_else(|| "Cannot determine profile directory".to_string())?;

    // Use std::process::Command to open the folder in the file manager
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(profile_dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {e}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(profile_dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {e}"))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(profile_dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {e}"))?;
    }

    Ok(())
}
