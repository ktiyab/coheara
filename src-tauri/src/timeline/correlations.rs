use chrono::NaiveDate;
use rusqlite::Connection;

use crate::db::DatabaseError;
use super::types::*;

const CORRELATION_WINDOW_DAYS: i64 = 14;

/// Detects temporal correlations between symptoms and medication events.
/// O(n*m) where n = symptoms, m = medication events — bounded for typical profiles.
pub fn detect_correlations(events: &[TimelineEvent]) -> Vec<TimelineCorrelation> {
    let symptoms: Vec<&TimelineEvent> = events
        .iter()
        .filter(|e| e.event_type == EventType::Symptom)
        .collect();

    let med_events: Vec<&TimelineEvent> = events
        .iter()
        .filter(|e| {
            matches!(
                e.event_type,
                EventType::MedicationStart
                    | EventType::MedicationStop
                    | EventType::MedicationDoseChange
            )
        })
        .collect();

    let mut correlations = Vec::new();

    for symptom in &symptoms {
        let symptom_date = match NaiveDate::parse_from_str(&symptom.date, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => continue,
        };

        for med in &med_events {
            let med_date = match NaiveDate::parse_from_str(&med.date, "%Y-%m-%d") {
                Ok(d) => d,
                Err(_) => continue,
            };

            let days_diff = (symptom_date - med_date).num_days();

            // Symptom appeared AFTER medication event (within window)
            if (0..=CORRELATION_WINDOW_DAYS).contains(&days_diff) {
                let corr_type = match med.event_type {
                    EventType::MedicationStart => {
                        CorrelationType::SymptomAfterMedicationStart
                    }
                    EventType::MedicationDoseChange => {
                        CorrelationType::SymptomAfterMedicationChange
                    }
                    _ => continue,
                };

                correlations.push(TimelineCorrelation {
                    source_id: symptom.id.clone(),
                    target_id: med.id.clone(),
                    correlation_type: corr_type,
                    description: format!(
                        "{} appeared {} day(s) after {}",
                        symptom.title, days_diff, med.title,
                    ),
                });
            }
        }
    }

    correlations
}

/// Fetches explicit symptom-medication links stored via related_medication_id.
pub fn fetch_explicit_correlations(
    conn: &Connection,
) -> Result<Vec<TimelineCorrelation>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT s.id AS symptom_id, s.specific,
                m.id AS med_id, m.generic_name
         FROM symptoms s
         JOIN medications m ON s.related_medication_id = m.id
         WHERE s.related_medication_id IS NOT NULL",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(TimelineCorrelation {
            source_id: row.get::<_, String>("symptom_id")?,
            target_id: row.get::<_, String>("med_id")?,
            correlation_type: CorrelationType::ExplicitLink,
            description: format!(
                "{} linked to {}",
                row.get::<_, String>("specific")?,
                row.get::<_, String>("generic_name")?,
            ),
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::from)
}
