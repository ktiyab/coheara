//! App-level database — global device registry and access grants.
//!
//! Unencrypted SQLite database stored at `profiles_dir/app.db`.
//! Analogous to Android's system-level database: persists device
//! pairings and access grants across profile switches.

use std::path::Path;

use rusqlite::Connection;
use tracing;

use super::DatabaseError;

/// Open (or create) the app-level database and run migrations.
///
/// This database is **unencrypted** — it stores device metadata and
/// access grants, not patient health data. Patient data remains in
/// per-profile encrypted databases.
pub fn open_app_database(profiles_dir: &Path) -> Result<Connection, DatabaseError> {
    let db_path = profiles_dir.join("app.db");
    let conn = Connection::open(&db_path)?;
    configure_pragmas(&conn)?;
    run_app_migrations(&conn)?;
    Ok(conn)
}

/// Open an in-memory app database (for testing).
pub fn open_memory_app_database() -> Result<Connection, DatabaseError> {
    let conn = Connection::open_in_memory()?;
    configure_pragmas(&conn)?;
    run_app_migrations(&conn)?;
    Ok(conn)
}

fn configure_pragmas(conn: &Connection) -> Result<(), DatabaseError> {
    conn.execute_batch(
        "PRAGMA journal_mode=DELETE;
         PRAGMA foreign_keys=ON;",
    )?;
    Ok(())
}

/// Run all pending app-level migrations.
///
/// Separate migration chain from per-profile databases — the app.db
/// has its own schema_version table and migration numbering.
fn run_app_migrations(conn: &Connection) -> Result<(), DatabaseError> {
    let current_version = get_current_version(conn);

    let migrations: Vec<(i64, &str)> = vec![(
        1,
        include_str!("../../resources/app_migrations/001_device_registry.sql"),
    )];

    for (version, sql) in migrations {
        if version > current_version {
            tracing::info!("Running app migration v{version}");
            conn.execute_batch(sql).map_err(|e| {
                DatabaseError::MigrationFailed {
                    version,
                    reason: e.to_string(),
                }
            })?;
        }
    }

    Ok(())
}

/// Get the current app schema version (0 if no schema exists yet).
fn get_current_version(conn: &Connection) -> i64 {
    conn.query_row(
        "SELECT MAX(version) FROM schema_version",
        [],
        |row| row.get::<_, i64>(0),
    )
    .unwrap_or(0)
}

/// Count tables in the app database (for verification).
pub fn count_app_tables(conn: &Connection) -> Result<i64, DatabaseError> {
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
    fn app_database_initializes_all_tables() {
        let conn = open_memory_app_database().unwrap();
        let count = count_app_tables(&conn).unwrap();
        // schema_version + device_registry + device_profile_access + profile_access_grants = 4
        assert_eq!(count, 4, "Expected 4 tables, got {count}");
    }

    #[test]
    fn app_schema_version_is_current() {
        let conn = open_memory_app_database().unwrap();
        let version: i64 = conn
            .query_row(
                "SELECT MAX(version) FROM schema_version",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn app_migration_idempotent() {
        let conn = open_memory_app_database().unwrap();
        let result = run_app_migrations(&conn);
        assert!(result.is_ok());
    }

    #[test]
    fn app_foreign_keys_enabled() {
        let conn = open_memory_app_database().unwrap();
        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(fk, 1);
    }

    #[test]
    fn app_database_opens_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_app_database(dir.path()).unwrap();
        let count = count_app_tables(&conn).unwrap();
        assert_eq!(count, 4);

        // Re-open — should be idempotent
        let conn2 = open_app_database(dir.path()).unwrap();
        let count2 = count_app_tables(&conn2).unwrap();
        assert_eq!(count2, 4);
    }

    #[test]
    fn cascade_delete_removes_device_access() {
        let conn = open_memory_app_database().unwrap();

        conn.execute(
            "INSERT INTO device_registry (device_id, device_name, device_model, owner_profile_id, public_key)
             VALUES ('dev-1', 'Phone', 'iPhone 15', 'profile-1', X'0102030405')",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO device_profile_access (device_id, profile_id, access_level)
             VALUES ('dev-1', 'profile-1', 'full')",
            [],
        )
        .unwrap();

        // Delete device — cascade should remove access entries
        conn.execute("DELETE FROM device_registry WHERE device_id = 'dev-1'", [])
            .unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM device_profile_access WHERE device_id = 'dev-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn access_level_check_constraint() {
        let conn = open_memory_app_database().unwrap();

        conn.execute(
            "INSERT INTO device_registry (device_id, device_name, device_model, owner_profile_id, public_key)
             VALUES ('dev-1', 'Phone', 'iPhone 15', 'profile-1', X'0102030405')",
            [],
        )
        .unwrap();

        // Valid access levels
        let r1 = conn.execute(
            "INSERT INTO device_profile_access (device_id, profile_id, access_level)
             VALUES ('dev-1', 'profile-1', 'full')",
            [],
        );
        assert!(r1.is_ok());

        let r2 = conn.execute(
            "INSERT INTO device_profile_access (device_id, profile_id, access_level)
             VALUES ('dev-1', 'profile-2', 'read_only')",
            [],
        );
        assert!(r2.is_ok());

        // Invalid access level
        let r3 = conn.execute(
            "INSERT INTO device_profile_access (device_id, profile_id, access_level)
             VALUES ('dev-1', 'profile-3', 'admin')",
            [],
        );
        assert!(r3.is_err());
    }

    #[test]
    fn profile_access_grants_unique_constraint() {
        let conn = open_memory_app_database().unwrap();

        conn.execute(
            "INSERT INTO profile_access_grants (id, granter_profile_id, grantee_profile_id, access_level)
             VALUES ('grant-1', 'alice', 'bob', 'read_only')",
            [],
        )
        .unwrap();

        // Duplicate granter→grantee should fail
        let result = conn.execute(
            "INSERT INTO profile_access_grants (id, granter_profile_id, grantee_profile_id, access_level)
             VALUES ('grant-2', 'alice', 'bob', 'full')",
            [],
        );
        assert!(result.is_err());

        // Reverse direction should succeed (unidirectional)
        let result = conn.execute(
            "INSERT INTO profile_access_grants (id, granter_profile_id, grantee_profile_id, access_level)
             VALUES ('grant-3', 'bob', 'alice', 'read_only')",
            [],
        );
        assert!(result.is_ok());
    }
}
