//! Duplicate detection for extracted items.
//!
//! Checks if an extracted item matches existing records in the database
//! to avoid creating duplicate entries. Returns a `DuplicateStatus` that
//! the frontend shows as a warning on the ReviewCard.

use chrono::NaiveDate;
use rusqlite::Connection;

use super::types::{DuplicateStatus, ExtractionDomain};

/// Check if an extracted item duplicates an existing DB record.
///
/// Returns `New` if no match, `AlreadyTracked` for exact match,
/// or `PossibleDuplicate` for similar but not exact.
pub fn check_duplicate(
    conn: &Connection,
    domain: ExtractionDomain,
    extracted_data: &serde_json::Value,
    conversation_date: NaiveDate,
) -> DuplicateStatus {
    match domain {
        ExtractionDomain::Symptom => check_symptom_duplicate(conn, extracted_data, conversation_date),
        ExtractionDomain::Medication => check_medication_duplicate(conn, extracted_data),
        ExtractionDomain::Appointment => check_appointment_duplicate(conn, extracted_data),
    }
}

/// Symptom: exact match on (category + specific) within 7 days of conversation.
fn check_symptom_duplicate(
    conn: &Connection,
    data: &serde_json::Value,
    conversation_date: NaiveDate,
) -> DuplicateStatus {
    let category = data.get("category").and_then(|v| v.as_str()).unwrap_or("");
    let specific = data.get("specific").and_then(|v| v.as_str()).unwrap_or("");

    if category.is_empty() || specific.is_empty() {
        return DuplicateStatus::New;
    }

    let from = conversation_date - chrono::Duration::days(7);
    let to = conversation_date + chrono::Duration::days(1);
    let from_str = from.format("%Y-%m-%d").to_string();
    let to_str = to.format("%Y-%m-%d").to_string();

    // Direct query — avoids pulling all symptoms through the repo layer
    let result: Result<(String,), _> = conn.query_row(
        "SELECT id FROM symptoms
         WHERE LOWER(category) = LOWER(?1)
           AND LOWER(specific) = LOWER(?2)
           AND onset_date >= ?3
           AND onset_date <= ?4
         LIMIT 1",
        rusqlite::params![category, specific, from_str, to_str],
        |row| Ok((row.get(0)?,)),
    );

    if let Ok((existing_id,)) = result {
        return DuplicateStatus::AlreadyTracked { existing_id };
    }

    // Same category, different specific → possible duplicate
    let result: Result<(String,), _> = conn.query_row(
        "SELECT id FROM symptoms
         WHERE LOWER(category) = LOWER(?1)
           AND onset_date >= ?2
           AND onset_date <= ?3
         LIMIT 1",
        rusqlite::params![category, from_str, to_str],
        |row| Ok((row.get(0)?,)),
    );

    if let Ok((existing_id,)) = result {
        return DuplicateStatus::PossibleDuplicate { existing_id };
    }

    DuplicateStatus::New
}

/// Medication: match on generic_name (case-insensitive).
fn check_medication_duplicate(
    conn: &Connection,
    data: &serde_json::Value,
) -> DuplicateStatus {
    let name = data.get("name").and_then(|v| v.as_str()).unwrap_or("");

    if name.is_empty() {
        return DuplicateStatus::New;
    }

    let result: Result<(String,), _> = conn.query_row(
        "SELECT id FROM medications
         WHERE LOWER(generic_name) = LOWER(?1) OR LOWER(brand_name) = LOWER(?1)
         LIMIT 1",
        rusqlite::params![name],
        |row| Ok((row.get(0)?,)),
    );

    if let Ok((existing_id,)) = result {
        return DuplicateStatus::AlreadyTracked { existing_id };
    }

    // Partial name match (LIKE)
    let pattern = format!("%{name}%");
    let result: Result<(String,), _> = conn.query_row(
        "SELECT id FROM medications
         WHERE LOWER(generic_name) LIKE LOWER(?1) OR LOWER(brand_name) LIKE LOWER(?1)
         LIMIT 1",
        rusqlite::params![pattern],
        |row| Ok((row.get(0)?,)),
    );

    if let Ok((existing_id,)) = result {
        return DuplicateStatus::PossibleDuplicate { existing_id };
    }

    DuplicateStatus::New
}

