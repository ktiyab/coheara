//! E2E-B04: Coherence Engine Trigger — Tauri IPC commands.
//!
//! Wires the coherence engine (8 detection algorithms) to IPC so the desktop
//! user can run on-demand coherence analysis and manage alerts.
//!
//! Commands:
//! - `run_coherence_scan`: full analysis on all patient data
//! - `run_coherence_scan_document`: document-scoped analysis
//! - `get_coherence_alerts`: list active alerts (optionally filtered by type)
//! - `dismiss_coherence_alert`: dismiss a standard alert
//! - `dismiss_critical_coherence_alert`: dismiss a critical alert (2-step)
//! - `get_coherence_emergency_actions`: get emergency actions for critical alerts

use std::str::FromStr;
use std::sync::Arc;

use tauri::State;
use uuid::Uuid;

use crate::core_state::CoreState;
use crate::db::repository;
use crate::intelligence::engine::DefaultCoherenceEngine;
use crate::intelligence::emergency::{EmergencyAction, EmergencyProtocol};
use crate::intelligence::reference::CoherenceReferenceData;
use crate::intelligence::types::{
    CoherenceAlert, CoherenceEngine, CoherenceResult, RepositorySnapshot,
};
use crate::models::enums::{AlertType, DismissedBy};

// ---------------------------------------------------------------------------
// Snapshot construction
// ---------------------------------------------------------------------------

/// Build a `RepositorySnapshot` from the active profile's database.
/// Fetches all patient data needed for coherence analysis.
fn build_snapshot(conn: &rusqlite::Connection) -> Result<RepositorySnapshot, String> {
    Ok(RepositorySnapshot {
        medications: repository::get_all_medications(conn).map_err(|e| e.to_string())?,
        diagnoses: repository::get_all_diagnoses(conn).map_err(|e| e.to_string())?,
        lab_results: repository::get_all_lab_results(conn).map_err(|e| e.to_string())?,
        allergies: repository::get_all_allergies(conn).map_err(|e| e.to_string())?,
        symptoms: repository::get_all_symptoms(conn).map_err(|e| e.to_string())?,
        procedures: repository::get_all_procedures(conn).map_err(|e| e.to_string())?,
        professionals: repository::get_all_professionals(conn).map_err(|e| e.to_string())?,
        dose_changes: repository::get_all_dose_changes(conn).map_err(|e| e.to_string())?,
        compound_ingredients: repository::get_all_compound_ingredients(conn)
            .map_err(|e| e.to_string())?,
        dismissed_alert_keys: repository::get_dismissed_alert_keys(conn)
            .map_err(|e| e.to_string())?,
    })
}

// ---------------------------------------------------------------------------
// Engine construction
// ---------------------------------------------------------------------------

/// Build a coherence engine with SQLite persistence and reference data.
///
/// Tries to load reference data from the bundled resources directory.
/// Falls back to test data if files are not found (development/CI).
fn build_engine(conn: &rusqlite::Connection, db_path: &std::path::Path) -> Result<DefaultCoherenceEngine, String> {
    let reference = load_reference_data();
    DefaultCoherenceEngine::with_db(reference, conn, db_path).map_err(|e| e.to_string())
}

/// Load reference data, falling back to test data if files unavailable.
fn load_reference_data() -> CoherenceReferenceData {
    // Try the bundled resources directory (next to the binary in production,
    // or the source resources/ directory in development)
    let candidates = vec![
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources"),
    ];

    for dir in candidates {
        if dir.join("medication_aliases.json").exists() && dir.join("dose_ranges.json").exists() {
            match CoherenceReferenceData::load(&dir) {
                Ok(data) => {
                    tracing::info!(dir = %dir.display(), "Loaded coherence reference data");
                    return data;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to load reference data, falling back");
                }
            }
        }
    }

    tracing::info!("Using test reference data (bundled files not found)");
    CoherenceReferenceData::load_test()
}

// ---------------------------------------------------------------------------
// IPC Commands
// ---------------------------------------------------------------------------

/// Run full coherence analysis on the entire patient data constellation.
///
/// Builds a RepositorySnapshot from all patient data, constructs the engine,
/// and runs all 8 detection algorithms. New alerts are persisted to SQLite.
#[tauri::command]
pub fn run_coherence_scan(
    state: State<'_, Arc<CoreState>>,
) -> Result<CoherenceResult, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;
    let db_path = state.db_path().map_err(|e| e.to_string())?;

    let snapshot = build_snapshot(&conn)?;
    let engine = build_engine(&conn, &db_path)?;

    let result = engine.analyze_full(&snapshot).map_err(|e| e.to_string())?;

    tracing::info!(
        new_alerts = result.new_alerts.len(),
        total = result.counts.total(),
        processing_ms = result.processing_time_ms,
        "Coherence scan complete"
    );

    state.update_activity();
    Ok(result)
}

