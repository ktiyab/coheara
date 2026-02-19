use chrono::NaiveDate;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::DatabaseError;
use crate::models::*;

pub fn insert_professional(conn: &Connection, prof: &Professional) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO professionals (id, name, specialty, institution, first_seen_date, last_seen_date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            prof.id.to_string(),
            prof.name,
            prof.specialty,
            prof.institution,
            prof.first_seen_date.map(|d| d.to_string()),
            prof.last_seen_date.map(|d| d.to_string()),
        ],
    )?;
    Ok(())
}

pub fn find_or_create_professional(
    conn: &Connection,
    name: &str,
    specialty: Option<&str>,
) -> Result<Professional, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, specialty, institution, first_seen_date, last_seen_date
         FROM professionals WHERE name = ?1 LIMIT 1",
    )?;

    let result = stmt.query_row(params![name], |row| {
        Ok(Professional {
            id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
            name: row.get(1)?,
            specialty: row.get(2)?,
            institution: row.get(3)?,
            first_seen_date: row
                .get::<_, Option<String>>(4)?
                .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
            last_seen_date: row
                .get::<_, Option<String>>(5)?
                .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
        })
    });

    match result {
        Ok(prof) => Ok(prof),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            let prof = Professional {
                id: Uuid::new_v4(),
                name: name.to_string(),
                specialty: specialty.map(|s| s.to_string()),
                institution: None,
                first_seen_date: Some(chrono::Local::now().date_naive()),
                last_seen_date: Some(chrono::Local::now().date_naive()),
            };
            insert_professional(conn, &prof)?;
            Ok(prof)
        }
        Err(e) => Err(e.into()),
    }
}

pub fn get_all_professionals(conn: &Connection) -> Result<Vec<Professional>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, specialty, institution, first_seen_date, last_seen_date
         FROM professionals",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(Professional {
            id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
            name: row.get(1)?,
            specialty: row.get(2)?,
            institution: row.get(3)?,
            first_seen_date: row
                .get::<_, Option<String>>(4)?
                .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
            last_seen_date: row
                .get::<_, Option<String>>(5)?
                .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
        })
    })?;

    rows.map(|r| r.map_err(DatabaseError::from)).collect()
}
