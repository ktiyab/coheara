use chrono::NaiveDate;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::DatabaseError;
use crate::models::*;

pub fn insert_procedure(conn: &Connection, proc: &Procedure) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO procedures (id, name, date, performing_professional_id, facility, outcome,
         follow_up_required, follow_up_date, document_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            proc.id.to_string(),
            proc.name,
            proc.date.map(|d| d.to_string()),
            proc.performing_professional_id.map(|id| id.to_string()),
            proc.facility,
            proc.outcome,
            proc.follow_up_required as i32,
            proc.follow_up_date.map(|d| d.to_string()),
            proc.document_id.to_string(),
        ],
    )?;
    Ok(())
}

pub fn get_all_procedures(conn: &Connection) -> Result<Vec<Procedure>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, date, performing_professional_id, facility, outcome,
         follow_up_required, follow_up_date, document_id
         FROM procedures ORDER BY date DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, i32>(6)?,
            row.get::<_, Option<String>>(7)?,
            row.get::<_, String>(8)?,
        ))
    })?;

    let mut procedures = Vec::new();
    for row in rows {
        let (id, name, date, prof_id, facility, outcome, follow_up, follow_up_date, doc_id) = row?;
        procedures.push(Procedure {
            id: Uuid::parse_str(&id)
                .map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
            name,
            date: date.and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
            performing_professional_id: prof_id.and_then(|s| Uuid::parse_str(&s).ok()),
            facility,
            outcome,
            follow_up_required: follow_up != 0,
            follow_up_date: follow_up_date
                .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
            document_id: Uuid::parse_str(&doc_id)
                .map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
        });
    }
    Ok(procedures)
}
