use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::crypto::ProfileSession;
use super::ImportError;

/// Copy source file to profile's originals/ directory, encrypted with profile key.
/// Returns the path of the encrypted staged file.
pub fn stage_file(
    source_path: &Path,
    document_id: &Uuid,
    session: &ProfileSession,
) -> Result<PathBuf, ImportError> {
    let extension = source_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");

    // Target: profiles/<uuid>/originals/<doc_uuid>.<ext>.enc
    let target_dir = session
        .db_path()
        .parent() // database/
        .and_then(|p| p.parent()) // profile dir
        .map(|p| p.join("originals"))
        .ok_or_else(|| ImportError::FileReadError("Invalid profile path".into()))?;

    std::fs::create_dir_all(&target_dir)?;

    let target_path = target_dir.join(format!("{}.{}.enc", document_id, extension));

    // Read source, encrypt, write to target
    let plaintext = std::fs::read(source_path)?;
    let encrypted = session.encrypt(&plaintext)?;
    std::fs::write(&target_path, encrypted.to_bytes())?;

    tracing::debug!(
        document_id = %document_id,
        size = plaintext.len(),
        "File staged and encrypted"
    );

    Ok(target_path)
}

/// Remove a staged file (if import is cancelled)
pub fn remove_staged(
    document_id: &Uuid,
    session: &ProfileSession,
) -> Result<(), ImportError> {
    let originals_dir = session
        .db_path()
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("originals"))
        .ok_or_else(|| ImportError::FileReadError("Invalid profile path".into()))?;

    // Find and remove files matching this document ID
    if originals_dir.exists() {
        let prefix = document_id.to_string();
        for entry in std::fs::read_dir(&originals_dir)? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().starts_with(&prefix) {
                std::fs::remove_file(entry.path())?;
            }
        }
    }
    Ok(())
}

/// Decrypt a staged file and return its original content
pub fn read_staged_file(
    staged_path: &Path,
    session: &ProfileSession,
) -> Result<Vec<u8>, ImportError> {
    let bytes = std::fs::read(staged_path)?;
    let encrypted = crate::crypto::EncryptedData::from_bytes(&bytes)?;
    let plaintext = session.decrypt(&encrypted)?;
    Ok(plaintext)
}

// ---------------------------------------------------------------------------
// SEC-02-G03/G04: Pre-import staging encryption
// ---------------------------------------------------------------------------

/// Encrypt bytes and write to a staging file. Used for mobile and WiFi staging
/// so plaintext medical data never hits disk in the staging directory.
pub fn write_encrypted_staging(
    plaintext: &[u8],
    path: &Path,
    key: &[u8; 32],
) -> Result<(), ImportError> {
    let encrypted = crate::crypto::EncryptedData::encrypt(key, plaintext)?;
    std::fs::write(path, encrypted.to_bytes())?;
    Ok(())
}

/// Decrypt a pre-import encrypted staging file to a temp file for import.
/// Returns a NamedTempFile that will be auto-deleted when dropped.
pub fn decrypt_staging_to_temp(
    encrypted_path: &Path,
    key: &[u8; 32],
) -> Result<tempfile::NamedTempFile, ImportError> {
    let bytes = std::fs::read(encrypted_path)?;
    let encrypted = crate::crypto::EncryptedData::from_bytes(&bytes)?;
    let plaintext = encrypted.decrypt(key)?;

    // Preserve original extension (minus any .enc suffix)
    let stem = encrypted_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("staged");
    let ext = encrypted_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");

    let suffix = if ext == "enc" {
        // File named like "uuid_photo.jpg.enc" → use the stem's extension
        Path::new(stem)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{e}"))
            .unwrap_or_else(|| ".bin".to_string())
    } else {
        format!(".{ext}")
    };

    let mut temp = tempfile::Builder::new()
        .suffix(&suffix)
        .tempfile()
        .map_err(|e| ImportError::FileReadError(format!("Temp file creation failed: {e}")))?;

    use std::io::Write;
    temp.write_all(&plaintext)
        .map_err(|e| ImportError::FileReadError(format!("Temp file write failed: {e}")))?;
    temp.flush()?;

    Ok(temp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::profile;

    fn setup_profile() -> (tempfile::TempDir, ProfileSession) {
        let dir = tempfile::tempdir().unwrap();
        let (info, _phrase) =
            profile::create_profile(dir.path(), "TestUser", "test_password_123", None, None).unwrap();
        let session = profile::open_profile(dir.path(), &info.id, "test_password_123").unwrap();
        (dir, session)
    }

    #[test]
    fn stage_file_encrypts_content() {
        let (_dir, session) = setup_profile();
        let source_dir = tempfile::tempdir().unwrap();
        let source_path = source_dir.path().join("prescription.jpg");
        let original_content = b"JPEG image content for testing";
        std::fs::write(&source_path, original_content).unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&source_path, &doc_id, &session).unwrap();

        assert!(staged_path.exists());
        assert!(staged_path.to_string_lossy().ends_with(".jpg.enc"));

        // Staged content should differ from original (it's encrypted)
        let staged_content = std::fs::read(&staged_path).unwrap();
        assert_ne!(staged_content, original_content);
    }

    #[test]
    fn staged_file_decrypts_to_original() {
        let (_dir, session) = setup_profile();
        let source_dir = tempfile::tempdir().unwrap();
        let source_path = source_dir.path().join("report.pdf");
        let original_content = b"PDF file content for round-trip test";
        std::fs::write(&source_path, original_content).unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&source_path, &doc_id, &session).unwrap();

        let decrypted = read_staged_file(&staged_path, &session).unwrap();
        assert_eq!(decrypted, original_content);
    }

    #[test]
    fn remove_staged_deletes_file() {
        let (_dir, session) = setup_profile();
        let source_dir = tempfile::tempdir().unwrap();
        let source_path = source_dir.path().join("test.txt");
        std::fs::write(&source_path, "content").unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&source_path, &doc_id, &session).unwrap();
        assert!(staged_path.exists());

        remove_staged(&doc_id, &session).unwrap();
        assert!(!staged_path.exists());
    }

    #[test]
    fn encrypted_staging_roundtrip() {
        let (_dir, session) = setup_profile();
        let staging_dir = tempfile::tempdir().unwrap();
        let path = staging_dir.path().join("photo.jpg");
        let content = b"JPEG medical image data";
        let key = session.key_bytes();

        write_encrypted_staging(content, &path, key).unwrap();

        // File on disk should NOT be plaintext
        let on_disk = std::fs::read(&path).unwrap();
        assert_ne!(on_disk, content);

        // Decrypt to temp and verify roundtrip
        let temp = decrypt_staging_to_temp(&path, key).unwrap();
        let recovered = std::fs::read(temp.path()).unwrap();
        assert_eq!(recovered, content);
    }

    #[test]
    fn encrypted_staging_wrong_key_fails() {
        let (_dir, session) = setup_profile();
        let staging_dir = tempfile::tempdir().unwrap();
        let path = staging_dir.path().join("secret.pdf");
        let key = session.key_bytes();

        write_encrypted_staging(b"sensitive data", &path, key).unwrap();

        let wrong_key = [0u8; 32];
        let result = decrypt_staging_to_temp(&path, &wrong_key);
        assert!(result.is_err());
    }
}
