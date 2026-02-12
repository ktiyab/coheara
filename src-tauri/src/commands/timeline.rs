//! L4-04: Timeline View — Tauri IPC commands.

use tauri::State;

use crate::db::open_database;
use crate::timeline::{self, TimelineData, TimelineFilter};

use super::state::AppState;

/// Fetches all timeline data in a single call.
/// Assembles events from all entity tables, detects correlations,
/// and returns the complete payload.
#[tauri::command]
pub fn get_timeline_data(
    filter: TimelineFilter,
    state: State<'_, AppState>,
) -> Result<TimelineData, String> {
    let guard = state
        .active_session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;
    let session = guard
        .as_ref()
        .ok_or_else(|| "No active profile session".to_string())?;

    let conn = open_database(session.db_path()).map_err(|e| e.to_string())?;

    let data = timeline::get_timeline_data(&conn, &filter).map_err(|e| e.to_string())?;

    state.update_activity();

    Ok(data)
}
