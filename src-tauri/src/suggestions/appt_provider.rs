//! Signal provider for appointments — surfaces upcoming appointments within 72h.

use std::collections::HashMap;

use chrono::{Local, NaiveDate};
use rusqlite::Connection;

use crate::appointment;
use crate::db::DatabaseError;

use super::{ScoredSuggestion, SignalProvider, SuggestionIntent};

pub struct ApptSignalProvider;

impl SignalProvider for ApptSignalProvider {
    fn collect(
        &self,
        conn: &Connection,
        recent_topics: &str,
    ) -> Result<Vec<ScoredSuggestion>, DatabaseError> {
        if recent_topics.contains("appointment") || recent_topics.contains("rendez-vous") {
            return Ok(Vec::new());
        }

        let appointments = appointment::list_appointments(conn)?;
        let today = Local::now().naive_local().date();

        for appt in &appointments {
            let date = match NaiveDate::parse_from_str(&appt.date, "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => continue,
            };

            let days_until = (date - today).num_days();
            if days_until < 0 || days_until > 3 {
                continue;
            }

            let (template_key, score) = if days_until <= 1 {
                ("chat.suggest_appt_prepare", 1.0)
            } else {
                ("chat.suggest_appt_questions", 0.6)
            };

            let day_label = match days_until {
                0 => "today".to_string(),
                1 => "tomorrow".to_string(),
                _ => date.format("%A").to_string(),
            };

            return Ok(vec![ScoredSuggestion {
                template_key: template_key.into(),
                params: HashMap::from([("date".into(), day_label)]),
                intent: SuggestionIntent::Query,
                score,
                domain: "appointment",
                entity_id: Some(appt.id.clone()),
                category: "appointments".into(),
            }]);
        }

        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;
    use chrono::Duration;
    use rusqlite::params;
    use uuid::Uuid;

    fn seed_appointment(conn: &Connection, days_from_now: i64) {
        let today = Local::now().naive_local().date();
        let appt_date = today + Duration::days(days_from_now);

        let prof_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO professionals (id, name, specialty) VALUES (?1, 'Dr. Smith', 'Cardiology')",
            params![prof_id],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO appointments (id, professional_id, date, type) VALUES (?1, ?2, ?3, 'upcoming')",
            params![Uuid::new_v4().to_string(), prof_id, appt_date.to_string()],
        )
        .unwrap();
    }

    #[test]
    fn upcoming_appointment_surfaces() {
        let conn = open_memory_database().unwrap();
        seed_appointment(&conn, 1);
        let provider = ApptSignalProvider;
        let results = provider.collect(&conn, "").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].template_key, "chat.suggest_appt_prepare");
        assert_eq!(results[0].score, 1.0);
    }

    #[test]
    fn far_appointment_ignored() {
        let conn = open_memory_database().unwrap();
        seed_appointment(&conn, 10);
        let provider = ApptSignalProvider;
        let results = provider.collect(&conn, "").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn discussed_appointment_suppressed() {
        let conn = open_memory_database().unwrap();
        seed_appointment(&conn, 1);
        let provider = ApptSignalProvider;
        let results = provider.collect(&conn, "appointment").unwrap();
        assert!(results.is_empty());
    }
}
