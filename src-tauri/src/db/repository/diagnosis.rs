use std::str::FromStr;

use chrono::NaiveDate;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::DatabaseError;
use crate::models::*;
use crate::models::enums::*;

pub fn insert_diagnosis(conn: &Connection, diag: &Diagnosis) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO diagnoses (id, name, icd_code, date_diagnosed, diagnosing_professional_id, status, document_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            diag.id.to_string(),
            diag.name,
            diag.icd_code,
            diag.date_diagnosed.map(|d| d.to_string()),
            diag.diagnosing_professional_id.map(|id| id.to_string()),
            diag.status.as_str(),
            diag.document_id.to_string(),
        ],
    )?;
    Ok(())
}

pub fn get_active_diagnoses(conn: &Connection) -> Result<Vec<Diagnosis>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, icd_code, date_diagnosed, diagnosing_professional_id, status, document_id
         FROM diagnoses WHERE status = 'active'",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
        ))
    })?;

    diagnosis_rows_to_vec(rows)
}

pub fn get_all_diagnoses(conn: &Connection) -> Result<Vec<Diagnosis>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, icd_code, date_diagnosed, diagnosing_professional_id, status, document_id
         FROM diagnoses",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
        ))
    })?;

    diagnosis_rows_to_vec(rows)
}

type DiagnosisRow = (String, String, Option<String>, Option<String>, Option<String>, String, String);

fn diagnosis_rows_to_vec(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<DiagnosisRow>>,
) -> Result<Vec<Diagnosis>, DatabaseError> {
    let mut diagnoses = Vec::new();
    for row in rows {
        let (id, name, icd_code, date_diagnosed, prof_id, status, doc_id) = row?;
        diagnoses.push(Diagnosis {
            id: Uuid::parse_str(&id)
                .map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
            name,
            icd_code,
            date_diagnosed: date_diagnosed
                .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
            diagnosing_professional_id: prof_id.and_then(|s| Uuid::parse_str(&s).ok()),
            status: DiagnosisStatus::from_str(&status)?,
            document_id: Uuid::parse_str(&doc_id)
                .map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
        });
    }
    Ok(diagnoses)
}
