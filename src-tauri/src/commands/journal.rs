//! L4-01 Symptom Journal — Tauri IPC commands.
//!
//! Six commands:
//! - `record_symptom`: guided symptom recording with temporal correlation detection
//! - `get_symptom_history`: filtered symptom history with joined medication/diagnosis names
//! - `resolve_symptom`: mark symptom as no longer active
//! - `delete_symptom`: hard-delete a symptom entry
//! - `check_journal_nudge`: determines whether to show a check-in nudge
//! - `get_symptom_categories`: returns categories with subcategories (static data)

use std::sync::Arc;

use tauri::State;

use crate::core_state::CoreState;
use crate::journal::{
    self, CategoryInfo, NudgeDecision, RecordResult, StoredSymptom, SymptomEntry, SymptomFilter,
};

/// Records a new symptom and returns the ID plus any temporal correlations.
#[tauri::command]
pub fn record_symptom(
    entry: SymptomEntry,
    state: State<'_, Arc<CoreState>>,
) -> Result<RecordResult, String> {
    // Validate severity
    if entry.severity < 1 || entry.severity > 5 {
        return Err("Severity must be between 1 and 5".into());
    }

    // Validate required fields
    if entry.category.trim().is_empty() {
        return Err("Category is required".into());
    }
    if entry.specific.trim().is_empty() {
        return Err("Specific symptom is required".into());
    }
    if entry.category.len() > 50 {
        return Err("Category too long".into());
    }
    if entry.specific.len() > 200 {
        return Err("Specific symptom description too long".into());
    }
    if let Some(ref notes) = entry.notes {
        if notes.len() > 500 {
            return Err("Notes must be 500 characters or fewer".into());
        }
    }

    // Validate onset_date format
    if chrono::NaiveDate::parse_from_str(&entry.onset_date, "%Y-%m-%d").is_err() {
        return Err("Invalid onset date format (expected YYYY-MM-DD)".into());
    }

    // Validate body_region if provided
    if let Some(ref region) = entry.body_region {
        if !journal::BODY_REGIONS.contains(&region.as_str()) {
            return Err(format!("Invalid body region: {region}"));
        }
    }

    let conn = state.open_db().map_err(|e| e.to_string())?;

    let symptom_id = journal::record_symptom(&conn, &entry).map_err(|e| e.to_string())?;

    // Check temporal correlations
    let correlations =
        journal::detect_temporal_correlation(&conn, &entry.onset_date).map_err(|e| e.to_string())?;

    state.update_activity();

    Ok(RecordResult {
        symptom_id: symptom_id.to_string(),
        correlations,
    })
}

/// Fetches symptom history with optional filters.
#[tauri::command]
pub fn get_symptom_history(
    filter: Option<SymptomFilter>,
    state: State<'_, Arc<CoreState>>,
) -> Result<Vec<StoredSymptom>, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;

    let symptoms = journal::fetch_symptoms_filtered(&conn, &filter).map_err(|e| e.to_string())?;

    state.update_activity();
    Ok(symptoms)
}

/// Resolves a symptom (marks as no longer active).
#[tauri::command]
pub fn resolve_symptom(
    symptom_id: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    if symptom_id.trim().is_empty() {
        return Err("Symptom ID is required".into());
    }

    let conn = state.open_db().map_err(|e| e.to_string())?;

    journal::resolve_symptom(&conn, &symptom_id).map_err(|e| e.to_string())?;

    state.update_activity();
    Ok(())
}

/// Deletes a symptom entry permanently.
#[tauri::command]
pub fn delete_symptom(
    symptom_id: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    if symptom_id.trim().is_empty() {
        return Err("Symptom ID is required".into());
    }

    let conn = state.open_db().map_err(|e| e.to_string())?;

    journal::delete_symptom(&conn, &symptom_id).map_err(|e| e.to_string())?;

    state.update_activity();
    Ok(())
}

/// Checks if a nudge should be shown to the patient.
#[tauri::command]
pub fn check_journal_nudge(
    state: State<'_, Arc<CoreState>>,
) -> Result<NudgeDecision, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;

    let nudge = journal::check_nudge(&conn).map_err(|e| e.to_string())?;

    state.update_activity();
    Ok(nudge)
}

/// Returns all symptom categories with their subcategories.
#[tauri::command]
pub fn get_symptom_categories() -> Vec<CategoryInfo> {
    journal::get_symptom_categories()
}
