use chrono::NaiveDateTime;
use rusqlite::{params, Connection};

use crate::db::DatabaseError;
use crate::models::*;

pub fn get_profile_trust(conn: &Connection) -> Result<ProfileTrust, DatabaseError> {
    conn.query_row(
        "SELECT total_documents, documents_verified, documents_corrected, extraction_accuracy, last_updated
         FROM profile_trust WHERE id = 1",
        [],
        |row| {
            Ok(ProfileTrust {
                total_documents: row.get(0)?,
                documents_verified: row.get(1)?,
                documents_corrected: row.get(2)?,
                extraction_accuracy: row.get(3)?,
                last_updated: row.get::<_, String>(4)
                    .map(|s| NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S").unwrap_or_default())?,
            })
        },
    ).map_err(|e| e.into())
}

pub fn update_profile_trust_verified(conn: &Connection) -> Result<(), DatabaseError> {
    conn.execute(
        "UPDATE profile_trust SET
         documents_verified = documents_verified + 1,
         total_documents = total_documents + 1,
         extraction_accuracy = CASE
           WHEN total_documents > 0
           THEN CAST(documents_verified + 1 AS REAL) / CAST(total_documents + 1 AS REAL)
           ELSE 1.0
         END,
         last_updated = datetime('now')
         WHERE id = 1",
        [],
    )?;
    Ok(())
}

pub fn update_profile_trust_corrected(conn: &Connection) -> Result<(), DatabaseError> {
    conn.execute(
        "UPDATE profile_trust SET
         documents_corrected = documents_corrected + 1,
         documents_verified = documents_verified + 1,
         total_documents = total_documents + 1,
         extraction_accuracy = CASE
           WHEN total_documents > 0
           THEN CAST(documents_verified + 1 AS REAL) / CAST(total_documents + 1 AS REAL)
           ELSE 1.0
         END,
         last_updated = datetime('now')
         WHERE id = 1",
        [],
    )?;
    Ok(())
}

/// Recalculate trust metrics from actual document data.
///
/// Counts documents by status instead of relying on incremental counters.
/// Use after cascade deletes, reprocessing, or any state where counters may drift.
pub fn recalculate_profile_trust(conn: &Connection) -> Result<(), DatabaseError> {
    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM documents",
        [],
        |r| r.get(0),
    )?;
    let verified: i64 = conn.query_row(
        "SELECT COUNT(*) FROM documents WHERE verified = 1",
        [],
        |r| r.get(0),
    )?;
    let accuracy = if total > 0 {
        verified as f64 / total as f64
    } else {
        0.0
    };
    conn.execute(
        "UPDATE profile_trust SET
         total_documents = ?1,
         documents_verified = ?2,
         extraction_accuracy = ?3,
         last_updated = datetime('now')
         WHERE id = 1",
        params![total, verified, accuracy],
    )?;
    Ok(())
}

/// Used after the storage pipeline has already called `update_profile_trust_verified`.
pub fn increment_documents_corrected(conn: &Connection) -> Result<(), DatabaseError> {
    conn.execute(
        "UPDATE profile_trust SET
         documents_corrected = documents_corrected + 1,
         last_updated = datetime('now')
         WHERE id = 1",
        [],
    )?;
    Ok(())
}
