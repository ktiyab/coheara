//! L5-01: Trust & Safety — cross-cutting hardening layer.
//!
//! Five sub-systems:
//! 1. Emergency Protocol — critical lab alerts with 2-step dismissal
//! 2. Dose Plausibility — cross-reference doses against known ranges
//! 3. Backup & Restore — encrypted .coheara-backup archives
//! 4. Cryptographic Erasure — profile deletion via key zeroing
//! 5. Privacy Verification — prove offline + encryption promises

mod backup;
mod dose;
mod emergency;
mod erasure;
pub mod fs_helpers;
mod recovery;

use crate::db::DatabaseError;
use thiserror::Error;

// ═══════════════════════════════════════════════════════════════════════════
// Error type
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Error, Debug)]
pub enum TrustError {
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Crypto error: {0}")]
    Crypto(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<crate::crypto::CryptoError> for TrustError {
    fn from(e: crate::crypto::CryptoError) -> Self {
        TrustError::Crypto(e.to_string())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Re-exports
// ═══════════════════════════════════════════════════════════════════════════

pub use backup::*;
pub use dose::*;
pub use emergency::*;
pub use erasure::*;
pub use recovery::*;

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;
    use std::io::Write;
    use std::path::Path;

    /// Set up a test database with the required tables and sample data.
    fn setup_test_db() -> rusqlite::Connection {
        let conn = open_memory_database().unwrap();

        // Insert parent document (required by lab_results FK)
        conn.execute(
            "INSERT INTO documents (id, type, title, ingestion_date, source_file)
             VALUES ('doc-1', 'lab_result', 'Blood Work Jan 2026', '2026-01-15', 'bloodwork.pdf')",
            [],
        )
        .unwrap();

        // Insert sample critical lab result
        conn.execute(
            "INSERT INTO lab_results (id, test_name, value, value_text, unit,
             reference_range_low, reference_range_high, abnormal_flag,
             collection_date, lab_facility, ordering_physician_id, document_id)
             VALUES ('lab-1', 'Potassium', 6.5, NULL, 'mEq/L', 3.5, 5.0,
             'critical_high', '2026-01-15', 'City Lab', NULL, 'doc-1')",
            [],
        )
        .unwrap();

        // Insert a normal lab result
        conn.execute(
            "INSERT INTO lab_results (id, test_name, value, value_text, unit,
             reference_range_low, reference_range_high, abnormal_flag,
             collection_date, lab_facility, ordering_physician_id, document_id)
             VALUES ('lab-2', 'Glucose', 95.0, NULL, 'mg/dL', 70.0, 100.0,
             'normal', '2026-01-15', 'City Lab', NULL, 'doc-1')",
            [],
        )
        .unwrap();

        // Insert dose references
        conn.execute(
            "INSERT INTO dose_references (generic_name, typical_min_mg, typical_max_mg, absolute_max_mg, unit, source)
             VALUES ('metformin', 500.0, 2550.0, 1000.0, 'mg', 'bundled')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO dose_references (generic_name, typical_min_mg, typical_max_mg, absolute_max_mg, unit, source)
             VALUES ('lisinopril', 2.5, 40.0, 40.0, 'mg', 'bundled')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO dose_references (generic_name, typical_min_mg, typical_max_mg, absolute_max_mg, unit, source)
             VALUES ('atorvastatin', 10.0, 80.0, 80.0, 'mg', 'bundled')",
            [],
        )
        .unwrap();

        // Insert medication alias for brand name resolution
        conn.execute(
            "INSERT INTO medication_aliases (generic_name, brand_name, country, source)
             VALUES ('metformin', 'Glucophage', 'US', 'bundled')",
            [],
        )
        .unwrap();

        conn
    }

    // ─── Emergency Protocol Tests ───

    #[test]
    fn test_critical_alert_fetch() {
        let conn = setup_test_db();
        let alerts = fetch_critical_alerts(&conn).unwrap();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].test_name, "Potassium");
        assert_eq!(alerts[0].abnormal_flag, "critical_high");
        assert!(!alerts[0].dismissed);
    }

    #[test]
    fn test_critical_alert_excludes_dismissed() {
        let conn = setup_test_db();

        // Dismiss the critical alert
        conn.execute(
            "INSERT INTO dismissed_alerts (id, alert_type, entity_ids, dismissed_date, reason, dismissed_by)
             VALUES ('dismiss-1', 'critical', '[\"lab-1\"]', '2026-01-16 10:00:00', 'Doctor reviewed', 'patient')",
            [],
        ).unwrap();

        let alerts = fetch_critical_alerts(&conn).unwrap();
        assert!(alerts.is_empty());
    }

    #[test]
    fn test_critical_dismiss_step1() {
        let conn = setup_test_db();
        let request = CriticalDismissRequest {
            alert_id: "lab-1".into(),
            step: DismissStep::AskConfirmation,
        };
        assert!(dismiss_critical_alert(&conn, &request).is_ok());
    }

    #[test]
    fn test_critical_dismiss_step1_not_found() {
        let conn = setup_test_db();
        let request = CriticalDismissRequest {
            alert_id: "nonexistent".into(),
            step: DismissStep::AskConfirmation,
        };
        let result = dismiss_critical_alert(&conn, &request);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_critical_dismiss_step2() {
        let conn = setup_test_db();
        let request = CriticalDismissRequest {
            alert_id: "lab-1".into(),
            step: DismissStep::ConfirmDismissal {
                reason: "My doctor has reviewed this result".into(),
            },
        };
        assert!(dismiss_critical_alert(&conn, &request).is_ok());

        // Verify dismissal record was created
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM dismissed_alerts WHERE alert_type = 'critical'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_critical_dismiss_requires_reason() {
        let conn = setup_test_db();
        let request = CriticalDismissRequest {
            alert_id: "lab-1".into(),
            step: DismissStep::ConfirmDismissal {
                reason: "".into(),
            },
        };
        let result = dismiss_critical_alert(&conn, &request);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Reason required"));
    }

    // ─── Dose Plausibility Tests ───

    #[test]
    fn test_dose_plausible() {
        let conn = setup_test_db();
        let result = check_dose_plausibility(&conn, "metformin", 500.0, "mg").unwrap();
        assert_eq!(result.plausibility, PlausibilityResult::Plausible);
    }

    #[test]
    fn test_dose_high() {
        let conn = setup_test_db();
        let result = check_dose_plausibility(&conn, "metformin", 3000.0, "mg").unwrap();
        assert!(matches!(result.plausibility, PlausibilityResult::HighDose { .. }));
    }

    #[test]
    fn test_dose_very_high() {
        let conn = setup_test_db();
        let result = check_dose_plausibility(&conn, "metformin", 50000.0, "mg").unwrap();
        assert!(matches!(result.plausibility, PlausibilityResult::VeryHighDose { .. }));
    }

    #[test]
    fn test_dose_low() {
        let conn = setup_test_db();
        let result = check_dose_plausibility(&conn, "metformin", 10.0, "mg").unwrap();
        assert!(matches!(result.plausibility, PlausibilityResult::LowDose { .. }));
    }

    #[test]
    fn test_dose_unknown_medication() {
        let conn = setup_test_db();
        let result = check_dose_plausibility(&conn, "xyzabc123", 100.0, "mg").unwrap();
        assert_eq!(result.plausibility, PlausibilityResult::UnknownMedication);
    }

    #[test]
    fn test_dose_unit_conversion() {
        assert!((convert_to_mg(1.0, "g") - 1000.0).abs() < f64::EPSILON);
        assert!((convert_to_mg(1000.0, "mcg") - 1.0).abs() < f64::EPSILON);
        assert!((convert_to_mg(500.0, "mg") - 500.0).abs() < f64::EPSILON);
        assert!((convert_to_mg(500.0, "ug") - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_dose_brand_name_resolution() {
        let conn = setup_test_db();
        // "Glucophage" should resolve to "metformin" via medication_aliases
        let result = check_dose_plausibility(&conn, "Glucophage", 500.0, "mg").unwrap();
        assert_eq!(result.plausibility, PlausibilityResult::Plausible);
    }

    #[test]
    fn test_dose_with_gram_unit() {
        let conn = setup_test_db();
        // 0.5g = 500mg metformin → plausible
        let result = check_dose_plausibility(&conn, "metformin", 0.5, "g").unwrap();
        assert_eq!(result.plausibility, PlausibilityResult::Plausible);
    }

    // ─── Backup & Restore Tests ───

    #[test]
    fn test_backup_create_and_preview() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_dir = tmp.path().join("test-profile");

        // Set up minimal profile structure
        std::fs::create_dir_all(profile_dir.join("database")).unwrap();
        std::fs::create_dir_all(profile_dir.join("originals")).unwrap();

        // Create a test database
        let db_path = profile_dir.join("database/coheara.db");
        let conn = crate::db::sqlite::open_database(&db_path, None).unwrap();
        conn.execute(
            "INSERT INTO documents (id, type, title, ingestion_date, source_file)
             VALUES ('doc-1', 'lab_result', 'Test Report', '2026-01-15', 'test.pdf')",
            [],
        )
        .unwrap();
        drop(conn);

        // Create a salt file
        let salt = crate::crypto::keys::generate_salt();
        std::fs::write(profile_dir.join("salt.bin"), salt).unwrap();

        // Create profile key
        let key = crate::crypto::keys::ProfileKey::derive("testpassword", &salt);

        // Create backup
        let backup_path = tmp.path().join("test.coheara-backup");
        let result = create_backup_with_key(
            &profile_dir, "Test Profile", &|p| key.encrypt(p), &backup_path, None,
        ).unwrap();

        assert!(backup_path.exists());
        assert_eq!(result.total_documents, 1);
        assert!(result.encrypted);
        assert!(result.total_size_bytes > 0);

        // Preview backup
        let preview = preview_backup(&backup_path).unwrap();
        assert_eq!(preview.metadata.version, 1);
        assert_eq!(preview.metadata.profile_name, "Test Profile");
        assert_eq!(preview.metadata.document_count, 1);
        assert!(preview.compatible);
        assert!(preview.compatibility_message.is_none());
    }

    #[test]
    fn test_backup_restore_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_dir = tmp.path().join("source-profile");

        // Create directories first
        std::fs::create_dir_all(profile_dir.join("database")).unwrap();

        // Derive key so we can create an encrypted DB (matching production flow)
        let salt = crate::crypto::keys::generate_salt();
        std::fs::write(profile_dir.join("salt.bin"), salt).unwrap();
        let key = crate::crypto::keys::ProfileKey::derive("mypassword", &salt);
        let db_path = profile_dir.join("database/coheara.db");
        let conn = crate::db::sqlite::open_database(&db_path, Some(key.as_bytes())).unwrap();
        conn.execute(
            "INSERT INTO documents (id, type, title, ingestion_date, source_file)
             VALUES ('doc-1', 'prescription', 'Report 1', '2026-01-15', 'report.pdf')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO documents (id, type, title, ingestion_date, source_file)
             VALUES ('doc-2', 'lab_result', 'Bloodwork', '2026-02-01', 'bloodwork.pdf')",
            [],
        )
        .unwrap();
        drop(conn);

        // Backup
        let backup_path = tmp.path().join("roundtrip.coheara-backup");
        create_backup_with_key(
            &profile_dir, "Round Trip Test", &|p| key.encrypt(p), &backup_path, Some(key.as_bytes()),
        ).unwrap();

        // Restore to new directory
        let restore_dir = tmp.path().join("restored-profile");
        let result = restore_backup(&backup_path, "mypassword", &restore_dir).unwrap();

        assert_eq!(result.documents_restored, 2);
        assert!(result.warnings.is_empty());
        assert!(restore_dir.join("database/coheara.db").exists());
    }

    #[test]
    fn test_backup_wrong_password() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_dir = tmp.path().join("wp-profile");

        std::fs::create_dir_all(profile_dir.join("database")).unwrap();
        let db_path = profile_dir.join("database/coheara.db");
        let conn = crate::db::sqlite::open_database(&db_path, None).unwrap();
        conn.execute(
            "INSERT INTO documents (id, type, title, ingestion_date, source_file)
             VALUES ('doc-1', 'lab_result', 'Test Report', '2026-01-15', 'test.pdf')",
            [],
        )
        .unwrap();
        drop(conn);

        let salt = crate::crypto::keys::generate_salt();
        std::fs::write(profile_dir.join("salt.bin"), salt).unwrap();
        let key = crate::crypto::keys::ProfileKey::derive("correctpassword", &salt);

        let backup_path = tmp.path().join("wp.coheara-backup");
        create_backup_with_key(
            &profile_dir, "WP Test", &|p| key.encrypt(p), &backup_path, None,
        ).unwrap();

        // Try restoring with wrong password
        let restore_dir = tmp.path().join("wp-restore");
        let result = restore_backup(&backup_path, "wrongpassword", &restore_dir);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Incorrect password") || err_msg.contains("corrupted"),
            "Expected password/corruption error, got: {err_msg}"
        );
    }

    #[test]
    fn test_backup_corrupted() {
        let tmp = tempfile::tempdir().unwrap();
        let backup_path = tmp.path().join("corrupted.coheara-backup");

        // Write just the magic bytes and truncate
        let mut file = std::fs::File::create(&backup_path).unwrap();
        file.write_all(b"COHEARA\x01").unwrap();
        drop(file);

        let result = preview_backup(&backup_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_backup_invalid_magic() {
        let tmp = tempfile::tempdir().unwrap();
        let backup_path = tmp.path().join("invalid.coheara-backup");

        let mut file = std::fs::File::create(&backup_path).unwrap();
        file.write_all(b"NOTVALID").unwrap();
        drop(file);

        let result = preview_backup(&backup_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Not a valid"));
    }

    // ─── Cryptographic Erasure Tests ───

    #[test]
    fn test_erasure_wrong_confirmation() {
        let tmp = tempfile::tempdir().unwrap();
        let request = ErasureRequest {
            profile_id: uuid::Uuid::new_v4().to_string(),
            confirmation_text: "delete my data".into(), // lowercase — should fail
            password: "password".into(),
        };
        let result = erase_profile_data(tmp.path(), &request);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("DELETE MY DATA"));
    }

    #[test]
    fn test_erasure_nonexistent_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let request = ErasureRequest {
            profile_id: uuid::Uuid::new_v4().to_string(),
            confirmation_text: "DELETE MY DATA".into(),
            password: "password".into(),
        };
        let result = erase_profile_data(tmp.path(), &request);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    // ─── Privacy Info Tests ───

    #[test]
    fn test_privacy_info() {
        let tmp = tempfile::tempdir().unwrap();
        let profile_dir = tmp.path().join("privacy-test");
        std::fs::create_dir_all(profile_dir.join("database")).unwrap();

        let db_path = profile_dir.join("database/coheara.db");
        let conn = crate::db::sqlite::open_database(&db_path, None).unwrap();
        conn.execute(
            "INSERT INTO documents (id, type, title, ingestion_date, source_file)
             VALUES ('doc-1', 'prescription', 'Test Doc', '2026-01-15', 'test.pdf')",
            [],
        )
        .unwrap();

        let info = get_privacy_info(&conn, &profile_dir).unwrap();
        assert_eq!(info.document_count, 1);
        assert!(info.total_data_size_bytes > 0);
        assert_eq!(info.encryption_algorithm, "AES-256-GCM");
        assert!(info.network_permissions.contains("offline"));
        assert!(info.telemetry.contains("None"));
    }

    // ─── Error Recovery Tests ───

    #[test]
    fn test_recovery_strategy_mapping() {
        assert!(matches!(
            recovery_for("Database error"),
            RecoveryStrategy::Retry
        ));
        assert!(matches!(
            recovery_for("Encryption error"),
            RecoveryStrategy::Fatal(_)
        ));
        assert!(matches!(
            recovery_for("Wrong password"),
            RecoveryStrategy::UserActionRequired(_)
        ));
        assert!(matches!(
            recovery_for("Request timeout"),
            RecoveryStrategy::RetryWithBackoff
        ));
        assert!(matches!(
            recovery_for("OCR extraction failed"),
            RecoveryStrategy::FallbackAvailable(_)
        ));
        assert!(matches!(
            recovery_for("Ollama not running"),
            RecoveryStrategy::FallbackAvailable(_)
        ));
    }

    // ─── Helper Tests ───

    #[test]
    fn test_convert_to_mg() {
        assert!((convert_to_mg(1.0, "g") - 1000.0).abs() < f64::EPSILON);
        assert!((convert_to_mg(1000.0, "mcg") - 1.0).abs() < f64::EPSILON);
        assert!((convert_to_mg(500.0, "mg") - 500.0).abs() < f64::EPSILON);
        assert!((convert_to_mg(100.0, "µg") - 0.1).abs() < f64::EPSILON);
        // Unknown unit defaults to value as-is (assumed mg)
        assert!((convert_to_mg(42.0, "tablets") - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_calculate_dir_size() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("file1.txt"), "hello").unwrap();
        std::fs::write(tmp.path().join("file2.txt"), "world!").unwrap();
        let size = fs_helpers::calculate_dir_size(tmp.path());
        assert_eq!(size, 11); // "hello" (5) + "world!" (6)
    }

    #[test]
    fn test_count_dir_contents() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.txt"), "aaa").unwrap();
        std::fs::create_dir(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("sub/b.txt"), "bbbbb").unwrap();
        let (count, bytes) = fs_helpers::count_dir_contents(tmp.path());
        assert_eq!(count, 2);
        assert_eq!(bytes, 8); // 3 + 5
    }

    #[test]
    fn test_calculate_dir_size_nonexistent() {
        let size = fs_helpers::calculate_dir_size(Path::new("/nonexistent/path"));
        assert_eq!(size, 0);
    }
}
