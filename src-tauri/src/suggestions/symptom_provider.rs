//! Signal provider for symptoms — surfaces unrevisited symptoms from the last 14 days.

use std::collections::HashMap;

use chrono::{Duration, Local};
use rusqlite::Connection;

use crate::db::DatabaseError;
use crate::db::repository::get_symptoms_in_date_range;

use super::{recency_decay, ScoredSuggestion, SignalProvider, SuggestionIntent};

pub struct SymptomSignalProvider;

impl SignalProvider for SymptomSignalProvider {
    fn collect(
        &self,
        conn: &Connection,
        recent_topics: &str,
    ) -> Result<Vec<ScoredSuggestion>, DatabaseError> {
        let mut candidates = Vec::new();
        let today = Local::now().naive_local().date();
        let since = today - Duration::days(14);

        let symptoms = get_symptoms_in_date_range(conn, &since, &today)?;

        for symptom in symptoms.iter().take(4) {
            if candidates.len() >= 2 {
                break;
            }
            let name_lower = symptom.specific.to_lowercase();
            if recent_topics.contains(&name_lower) {
                continue;
            }

            let days = (today - symptom.onset_date).num_days() as f32;

            if symptom.still_active && days >= 5.0 {
                // Active symptom persisting > 5 days → query
                candidates.push(ScoredSuggestion {
                    template_key: "chat.suggest_symptom_persists".into(),
                    params: HashMap::from([
                        ("symptom".into(), symptom.specific.clone()),
                        ("days".into(), (days as i32).to_string()),
                    ]),
                    intent: SuggestionIntent::Query,
                    score: 0.6 * recency_decay(days, 14.0),
                    domain: "symptom",
                    entity_id: Some(name_lower),
                    category: "symptoms".into(),
                });
            } else {
                // Recent symptom → follow-up expression
                candidates.push(ScoredSuggestion {
                    template_key: "chat.suggest_symptom_followup".into(),
                    params: HashMap::from([("symptom".into(), symptom.specific.clone())]),
                    intent: SuggestionIntent::Expression,
                    score: 0.7 * recency_decay(days, 14.0),
                    domain: "symptom",
                    entity_id: Some(name_lower),
                    category: "symptoms".into(),
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

    fn seed_symptom(conn: &Connection, specific: &str, still_active: bool, days_ago: i64) {
        let today = Local::now().naive_local().date();
        let onset = today - Duration::days(days_ago);
        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, still_active, source)
             VALUES (?1, 'pain', ?2, 5, ?3, ?4, ?5, 'patient_reported')",
            params![
                Uuid::new_v4().to_string(),
                specific,
                onset.to_string(),
                today.to_string(),
                still_active as i32,
            ],
        )
        .unwrap();
    }

    #[test]
    fn recent_symptom_surfaces() {
        let conn = open_memory_database().unwrap();
        seed_symptom(&conn, "headache", false, 2);
        let provider = SymptomSignalProvider;
        let results = provider.collect(&conn, "").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].template_key, "chat.suggest_symptom_followup");
        assert_eq!(results[0].params["symptom"], "headache");
    }

    #[test]
    fn persisting_symptom_gets_persist_template() {
        let conn = open_memory_database().unwrap();
        seed_symptom(&conn, "back pain", true, 7);
        let provider = SymptomSignalProvider;
        let results = provider.collect(&conn, "").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].template_key, "chat.suggest_symptom_persists");
        assert_eq!(results[0].params["days"], "7");
    }

    #[test]
    fn discussed_symptom_suppressed() {
        let conn = open_memory_database().unwrap();
        seed_symptom(&conn, "headache", false, 2);
        let provider = SymptomSignalProvider;
        let results = provider.collect(&conn, "headache").unwrap();
        assert!(results.is_empty());
    }
}
