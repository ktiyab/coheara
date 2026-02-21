//! Extraction Dispatcher — routes confirmed pending items to domain tables.
//!
//! Maps extracted JSON data to the appropriate domain-specific insert functions:
//! - Symptoms → journal::record_symptom()
//! - Medications → insert_medication() (with conversation-source document)
//! - Appointments → create_professional() + create_appointment()

use chrono::{Local, NaiveDate};
use rusqlite::Connection;
use uuid::Uuid;

use super::error::ExtractionError;
use super::types::{DispatchResult, ExtractionDomain, PendingReviewItem};
use crate::db::DatabaseError;
use crate::models::enums::{DoseType, FrequencyType, MedicationStatus};

/// Dispatch a confirmed pending item to its domain table.
///
/// Returns the ID of the created record.
pub fn dispatch_item(
    conn: &Connection,
    item: &PendingReviewItem,
) -> Result<DispatchResult, ExtractionError> {
    match item.domain {
        ExtractionDomain::Symptom => dispatch_symptom(conn, item),
        ExtractionDomain::Medication => dispatch_medication(conn, item),
        ExtractionDomain::Appointment => dispatch_appointment(conn, item),
    }
}

/// Dispatch a symptom extraction to the symptoms table via journal::record_symptom.
fn dispatch_symptom(
    conn: &Connection,
    item: &PendingReviewItem,
) -> Result<DispatchResult, ExtractionError> {
    let data = &item.extracted_data;

    let category = data
        .get("category")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ExtractionError::Validation("Symptom missing category".into()))?;

    let specific = data
        .get("specific")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ExtractionError::Validation("Symptom missing specific".into()))?;

    let severity = data
        .get("severity_hint")
        .and_then(|v| v.as_u64())
        .unwrap_or(3) as u8;

    let onset_date = data
        .get("onset_hint")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            // Default to today if no onset hint
            ""
        });

    let onset_date = if onset_date.is_empty() {
        Local::now().naive_local().format("%Y-%m-%d").to_string()
    } else {
        onset_date.to_string()
    };

    let entry = crate::journal::SymptomEntry {
        category: category.to_string(),
        specific: specific.to_string(),
        severity: severity.clamp(1, 5),
        onset_date,
        onset_time: data.get("onset_time").and_then(|v| v.as_str()).map(String::from),
        body_region: data.get("body_region").and_then(|v| v.as_str()).map(String::from),
        duration: data.get("duration").and_then(|v| v.as_str()).map(String::from),
        character: data.get("character").and_then(|v| v.as_str()).map(String::from),
        aggravating: extract_string_array(data, "aggravating"),
        relieving: extract_string_array(data, "relieving"),
        timing_pattern: data.get("timing_pattern").and_then(|v| v.as_str()).map(String::from),
        notes: data.get("notes").and_then(|v| v.as_str()).map(String::from),
    };

    let symptom_id = crate::journal::record_symptom(conn, &entry)
        .map_err(|e| ExtractionError::Database(e))?;

    Ok(DispatchResult {
        item_id: item.id.clone(),
        domain: ExtractionDomain::Symptom,
        success: true,
        created_record_id: Some(symptom_id.to_string()),
        error: None,
    })
}

/// Dispatch a medication extraction to the medications table.
///
/// Creates a conversation-source document record (type='other') to satisfy
/// the NOT NULL document_id FK constraint, since conversation-extracted
/// medications don't have a source document.
fn dispatch_medication(
    conn: &Connection,
    item: &PendingReviewItem,
) -> Result<DispatchResult, ExtractionError> {
    let data = &item.extracted_data;

    let name = data
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ExtractionError::Validation("Medication missing name".into()))?;

    // Create a conversation-source document for the FK constraint
    let doc_id = create_conversation_source_document(conn, &item.conversation_id, name)?;

    let med_id = Uuid::new_v4();
    let start_date = data
        .get("start_date_hint")
        .and_then(|v| v.as_str())
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

    let medication = crate::models::Medication {
        id: med_id,
        generic_name: name.to_string(),
        brand_name: data.get("brand_name").and_then(|v| v.as_str()).map(String::from),
        dose: data.get("dose").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        frequency: data.get("frequency").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        frequency_type: FrequencyType::Scheduled,
        route: data.get("route").and_then(|v| v.as_str()).unwrap_or("oral").to_string(),
        prescriber_id: None,
        start_date,
        end_date: None,
        reason_start: data.get("reason").and_then(|v| v.as_str()).map(String::from),
        reason_stop: None,
        is_otc: data.get("is_otc").and_then(|v| v.as_bool()).unwrap_or(false),
        status: MedicationStatus::Active,
        administration_instructions: data
            .get("instructions")
            .and_then(|v| v.as_str())
            .map(String::from),
        max_daily_dose: None,
        condition: data.get("condition").and_then(|v| v.as_str()).map(String::from),
        dose_type: DoseType::Fixed,
        is_compound: false,
        document_id: Uuid::parse_str(&doc_id)
            .map_err(|e| ExtractionError::Validation(format!("Invalid document UUID: {e}")))?,
    };

    crate::db::insert_medication(conn, &medication)
        .map_err(|e| ExtractionError::Database(e))?;

    Ok(DispatchResult {
        item_id: item.id.clone(),
        domain: ExtractionDomain::Medication,
        success: true,
        created_record_id: Some(med_id.to_string()),
        error: None,
    })
}

