//! Extraction Dispatcher — routes confirmed pending items to domain tables.
//!
//! Maps extracted JSON data to the appropriate domain-specific insert functions:
//! - Symptoms → journal::record_symptom() + detect_temporal_correlation()
//! - Medications → insert_medication() (with conversation-source document)
//! - Appointments → create_professional() (with dedup) + create_appointment()

use chrono::{Local, NaiveDate};
use rusqlite::Connection;
use tracing::warn;
use uuid::Uuid;

use super::duplicate::{check_duplicate};
use super::error::ExtractionError;
use super::types::{DispatchResult, DuplicateStatus, ExtractionDomain, PendingReviewItem};
use crate::db::DatabaseError;
use crate::models::enums::{DoseType, FrequencyType, MedicationStatus};

/// Dispatch a confirmed pending item to its domain table.
///
/// Returns the ID of the created record, plus optional correlations/warnings.
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
///
/// Validates category against CATEGORIES whitelist, body_region against BODY_REGIONS,
/// then calls detect_temporal_correlation for medication change correlations.
fn dispatch_symptom(
    conn: &Connection,
    item: &PendingReviewItem,
) -> Result<DispatchResult, ExtractionError> {
    let data = &item.extracted_data;

    let raw_category = data
        .get("category")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ExtractionError::Validation("Symptom missing category".into()))?;

    let specific = data
        .get("specific")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ExtractionError::Validation("Symptom missing specific".into()))?;

    // Validate category against whitelist — clamp to "Other" if invalid
    let category = if crate::journal::CATEGORIES.contains(&raw_category) {
        raw_category.to_string()
    } else {
        warn!(
            raw_category = raw_category,
            "Extracted category not in CATEGORIES whitelist, clamping to Other"
        );
        "Other".to_string()
    };

    // Validate body_region against whitelist — set to None if invalid
    let body_region = data
        .get("body_region")
        .and_then(|v| v.as_str())
        .and_then(|br| {
            if crate::journal::BODY_REGIONS.contains(&br) {
                Some(br.to_string())
            } else {
                warn!(
                    body_region = br,
                    "Extracted body_region not in BODY_REGIONS whitelist, discarding"
                );
                None
            }
        });

    let severity = data
        .get("severity_hint")
        .and_then(|v| v.as_u64())
        .unwrap_or(3) as u8;

    let onset_date = data
        .get("onset_hint")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let onset_date = if onset_date.is_empty() {
        Local::now().naive_local().format("%Y-%m-%d").to_string()
    } else {
        onset_date.to_string()
    };

    let entry = crate::journal::SymptomEntry {
        category,
        specific: specific.to_string(),
        severity: severity.clamp(1, 5),
        onset_date: onset_date.clone(),
        onset_time: data.get("onset_time").and_then(|v| v.as_str()).map(String::from),
        body_region,
        duration: data.get("duration").and_then(|v| v.as_str()).map(String::from),
        character: data.get("character").and_then(|v| v.as_str()).map(String::from),
        aggravating: extract_string_array(data, "aggravating"),
        relieving: extract_string_array(data, "relieving"),
        timing_pattern: data.get("timing_pattern").and_then(|v| v.as_str()).map(String::from),
        notes: data.get("notes").and_then(|v| v.as_str()).map(String::from),
    };

    let symptom_id = crate::journal::record_symptom(conn, &entry)
        .map_err(ExtractionError::Database)?;

    // Detect temporal correlations with medication changes
    let correlations = crate::journal::detect_temporal_correlation(conn, &onset_date)
        .map_err(ExtractionError::Database)?;

    let correlations_opt = if correlations.is_empty() {
        None
    } else {
        Some(correlations)
    };

    // Check for duplicate warning
    let duplicate_warning = match check_duplicate(
        conn,
        ExtractionDomain::Symptom,
        data,
        Local::now().naive_local().date(),
    ) {
        DuplicateStatus::PossibleDuplicate { .. } => {
            Some("Similar symptom recently recorded".to_string())
        }
        _ => None,
    };

    Ok(DispatchResult {
        item_id: item.id.clone(),
        domain: ExtractionDomain::Symptom,
        success: true,
        created_record_id: Some(symptom_id.to_string()),
        error: None,
        correlations: correlations_opt,
        duplicate_warning,
    })
}

