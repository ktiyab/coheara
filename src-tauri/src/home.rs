//! L3-02 Home & Document Feed — types and repository functions.
//!
//! Provides the data layer for the home screen: recent document feed,
//! profile stats, onboarding progress, and entity counts per document.
//! All functions operate on the profile's SQLite database via rusqlite.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::db::DatabaseError;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Document review status derived from `documents.verified` column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentStatus {
    PendingReview,
    Confirmed,
}

/// A document card for the home feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentCard {
    pub id: String,
    pub document_type: String,
    pub source_filename: String,
    pub professional_name: Option<String>,
    pub professional_specialty: Option<String>,
    pub document_date: Option<String>,
    pub imported_at: String,
    pub status: DocumentStatus,
    pub entity_summary: EntitySummary,
}

/// Counts of entities extracted from a document.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntitySummary {
    pub medications: u32,
    pub lab_results: u32,
    pub diagnoses: u32,
    pub allergies: u32,
    pub procedures: u32,
    pub referrals: u32,
}

/// Aggregated profile stats for the home header.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileStats {
    pub total_documents: u32,
    pub documents_pending_review: u32,
    pub total_medications: u32,
    pub total_lab_results: u32,
    pub last_document_date: Option<String>,
    pub extraction_accuracy: Option<f64>,
}

/// Onboarding milestone tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingProgress {
    pub first_document_loaded: bool,
    pub first_document_reviewed: bool,
    pub first_question_asked: bool,
    pub three_documents_loaded: bool,
    pub first_symptom_recorded: bool,
}

impl OnboardingProgress {
    pub fn is_complete(&self) -> bool {
        self.first_document_loaded
            && self.first_document_reviewed
            && self.first_question_asked
            && self.three_documents_loaded
            && self.first_symptom_recorded
    }

    pub fn completed_count(&self) -> u32 {
        [
            self.first_document_loaded,
            self.first_document_reviewed,
            self.first_question_asked,
            self.three_documents_loaded,
            self.first_symptom_recorded,
        ]
        .iter()
        .filter(|&&v| v)
        .count() as u32
    }
}

/// Home screen data — single fetch for all home content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeData {
    pub stats: ProfileStats,
    pub recent_documents: Vec<DocumentCard>,
    pub onboarding: OnboardingProgress,
    pub critical_alerts: Vec<crate::trust::CriticalLabAlert>,
}

// ---------------------------------------------------------------------------
// Repository functions
// ---------------------------------------------------------------------------

/// Extract filename from a file path string.
fn extract_filename(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_string()
}

/// Fetches recent documents with professional info for the home feed.
pub fn fetch_recent_documents(
    conn: &Connection,
    limit: u32,
    offset: u32,
) -> Result<Vec<DocumentCard>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT d.id, d.type, d.source_file, d.document_date, d.ingestion_date,
                d.verified,
                p.name AS prof_name, p.specialty AS prof_specialty
         FROM documents d
         LEFT JOIN professionals p ON d.professional_id = p.id
         ORDER BY d.ingestion_date DESC
         LIMIT ?1 OFFSET ?2",
    )?;

    let rows = stmt.query_map(params![limit, offset], |row| {
        let id: String = row.get(0)?;
        let doc_type: String = row.get(1)?;
        let source_file: String = row.get(2)?;
        let document_date: Option<String> = row.get(3)?;
        let ingestion_date: String = row.get(4)?;
        let verified: bool = row.get(5)?;
        let prof_name: Option<String> = row.get(6)?;
        let prof_specialty: Option<String> = row.get(7)?;

        Ok(DocumentCard {
            id,
            document_type: format_document_type(&doc_type),
            source_filename: extract_filename(&source_file),
            professional_name: prof_name,
            professional_specialty: prof_specialty,
            document_date,
            imported_at: ingestion_date,
            status: if verified {
                DocumentStatus::Confirmed
            } else {
                DocumentStatus::PendingReview
            },
            entity_summary: EntitySummary::default(),
        })
    })?;

    let mut cards: Vec<DocumentCard> = Vec::new();
    for row in rows {
        let mut card = row?;
        let doc_id = card.id.clone();
        card.entity_summary = fetch_entity_counts(conn, &doc_id)?;
        cards.push(card);
    }
    Ok(cards)
}