/// Appointment: match on date + professional name.
fn check_appointment_duplicate(
    conn: &Connection,
    data: &serde_json::Value,
) -> DuplicateStatus {
    let date_hint = data.get("date_hint").and_then(|v| v.as_str()).unwrap_or("");
    let professional_name = data.get("professional_name").and_then(|v| v.as_str()).unwrap_or("");

    if date_hint.is_empty() {
        return DuplicateStatus::New;
    }

    if !professional_name.is_empty() {
        // Match both date and professional
        let result: Result<(String,), _> = conn.query_row(
            "SELECT a.id FROM appointments a
             JOIN professionals p ON a.professional_id = p.id
             WHERE a.date = ?1 AND LOWER(p.name) = LOWER(?2)
             LIMIT 1",
            rusqlite::params![date_hint, professional_name],
            |row| Ok((row.get(0)?,)),
        );

        if let Ok((existing_id,)) = result {
            return DuplicateStatus::AlreadyTracked { existing_id };
        }
    }

    // Match date only (any professional)
    let result: Result<(String,), _> = conn.query_row(
        "SELECT id FROM appointments WHERE date = ?1 LIMIT 1",
        rusqlite::params![date_hint],
        |row| Ok((row.get(0)?,)),
    );

    if let Ok((existing_id,)) = result {
        if professional_name.is_empty() {
            return DuplicateStatus::AlreadyTracked { existing_id };
        }
        return DuplicateStatus::PossibleDuplicate { existing_id };
    }

    DuplicateStatus::New
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;

    fn setup_db() -> Connection {
        let conn = open_memory_database().expect("Failed to open in-memory DB");

        // Seed a document (FK for medications)
        conn.execute(
            "INSERT INTO documents (id, type, title, ingestion_date, source_file)
             VALUES ('doc-test', 'prescription', 'Test Rx', '2026-02-20', '/test.pdf')",
            [],
        ).unwrap();

        // Seed a professional (FK for appointments)
        conn.execute(
            "INSERT INTO professionals (id, name, specialty)
             VALUES ('prof-1', 'Dr. Martin', 'Neurologist')",
            [],
        ).unwrap();

        conn
    }

    #[test]
    fn symptom_exact_duplicate_detected() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, source)
             VALUES ('sym-1', 'Pain', 'Headache', 3, '2026-02-19', '2026-02-19', 'patient_reported')",
            [],
        ).unwrap();

        let data = serde_json::json!({"category": "Pain", "specific": "Headache"});
        let conv_date = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();

        let status = check_duplicate(&conn, ExtractionDomain::Symptom, &data, conv_date);
        assert!(matches!(status, DuplicateStatus::AlreadyTracked { .. }));
    }

    #[test]
    fn symptom_possible_duplicate_same_category() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, source)
             VALUES ('sym-1', 'Pain', 'Back pain', 4, '2026-02-18', '2026-02-18', 'patient_reported')",
            [],
        ).unwrap();

        let data = serde_json::json!({"category": "Pain", "specific": "Headache"});
        let conv_date = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();

        let status = check_duplicate(&conn, ExtractionDomain::Symptom, &data, conv_date);
        assert!(matches!(status, DuplicateStatus::PossibleDuplicate { .. }));
    }

    #[test]
    fn symptom_no_duplicate_outside_window() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, source)
             VALUES ('sym-1', 'Pain', 'Headache', 3, '2026-01-01', '2026-01-01', 'patient_reported')",
            [],
        ).unwrap();

        let data = serde_json::json!({"category": "Pain", "specific": "Headache"});
        let conv_date = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();

        let status = check_duplicate(&conn, ExtractionDomain::Symptom, &data, conv_date);
        assert!(matches!(status, DuplicateStatus::New));
    }

    #[test]
    fn symptom_new_when_empty_db() {
        let conn = setup_db();
        let data = serde_json::json!({"category": "Pain", "specific": "Headache"});
        let conv_date = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();

        let status = check_duplicate(&conn, ExtractionDomain::Symptom, &data, conv_date);
        assert!(matches!(status, DuplicateStatus::New));
    }

    #[test]
    fn medication_exact_duplicate_by_generic_name() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, document_id)
             VALUES ('med-1', 'Ibuprofen', '400mg', 'as needed', 'as_needed', 'active', 'doc-test')",
            [],
        ).unwrap();

        let data = serde_json::json!({"name": "Ibuprofen"});
        let conv_date = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();

        let status = check_duplicate(&conn, ExtractionDomain::Medication, &data, conv_date);
        assert!(matches!(status, DuplicateStatus::AlreadyTracked { .. }));
    }

    #[test]
    fn medication_exact_duplicate_case_insensitive() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, document_id)
             VALUES ('med-1', 'Lisinopril', '10mg', 'once daily', 'scheduled', 'active', 'doc-test')",
            [],
        ).unwrap();

        let data = serde_json::json!({"name": "lisinopril"});
        let conv_date = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();

        let status = check_duplicate(&conn, ExtractionDomain::Medication, &data, conv_date);
        assert!(matches!(status, DuplicateStatus::AlreadyTracked { .. }));
    }

    #[test]
    fn medication_new_when_different_name() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, document_id)
             VALUES ('med-1', 'Lisinopril', '10mg', 'once daily', 'scheduled', 'active', 'doc-test')",
            [],
        ).unwrap();

        let data = serde_json::json!({"name": "Metformin"});
        let conv_date = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();

        let status = check_duplicate(&conn, ExtractionDomain::Medication, &data, conv_date);
        assert!(matches!(status, DuplicateStatus::New));
    }

    #[test]
    fn appointment_exact_duplicate_by_date_and_professional() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO appointments (id, professional_id, date, type)
             VALUES ('apt-1', 'prof-1', '2026-02-25', 'upcoming')",
            [],
        ).unwrap();

        let data = serde_json::json!({"date_hint": "2026-02-25", "professional_name": "Dr. Martin"});
        let conv_date = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();

        let status = check_duplicate(&conn, ExtractionDomain::Appointment, &data, conv_date);
        assert!(matches!(status, DuplicateStatus::AlreadyTracked { .. }));
    }

    #[test]
    fn appointment_possible_duplicate_same_date_different_doctor() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO appointments (id, professional_id, date, type)
             VALUES ('apt-1', 'prof-1', '2026-02-25', 'upcoming')",
            [],
        ).unwrap();

        let data = serde_json::json!({"date_hint": "2026-02-25", "professional_name": "Dr. Smith"});
        let conv_date = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();

        let status = check_duplicate(&conn, ExtractionDomain::Appointment, &data, conv_date);
        assert!(matches!(status, DuplicateStatus::PossibleDuplicate { .. }));
    }

    #[test]
    fn appointment_new_when_no_date() {
        let conn = setup_db();
        let data = serde_json::json!({"professional_name": "Dr. Martin"});
        let conv_date = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();

        let status = check_duplicate(&conn, ExtractionDomain::Appointment, &data, conv_date);
        assert!(matches!(status, DuplicateStatus::New));
    }

    #[test]
    fn empty_data_returns_new() {
        let conn = setup_db();
        let data = serde_json::json!({});
        let conv_date = NaiveDate::from_ymd_opt(2026, 2, 20).unwrap();

        for domain in ExtractionDomain::all() {
            let status = check_duplicate(&conn, *domain, &data, conv_date);
            assert!(matches!(status, DuplicateStatus::New), "Expected New for empty data in {domain}");
        }
    }
}