/// Dispatch a medication extraction to the medications table.
///
/// Creates a conversation-source document record (type='other') to satisfy
/// the NOT NULL document_id FK constraint. Defaults is_otc to true for
/// chat-extracted medications. Validates field lengths.
fn dispatch_medication(
    conn: &Connection,
    item: &PendingReviewItem,
) -> Result<DispatchResult, ExtractionError> {
    let data = &item.extracted_data;

    let name = data
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ExtractionError::Validation("Medication missing name".into()))?;

    // Truncate fields to max lengths (name: 200, dose: 100, frequency: 200)
    let name = truncate_str(name, 200);
    let dose = data.get("dose").and_then(|v| v.as_str()).unwrap_or("");
    let dose = truncate_str(dose, 100);
    let frequency = data.get("frequency").and_then(|v| v.as_str()).unwrap_or("");
    let frequency = truncate_str(frequency, 200);

    // Create a conversation-source document for the FK constraint
    let doc_id = create_conversation_source_document(conn, &item.conversation_id, &name)?;

    let med_id = Uuid::new_v4();
    let start_date = data
        .get("start_date_hint")
        .and_then(|v| v.as_str())
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

    // Default is_otc to true for chat-extracted medications
    let is_otc = data.get("is_otc").and_then(|v| v.as_bool()).unwrap_or(true);

    let medication = crate::models::Medication {
        id: med_id,
        generic_name: name.to_string(),
        brand_name: data.get("brand_name").and_then(|v| v.as_str()).map(String::from),
        dose: dose.to_string(),
        frequency: frequency.to_string(),
        frequency_type: FrequencyType::Scheduled,
        route: data.get("route").and_then(|v| v.as_str()).unwrap_or("oral").to_string(),
        prescriber_id: None,
        start_date,
        end_date: None,
        reason_start: data.get("reason").and_then(|v| v.as_str()).map(String::from),
        reason_stop: None,
        is_otc,
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
        .map_err(ExtractionError::Database)?;

    // Check for duplicate warning
    let duplicate_warning = match check_duplicate(
        conn,
        ExtractionDomain::Medication,
        data,
        Local::now().naive_local().date(),
    ) {
        DuplicateStatus::AlreadyTracked { .. } => {
            Some(format!("Medication '{}' already exists", name))
        }
        DuplicateStatus::PossibleDuplicate { .. } => {
            Some(format!("Similar medication to '{}' found", name))
        }
        DuplicateStatus::New => None,
    };

    Ok(DispatchResult {
        item_id: item.id.clone(),
        domain: ExtractionDomain::Medication,
        success: true,
        created_record_id: Some(med_id.to_string()),
        error: None,
        correlations: None,
        duplicate_warning,
    })
}

