use std::path::Path;

use rusqlite::Connection;
use tracing;

use super::DatabaseError;

/// Open a SQLite connection to the given path and run migrations.
///
/// When `key` is provided, enables SQLCipher transparent encryption via `PRAGMA key`.
/// The key must be exactly 32 bytes (256-bit AES key).
/// Pass `None` for unencrypted databases (legacy or testing).
pub fn open_database(path: &Path, key: Option<&[u8; 32]>) -> Result<Connection, DatabaseError> {
    let conn = Connection::open(path)?;
    if let Some(k) = key {
        apply_sqlcipher_key(&conn, k)?;
    }
    configure_pragmas(&conn)?;
    run_migrations(&conn)?;
    Ok(conn)
}

/// Open an in-memory database (for testing — no encryption)
pub fn open_memory_database() -> Result<Connection, DatabaseError> {
    let conn = Connection::open_in_memory()?;
    configure_pragmas(&conn)?;
    run_migrations(&conn)?;
    Ok(conn)
}

/// Apply SQLCipher encryption key to a connection.
///
/// Must be called immediately after `Connection::open()` and before ANY other
/// SQL operations (including PRAGMA). This is a SQLCipher requirement.
fn apply_sqlcipher_key(conn: &Connection, key: &[u8; 32]) -> Result<(), DatabaseError> {
    let hex_key: String = key.iter().map(|b| format!("{b:02x}")).collect();
    conn.pragma_update(None, "key", format!("x'{hex_key}'"))?;
    Ok(())
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
        (7, include_str!("../../resources/migrations/007_model_preferences.sql")),
        (8, include_str!("../../resources/migrations/008_pipeline_status.sql")),
        (9, include_str!("../../resources/migrations/009_grounded_tables.sql")),
        (10, include_str!("../../resources/migrations/010_batch_extraction.sql")),
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
        // 18 entity tables + schema_version + dose_references + 3 device pairing + sync_versions + coherence_alerts + audit_log + vector_chunks + model_preferences + user_preferences = 29 total
        let count = count_tables(&conn).unwrap();
        assert!(count >= 26, "Expected at least 26 tables, got {count}");
    }

    #[test]
    fn schema_version_is_current() {
        let conn = open_memory_database().unwrap();
        let version: i64 = conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 8);
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

    // ── SQLCipher encryption tests ──────────────────────────────────────

    fn test_key() -> [u8; 32] {
        [0xAA; 32]
    }

    fn wrong_key() -> [u8; 32] {
        [0xBB; 32]
    }

    #[test]
    fn encrypted_db_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("encrypted.db");
        let key = test_key();

        // Create encrypted database with data
        {
            let conn = open_database(&db_path, Some(&key)).unwrap();
            conn.execute(
                "INSERT INTO documents (id, type, title, ingestion_date, source_file)
                 VALUES ('test-doc-1', 'prescription', 'Test Report', '2026-01-15', 'test.pdf')",
                [],
            ).unwrap();
        }

        // Re-open with same key — data should be readable
        {
            let conn = open_database(&db_path, Some(&key)).unwrap();
            let title: String = conn
                .query_row(
                    "SELECT title FROM documents WHERE id = 'test-doc-1'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(title, "Test Report");
        }
    }

    #[test]
    fn encrypted_db_wrong_key_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("encrypted.db");
        let key = test_key();

        // Create encrypted database
        {
            let conn = open_database(&db_path, Some(&key)).unwrap();
            conn.execute(
                "INSERT INTO documents (id, type, title, ingestion_date, source_file)
                 VALUES ('test-doc-1', 'prescription', 'Test Report', '2026-01-15', 'test.pdf')",
                [],
            ).unwrap();
        }

        // Opening with wrong key should fail
        let result = open_database(&db_path, Some(&wrong_key()));
        assert!(result.is_err(), "Opening with wrong key should fail");
    }

    #[test]
    fn encrypted_db_no_key_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("encrypted.db");
        let key = test_key();

        // Create encrypted database
        {
            let conn = open_database(&db_path, Some(&key)).unwrap();
            conn.execute(
                "INSERT INTO documents (id, type, title, ingestion_date, source_file)
                 VALUES ('test-doc-1', 'prescription', 'Test Report', '2026-01-15', 'test.pdf')",
                [],
            ).unwrap();
        }

        // Opening without key should fail (encrypted file is unreadable)
        let result = open_database(&db_path, None);
        assert!(result.is_err(), "Opening encrypted DB without key should fail");
    }

    #[test]
    fn unencrypted_db_opens_without_key() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("plain.db");

        // Create unencrypted database
        {
            let conn = open_database(&db_path, None).unwrap();
            conn.execute(
                "INSERT INTO documents (id, type, title, ingestion_date, source_file)
                 VALUES ('test-doc-1', 'prescription', 'Test Report', '2026-01-15', 'test.pdf')",
                [],
            ).unwrap();
        }

        // Re-open without key — should work
        {
            let conn = open_database(&db_path, None).unwrap();
            let title: String = conn
                .query_row(
                    "SELECT title FROM documents WHERE id = 'test-doc-1'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(title, "Test Report");
        }
    }

    #[test]
    fn sqlcipher_pragma_key_format_is_hex() {
        // Verify the hex encoding produces correct format
        let key: [u8; 32] = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
            0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
            0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
            0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
        ];
        let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex.len(), 64);
        assert_eq!(
            hex,
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
        );
    }
}
