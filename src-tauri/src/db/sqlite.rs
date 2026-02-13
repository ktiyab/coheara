use std::path::Path;

use rusqlite::Connection;
use tracing;

use super::DatabaseError;

/// Open a SQLite connection to the given path and run migrations
pub fn open_database(path: &Path) -> Result<Connection, DatabaseError> {
    let conn = Connection::open(path)?;
    configure_pragmas(&conn)?;
    run_migrations(&conn)?;
    Ok(conn)
}

/// Open an in-memory database (for testing)
pub fn open_memory_database() -> Result<Connection, DatabaseError> {
    let conn = Connection::open_in_memory()?;
    configure_pragmas(&conn)?;
    run_migrations(&conn)?;
    Ok(conn)
}

fn configure_pragmas(conn: &Connection) -> Result<(), DatabaseError> {
    conn.execute_batch(
        "PRAGMA journal_mode=DELETE;
         PRAGMA foreign_keys=ON;"
    )?;
    Ok(())
}

/// Run all pending migrations
pub fn run_migrations(conn: &Connection) -> Result<(), DatabaseError> {
    let current_version = get_current_version(conn);

    let migrations: Vec<(i64, &str)> = vec![
        (1, include_str!("../../resources/migrations/001_initial.sql")),
        (2, include_str!("../../resources/migrations/002_device_pairing.sql")),
        (3, include_str!("../../resources/migrations/003_sync_versions.sql")),
        (4, include_str!("../../resources/migrations/004_coherence_alerts.sql")),
        (5, include_str!("../../resources/migrations/005_audit_log.sql")),
        (6, include_str!("../../resources/migrations/006_vector_chunks.sql")),
    ];

    for (version, sql) in migrations {
        if version > current_version {
            tracing::info!("Running migration v{version}");
            conn.execute_batch(sql).map_err(|e| DatabaseError::MigrationFailed {
                version,
                reason: e.to_string(),
            })?;
        }
    }

    Ok(())
}

/// Get the current schema version (0 if no schema exists yet)
fn get_current_version(conn: &Connection) -> i64 {
    conn.query_row(
        "SELECT MAX(version) FROM schema_version",
        [],
        |row| row.get::<_, i64>(0),
    )
    .unwrap_or(0)
}

/// Count tables in the database (for verification)
pub fn count_tables(conn: &Connection) -> Result<i64, DatabaseError> {
    let count = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_initializes_all_tables() {
        let conn = open_memory_database().unwrap();
        // 18 entity tables + schema_version + dose_references + 3 device pairing + sync_versions + coherence_alerts + audit_log + vector_chunks = 27 total
        let count = count_tables(&conn).unwrap();
        assert!(count >= 24, "Expected at least 24 tables, got {count}");
    }

    #[test]
    fn schema_version_is_current() {
        let conn = open_memory_database().unwrap();
        let version: i64 = conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 6);
    }

    #[test]
    fn migration_idempotent() {
        let conn = open_memory_database().unwrap();
        // Run migrations again — should not error
        let result = run_migrations(&conn);
        assert!(result.is_ok());
    }

    #[test]
    fn foreign_keys_enabled() {
        let conn = open_memory_database().unwrap();
        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(fk, 1);
    }

    #[test]
    fn profile_trust_singleton_initialized() {
        let conn = open_memory_database().unwrap();
        let total: i32 = conn
            .query_row(
                "SELECT total_documents FROM profile_trust WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(total, 0);
    }
}