/// Dispatch an appointment extraction.
///
/// Deduplicates professionals by name (case-insensitive) before creating.
/// Validates specialty against SPECIALTIES whitelist.
fn dispatch_appointment(
    conn: &Connection,
    item: &PendingReviewItem,
) -> Result<DispatchResult, ExtractionError> {
    let data = &item.extracted_data;

    let professional_name = data
        .get("professional_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");

    let raw_specialty = data
        .get("specialty")
        .and_then(|v| v.as_str())
        .unwrap_or("General");

    // Validate specialty against whitelist
    let specialty = if crate::appointment::SPECIALTIES.contains(&raw_specialty) || raw_specialty == "Other" {
        raw_specialty.to_string()
    } else {
        warn!(
            raw_specialty = raw_specialty,
            "Extracted specialty not in SPECIALTIES whitelist, defaulting to Other"
        );
        "Other".to_string()
    };

    let institution = data
        .get("institution")
        .and_then(|v| v.as_str())
        .map(String::from);

    // Deduplicate professional by case-insensitive name match
    let prof_id = find_existing_professional(conn, professional_name)
        .unwrap_or_else(|| {
            let prof = crate::appointment::NewProfessional {
                name: professional_name.to_string(),
                specialty: specialty.clone(),
                institution,
            };
            crate::appointment::create_professional(conn, &prof)
                .unwrap_or_else(|e| {
                    warn!(error = %e, "Failed to create professional, using placeholder");
                    Uuid::new_v4().to_string()
                })
        });

    // Parse appointment date
    let date_str = data
        .get("date_hint")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ExtractionError::Validation("Appointment missing date".into()))?;

    let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|e| ExtractionError::Validation(format!("Invalid appointment date: {e}")))?;

    let appt_id = crate::appointment::create_appointment(conn, &prof_id, &date)
        .map_err(ExtractionError::Database)?;

    Ok(DispatchResult {
        item_id: item.id.clone(),
        domain: ExtractionDomain::Appointment,
        success: true,
        created_record_id: Some(appt_id),
        error: None,
        correlations: None,
        duplicate_warning: None,
    })
}