/// Dispatch an appointment extraction.
///
/// Creates a professional record if needed, then creates the appointment.
fn dispatch_appointment(
    conn: &Connection,
    item: &PendingReviewItem,
) -> Result<DispatchResult, ExtractionError> {
    let data = &item.extracted_data;

    let professional_name = data
        .get("professional_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");

    let specialty = data
        .get("specialty")
        .and_then(|v| v.as_str())
        .unwrap_or("General");

    let institution = data
        .get("institution")
        .and_then(|v| v.as_str())
        .map(String::from);

    // Create professional
    let prof = crate::appointment::NewProfessional {
        name: professional_name.to_string(),
        specialty: specialty.to_string(),
        institution,
    };

    let prof_id = crate::appointment::create_professional(conn, &prof)
        .map_err(|e| ExtractionError::Database(e))?;

    // Parse appointment date
    let date_str = data
        .get("date_hint")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ExtractionError::Validation("Appointment missing date".into()))?;

    let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|e| ExtractionError::Validation(format!("Invalid appointment date: {e}")))?;

    let appt_id = crate::appointment::create_appointment(conn, &prof_id, &date)
        .map_err(|e| ExtractionError::Database(e))?;

    Ok(DispatchResult {
        item_id: item.id.clone(),
        domain: ExtractionDomain::Appointment,
        success: true,
        created_record_id: Some(appt_id),
        error: None,
    })
}

/// Create a minimal document record as a source reference for conversation-extracted data.
///
/// Uses type='other' with a conversation:{id} source_file convention.
fn create_conversation_source_document(
    conn: &Connection,
    conversation_id: &str,
    item_name: &str,
) -> Result<String, ExtractionError> {
    let doc_id = Uuid::new_v4().to_string();
    let now = Local::now().naive_local().format("%Y-%m-%d %H:%M:%S").to_string();

    conn.execute(
        "INSERT INTO documents (id, type, title, ingestion_date, source_file, verified, source_deleted)
         VALUES (?1, 'other', ?2, ?3, ?4, 0, 0)",
        rusqlite::params![
            doc_id,
            format!("Chat extraction: {item_name}"),
            now,
            format!("conversation:{conversation_id}"),
        ],
    )
    .map_err(|e| ExtractionError::Database(DatabaseError::Sqlite(e)))?;

    Ok(doc_id)
}

