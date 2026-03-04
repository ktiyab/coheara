use std::str::FromStr;

use chrono::NaiveDate;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::DatabaseError;
use crate::models::*;
use crate::models::enums::*;

pub fn insert_allergy(conn: &Connection, allergy: &Allergy) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO allergies (id, allergen, reaction, severity, allergen_category, date_identified, source, document_id, verified)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            allergy.id.to_string(),
            allergy.allergen,
            allergy.reaction,
            allergy.severity.as_str(),
            allergy.allergen_category.as_ref().map(|c| c.as_str()),
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
        "SELECT id, allergen, reaction, severity, allergen_category, date_identified, source, document_id, verified
         FROM allergies",
    )?;

    let rows = stmt.query_map([], row_to_allergy)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

/// Get a single allergy by ID.
pub fn get_allergy_by_id(conn: &Connection, id: &Uuid) -> Result<Option<Allergy>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, allergen, reaction, severity, allergen_category, date_identified, source, document_id, verified
         FROM allergies WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id.to_string()], row_to_allergy)?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Update an existing allergy (partial update: only non-None fields change).
pub fn update_allergy(
    conn: &Connection,
    id: &Uuid,
    allergen: Option<&str>,
    reaction: Option<Option<&str>>,
    severity: Option<&AllergySeverity>,
    allergen_category: Option<Option<&AllergenCategory>>,
    date_identified: Option<Option<NaiveDate>>,
) -> Result<(), DatabaseError> {
    let mut sets = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(v) = allergen {
        sets.push("allergen = ?");
        values.push(Box::new(v.to_string()));
    }
    if let Some(v) = reaction {
        sets.push("reaction = ?");
        values.push(Box::new(v.map(|s| s.to_string())));
    }
    if let Some(v) = severity {
        sets.push("severity = ?");
        values.push(Box::new(v.as_str().to_string()));
    }
    if let Some(v) = allergen_category {
        sets.push("allergen_category = ?");
        values.push(Box::new(v.map(|c| c.as_str().to_string())));
    }
    if let Some(v) = date_identified {
        sets.push("date_identified = ?");
        values.push(Box::new(v.map(|d| d.to_string())));
    }

    if sets.is_empty() {
        return Ok(());
    }

    let sql = format!("UPDATE allergies SET {} WHERE id = ?", sets.join(", "));
    values.push(Box::new(id.to_string()));

    let params: Vec<&dyn rusqlite::types::ToSql> = values.iter().map(|v| v.as_ref()).collect();
    let affected = conn.execute(&sql, params.as_slice())?;
    if affected == 0 {
        return Err(DatabaseError::NotFound {
            entity_type: "allergy".into(),
            id: id.to_string(),
        });
    }
    Ok(())
}

/// Delete an allergy by ID.
pub fn delete_allergy(conn: &Connection, id: &Uuid) -> Result<bool, DatabaseError> {
    let affected = conn.execute(
        "DELETE FROM allergies WHERE id = ?1",
        params![id.to_string()],
    )?;
    Ok(affected > 0)
}

/// Set verified = true for an allergy.
pub fn verify_allergy(conn: &Connection, id: &Uuid) -> Result<(), DatabaseError> {
    let affected = conn.execute(
        "UPDATE allergies SET verified = 1 WHERE id = ?1",
        params![id.to_string()],
    )?;
    if affected == 0 {
        return Err(DatabaseError::NotFound {
            entity_type: "allergy".into(),
            id: id.to_string(),
        });
    }
    Ok(())
}

fn row_to_allergy(row: &rusqlite::Row) -> Result<Allergy, rusqlite::Error> {
    let id_str: String = row.get(0)?;
    let allergen: String = row.get(1)?;
    let reaction: Option<String> = row.get(2)?;
    let severity_str: String = row.get(3)?;
    let category_str: Option<String> = row.get(4)?;
    let date_str: Option<String> = row.get(5)?;
    let source_str: String = row.get(6)?;
    let document_id_str: Option<String> = row.get(7)?;
    let verified_int: i32 = row.get(8)?;

    Ok(Allergy {
        id: Uuid::parse_str(&id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        allergen,
        reaction,
        severity: AllergySeverity::from_str(&severity_str)
            .unwrap_or(AllergySeverity::Moderate),
        allergen_category: category_str
            .and_then(|s| AllergenCategory::from_str(&s).ok()),
        date_identified: date_str
            .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
        source: AllergySource::from_str(&source_str)
            .unwrap_or(AllergySource::DocumentExtracted),
        document_id: document_id_str.and_then(|s| Uuid::parse_str(&s).ok()),
        verified: verified_int != 0,
    })
}
