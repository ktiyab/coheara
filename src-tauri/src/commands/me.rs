//! L3-06 + ME-06: Me Screen - IPC commands.
//!
//! Commands:
//! - `get_me_overview`: unified fetch for the entire Me screen
//! - `record_vital_sign`: record a vital sign measurement (ME-04)
//! - `record_screening`: record a screening/vaccination date (ME-06)
//! - `delete_screening_record`: remove a screening record (ME-06)

use std::sync::Arc;

use tauri::State;

use crate::core_state::CoreState;
use crate::me::MeOverview;
use crate::models::{VitalSign, VitalSource, VitalType};

/// Fetches all Me screen data in a single call.
///
/// Assembles identity, invariant insights, and domain summaries
/// from the active profile's data + invariant engine.
///
/// ME-04: `lang` is the UI locale from the frontend (e.g. "fr", "de").
/// Falls back to "en" if empty or unsupported, ensuring invariant labels
/// always match the user's active display language.
#[tauri::command]
pub fn get_me_overview(
    lang: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<MeOverview, String> {
    let lang = match lang.as_str() {
        "fr" | "de" => lang.as_str(),
        _ => "en",
    };
    let conn = state.open_db().map_err(|e| e.to_string())?;
    let overview =
        crate::me::assemble_me_overview(&conn, &state, lang).map_err(|e| e.to_string())?;
    state.update_activity();
    Ok(overview)
}

/// ME-04: Record a vital sign measurement (manual entry).
///
/// Used by enrollment (weight/height) and future manual vital entry.
/// Validates `vital_type` string and stores with `VitalSource::Manual`.
#[tauri::command]
pub fn record_vital_sign(
    vital_type: String,
    value: f64,
    value_secondary: Option<f64>,
    notes: Option<String>,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    let vtype = VitalType::from_str(&vital_type)
        .ok_or_else(|| format!("Invalid vital type: {vital_type}"))?;

    let conn = state.open_db().map_err(|e| e.to_string())?;
    let now = chrono::Local::now().naive_local();

    let vs = VitalSign {
        id: uuid::Uuid::new_v4(),
        vital_type: vtype,
        value_primary: value,
        value_secondary,
        unit: vtype.default_unit().to_string(),
        recorded_at: now,
        notes,
        source: VitalSource::Manual,
        created_at: now,
    };

    crate::db::insert_vital_sign(&conn, &vs).map_err(|e| e.to_string())?;
    state.update_activity();
    Ok(())
}

/// ME-06: Record a screening or vaccination date.
///
/// Validates screening_key against known schedules, date is not in the future,
/// and dose_number is within bounds.
#[tauri::command]
pub fn record_screening(
    screening_key: String,
    dose_number: i32,
    completed_at: String,
    provider: Option<String>,
    notes: Option<String>,
    state: State<'_, Arc<CoreState>>,
) -> Result<String, String> {
    // Validate screening key exists
    let schedule = crate::invariants::screening::find_schedule(&screening_key)
        .ok_or_else(|| format!("Unknown screening key: {screening_key}"))?;

    // Validate dose number
    if dose_number < 1 {
        return Err("Dose number must be at least 1".to_string());
    }
    if schedule.total_doses > 0 && dose_number > schedule.total_doses as i32 {
        return Err(format!(
            "Dose number {dose_number} exceeds total doses {} for {screening_key}",
            schedule.total_doses
        ));
    }

    // Parse and validate date
    let date = chrono::NaiveDate::parse_from_str(&completed_at, "%Y-%m-%d")
        .map_err(|_| format!("Invalid date format: {completed_at}. Expected YYYY-MM-DD"))?;
    let today = chrono::Local::now().date_naive();
    if date > today {
        return Err("Date cannot be in the future".to_string());
    }

    let conn = state.open_db().map_err(|e| e.to_string())?;
    let profile_id = {
        let guard = state.read_session().map_err(|e| e.to_string())?;
        match guard.as_ref() {
            Some(s) => s.profile_id.to_string(),
            None => return Err("No active profile session".to_string()),
        }
    };

    let record_id = crate::db::insert_screening_record(
        &conn,
        &profile_id,
        &screening_key,
        dose_number,
        date,
        provider.as_deref(),
        notes.as_deref(),
    )
    .map_err(|e| e.to_string())?;

    state.update_activity();
    Ok(record_id)
}

/// ME-06: Delete a screening record by ID.
///
/// Scoped to the active profile for safety.
#[tauri::command]
pub fn delete_screening_record(
    record_id: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<bool, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;
    let profile_id = {
        let guard = state.read_session().map_err(|e| e.to_string())?;
        match guard.as_ref() {
            Some(s) => s.profile_id.to_string(),
            None => return Err("No active profile session".to_string()),
        }
    };

    let deleted = crate::db::delete_screening_record(&conn, &record_id, &profile_id)
        .map_err(|e| e.to_string())?;
    state.update_activity();
    Ok(deleted)
}
