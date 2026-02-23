use rusqlite::{params, Connection};

use crate::db::DatabaseError;

/// Insert a batch of audit entries into the audit_log table.
/// E8: Now includes optional profile_id for multi-profile access tracking.
pub fn insert_audit_entries(
    conn: &Connection,
    entries: &[(String, String, String, String, Option<String>)], // (timestamp, source, action, entity, profile_id)
) -> Result<(), DatabaseError> {
    let mut stmt = conn.prepare(
        "INSERT INTO audit_log (timestamp, source, action, entity, profile_id) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for (timestamp, source, action, entity, profile_id) in entries {
        stmt.execute(params![timestamp, source, action, entity, profile_id])?;
    }
    Ok(())
}

/// Prune audit entries older than the given number of days.
pub fn prune_audit_log(conn: &Connection, retention_days: i64) -> Result<usize, DatabaseError> {
    let deleted = conn.execute(
        "DELETE FROM audit_log WHERE timestamp < datetime('now', ?1)",
        params![format!("-{retention_days} days")],
    )?;
    Ok(deleted)
}

/// E8: Query audit entries for a specific profile within the last N days.
/// Returns (timestamp, source, action, entity) tuples.
pub fn query_audit_by_profile(
    conn: &Connection,
    profile_id: &str,
    days: i64,
) -> Result<Vec<(String, String, String, String)>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT timestamp, source, action, entity FROM audit_log
         WHERE profile_id = ?1 AND timestamp >= datetime('now', ?2)
         ORDER BY timestamp DESC",
    )?;
    let rows = stmt
        .query_map(params![profile_id, format!("-{days} days")], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