/// Find an existing professional by case-insensitive name match.
/// Returns the professional_id if found, None otherwise.
fn find_existing_professional(conn: &Connection, name: &str) -> Option<String> {
    conn.query_row(
        "SELECT id FROM professionals WHERE LOWER(name) = LOWER(?1) LIMIT 1",
        rusqlite::params![name],
        |row| row.get::<_, String>(0),
    )
    .ok()
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

/// Truncate a string to a maximum byte length, cutting at a char boundary.
fn truncate_str(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        return s;
    }
    // Find char boundary at or before max_len
    let mut end = max_len;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
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
                "body_region": "head",
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
                "specialty": "Neurologist",
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
                "specialty": "GP"
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

    // ── New validation tests ──

    #[test]
    fn symptom_category_clamped_to_other_when_invalid() {
        let conn = setup_db();
        let item = create_pending_item(
            "conv-1",
            "batch-1",
            ExtractionDomain::Symptom,
            serde_json::json!({
                "category": "InvalidCategory",
                "specific": "Something",
                "severity_hint": 3
            }),
            0.7,
            Grounding::Grounded,
            None,
            vec![],
        );

        let result = dispatch_item(&conn, &item).unwrap();
        assert!(result.success);

        let category: String = conn
            .query_row(
                "SELECT category FROM symptoms WHERE specific = 'Something'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(category, "Other");
    }

    #[test]
    fn symptom_body_region_discarded_when_invalid() {
        let conn = setup_db();
        let item = create_pending_item(
            "conv-1",
            "batch-1",
            ExtractionDomain::Symptom,
            serde_json::json!({
                "category": "Pain",
                "specific": "Headache",
                "body_region": "left_eyebrow",
                "severity_hint": 2
            }),
            0.7,
            Grounding::Grounded,
            None,
            vec![],
        );

        let result = dispatch_item(&conn, &item).unwrap();
        assert!(result.success);

        let region: Option<String> = conn
            .query_row(
                "SELECT body_region FROM symptoms WHERE specific = 'Headache'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(region.is_none(), "Invalid body_region should be discarded");
    }

    #[test]
    fn symptom_dispatch_returns_correlations_when_medication_changed() {
        let conn = setup_db();

        // Seed a medication change near onset date
        let doc_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO documents (id, type, title, ingestion_date, source_file)
             VALUES (?1, 'prescription', 'Test Rx', '2026-02-01', '/test.pdf')",
            rusqlite::params![doc_id],
        ).unwrap();
        let med_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, start_date, document_id)
             VALUES (?1, 'Lisinopril', '10mg', 'daily', 'scheduled', 'active', '2026-02-18', ?2)",
            rusqlite::params![med_id, doc_id],
        ).unwrap();

        let item = create_pending_item(
            "conv-1",
            "batch-1",
            ExtractionDomain::Symptom,
            serde_json::json!({
                "category": "Pain",
                "specific": "Headache",
                "severity_hint": 3,
                "onset_hint": "2026-02-20"
            }),
            0.8,
            Grounding::Grounded,
            None,
            vec![],
        );

        let result = dispatch_item(&conn, &item).unwrap();
        assert!(result.success);
        assert!(result.correlations.is_some(), "Should detect temporal correlation");
        let correlations = result.correlations.unwrap();
        assert!(!correlations.is_empty());
        assert_eq!(correlations[0].medication_name, "Lisinopril");
    }

    #[test]
    fn medication_defaults_is_otc_true() {
        let conn = setup_db();
        let item = create_pending_item(
            "conv-1",
            "batch-1",
            ExtractionDomain::Medication,
            serde_json::json!({
                "name": "Aspirin",
                "dose": "100mg"
                // no is_otc field
            }),
            0.8,
            Grounding::Grounded,
            None,
            vec![],
        );

        let result = dispatch_item(&conn, &item).unwrap();
        assert!(result.success);

        let is_otc: bool = conn
            .query_row(
                "SELECT is_otc FROM medications WHERE generic_name = 'Aspirin'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(is_otc, "Chat-extracted medications should default to OTC");
    }

    #[test]
    fn medication_name_length_truncated() {
        let conn = setup_db();
        let long_name = "A".repeat(250);
        let item = create_pending_item(
            "conv-1",
            "batch-1",
            ExtractionDomain::Medication,
            serde_json::json!({
                "name": long_name,
                "dose": "100mg"
            }),
            0.8,
            Grounding::Grounded,
            None,
            vec![],
        );

        let result = dispatch_item(&conn, &item).unwrap();
        assert!(result.success);

        let stored_name: String = conn
            .query_row(
                "SELECT generic_name FROM medications ORDER BY rowid DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(stored_name.len() <= 200, "Name should be truncated to 200 chars");
    }

    #[test]
    fn appointment_reuses_existing_professional() {
        let conn = setup_db();

        // Pre-create a professional
        conn.execute(
            "INSERT INTO professionals (id, name, specialty)
             VALUES ('prof-existing', 'Dr. Martin', 'Neurologist')",
            [],
        ).unwrap();

        let item = make_appointment_item(); // has "Dr. Martin"

        let result = dispatch_item(&conn, &item).unwrap();
        assert!(result.success);

        // Should still have only 1 professional (reused existing)
        let prof_count: u32 = conn
            .query_row("SELECT COUNT(*) FROM professionals", [], |row| row.get(0))
            .unwrap();
        assert_eq!(prof_count, 1, "Should reuse existing professional, not create duplicate");
    }

    #[test]
    fn appointment_specialty_defaults_to_other() {
        let conn = setup_db();
        let item = create_pending_item(
            "conv-1",
            "batch-1",
            ExtractionDomain::Appointment,
            serde_json::json!({
                "professional_name": "Dr. New",
                "specialty": "Podiatrist",
                "date_hint": "2026-03-20"
            }),
            0.8,
            Grounding::Grounded,
            None,
            vec![],
        );

        let result = dispatch_item(&conn, &item).unwrap();
        assert!(result.success);

        let specialty: String = conn
            .query_row(
                "SELECT specialty FROM professionals WHERE name = 'Dr. New'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(specialty, "Other", "Invalid specialty should default to Other");
    }

    #[test]
    fn truncate_str_preserves_short_strings() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn truncate_str_truncates_long_strings() {
        let s = "a".repeat(300);
        assert_eq!(truncate_str(&s, 200).len(), 200);
    }

    #[test]
    fn truncate_str_respects_char_boundaries() {
        let s = "héllo world"; // é is 2 bytes
        let result = truncate_str(s, 2);
        assert_eq!(result, "h"); // Can't cut in middle of é
    }
}
