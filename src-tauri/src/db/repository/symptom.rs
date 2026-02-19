use std::str::FromStr;

use chrono::NaiveDate;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::DatabaseError;
use crate::models::*;
use crate::models::enums::*;

pub fn insert_symptom(conn: &Connection, symptom: &Symptom) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO symptoms (id, category, specific, severity, body_region, duration, character,
         aggravating, relieving, timing_pattern, onset_date, onset_time, recorded_date, still_active,
         resolved_date, related_medication_id, related_diagnosis_id, source, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
        params![
            symptom.id.to_string(),
            symptom.category,
            symptom.specific,
            symptom.severity,
            symptom.body_region,
            symptom.duration,
            symptom.character,
            symptom.aggravating,
            symptom.relieving,
            symptom.timing_pattern,
            symptom.onset_date.to_string(),
            symptom.onset_time,
            symptom.recorded_date.to_string(),
            symptom.still_active as i32,
            symptom.resolved_date.map(|d| d.to_string()),
            symptom.related_medication_id.map(|id| id.to_string()),
            symptom.related_diagnosis_id.map(|id| id.to_string()),
            symptom.source.as_str(),
            symptom.notes,
        ],
    )?;
    Ok(())
}

pub fn get_symptoms_in_date_range(
    conn: &Connection,
    from: &NaiveDate,
    to: &NaiveDate,
) -> Result<Vec<Symptom>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, category, specific, severity, body_region, duration, character,
         aggravating, relieving, timing_pattern, onset_date, onset_time, recorded_date,
         still_active, resolved_date, related_medication_id, related_diagnosis_id, source, notes
         FROM symptoms WHERE onset_date BETWEEN ?1 AND ?2 ORDER BY onset_date DESC",
    )?;

    let rows = stmt.query_map(params![from.to_string(), to.to_string()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i32>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Option<String>>(7)?,
            row.get::<_, Option<String>>(8)?,
            row.get::<_, Option<String>>(9)?,
            row.get::<_, String>(10)?,
            row.get::<_, Option<String>>(11)?,
            row.get::<_, String>(12)?,
            row.get::<_, i32>(13)?,
            row.get::<_, Option<String>>(14)?,
            row.get::<_, Option<String>>(15)?,
            row.get::<_, Option<String>>(16)?,
            row.get::<_, String>(17)?,
            row.get::<_, Option<String>>(18)?,
        ))
    })?;

    symptom_rows_to_vec(rows)
}

pub fn get_all_symptoms(conn: &Connection) -> Result<Vec<Symptom>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, category, specific, severity, body_region, duration, character,
         aggravating, relieving, timing_pattern, onset_date, onset_time, recorded_date,
         still_active, resolved_date, related_medication_id, related_diagnosis_id, source, notes
         FROM symptoms ORDER BY onset_date DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i32>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Option<String>>(7)?,
            row.get::<_, Option<String>>(8)?,
            row.get::<_, Option<String>>(9)?,
            row.get::<_, String>(10)?,
            row.get::<_, Option<String>>(11)?,
            row.get::<_, String>(12)?,
            row.get::<_, i32>(13)?,
            row.get::<_, Option<String>>(14)?,
            row.get::<_, Option<String>>(15)?,
            row.get::<_, Option<String>>(16)?,
            row.get::<_, String>(17)?,
            row.get::<_, Option<String>>(18)?,
        ))
    })?;

    symptom_rows_to_vec(rows)
}

type SymptomRow = (
    String, String, String, i32,
    Option<String>, Option<String>, Option<String>,
    Option<String>, Option<String>, Option<String>,
    String, Option<String>, String,
    i32, Option<String>, Option<String>, Option<String>,
    String, Option<String>,
);

fn symptom_rows_to_vec(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<SymptomRow>>,
) -> Result<Vec<Symptom>, DatabaseError> {
    let mut symptoms = Vec::new();
    for row in rows {
        let (
            id, category, specific, severity, body_region, duration, character,
            aggravating, relieving, timing_pattern, onset_date, onset_time, recorded_date,
            still_active, resolved_date, related_med_id, related_diag_id, source, notes,
        ) = row?;
        symptoms.push(Symptom {
            id: Uuid::parse_str(&id)
                .map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
            category,
            specific,
            severity,
            body_region,
            duration,
            character,
            aggravating,
            relieving,
            timing_pattern,
            onset_date: NaiveDate::parse_from_str(&onset_date, "%Y-%m-%d").unwrap_or_default(),
            onset_time,
            recorded_date: NaiveDate::parse_from_str(&recorded_date, "%Y-%m-%d")
                .unwrap_or_default(),
            still_active: still_active != 0,
            resolved_date: resolved_date
                .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
            related_medication_id: related_med_id.and_then(|s| Uuid::parse_str(&s).ok()),
            related_diagnosis_id: related_diag_id.and_then(|s| Uuid::parse_str(&s).ok()),
            source: SymptomSource::from_str(&source)?,
            notes,
        });
    }
    Ok(symptoms)
}