/// Counts entities per document across all entity tables.
pub fn fetch_entity_counts(
    conn: &Connection,
    document_id: &str,
) -> Result<EntitySummary, DatabaseError> {
    let count = |table: &str| -> Result<u32, DatabaseError> {
        conn.query_row(
            &format!(
                "SELECT COUNT(*) FROM {} WHERE document_id = ?1",
                table
            ),
            params![document_id],
            |row| row.get(0),
        )
        .map_err(DatabaseError::from)
    };

    Ok(EntitySummary {
        medications: count("medications")?,
        lab_results: count("lab_results")?,
        diagnoses: count("diagnoses")?,
        allergies: count("allergies")?,
        procedures: count("procedures")?,
        referrals: count("referrals")?,
    })
}

/// Fetches aggregated profile stats for the home header.
pub fn fetch_profile_stats(conn: &Connection) -> Result<ProfileStats, DatabaseError> {
    let total_documents: u32 =
        conn.query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))?;

    let documents_pending: u32 = conn.query_row(
        "SELECT COUNT(*) FROM documents WHERE verified = 0",
        [],
        |row| row.get(0),
    )?;

    let total_medications: u32 = conn.query_row(
        "SELECT COUNT(*) FROM medications WHERE status = 'active'",
        [],
        |row| row.get(0),
    )?;

    let total_lab_results: u32 =
        conn.query_row("SELECT COUNT(*) FROM lab_results", [], |row| row.get(0))?;

    let last_doc_date: Option<String> = conn
        .query_row(
            "SELECT MAX(ingestion_date) FROM documents",
            [],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    let extraction_accuracy: Option<f64> = conn
        .query_row(
            "SELECT extraction_accuracy FROM profile_trust WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .ok();

    Ok(ProfileStats {
        total_documents,
        documents_pending_review: documents_pending,
        total_medications,
        total_lab_results,
        last_document_date: last_doc_date,
        extraction_accuracy,
    })
}

/// Computes onboarding milestone state from database counts.
pub fn compute_onboarding(conn: &Connection) -> Result<OnboardingProgress, DatabaseError> {
    let doc_count: u32 =
        conn.query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))?;

    let reviewed_count: u32 = conn.query_row(
        "SELECT COUNT(*) FROM documents WHERE verified = 1",
        [],
        |row| row.get(0),
    )?;

    let question_count: u32 = conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE role = 'patient'",
        [],
        |row| row.get(0),
    )?;

    let symptom_count: u32 =
        conn.query_row("SELECT COUNT(*) FROM symptoms", [], |row| row.get(0))?;

    Ok(OnboardingProgress {
        first_document_loaded: doc_count >= 1,
        first_document_reviewed: reviewed_count >= 1,
        first_question_asked: question_count >= 1,
        three_documents_loaded: doc_count >= 3,
        first_symptom_recorded: symptom_count >= 1,
    })
}

