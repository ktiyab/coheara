//! L4-02 Appointment Prep — Tauri IPC commands.
//!
//! Five commands:
//! - `list_professionals`: known professionals for selector
//! - `prepare_appointment`: create appointment + generate prep
//! - `export_prep_pdf`: export prep as PDF files
//! - `save_appointment_notes`: post-appointment guided notes
//! - `list_appointments`: appointment history

use std::sync::Arc;

use tauri::State;

use crate::appointment::{
    self, AppointmentPrep, AppointmentRequest, PostAppointmentNotes,
    ProfessionalInfo, StoredAppointment, SPECIALTIES,
};
use crate::core_state::CoreState;

/// Lists known professionals ordered by last_seen_date DESC.
#[tauri::command]
pub fn list_professionals(
    state: State<'_, Arc<CoreState>>,
) -> Result<Vec<ProfessionalInfo>, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;
    state.update_activity();

    appointment::list_professionals(&conn).map_err(|e| e.to_string())
}

/// Creates appointment and generates prep (patient + professional copies).
#[tauri::command]
pub fn prepare_appointment(
    request: AppointmentRequest,
    state: State<'_, Arc<CoreState>>,
) -> Result<AppointmentPrep, String> {
    // Validate date format
    let date = chrono::NaiveDate::parse_from_str(&request.date, "%Y-%m-%d")
        .map_err(|_| "Invalid date format. Use YYYY-MM-DD")?;

    // Must have either professional_id or new_professional
    if request.professional_id.is_none() && request.new_professional.is_none() {
        return Err("Must provide professional_id or new_professional".into());
    }

    // Validate new professional fields if provided
    if let Some(ref new_prof) = request.new_professional {
        if new_prof.name.trim().is_empty() {
            return Err("Professional name is required".into());
        }
        if new_prof.name.len() > 200 {
            return Err("Professional name too long".into());
        }
        if new_prof.specialty.trim().is_empty() {
            return Err("Specialty is required".into());
        }
        if !SPECIALTIES.contains(&new_prof.specialty.as_str()) && new_prof.specialty != "Other" {
            return Err(format!("Invalid specialty: {}", new_prof.specialty));
        }
        if let Some(ref inst) = new_prof.institution {
            if inst.len() > 200 {
                return Err("Institution name too long".into());
            }
        }
    }

    let conn = state.open_db().map_err(|e| e.to_string())?;

    // Resolve professional (existing or create new)
    let professional_id = match request.professional_id {
        Some(id) => id,
        None => {
            let new_prof = request.new_professional.as_ref().unwrap();
            appointment::create_professional(&conn, new_prof).map_err(|e| e.to_string())?
        }
    };

    // Create appointment record
    let appointment_id = appointment::create_appointment(&conn, &professional_id, &date)
        .map_err(|e| e.to_string())?;

    // Generate full prep
    let prep = appointment::prepare_appointment_prep(&conn, &professional_id, date, &appointment_id)
        .map_err(|e| format!("Failed to generate preparation: {e}"))?;

    state.update_activity();
    Ok(prep)
}

/// Exports prep as PDF. Returns list of created file paths.
#[tauri::command]
pub fn export_prep_pdf(
    prep: AppointmentPrep,
    copy_type: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<Vec<String>, String> {
    if !["patient", "professional", "both"].contains(&copy_type.as_str()) {
        return Err("copy_type must be 'patient', 'professional', or 'both'".into());
    }

    let guard = state.read_session().map_err(|e| e.to_string())?;
    let session = guard.as_ref().ok_or("No active profile session")?;
    let db_path = session.db_path().to_owned();
    drop(guard);

    state.update_activity();

    let safe_name = prep.professional_name.replace(' ', "-");
    let mut paths = Vec::new();

    if copy_type == "patient" || copy_type == "both" {
        let pdf = appointment::generate_patient_pdf(&prep.patient_copy)
            .map_err(|e| format!("Patient PDF error: {e}"))?;
        let filename = format!("patient-prep-{}-{}.pdf", safe_name, prep.appointment_date);
        let path = appointment::export_pdf_to_file(&pdf, &filename, &db_path)
            .map_err(|e| e.to_string())?;
        paths.push(path.to_string_lossy().into_owned());
    }

    if copy_type == "professional" || copy_type == "both" {
        let pdf = appointment::generate_professional_pdf(&prep.professional_copy)
            .map_err(|e| format!("Professional PDF error: {e}"))?;
        let filename = format!("professional-summary-{}-{}.pdf", safe_name, prep.appointment_date);
        let path = appointment::export_pdf_to_file(&pdf, &filename, &db_path)
            .map_err(|e| e.to_string())?;
        paths.push(path.to_string_lossy().into_owned());
    }

    Ok(paths)
}

/// Saves post-appointment notes. Marks appointment as completed.
#[tauri::command]
pub fn save_appointment_notes(
    notes: PostAppointmentNotes,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    // Validate required fields
    if notes.appointment_id.trim().is_empty() {
        return Err("Appointment ID is required".into());
    }
    if notes.doctor_said.trim().is_empty() {
        return Err("'What did the doctor say?' is required".into());
    }
    if notes.doctor_said.len() > 5000 {
        return Err("Doctor notes too long (max 5000 chars)".into());
    }
    if notes.changes_made.trim().is_empty() {
        return Err("'Any changes to medications?' is required".into());
    }
    if notes.changes_made.len() > 5000 {
        return Err("Changes notes too long (max 5000 chars)".into());
    }
    if let Some(ref fu) = notes.follow_up {
        if fu.len() > 2000 {
            return Err("Follow-up notes too long (max 2000 chars)".into());
        }
    }
    if let Some(ref gn) = notes.general_notes {
        if gn.len() > 2000 {
            return Err("General notes too long (max 2000 chars)".into());
        }
    }

    let conn = state.open_db().map_err(|e| e.to_string())?;
    state.update_activity();

    appointment::save_post_notes(&conn, &notes).map_err(|e| e.to_string())
}

/// Lists all appointments with professional info, ordered by date DESC.
#[tauri::command]
pub fn list_appointments(
    state: State<'_, Arc<CoreState>>,
) -> Result<Vec<StoredAppointment>, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;
    state.update_activity();

    appointment::list_appointments(&conn).map_err(|e| e.to_string())
}
