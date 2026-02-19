use rusqlite::{params, Connection};

use crate::db::DatabaseError;

/// Insert a batch of audit entries into the audit_log table.
pub fn insert_audit_entries(
    conn: &Connection,
    entries: &[(String, String, String, String)], // (timestamp, source, action, entity)
) -> Result<(), DatabaseError> {
    let mut stmt = conn.prepare(
        "INSERT INTO audit_log (timestamp, source, action, entity) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (timestamp, source, action, entity) in entries {
        stmt.execute(params![timestamp, source, action, entity])?;
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
