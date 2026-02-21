//! Signal provider for documents — surfaces recently imported unreviewed documents.

use std::collections::HashMap;

use chrono::Local;
use rusqlite::Connection;

use crate::db::DatabaseError;
use crate::db::repository::get_recent_documents;
use crate::models::enums::{DocumentType, PipelineStatus};

use super::{recency_decay, ScoredSuggestion, SignalProvider, SuggestionIntent};

pub struct DocSignalProvider;

impl SignalProvider for DocSignalProvider {
    fn collect(
        &self,
        conn: &Connection,
        recent_topics: &str,
    ) -> Result<Vec<ScoredSuggestion>, DatabaseError> {
        let mut candidates = Vec::new();
        let now = Local::now().naive_local();

        let docs = get_recent_documents(conn, 7)?;

        for doc in docs.iter().take(4) {
            if candidates.len() >= 2 {
                break;
            }
            if recent_topics.contains(&doc.title.to_lowercase()) {
                continue;
            }

            let hours_since = (now - doc.ingestion_date).num_hours() as f32;
            let days = hours_since / 24.0;

            if doc.doc_type == DocumentType::Prescription {
                candidates.push(ScoredSuggestion {
                    template_key: "chat.suggest_doc_prescription".into(),
                    params: HashMap::new(),
                    intent: SuggestionIntent::Query,
                    score: 0.6 * recency_decay(days, 7.0),
                    domain: "document",
                    entity_id: Some(doc.id.to_string()),
                    category: "documents".into(),
                });
            } else if !doc.verified
                || doc.pipeline_status == PipelineStatus::Imported
                || doc.pipeline_status == PipelineStatus::PendingReview
            {
                let doc_type_label = match doc.doc_type {
                    DocumentType::Prescription => "prescription",
                    DocumentType::LabResult => "lab report",
                    DocumentType::ClinicalNote => "doctor note",
                    _ => "document",
                };
                candidates.push(ScoredSuggestion {
                    template_key: "chat.suggest_doc_review".into(),
                    params: HashMap::from([("doc_type".into(), doc_type_label.into())]),
                    intent: SuggestionIntent::Query,
                    score: 0.8 * recency_decay(days, 7.0),
                    domain: "document",
                    entity_id: Some(doc.id.to_string()),
                    category: "documents".into(),
                });
            }
        }

        Ok(candidates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;
    use rusqlite::params;
    use uuid::Uuid;

    fn seed_doc(conn: &Connection, doc_type: &str, verified: bool) -> String {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO documents (id, type, title, source_file, ingestion_date, verified, pipeline_status)
             VALUES (?1, ?2, 'Test Document', 'test.pdf', datetime('now'), ?3, 'imported')",
            params![id, doc_type, verified as i32],
        )
        .unwrap();
        id
    }

    #[test]
    fn unreviewed_doc_surfaces() {
        let conn = open_memory_database().unwrap();
        seed_doc(&conn, "lab_result", false);
        let provider = DocSignalProvider;
        let results = provider.collect(&conn, "").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].template_key, "chat.suggest_doc_review");
        assert!(results[0].score > 0.0);
    }

    #[test]
    fn prescription_gets_prescription_template() {
        let conn = open_memory_database().unwrap();
        seed_doc(&conn, "prescription", false);
        let provider = DocSignalProvider;
        let results = provider.collect(&conn, "").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].template_key, "chat.suggest_doc_prescription");
    }

    #[test]
    fn discussed_doc_suppressed() {
        let conn = open_memory_database().unwrap();
        seed_doc(&conn, "lab_result", false);
        let provider = DocSignalProvider;
        let results = provider.collect(&conn, "test document").unwrap();
        assert!(results.is_empty());
    }
}
