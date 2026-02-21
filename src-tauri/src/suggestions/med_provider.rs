//! Signal provider for medications — surfaces active meds not recently discussed.

use std::collections::HashMap;

use chrono::Local;
use rusqlite::Connection;

use crate::db::DatabaseError;
use crate::db::repository::get_active_medications;

use super::{recency_decay, staleness, ScoredSuggestion, SignalProvider, SuggestionIntent};

pub struct MedSignalProvider;

impl SignalProvider for MedSignalProvider {
    fn collect(
        &self,
        conn: &Connection,
        recent_topics: &str,
    ) -> Result<Vec<ScoredSuggestion>, DatabaseError> {
        let mut candidates = Vec::new();
        let today = Local::now().naive_local().date();

        let meds = get_active_medications(conn)?;

        for med in meds.iter().take(4) {
            if candidates.len() >= 2 {
                break;
            }
            let name_lower = med.generic_name.to_lowercase();
            if recent_topics.contains(&name_lower) {
                continue;
            }

            // Recently started medication (< 14 days) → side effects query
            if let Some(start) = med.start_date {
                let days_since_start = (today - start).num_days() as f32;
                if days_since_start < 14.0 && days_since_start >= 0.0 {
                    candidates.push(ScoredSuggestion {
                        template_key: "chat.suggest_med_sideeffects".into(),
                        params: HashMap::from([(
                            "medication_name".into(),
                            med.generic_name.clone(),
                        )]),
                        intent: SuggestionIntent::Query,
                        score: 0.7 * recency_decay(days_since_start, 14.0),
                        domain: "medication",
                        entity_id: Some(name_lower.clone()),
                        category: "medications".into(),
                    });
                    continue;
                }
            }

            // Older active medication → expression: how are you feeling?
            // Use staleness based on how long since the user might have discussed it.
            // Without tracking per-entity mentions, use a fixed staleness of 1.0
            // (assume not discussed unless suppressed by recent_topics above).
            let stale = if recent_topics.is_empty() {
                1.0
            } else {
                staleness(7.0, 7.0) // Not found in recent_topics → fully stale
            };
            candidates.push(ScoredSuggestion {
                template_key: "chat.suggest_med_feeling".into(),
                params: HashMap::from([(
                    "medication_name".into(),
                    med.generic_name.clone(),
                )]),
                intent: SuggestionIntent::Expression,
                score: 0.8 * stale,
                domain: "medication",
                entity_id: Some(name_lower),
                category: "medications".into(),
            });
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

    fn seed_med(conn: &Connection, name: &str, start_date: Option<&str>) {
        let doc_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO documents (id, type, title, source_file, ingestion_date, verified)
             VALUES (?1, 'prescription', 'Rx', 'test.pdf', datetime('now'), 0)",
            params![doc_id],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO medications (id, document_id, generic_name, dose, frequency, frequency_type, status, start_date)
             VALUES (?1, ?2, ?3, '500mg', 'daily', 'scheduled', 'active', ?4)",
            params![Uuid::new_v4().to_string(), doc_id, name, start_date],
        )
        .unwrap();
    }

    #[test]
    fn active_med_surfaces() {
        let conn = open_memory_database().unwrap();
        seed_med(&conn, "Metformin", None);
        let provider = MedSignalProvider;
        let results = provider.collect(&conn, "").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].params["medication_name"], "Metformin");
        assert!(results[0].score > 0.0);
    }

    #[test]
    fn recently_started_gets_sideeffects_template() {
        let conn = open_memory_database().unwrap();
        let today = Local::now().format("%Y-%m-%d").to_string();
        seed_med(&conn, "Lisinopril", Some(&today));
        let provider = MedSignalProvider;
        let results = provider.collect(&conn, "").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].template_key, "chat.suggest_med_sideeffects");
    }

    #[test]
    fn discussed_med_suppressed() {
        let conn = open_memory_database().unwrap();
        seed_med(&conn, "Metformin", None);
        let provider = MedSignalProvider;
        let results = provider.collect(&conn, "metformin").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn max_two_candidates() {
        let conn = open_memory_database().unwrap();
        seed_med(&conn, "Metformin", None);
        seed_med(&conn, "Lisinopril", None);
        seed_med(&conn, "Aspirin", None);
        let provider = MedSignalProvider;
        let results = provider.collect(&conn, "").unwrap();
        assert!(results.len() <= 2);
    }
}
