//! Signal provider for lab results — surfaces abnormal/critical labs not yet discussed.

use std::collections::HashMap;

use chrono::{Duration, Local};
use rusqlite::Connection;

use crate::db::DatabaseError;
use crate::db::repository::{get_critical_labs, get_lab_results_since};
use crate::models::enums::AbnormalFlag;

use super::{recency_decay, ScoredSuggestion, SignalProvider, SuggestionIntent};

pub struct LabSignalProvider;

impl SignalProvider for LabSignalProvider {
    fn collect(
        &self,
        conn: &Connection,
        recent_topics: &str,
    ) -> Result<Vec<ScoredSuggestion>, DatabaseError> {
        let mut candidates = Vec::new();
        let today = Local::now().naive_local().date();
        let since = today - Duration::days(14);

        // Critical labs (highest priority)
        let critical = get_critical_labs(conn)?;
        for lab in critical.iter().take(2) {
            if recent_topics.contains(&lab.test_name.to_lowercase()) {
                continue;
            }
            let days = (today - lab.collection_date).num_days() as f32;
            candidates.push(ScoredSuggestion {
                template_key: "chat.suggest_lab_critical".into(),
                params: HashMap::from([("test_name".into(), lab.test_name.clone())]),
                intent: SuggestionIntent::Query,
                score: 1.0 * recency_decay(days, 14.0),
                domain: "lab",
                entity_id: Some(lab.test_name.to_lowercase()),
                category: "labs".into(),
            });
        }

        // Abnormal (non-critical) labs
        if candidates.len() < 2 {
            let recent = get_lab_results_since(conn, &since)?;
            for lab in recent.iter() {
                if candidates.len() >= 2 {
                    break;
                }
                if lab.abnormal_flag == AbnormalFlag::Normal {
                    continue;
                }
                // Skip if already added as critical
                if candidates
                    .iter()
                    .any(|c| c.entity_id.as_deref() == Some(&lab.test_name.to_lowercase()))
                {
                    continue;
                }
                if recent_topics.contains(&lab.test_name.to_lowercase()) {
                    continue;
                }
                let days = (today - lab.collection_date).num_days() as f32;
                candidates.push(ScoredSuggestion {
                    template_key: "chat.suggest_lab_abnormal".into(),
                    params: HashMap::from([("test_name".into(), lab.test_name.clone())]),
                    intent: SuggestionIntent::Query,
                    score: 0.5 * recency_decay(days, 14.0),
                    domain: "lab",
                    entity_id: Some(lab.test_name.to_lowercase()),
                    category: "labs".into(),
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

    fn seed_lab(conn: &Connection, test_name: &str, flag: &str) -> String {
        let doc_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO documents (id, type, title, source_file, ingestion_date, verified)
             VALUES (?1, 'lab_result', 'Lab', 'test.pdf', datetime('now'), 0)",
            params![doc_id],
        )
        .unwrap();

        let lab_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO lab_results (id, test_name, abnormal_flag, collection_date, document_id)
             VALUES (?1, ?2, ?3, date('now'), ?4)",
            params![lab_id, test_name, flag, doc_id],
        )
        .unwrap();
        lab_id
    }

    #[test]
    fn critical_lab_surfaces() {
        let conn = open_memory_database().unwrap();
        seed_lab(&conn, "Creatinine", "critical_high");
        let provider = LabSignalProvider;
        let results = provider.collect(&conn, "").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].template_key, "chat.suggest_lab_critical");
        assert_eq!(results[0].params["test_name"], "Creatinine");
        assert!(results[0].score > 0.0);
    }

    #[test]
    fn abnormal_lab_surfaces() {
        let conn = open_memory_database().unwrap();
        seed_lab(&conn, "Glucose", "high");
        let provider = LabSignalProvider;
        let results = provider.collect(&conn, "").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].template_key, "chat.suggest_lab_abnormal");
    }

    #[test]
    fn recently_discussed_suppressed() {
        let conn = open_memory_database().unwrap();
        seed_lab(&conn, "Creatinine", "critical_high");
        let provider = LabSignalProvider;
        let results = provider.collect(&conn, "creatinine").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn normal_lab_ignored() {
        let conn = open_memory_database().unwrap();
        seed_lab(&conn, "Hemoglobin", "normal");
        let provider = LabSignalProvider;
        let results = provider.collect(&conn, "").unwrap();
        assert!(results.is_empty());
    }
}
