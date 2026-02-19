use std::str::FromStr;

use chrono::NaiveDate;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::DatabaseError;
use crate::models::*;
use crate::models::enums::*;

pub fn insert_allergy(conn: &Connection, allergy: &Allergy) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO allergies (id, allergen, reaction, severity, date_identified, source, document_id, verified)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            allergy.id.to_string(),
            allergy.allergen,
            allergy.reaction,
            allergy.severity.as_str(),
            allergy.date_identified.map(|d| d.to_string()),
            allergy.source.as_str(),
            allergy.document_id.map(|id| id.to_string()),
            allergy.verified as i32,
        ],
    )?;
    Ok(())
}

pub fn get_all_allergies(conn: &Connection) -> Result<Vec<Allergy>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, allergen, reaction, severity, date_identified, source, document_id, verified
         FROM allergies",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, i32>(7)?,
        ))
    })?;

    let mut allergies = Vec::new();
    for row in rows {
        let (id, allergen, reaction, severity, date_identified, source, document_id, verified) =
            row?;
        allergies.push(Allergy {
            id: Uuid::parse_str(&id)
                .map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
            allergen,
            reaction,
            severity: AllergySeverity::from_str(&severity)?,
            date_identified: date_identified
                .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
            source: AllergySource::from_str(&source)?,
            document_id: document_id.and_then(|s| Uuid::parse_str(&s).ok()),
            verified: verified != 0,
        });
    }
    Ok(allergies)
}