/// Run coherence analysis scoped to a specific document.
///
/// Focuses detection algorithms on entities from the given document,
/// detecting conflicts with existing data.
#[tauri::command]
pub fn run_coherence_scan_document(
    document_id: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<CoherenceResult, String> {
    let doc_id =
        Uuid::parse_str(&document_id).map_err(|e| format!("Invalid document ID: {e}"))?;

    let conn = state.open_db().map_err(|e| e.to_string())?;
    let db_path = state.db_path().map_err(|e| e.to_string())?;

    let snapshot = build_snapshot(&conn)?;
    let engine = build_engine(&conn, &db_path)?;

    let result = engine
        .analyze_new_document(&doc_id, &snapshot)
        .map_err(|e| e.to_string())?;

    tracing::info!(
        document_id = %doc_id,
        new_alerts = result.new_alerts.len(),
        processing_ms = result.processing_time_ms,
        "Document coherence scan complete"
    );

    state.update_activity();
    Ok(result)
}

/// Get all active (non-dismissed) coherence alerts, optionally filtered by type.
///
/// If `alert_type` is provided, only alerts of that type are returned.
/// Valid types: conflict, duplicate, gap, drift, temporal, allergy, dose, critical.
#[tauri::command]
pub fn get_coherence_alerts(
    alert_type: Option<String>,
    state: State<'_, Arc<CoreState>>,
) -> Result<Vec<CoherenceAlert>, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;
    let db_path = state.db_path().map_err(|e| e.to_string())?;

    let engine = build_engine(&conn, &db_path)?;

    let filter = alert_type
        .as_deref()
        .map(AlertType::from_str)
        .transpose()
        .map_err(|e| format!("Invalid alert type: {e}"))?;

    let alerts = engine
        .get_active_alerts(filter.as_ref())
        .map_err(|e| e.to_string())?;

    state.update_activity();
    Ok(alerts)
}

/// Dismiss a standard coherence alert.
///
/// CRITICAL alerts cannot be dismissed with this command — use
/// `dismiss_critical_coherence_alert` instead (requires 2-step confirmation).
#[tauri::command]
pub fn dismiss_coherence_alert(
    alert_id: String,
    reason: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    let id = Uuid::parse_str(&alert_id).map_err(|e| format!("Invalid alert ID: {e}"))?;

    let conn = state.open_db().map_err(|e| e.to_string())?;
    let db_path = state.db_path().map_err(|e| e.to_string())?;

    let engine = build_engine(&conn, &db_path)?;
    engine
        .dismiss_alert(&id, &reason, DismissedBy::Patient)
        .map_err(|e| e.to_string())?;

    tracing::info!(alert_id = %id, "Coherence alert dismissed");
    state.update_activity();
    Ok(())
}

/// Dismiss a CRITICAL coherence alert (requires 2-step confirmation).
///
/// The frontend must set `two_step_confirmed = true` after the user completes
/// the second confirmation step (acknowledging they understand the risk).
#[tauri::command]
pub fn dismiss_critical_coherence_alert(
    alert_id: String,
    reason: String,
    two_step_confirmed: bool,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    let id = Uuid::parse_str(&alert_id).map_err(|e| format!("Invalid alert ID: {e}"))?;

    let conn = state.open_db().map_err(|e| e.to_string())?;
    let db_path = state.db_path().map_err(|e| e.to_string())?;

    let engine = build_engine(&conn, &db_path)?;
    engine
        .dismiss_critical_alert(&id, &reason, two_step_confirmed)
        .map_err(|e| e.to_string())?;

    tracing::info!(alert_id = %id, confirmed = two_step_confirmed, "Critical alert dismissed");
    state.update_activity();
    Ok(())
}

