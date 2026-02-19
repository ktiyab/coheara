use std::str::FromStr;

use chrono::NaiveDate;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::DatabaseError;
use crate::models::*;
use crate::models::enums::*;

pub fn insert_lab_result(conn: &Connection, lab: &LabResult) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO lab_results (id, test_name, test_code, value, value_text, unit,
         reference_range_low, reference_range_high, abnormal_flag, collection_date,
         lab_facility, ordering_physician_id, document_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            lab.id.to_string(),
            lab.test_name,
            lab.test_code,
            lab.value,
            lab.value_text,
            lab.unit,
            lab.reference_range_low,
            lab.reference_range_high,
            lab.abnormal_flag.as_str(),
            lab.collection_date.to_string(),
            lab.lab_facility,
            lab.ordering_physician_id.map(|id| id.to_string()),
            lab.document_id.to_string(),
        ],
    )?;
    Ok(())
}

pub fn get_critical_labs(conn: &Connection) -> Result<Vec<LabResult>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, test_name, test_code, value, value_text, unit,
         reference_range_low, reference_range_high, abnormal_flag, collection_date,
         lab_facility, ordering_physician_id, document_id
         FROM lab_results WHERE abnormal_flag IN ('critical_low', 'critical_high')"
    )?;

    let rows = stmt.query_map([], |row| Ok(lab_row_from_rusqlite(row)))?;

    let mut labs = Vec::new();
    for row in rows {
        labs.push(lab_from_row(row??)?);
    }
    Ok(labs)
}

pub fn get_lab_results_since(
    conn: &Connection,
    since: &NaiveDate,
) -> Result<Vec<LabResult>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, test_name, test_code, value, value_text, unit,
         reference_range_low, reference_range_high, abnormal_flag, collection_date,
         lab_facility, ordering_physician_id, document_id
         FROM lab_results WHERE collection_date >= ?1 ORDER BY collection_date DESC",
    )?;

    let rows = stmt.query_map(params![since.to_string()], |row| Ok(lab_row_from_rusqlite(row)))?;

    let mut labs = Vec::new();
    for row in rows {
        labs.push(lab_from_row(row??)?);
    }
    Ok(labs)
}

pub fn get_lab_results_by_test_name(
    conn: &Connection,
    test_name: &str,
) -> Result<Vec<LabResult>, DatabaseError> {
    let pattern = format!("%{test_name}%");
    let mut stmt = conn.prepare(
        "SELECT id, test_name, test_code, value, value_text, unit,
         reference_range_low, reference_range_high, abnormal_flag, collection_date,
         lab_facility, ordering_physician_id, document_id
         FROM lab_results WHERE LOWER(test_name) LIKE LOWER(?1) ORDER BY collection_date DESC",
    )?;

    let rows = stmt.query_map(params![pattern], |row| Ok(lab_row_from_rusqlite(row)))?;

    let mut labs = Vec::new();
    for row in rows {
        labs.push(lab_from_row(row??)?);
    }
    Ok(labs)
}

/// All lab results for coherence critical-lab and temporal detection.
pub fn get_all_lab_results(conn: &Connection) -> Result<Vec<LabResult>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, test_name, test_code, value, value_text, unit,
         reference_range_low, reference_range_high, abnormal_flag, collection_date,
         lab_facility, ordering_physician_id, document_id
         FROM lab_results ORDER BY collection_date DESC",
    )?;

    let rows = stmt.query_map([], |row| Ok(lab_row_from_rusqlite(row)))?;

    let mut labs = Vec::new();
    for row in rows {
        labs.push(lab_from_row(row??)?);
    }
    Ok(labs)
}

// Internal row type for LabResult mapping
struct LabRow {
    id: String,
    test_name: String,
    test_code: Option<String>,
    value: Option<f64>,
    value_text: Option<String>,
    unit: Option<String>,
    reference_range_low: Option<f64>,
    reference_range_high: Option<f64>,
    abnormal_flag: String,
    collection_date: String,
    lab_facility: Option<String>,
    ordering_physician_id: Option<String>,
    document_id: String,
}

fn lab_row_from_rusqlite(row: &rusqlite::Row<'_>) -> Result<LabRow, rusqlite::Error> {
    Ok(LabRow {
        id: row.get(0)?,
        test_name: row.get(1)?,
        test_code: row.get(2)?,
        value: row.get(3)?,
        value_text: row.get(4)?,
        unit: row.get(5)?,
        reference_range_low: row.get(6)?,
        reference_range_high: row.get(7)?,
        abnormal_flag: row.get(8)?,
        collection_date: row.get(9)?,
        lab_facility: row.get(10)?,
        ordering_physician_id: row.get(11)?,
        document_id: row.get(12)?,
    })
}

fn lab_from_row(row: LabRow) -> Result<LabResult, DatabaseError> {
    Ok(LabResult {
        id: Uuid::parse_str(&row.id).map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
        test_name: row.test_name,
        test_code: row.test_code,
        value: row.value,
        value_text: row.value_text,
        unit: row.unit,
        reference_range_low: row.reference_range_low,
        reference_range_high: row.reference_range_high,
        abnormal_flag: AbnormalFlag::from_str(&row.abnormal_flag)?,
        collection_date: NaiveDate::parse_from_str(&row.collection_date, "%Y-%m-%d").unwrap_or_default(),
        lab_facility: row.lab_facility,
        ordering_physician_id: row.ordering_physician_id.and_then(|s| Uuid::parse_str(&s).ok()),
        document_id: Uuid::parse_str(&row.document_id).map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
    })
}
