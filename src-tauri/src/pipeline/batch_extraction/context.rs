//! Patient context loading for the extraction pipeline.
//!
//! Loads current patient data from the database to provide disambiguation
//! context to the LLM during extraction. Active medications, recent symptoms,
//! allergies, and known professionals help the LLM produce better extractions.

use chrono::Utc;
use rusqlite::Connection;

use super::error::ExtractionError;
use super::types::{ActiveMedicationSummary, PatientContext, ProfessionalSummary, RecentSymptomSummary};
use crate::db::repository::{
    get_active_medications, get_all_allergies, get_all_professionals, get_symptoms_in_date_range,
};

/// Load patient context from the database for extraction disambiguation.
///
/// Pulls active medications, recent symptoms (last 30 days), all allergies,
/// and all known professionals. Falls back to empty context on any failure.
pub fn load_patient_context(conn: &Connection) -> Result<PatientContext, ExtractionError> {
    let today = Utc::now().date_naive();
    let thirty_days_ago = today - chrono::Duration::days(30);

    let active_medications = load_active_medications(conn)?;
    let recent_symptoms = load_recent_symptoms(conn, &thirty_days_ago, &today)?;
    let known_allergies = load_known_allergies(conn)?;
    let known_professionals = load_known_professionals(conn)?;

    Ok(PatientContext {
        active_medications,
        recent_symptoms,
        known_allergies,
        known_professionals,
        date_of_birth: None, // Profile DOB not yet in schema
    })
}

fn load_active_medications(conn: &Connection) -> Result<Vec<ActiveMedicationSummary>, ExtractionError> {
    let meds = get_active_medications(conn)?;
    Ok(meds
        .iter()
        .map(|m| ActiveMedicationSummary {
            name: m.generic_name.clone(),
            dose: m.dose.clone(),
            frequency: m.frequency.clone(),
        })
        .collect())
}

fn load_recent_symptoms(
    conn: &Connection,
    from: &chrono::NaiveDate,
    to: &chrono::NaiveDate,
) -> Result<Vec<RecentSymptomSummary>, ExtractionError> {
    let symptoms = get_symptoms_in_date_range(conn, from, to)?;
    Ok(symptoms
        .iter()
        .map(|s| RecentSymptomSummary {
            category: s.category.clone(),
            specific: s.specific.clone(),
            severity: s.severity,
            onset_date: s.onset_date,
        })
        .collect())
}

fn load_known_allergies(conn: &Connection) -> Result<Vec<String>, ExtractionError> {
    let allergies = get_all_allergies(conn)?;
    Ok(allergies.iter().map(|a| a.allergen.clone()).collect())
}

fn load_known_professionals(conn: &Connection) -> Result<Vec<ProfessionalSummary>, ExtractionError> {
    let professionals = get_all_professionals(conn)?;
    Ok(professionals
        .iter()
        .map(|p| ProfessionalSummary {
            name: p.name.clone(),
            specialty: p.specialty.clone(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;
    use uuid::Uuid;

    fn setup_db() -> Connection {
        let conn = open_memory_database().expect("Failed to open in-memory DB");

        // Seed a document (FK for medications)
        conn.execute(
            "INSERT INTO documents (id, type, title, ingestion_date, source_file)
             VALUES ('00000000-0000-4000-8000-000000000001', 'prescription', 'Test Rx', '2026-02-20', '/test.pdf')",
            [],
        )
        .unwrap();

        conn
    }

    #[test]
    fn loads_empty_context_from_empty_db() {
        let conn = setup_db();
        let ctx = load_patient_context(&conn).unwrap();

        assert!(ctx.active_medications.is_empty());
        assert!(ctx.recent_symptoms.is_empty());
        assert!(ctx.known_allergies.is_empty());
        assert!(ctx.known_professionals.is_empty());
        assert!(ctx.date_of_birth.is_none());
    }

    #[test]
    fn loads_active_medications() {
        let conn = setup_db();
        let med1 = Uuid::new_v4().to_string();
        let med2 = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, document_id)
             VALUES (?1, 'Lisinopril', '10mg', 'once daily', 'scheduled', 'active', '00000000-0000-4000-8000-000000000001')",
            rusqlite::params![med1],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, document_id)
             VALUES (?1, 'Aspirin', '81mg', 'once daily', 'scheduled', 'stopped', '00000000-0000-4000-8000-000000000001')",
            rusqlite::params![med2],
        )
        .unwrap();

        let ctx = load_patient_context(&conn).unwrap();

        assert_eq!(ctx.active_medications.len(), 1);
        assert_eq!(ctx.active_medications[0].name, "Lisinopril");
        assert_eq!(ctx.active_medications[0].dose, "10mg");
    }

    #[test]
    fn loads_recent_symptoms() {
        let conn = setup_db();
        let today = Utc::now().date_naive();
        let today_str = today.format("%Y-%m-%d").to_string();
        let sym1 = Uuid::new_v4().to_string();
        let sym2 = Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, source)
             VALUES (?1, 'Pain', 'Headache', 3, ?2, ?2, 'patient_reported')",
            rusqlite::params![sym1, today_str],
        )
        .unwrap();

        // Old symptom (outside 30-day window)
        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, source)
             VALUES (?1, 'Pain', 'Back pain', 2, '2025-01-01', '2025-01-01', 'patient_reported')",
            rusqlite::params![sym2],
        )
        .unwrap();

        let ctx = load_patient_context(&conn).unwrap();

        assert_eq!(ctx.recent_symptoms.len(), 1);
        assert_eq!(ctx.recent_symptoms[0].specific, "Headache");
        assert_eq!(ctx.recent_symptoms[0].severity, 3);
    }

    #[test]
    fn loads_known_allergies() {
        let conn = setup_db();
        let alg1 = Uuid::new_v4().to_string();
        let alg2 = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO allergies (id, allergen, severity, source, verified)
             VALUES (?1, 'Penicillin', 'severe', 'patient_reported', 1)",
            rusqlite::params![alg1],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO allergies (id, allergen, severity, source, verified)
             VALUES (?1, 'Sulfa', 'moderate', 'document_extracted', 0)",
            rusqlite::params![alg2],
        )
        .unwrap();

        let ctx = load_patient_context(&conn).unwrap();

        assert_eq!(ctx.known_allergies.len(), 2);
        assert!(ctx.known_allergies.contains(&"Penicillin".to_string()));
        assert!(ctx.known_allergies.contains(&"Sulfa".to_string()));
    }

    #[test]
    fn loads_known_professionals() {
        let conn = setup_db();
        let prof1 = Uuid::new_v4().to_string();
        let prof2 = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO professionals (id, name, specialty)
             VALUES (?1, 'Dr. Martin', 'Neurologist')",
            rusqlite::params![prof1],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO professionals (id, name)
             VALUES (?1, 'Dr. Chen')",
            rusqlite::params![prof2],
        )
        .unwrap();

        let ctx = load_patient_context(&conn).unwrap();

        assert_eq!(ctx.known_professionals.len(), 2);
        assert_eq!(ctx.known_professionals[0].name, "Dr. Martin");
        assert_eq!(ctx.known_professionals[0].specialty, Some("Neurologist".to_string()));
        assert_eq!(ctx.known_professionals[1].name, "Dr. Chen");
        assert_eq!(ctx.known_professionals[1].specialty, None);
    }
}
