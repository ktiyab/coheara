//! L3-02 Home & Document Feed — Tauri IPC commands.
//!
//! Three commands:
//! - `get_home_data`: unified fetch for the entire home screen
//! - `get_more_documents`: paginated document feed for infinite scroll
//! - `dismiss_alert`: dismiss a coherence observation

use std::sync::Arc;

use rusqlite::params;
use tauri::State;
use uuid::Uuid;

use crate::core_state::CoreState;
use crate::home::{
    compute_onboarding, fetch_document_detail, fetch_profile_stats, fetch_recent_documents,
    DocumentCard, DocumentDetail, HomeData,
};

/// Valid alert types matching the dismissed_alerts CHECK constraint.
const VALID_ALERT_TYPES: &[&str] = &[
    "conflict",
    "gap",
    "drift",
    "ambiguity",
    "duplicate",
    "allergy",
    "dose",
    "critical",
    "temporal",
];

/// Fetches all home screen data in a single call.
#[tauri::command]
pub fn get_home_data(state: State<'_, Arc<CoreState>>) -> Result<HomeData, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;

    let stats = fetch_profile_stats(&conn).map_err(|e| e.to_string())?;
    let recent_documents = fetch_recent_documents(&conn, 20, 0).map_err(|e| e.to_string())?;
    let onboarding = compute_onboarding(&conn).map_err(|e| e.to_string())?;
    let critical_alerts = crate::trust::fetch_critical_alerts(&conn).unwrap_or_default();

    state.update_activity();

    Ok(HomeData {
        stats,
        recent_documents,
        onboarding,
        critical_alerts,
    })
}

/// Fetches more documents for infinite scroll.
#[tauri::command]
pub fn get_more_documents(
    offset: u32,
    limit: u32,
    state: State<'_, Arc<CoreState>>,
) -> Result<Vec<DocumentCard>, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;
    let clamped_limit = limit.min(50);

    state.update_activity();

    fetch_recent_documents(&conn, clamped_limit, offset).map_err(|e| e.to_string())
}

/// Fetches detailed document info with all linked entities.
#[tauri::command]
pub fn get_document_detail(
    document_id: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<DocumentDetail, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;

    state.update_activity();

    fetch_document_detail(&conn, &document_id).map_err(|e| e.to_string())
}

/// Dismisses a coherence observation with reason.
/// `alert_type` must be a valid AlertType string (e.g., "conflict", "dose", "critical").
#[tauri::command]
pub fn dismiss_alert(
    alert_id: String,
    alert_type: String,
    reason: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;
    let _alert_uuid =
        Uuid::parse_str(&alert_id).map_err(|e| format!("Invalid alert ID: {e}"))?;

    if !VALID_ALERT_TYPES.contains(&alert_type.as_str()) {
        return Err(format!("Invalid alert type: {alert_type}"));
    }

    let dismiss_id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO dismissed_alerts (id, alert_type, entity_ids, dismissed_date, reason, dismissed_by)
         VALUES (?1, ?2, '[]', datetime('now'), ?3, 'patient')",
        params![dismiss_id, alert_type, reason],
    )
    .map_err(|e| format!("Failed to dismiss alert: {e}"))?;

    state.update_activity();

    Ok(())
}

/// Spec 46 [CG-06] + Spec 49: Full-text search across documents.
#[tauri::command]
pub fn search_documents(
    query: String,
    doc_type_filter: Option<String>,
    state: State<'_, Arc<CoreState>>,
) -> Result<Vec<crate::db::DocumentSearchResult>, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;

    crate::db::search_documents_fts(&conn, &query, doc_type_filter.as_deref(), 50)
        .map_err(|e| e.to_string())
}
