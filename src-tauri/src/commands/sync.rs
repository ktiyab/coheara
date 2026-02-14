//! E2E-B07: Tauri IPC commands for the Sync Engine.
//!
//! Desktop UI commands for monitoring and managing sync state:
//! - get_sync_versions: Read current version counters
//! - reset_sync_versions: Reset all counters to force full resync
//! - get_sync_summary: High-level sync overview for status display

use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::core_state::CoreState;
use crate::sync;

/// Sync status summary for the desktop UI.
#[derive(Debug, Clone, Serialize)]
pub struct SyncSummary {
    pub versions: sync::SyncVersions,
    pub total_version: i64,
    pub paired_device_count: usize,
}

/// Get current sync version counters.
///
/// Returns the monotonic version counter for each of the 6 entity types.
/// These counters auto-increment via SQLite triggers on data changes.
#[tauri::command]
pub fn get_sync_versions(
    state: State<'_, Arc<CoreState>>,
) -> Result<sync::SyncVersions, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;
    state.update_activity();
    sync::get_sync_versions(&conn).map_err(|e| e.to_string())
}

/// Reset all sync version counters to zero.
///
/// Forces every paired mobile device to do a full resync on their
/// next sync request. Useful for troubleshooting or after data
/// migration.
#[tauri::command]
pub fn reset_sync_versions(
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;

    conn.execute_batch(
        "UPDATE sync_versions SET version = 0, updated_at = datetime('now')",
    )
    .map_err(|e| format!("Failed to reset sync versions: {e}"))?;

    state.log_access(
        crate::core_state::AccessSource::DesktopUi,
        "reset_sync_versions",
        "sync_versions",
    );

    state.update_activity();
    tracing::info!("Sync versions reset to 0 — next mobile sync will be full");
    Ok(())
}

/// Get sync summary for the desktop status display.
///
/// Returns version counters, total version sum, and paired device count.
#[tauri::command]
pub fn get_sync_summary(
    state: State<'_, Arc<CoreState>>,
) -> Result<SyncSummary, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;
    let versions = sync::get_sync_versions(&conn).map_err(|e| e.to_string())?;

    let total_version = versions.medications
        + versions.labs
        + versions.timeline
        + versions.alerts
        + versions.appointments
        + versions.profile;

    let paired_device_count = state
        .read_devices()
        .map(|d| d.device_count())
        .unwrap_or(0);

    state.update_activity();

    Ok(SyncSummary {
        versions,
        total_version,
        paired_device_count,
    })
}