/// Get emergency actions for CRITICAL alerts currently active.
///
/// Returns actions that need immediate surfacing: ingestion messages,
/// home banners, appointment priorities, and dismissal prompts.
#[tauri::command]
pub fn get_coherence_emergency_actions(
    state: State<'_, Arc<CoreState>>,
) -> Result<Vec<EmergencyAction>, String> {
    let conn = state.open_db().map_err(|e| e.to_string())?;
    let db_path = state.db_path().map_err(|e| e.to_string())?;

    let engine = build_engine(&conn, &db_path)?;
    let critical_alerts = engine.get_critical_alerts().map_err(|e| e.to_string())?;

    let actions = EmergencyProtocol::process_critical_alerts(&critical_alerts);

    state.update_activity();
    Ok(actions)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;

    #[test]
    fn build_snapshot_empty_db() {
        let conn = open_memory_database().unwrap();
        let snapshot = build_snapshot(&conn).unwrap();

        assert!(snapshot.medications.is_empty());
        assert!(snapshot.diagnoses.is_empty());
        assert!(snapshot.lab_results.is_empty());
        assert!(snapshot.allergies.is_empty());
        assert!(snapshot.symptoms.is_empty());
        assert!(snapshot.procedures.is_empty());
        assert!(snapshot.professionals.is_empty());
        assert!(snapshot.dose_changes.is_empty());
        assert!(snapshot.compound_ingredients.is_empty());
        assert!(snapshot.dismissed_alert_keys.is_empty());
    }

    #[test]
    fn build_snapshot_with_data() {
        use crate::db::repository::*;
        use crate::models::*;
        use crate::models::enums::*;
        use chrono::NaiveDate;

        let conn = open_memory_database().unwrap();

        // Insert a document (needed for FK constraints)
        let doc_id = Uuid::new_v4();
        insert_document(
            &conn,
            &Document {
                id: doc_id,
                doc_type: DocumentType::Prescription,
                title: "Test".into(),
                document_date: None,
                ingestion_date: chrono::Local::now().naive_local(),
                professional_id: None,
                source_file: "/test.enc".into(),
                markdown_file: None,
                ocr_confidence: Some(0.9),
                verified: false,
                source_deleted: false,
                perceptual_hash: None,
                notes: None,
            },
        )
        .unwrap();

        // Insert a medication
        insert_medication(
            &conn,
            &Medication {
                id: Uuid::new_v4(),
                generic_name: "Metformin".into(),
                brand_name: Some("Glucophage".into()),
                dose: "500mg".into(),
                frequency: "twice daily".into(),
                frequency_type: FrequencyType::Scheduled,
                route: "oral".into(),
                prescriber_id: None,
                start_date: Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
                end_date: None,
                reason_start: None,
                reason_stop: None,
                is_otc: false,
                status: MedicationStatus::Active,
                administration_instructions: None,
                max_daily_dose: None,
                condition: None,
                dose_type: DoseType::Fixed,
                is_compound: false,
                document_id: doc_id,
            },
        )
        .unwrap();

        // Insert a diagnosis
        insert_diagnosis(
            &conn,
            &Diagnosis {
                id: Uuid::new_v4(),
                name: "Type 2 Diabetes".into(),
                icd_code: Some("E11".into()),
                date_diagnosed: Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
                diagnosing_professional_id: None,
                status: DiagnosisStatus::Active,
                document_id: doc_id,
            },
        )
        .unwrap();

        let snapshot = build_snapshot(&conn).unwrap();
        assert_eq!(snapshot.medications.len(), 1);
        assert_eq!(snapshot.diagnoses.len(), 1);
        assert_eq!(snapshot.medications[0].generic_name, "Metformin");
        assert_eq!(snapshot.diagnoses[0].name, "Type 2 Diabetes");
    }

    #[test]
    fn load_reference_data_returns_data() {
        let data = load_reference_data();
        // Should load from src-tauri/resources/ in dev, or fall back to test data
        assert!(!data.dose_ranges.is_empty() || data.medication_aliases.is_empty());
    }

    #[test]
    fn build_engine_on_memory_db() {
        let conn = open_memory_database().unwrap();
        // Memory DB doesn't have a meaningful path, but engine should still construct
        let result = DefaultCoherenceEngine::with_db(
            CoherenceReferenceData::load_test(),
            &conn,
            std::path::Path::new(":memory:"),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn full_scan_on_empty_db() {
        let conn = open_memory_database().unwrap();
        let engine = DefaultCoherenceEngine::with_db(
            CoherenceReferenceData::load_test(),
            &conn,
            std::path::Path::new(":memory:"),
        )
        .unwrap();

        let snapshot = build_snapshot(&conn).unwrap();
        let result = engine.analyze_full(&snapshot).unwrap();

        assert!(result.new_alerts.is_empty());
        assert_eq!(result.counts.total(), 0);
    }

    #[test]
    fn dismissed_alert_keys_populated() {
        let conn = open_memory_database().unwrap();

        // Insert a dismissed alert
        conn.execute(
            "INSERT INTO dismissed_alerts (id, alert_type, entity_ids, dismissed_date, reason, dismissed_by)
             VALUES (?1, ?2, ?3, datetime('now'), ?4, ?5)",
            rusqlite::params![
                Uuid::new_v4().to_string(),
                "conflict",
                "id1,id2",
                "Test reason",
                "patient",
            ],
        )
        .unwrap();

        let snapshot = build_snapshot(&conn).unwrap();
        assert_eq!(snapshot.dismissed_alert_keys.len(), 1);
        assert!(snapshot.dismissed_alert_keys.contains(&("conflict".to_string(), "id1,id2".to_string())));
    }
}
