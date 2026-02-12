//! L3-05 Medication List — Tauri IPC commands.
//!
//! Five commands:
//! - `get_medications`: unified fetch with filters for list screen
//! - `get_medication_detail`: full detail for a single medication
//! - `add_otc_medication`: patient-reported OTC entry
//! - `get_dose_history`: dose change timeline for a medication
//! - `search_medication_alias`: autocomplete for OTC form

use chrono::NaiveDate;
use rusqlite::params;
use tauri::State;
use uuid::Uuid;

use crate::db::sqlite::open_database;
use crate::medications::{
    enrich_medication_cards, fetch_compound_ingredients, fetch_dose_history,
    fetch_medication_aliases, fetch_medication_instructions,
    fetch_medication_status_counts, fetch_medications_filtered, fetch_prescriber_options,
    fetch_single_medication_card, fetch_source_document, fetch_tapering_steps,
    get_or_create_patient_reported_document, search_medication_aliases, AliasSearchResult,
    DoseChangeView, MedicationDetail, MedicationListData, MedicationListFilter,
    OtcMedicationInput,
};

use super::state::AppState;

/// Fetches all medication list data in a single call.
#[tauri::command]
pub fn get_medications(
    filter: MedicationListFilter,
    state: State<'_, AppState>,
) -> Result<MedicationListData, String> {
    let guard = state
        .active_session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;
    let session = guard
        .as_ref()
        .ok_or_else(|| "No active profile session".to_string())?;

    let conn = open_database(session.db_path()).map_err(|e| e.to_string())?;

    let cards = fetch_medications_filtered(&conn, &filter).map_err(|e| e.to_string())?;
    let medications = enrich_medication_cards(&conn, cards);

    let (total_active, total_paused, total_stopped) =
        fetch_medication_status_counts(&conn).map_err(|e| e.to_string())?;

    let prescribers = fetch_prescriber_options(&conn).map_err(|e| e.to_string())?;

    state.update_activity();

    Ok(MedicationListData {
        medications,
        total_active,
        total_paused,
        total_stopped,
        prescribers,
    })
}

/// Fetches full detail for a single medication.
#[tauri::command]
pub fn get_medication_detail(
    medication_id: String,
    state: State<'_, AppState>,
) -> Result<MedicationDetail, String> {
    let guard = state
        .active_session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;
    let session = guard
        .as_ref()
        .ok_or_else(|| "No active profile session".to_string())?;

    let conn = open_database(session.db_path()).map_err(|e| e.to_string())?;
    let med_uuid =
        Uuid::parse_str(&medication_id).map_err(|e| format!("Invalid medication ID: {e}"))?;

    let card = fetch_single_medication_card(&conn, &med_uuid)
        .map_err(|e| e.to_string())?
        .ok_or("Medication not found")?;

    let instructions =
        fetch_medication_instructions(&conn, &med_uuid).map_err(|e| e.to_string())?;

    let compound_ingredients = if card.is_compound {
        fetch_compound_ingredients(&conn, &med_uuid).map_err(|e| e.to_string())?
    } else {
        Vec::new()
    };

    let tapering_steps = if card.has_tapering {
        fetch_tapering_steps(&conn, &med_uuid).map_err(|e| e.to_string())?
    } else {
        Vec::new()
    };

    let aliases =
        fetch_medication_aliases(&conn, &card.generic_name).map_err(|e| e.to_string())?;

    let dose_changes = fetch_dose_history(&conn, &med_uuid).map_err(|e| e.to_string())?;

    let (document_title, document_date) = fetch_source_document(&conn, &med_uuid);

    state.update_activity();

    Ok(MedicationDetail {
        medication: card,
        instructions,
        compound_ingredients,
        tapering_steps,
        aliases,
        dose_changes,
        document_title,
        document_date,
    })
}