/// Format a raw document type string for display.
fn format_document_type(raw: &str) -> String {
    match raw {
        "prescription" => "Prescription".to_string(),
        "lab_result" => "Lab Report".to_string(),
        "clinical_note" => "Clinical Note".to_string(),
        "discharge_summary" => "Discharge Summary".to_string(),
        "radiology_report" => "Radiology Report".to_string(),
        "pharmacy_record" => "Pharmacy Record".to_string(),
        "other" => "Other".to_string(),
        _ => raw.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;
    use uuid::Uuid;

    /// Insert a test document.
    fn insert_test_document(
        conn: &Connection,
        id: &str,
        doc_type: &str,
        verified: bool,
        source_file: &str,
    ) {
        conn.execute(
            "INSERT INTO documents (id, type, title, ingestion_date, source_file, verified)
             VALUES (?1, ?2, ?3, datetime('now'), ?4, ?5)",
            params![id, doc_type, "Test Doc", source_file, verified],
        )
        .unwrap();
    }

    /// Insert a test professional and return their ID.
    fn insert_test_professional(conn: &Connection, id: &str, name: &str, specialty: &str) {
        conn.execute(
            "INSERT INTO professionals (id, name, specialty) VALUES (?1, ?2, ?3)",
            params![id, name, specialty],
        )
        .unwrap();
    }

    /// Insert a test medication linked to a document.
    fn insert_test_medication(conn: &Connection, doc_id: &str) {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, document_id)
             VALUES (?1, 'Metformin', '500mg', 'twice daily', 'scheduled', 'active', ?2)",
            params![id, doc_id],
        )
        .unwrap();
    }

    /// Insert a test lab result linked to a document.
    fn insert_test_lab_result(conn: &Connection, doc_id: &str) {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO lab_results (id, test_name, abnormal_flag, collection_date, document_id)
             VALUES (?1, 'HbA1c', 'normal', '2025-01-15', ?2)",
            params![id, doc_id],
        )
        .unwrap();
    }

    /// Insert a test diagnosis linked to a document.
    fn insert_test_diagnosis(conn: &Connection, doc_id: &str) {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO diagnoses (id, name, status, document_id)
             VALUES (?1, 'Type 2 Diabetes', 'active', ?2)",
            params![id, doc_id],
        )
        .unwrap();
    }

    // -----------------------------------------------------------------------
    // fetch_recent_documents
    // -----------------------------------------------------------------------

    #[test]
    fn fetch_recent_documents_empty() {
        let conn = open_memory_database().unwrap();
        let docs = fetch_recent_documents(&conn, 20, 0).unwrap();
        assert!(docs.is_empty());
    }

    #[test]
    fn fetch_recent_documents_ordered() {
        let conn = open_memory_database().unwrap();
        let id1 = Uuid::new_v4().to_string();
        let id2 = Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO documents (id, type, title, ingestion_date, source_file, verified)
             VALUES (?1, 'prescription', 'Doc A', '2025-01-10 10:00:00', '/tmp/a.pdf', 0)",
            params![id1],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO documents (id, type, title, ingestion_date, source_file, verified)
             VALUES (?1, 'lab_result', 'Doc B', '2025-01-15 14:00:00', '/tmp/b.pdf', 1)",
            params![id2],
        )
        .unwrap();

        let docs = fetch_recent_documents(&conn, 20, 0).unwrap();
        assert_eq!(docs.len(), 2);
        // Most recent first
        assert_eq!(docs[0].id, id2);
        assert_eq!(docs[1].id, id1);
    }

    #[test]
    fn fetch_recent_documents_with_professional() {
        let conn = open_memory_database().unwrap();
        let prof_id = Uuid::new_v4().to_string();
        let doc_id = Uuid::new_v4().to_string();

        insert_test_professional(&conn, &prof_id, "Dr. Chen", "Cardiology");
        conn.execute(
            "INSERT INTO documents (id, type, title, ingestion_date, source_file, verified, professional_id)
             VALUES (?1, 'prescription', 'Test', datetime('now'), '/docs/rx.pdf', 0, ?2)",
            params![doc_id, prof_id],
        )
        .unwrap();

        let docs = fetch_recent_documents(&conn, 20, 0).unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].professional_name.as_deref(), Some("Dr. Chen"));
        assert_eq!(
            docs[0].professional_specialty.as_deref(),
            Some("Cardiology")
        );
    }

    #[test]
    fn fetch_recent_documents_status_mapping() {
        let conn = open_memory_database().unwrap();
        let id1 = Uuid::new_v4().to_string();
        let id2 = Uuid::new_v4().to_string();

        insert_test_document(&conn, &id1, "prescription", false, "/tmp/a.pdf");
        insert_test_document(&conn, &id2, "lab_result", true, "/tmp/b.pdf");

        let docs = fetch_recent_documents(&conn, 20, 0).unwrap();
        let pending = docs.iter().find(|d| d.id == id1).unwrap();
        let confirmed = docs.iter().find(|d| d.id == id2).unwrap();

        assert_eq!(pending.status, DocumentStatus::PendingReview);
        assert_eq!(confirmed.status, DocumentStatus::Confirmed);
    }

    #[test]
    fn fetch_recent_documents_filename_extraction() {
        let conn = open_memory_database().unwrap();
        let id = Uuid::new_v4().to_string();
        insert_test_document(&conn, &id, "prescription", false, "/home/user/docs/blood_work.pdf");

        let docs = fetch_recent_documents(&conn, 20, 0).unwrap();
        assert_eq!(docs[0].source_filename, "blood_work.pdf");
    }

    #[test]
    fn fetch_recent_documents_type_formatting() {
        let conn = open_memory_database().unwrap();
        let id = Uuid::new_v4().to_string();
        insert_test_document(&conn, &id, "discharge_summary", false, "/tmp/ds.pdf");

        let docs = fetch_recent_documents(&conn, 20, 0).unwrap();
        assert_eq!(docs[0].document_type, "Discharge Summary");
    }

    // -----------------------------------------------------------------------
    // fetch_entity_counts
    // -----------------------------------------------------------------------

    #[test]
    fn fetch_entity_counts_zero() {
        let conn = open_memory_database().unwrap();
        let doc_id = Uuid::new_v4().to_string();
        insert_test_document(&conn, &doc_id, "prescription", false, "/tmp/a.pdf");

        let counts = fetch_entity_counts(&conn, &doc_id).unwrap();
        assert_eq!(counts.medications, 0);
        assert_eq!(counts.lab_results, 0);
        assert_eq!(counts.diagnoses, 0);
        assert_eq!(counts.allergies, 0);
        assert_eq!(counts.procedures, 0);
        assert_eq!(counts.referrals, 0);
    }

    #[test]
    fn fetch_entity_counts_with_data() {
        let conn = open_memory_database().unwrap();
        let doc_id = Uuid::new_v4().to_string();
        insert_test_document(&conn, &doc_id, "prescription", false, "/tmp/a.pdf");

        insert_test_medication(&conn, &doc_id);
        insert_test_medication(&conn, &doc_id);
        insert_test_lab_result(&conn, &doc_id);
        insert_test_diagnosis(&conn, &doc_id);

        let counts = fetch_entity_counts(&conn, &doc_id).unwrap();
        assert_eq!(counts.medications, 2);
        assert_eq!(counts.lab_results, 1);
        assert_eq!(counts.diagnoses, 1);
        assert_eq!(counts.allergies, 0);
    }

    // -----------------------------------------------------------------------
    // fetch_profile_stats
    // -----------------------------------------------------------------------

    #[test]
    fn profile_stats_empty() {
        let conn = open_memory_database().unwrap();
        let stats = fetch_profile_stats(&conn).unwrap();
        assert_eq!(stats.total_documents, 0);
        assert_eq!(stats.documents_pending_review, 0);
        assert_eq!(stats.total_medications, 0);
        assert_eq!(stats.total_lab_results, 0);
        assert!(stats.last_document_date.is_none());
    }

    #[test]
    fn profile_stats_with_data() {
        let conn = open_memory_database().unwrap();
        let doc1 = Uuid::new_v4().to_string();
        let doc2 = Uuid::new_v4().to_string();

        insert_test_document(&conn, &doc1, "prescription", false, "/tmp/a.pdf");
        insert_test_document(&conn, &doc2, "lab_result", true, "/tmp/b.pdf");
        insert_test_medication(&conn, &doc1);
        insert_test_lab_result(&conn, &doc2);

        let stats = fetch_profile_stats(&conn).unwrap();
        assert_eq!(stats.total_documents, 2);
        assert_eq!(stats.documents_pending_review, 1);
        assert_eq!(stats.total_medications, 1);
        assert_eq!(stats.total_lab_results, 1);
        assert!(stats.last_document_date.is_some());
    }

    // -----------------------------------------------------------------------
    // compute_onboarding
    // -----------------------------------------------------------------------

    #[test]
    fn onboarding_progress_none() {
        let conn = open_memory_database().unwrap();
        let progress = compute_onboarding(&conn).unwrap();
        assert!(!progress.first_document_loaded);
        assert!(!progress.first_document_reviewed);
        assert!(!progress.first_question_asked);
        assert!(!progress.three_documents_loaded);
        assert!(!progress.first_symptom_recorded);
        assert!(!progress.is_complete());
        assert_eq!(progress.completed_count(), 0);
    }

    #[test]
    fn onboarding_progress_partial() {
        let conn = open_memory_database().unwrap();
        let doc_id = Uuid::new_v4().to_string();
        insert_test_document(&conn, &doc_id, "prescription", true, "/tmp/a.pdf");

        let progress = compute_onboarding(&conn).unwrap();
        assert!(progress.first_document_loaded);
        assert!(progress.first_document_reviewed);
        assert!(!progress.first_question_asked);
        assert!(!progress.three_documents_loaded);
        assert!(!progress.first_symptom_recorded);
        assert!(!progress.is_complete());
        assert_eq!(progress.completed_count(), 2);
    }

    #[test]
    fn onboarding_progress_complete() {
        let conn = open_memory_database().unwrap();

        // Load 3 verified documents
        for _ in 0..3 {
            let doc_id = Uuid::new_v4().to_string();
            insert_test_document(&conn, &doc_id, "prescription", true, "/tmp/a.pdf");
        }

        // Ask a question (insert patient message in a conversation)
        let conv_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO conversations (id, started_at) VALUES (?1, datetime('now'))",
            params![conv_id],
        )
        .unwrap();
        let msg_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, timestamp)
             VALUES (?1, ?2, 'patient', 'What medications am I taking?', datetime('now'))",
            params![msg_id, conv_id],
        )
        .unwrap();

        // Record a symptom
        let symptom_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, source)
             VALUES (?1, 'Pain', 'Headache', 3, '2025-01-15', '2025-01-15', 'patient_reported')",
            params![symptom_id],
        )
        .unwrap();

        let progress = compute_onboarding(&conn).unwrap();
        assert!(progress.is_complete());
        assert_eq!(progress.completed_count(), 5);
    }

    // -----------------------------------------------------------------------
    // Pagination
    // -----------------------------------------------------------------------

    #[test]
    fn more_documents_pagination() {
        let conn = open_memory_database().unwrap();
        for i in 0..5 {
            let id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO documents (id, type, title, ingestion_date, source_file, verified)
                 VALUES (?1, 'prescription', ?2, datetime('now', ?3), '/tmp/a.pdf', 0)",
                params![id, format!("Doc {i}"), format!("-{i} minutes")],
            )
            .unwrap();
        }

        let page1 = fetch_recent_documents(&conn, 2, 0).unwrap();
        assert_eq!(page1.len(), 2);

        let page2 = fetch_recent_documents(&conn, 2, 2).unwrap();
        assert_eq!(page2.len(), 2);

        let page3 = fetch_recent_documents(&conn, 2, 4).unwrap();
        assert_eq!(page3.len(), 1);

        // No overlap
        assert_ne!(page1[0].id, page2[0].id);
    }

    // -----------------------------------------------------------------------
    // extract_filename
    // -----------------------------------------------------------------------

    #[test]
    fn filename_extraction() {
        assert_eq!(extract_filename("/home/user/docs/blood.pdf"), "blood.pdf");
        assert_eq!(extract_filename("just_a_name.pdf"), "just_a_name.pdf");
        assert_eq!(extract_filename("/tmp/a"), "a");
    }
}