/// Extract a JSON array of strings from a field, returning empty vec if missing.
fn extract_string_array(data: &serde_json::Value, field: &str) -> Vec<String> {
    data.get(field)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;
    use crate::pipeline::batch_extraction::store::create_pending_item;
    use crate::pipeline::batch_extraction::types::Grounding;

    fn setup_db() -> Connection {
        open_memory_database().expect("Failed to open in-memory DB")
    }

    fn make_symptom_item() -> PendingReviewItem {
        create_pending_item(
            "conv-1",
            "batch-1",
            ExtractionDomain::Symptom,
            serde_json::json!({
                "category": "Pain",
                "specific": "Headache",
                "severity_hint": 4,
                "body_region": "right side",
                "duration": "3 days",
                "character": "Throbbing",
                "aggravating": ["light", "noise"],
                "relieving": ["rest"],
                "notes": "Gets worse in the evening"
            }),
            0.85,
            Grounding::Grounded,
            None,
            vec!["msg-0".to_string()],
        )
    }

    fn make_appointment_item() -> PendingReviewItem {
        create_pending_item(
            "conv-1",
            "batch-1",
            ExtractionDomain::Appointment,
            serde_json::json!({
                "professional_name": "Dr. Martin",
                "specialty": "Neurology",
                "institution": "City Hospital",
                "date_hint": "2026-03-15"
            }),
            0.9,
            Grounding::Grounded,
            None,
            vec!["msg-1".to_string()],
        )
    }

    fn make_medication_item() -> PendingReviewItem {
        create_pending_item(
            "conv-1",
            "batch-1",
            ExtractionDomain::Medication,
            serde_json::json!({
                "name": "Ibuprofen",
                "dose": "400mg",
                "frequency": "3x daily",
                "route": "oral",
                "is_otc": true,
                "reason": "Headache management"
            }),
            0.8,
            Grounding::Grounded,
            None,
            vec!["msg-0".to_string()],
        )
    }

    #[test]
    fn dispatches_symptom_to_symptoms_table() {
        let conn = setup_db();
        let item = make_symptom_item();

        let result = dispatch_item(&conn, &item).unwrap();

        assert!(result.success);
        assert_eq!(result.domain, ExtractionDomain::Symptom);
        assert!(result.created_record_id.is_some());

        // Verify inserted in symptoms table
        let count: u32 = conn
            .query_row("SELECT COUNT(*) FROM symptoms WHERE category = 'Pain'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn dispatches_appointment_to_appointments_table() {
        let conn = setup_db();
        let item = make_appointment_item();

        let result = dispatch_item(&conn, &item).unwrap();

        assert!(result.success);
        assert_eq!(result.domain, ExtractionDomain::Appointment);

        // Verify professional created
        let prof_count: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM professionals WHERE name = 'Dr. Martin'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(prof_count, 1);

        // Verify appointment created
        let appt_count: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM appointments WHERE date = '2026-03-15'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(appt_count, 1);
    }

    #[test]
    fn dispatches_medication_with_source_document() {
        let conn = setup_db();
        let item = make_medication_item();

        let result = dispatch_item(&conn, &item).unwrap();

        assert!(result.success);
        assert_eq!(result.domain, ExtractionDomain::Medication);

        // Verify medication inserted
        let med_count: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM medications WHERE generic_name = 'Ibuprofen'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(med_count, 1);

        // Verify conversation-source document created
        let doc_count: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM documents WHERE source_file LIKE 'conversation:%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(doc_count, 1);
    }

    #[test]
    fn symptom_missing_category_returns_error() {
        let conn = setup_db();
        let item = create_pending_item(
            "conv-1",
            "batch-1",
            ExtractionDomain::Symptom,
            serde_json::json!({"specific": "Headache"}), // no category
            0.8,
            Grounding::Grounded,
            None,
            vec![],
        );

        let result = dispatch_item(&conn, &item);
        assert!(result.is_err());
    }

    #[test]
    fn appointment_missing_date_returns_error() {
        let conn = setup_db();
        let item = create_pending_item(
            "conv-1",
            "batch-1",
            ExtractionDomain::Appointment,
            serde_json::json!({
                "professional_name": "Dr. Smith",
                "specialty": "General"
            }), // no date_hint
            0.8,
            Grounding::Grounded,
            None,
            vec![],
        );

        let result = dispatch_item(&conn, &item);
        assert!(result.is_err());
    }

    #[test]
    fn medication_missing_name_returns_error() {
        let conn = setup_db();
        let item = create_pending_item(
            "conv-1",
            "batch-1",
            ExtractionDomain::Medication,
            serde_json::json!({"dose": "500mg"}), // no name
            0.8,
            Grounding::Grounded,
            None,
            vec![],
        );

        let result = dispatch_item(&conn, &item);
        assert!(result.is_err());
    }

    #[test]
    fn symptom_defaults_severity_to_three() {
        let conn = setup_db();
        let item = create_pending_item(
            "conv-1",
            "batch-1",
            ExtractionDomain::Symptom,
            serde_json::json!({
                "category": "Digestive",
                "specific": "Nausea"
                // no severity_hint
            }),
            0.7,
            Grounding::Partial,
            None,
            vec![],
        );

        let result = dispatch_item(&conn, &item).unwrap();
        assert!(result.success);

        let severity: i32 = conn
            .query_row(
                "SELECT severity FROM symptoms WHERE specific = 'Nausea'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(severity, 3);
    }

    #[test]
    fn extract_string_array_handles_missing() {
        let data = serde_json::json!({"name": "test"});
        assert!(extract_string_array(&data, "aggravating").is_empty());
    }

    #[test]
    fn extract_string_array_parses_values() {
        let data = serde_json::json!({"items": ["a", "b", "c"]});
        let result = extract_string_array(&data, "items");
        assert_eq!(result, vec!["a", "b", "c"]);
    }
}