/// Adds a patient-reported OTC medication.
#[tauri::command]
pub fn add_otc_medication(
    input: OtcMedicationInput,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let guard = state
        .active_session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;
    let session = guard
        .as_ref()
        .ok_or_else(|| "No active profile session".to_string())?;

    let conn = open_database(session.db_path()).map_err(|e| e.to_string())?;

    // Validate required fields
    if input.name.trim().is_empty() {
        return Err("Medication name is required".into());
    }
    if input.name.trim().len() > 200 {
        return Err("Medication name is too long (max 200 characters)".into());
    }
    if input.dose.trim().is_empty() {
        return Err("Dose is required".into());
    }
    if input.dose.trim().len() > 100 {
        return Err("Dose is too long (max 100 characters)".into());
    }
    if input.frequency.trim().is_empty() {
        return Err("Frequency is required".into());
    }
    if input.frequency.trim().len() > 200 {
        return Err("Frequency is too long (max 200 characters)".into());
    }
    if let Some(ref reason) = input.reason {
        if reason.len() > 500 {
            return Err("Reason is too long (max 500 characters)".into());
        }
    }
    if let Some(ref instructions) = input.instructions {
        if instructions.len() > 1000 {
            return Err("Instructions are too long (max 1000 characters)".into());
        }
    }

    let start_date = input
        .start_date
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

    // Validate start_date is not in the future
    if let Some(date) = start_date {
        if date > chrono::Local::now().date_naive() {
            return Err("Start date cannot be in the future".into());
        }
    }

    let med_id = Uuid::new_v4();
    let patient_doc_id =
        get_or_create_patient_reported_document(&conn).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO medications (
            id, generic_name, brand_name, dose, frequency, frequency_type,
            route, prescriber_id, start_date, end_date, reason_start,
            reason_stop, is_otc, status, administration_instructions,
            max_daily_dose, condition, dose_type, is_compound, document_id
        ) VALUES (
            ?1, ?2, NULL, ?3, ?4, 'scheduled',
            ?5, NULL, ?6, NULL, ?7,
            NULL, 1, 'active', ?8,
            NULL, NULL, 'fixed', 0, ?9
        )",
        params![
            med_id.to_string(),
            input.name.trim(),
            input.dose.trim(),
            input.frequency.trim(),
            input.route.trim(),
            start_date.map(|d| d.to_string()),
            input.reason.as_deref().map(str::trim),
            input.instructions.as_deref().map(str::trim),
            patient_doc_id.to_string(),
        ],
    )
    .map_err(|e| format!("Failed to add medication: {e}"))?;

    // Store instructions in dedicated table if provided
    if let Some(ref instructions) = input.instructions {
        if !instructions.trim().is_empty() {
            let instr_id = Uuid::new_v4();
            conn.execute(
                "INSERT INTO medication_instructions (id, medication_id, instruction, timing, source_document_id)
                 VALUES (?1, ?2, ?3, NULL, ?4)",
                params![
                    instr_id.to_string(),
                    med_id.to_string(),
                    instructions.trim(),
                    patient_doc_id.to_string(),
                ],
            )
            .map_err(|e| format!("Failed to store instruction: {e}"))?;
        }
    }

    state.update_activity();

    tracing::info!(
        medication_id = %med_id,
        name = %input.name.trim(),
        "OTC medication added by patient"
    );

    Ok(med_id.to_string())
}

/// Fetches dose change history for a medication.
#[tauri::command]
pub fn get_dose_history(
    medication_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<DoseChangeView>, String> {
    let guard = state
        .active_session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;
    let session = guard
        .as_ref()
        .ok_or_else(|| "No active profile session".to_string())?;

    let conn = open_database(session.db_path()).map_err(|e| e.to_string())?;
    let med_uuid =
        Uuid::parse_str(&medication_id).map_err(|e| format!("Invalid medication ID: {e}"))?;

    let history = fetch_dose_history(&conn, &med_uuid).map_err(|e| e.to_string())?;

    state.update_activity();

    Ok(history)
}

/// Searches medication aliases for autocomplete.
#[tauri::command]
pub fn search_medication_alias(
    query: String,
    limit: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<AliasSearchResult>, String> {
    let guard = state
        .active_session
        .lock()
        .map_err(|_| "Failed to acquire session lock".to_string())?;
    let session = guard
        .as_ref()
        .ok_or_else(|| "No active profile session".to_string())?;

    let conn = open_database(session.db_path()).map_err(|e| e.to_string())?;
    let clamped_limit = limit.unwrap_or(10).min(50);

    let results =
        search_medication_aliases(&conn, &query, clamped_limit).map_err(|e| e.to_string())?;

    state.update_activity();

    Ok(results)
}
