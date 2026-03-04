//! ME-06: Screening record repository functions.
//!
//! CRUD operations for the screening_records table.
//! Stores user-reported screening completions and vaccine doses.

use chrono::NaiveDate;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::DatabaseError;

/// A user-reported screening or vaccination record.
#[derive(Debug, Clone)]
pub struct ScreeningRecord {
    pub id: String,
    pub profile_id: String,
    pub screening_key: String,
    pub dose_number: i32,
    pub completed_at: NaiveDate,
    pub provider: Option<String>,
    pub notes: Option<String>,
}

/// Get all screening records for a profile.
pub fn get_screening_records(
    conn: &Connection,
    profile_id: &str,
) -> Result<Vec<ScreeningRecord>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, profile_id, screening_key, dose_number, completed_at, provider, notes
         FROM screening_records
         WHERE profile_id = ?1
         ORDER BY screening_key, dose_number",
    )?;
    let rows = stmt.query_map(params![profile_id], |row| {
        let completed_str: String = row.get(4)?;
        let completed_at = NaiveDate::parse_from_str(&completed_str, "%Y-%m-%d")
            .unwrap_or_else(|_| NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());
        Ok(ScreeningRecord {
            id: row.get(0)?,
            profile_id: row.get(1)?,
            screening_key: row.get(2)?,
            dose_number: row.get(3)?,
            completed_at,
            provider: row.get(5)?,
            notes: row.get(6)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

/// Get records for a specific screening key.
pub fn get_records_for_screening(
    conn: &Connection,
    profile_id: &str,
    screening_key: &str,
) -> Result<Vec<ScreeningRecord>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, profile_id, screening_key, dose_number, completed_at, provider, notes
         FROM screening_records
         WHERE profile_id = ?1 AND screening_key = ?2
         ORDER BY dose_number",
    )?;
    let rows = stmt.query_map(params![profile_id, screening_key], |row| {
        let completed_str: String = row.get(4)?;
        let completed_at = NaiveDate::parse_from_str(&completed_str, "%Y-%m-%d")
            .unwrap_or_else(|_| NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());
        Ok(ScreeningRecord {
            id: row.get(0)?,
            profile_id: row.get(1)?,
            screening_key: row.get(2)?,
            dose_number: row.get(3)?,
            completed_at,
            provider: row.get(5)?,
            notes: row.get(6)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

/// Insert a new screening record.
///
/// Returns the generated record ID.
pub fn insert_screening_record(
    conn: &Connection,
    profile_id: &str,
    screening_key: &str,
    dose_number: i32,
    completed_at: NaiveDate,
    provider: Option<&str>,
    notes: Option<&str>,
) -> Result<String, DatabaseError> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO screening_records (id, profile_id, screening_key, dose_number, completed_at, provider, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            profile_id,
            screening_key,
            dose_number,
            completed_at.format("%Y-%m-%d").to_string(),
            provider,
            notes,
        ],
    )?;
    Ok(id)
}

/// Get all screening records in the database (profile-agnostic).
///
/// Used by RAG pipeline where the DB connection is already profile-scoped.
pub fn get_all_screening_records(conn: &Connection) -> Result<Vec<ScreeningRecord>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, profile_id, screening_key, dose_number, completed_at, provider, notes
         FROM screening_records
         ORDER BY screening_key, dose_number",
    )?;
    let rows = stmt.query_map([], |row| {
        let completed_str: String = row.get(4)?;
        let completed_at = NaiveDate::parse_from_str(&completed_str, "%Y-%m-%d")
            .unwrap_or_else(|_| NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());
        Ok(ScreeningRecord {
            id: row.get(0)?,
            profile_id: row.get(1)?,
            screening_key: row.get(2)?,
            dose_number: row.get(3)?,
            completed_at,
            provider: row.get(5)?,
            notes: row.get(6)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

/// Delete a screening record by ID, scoped to profile.
///
/// Returns true if a row was deleted, false if not found.
pub fn delete_screening_record(
    conn: &Connection,
    record_id: &str,
    profile_id: &str,
) -> Result<bool, DatabaseError> {
    let rows = conn.execute(
        "DELETE FROM screening_records WHERE id = ?1 AND profile_id = ?2",
        params![record_id, profile_id],
    )?;
    Ok(rows > 0)
}
