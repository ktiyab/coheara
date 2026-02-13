//! L4-04: Timeline View — Tauri IPC commands.

use std::sync::Arc;

use tauri::State;

use crate::core_state::CoreState;
use crate::timeline::{self, TimelineData, TimelineFilter};

/// Fetches all timeline data in a single call.
/// Assembles events from all entity tables, detects correlations,
/// and returns the complete payload.
#[tauri::command]
pub fn get_timeline_data(
    filter: TimelineFilter,
    state: State<'_, Arc<CoreState>>,
) -> Result<TimelineData, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;

    let data = timeline::get_timeline_data(&conn, &filter).map_err(|e| e.to_string())?;

    state.update_activity();

    Ok(data)
}
