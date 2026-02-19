use std::path::{Path, PathBuf};

use uuid::Uuid;

use super::StorageError;
use crate::crypto::ProfileSession;

/// Save encrypted Markdown to the profile's markdown directory.
/// Returns the relative path from the profile root (e.g., "markdown/<doc_id>.md.enc").
pub fn save_encrypted_markdown(
    session: &ProfileSession,
    profiles_dir: &Path,
    document_id: &Uuid,
    markdown: &str,
) -> Result<String, StorageError> {
    let profile_dir = profiles_dir.join(session.profile_id.to_string());
    let markdown_dir = profile_dir.join("markdown");

    std::fs::create_dir_all(&markdown_dir)?;

    let filename = format!("{}.md.enc", document_id);
    let full_path = markdown_dir.join(&filename);

    let encrypted = session
        .encrypt(markdown.as_bytes())
        .map_err(StorageError::Crypto)?;

    std::fs::write(&full_path, encrypted.to_bytes())?;

    let relative_path = format!("markdown/{filename}");
    Ok(relative_path)
}

/// Read and decrypt a Markdown file from the profile's markdown directory.
pub fn read_encrypted_markdown(
    session: &ProfileSession,
    profiles_dir: &Path,
    relative_path: &str,
) -> Result<String, StorageError> {
    let profile_dir = profiles_dir.join(session.profile_id.to_string());
    let full_path = profile_dir.join(relative_path);

    let bytes = std::fs::read(&full_path)?;
    let encrypted = crate::crypto::EncryptedData::from_bytes(&bytes)
        .map_err(StorageError::Crypto)?;

    let plaintext = session.decrypt(&encrypted).map_err(StorageError::Crypto)?;

    String::from_utf8(plaintext)
        .map_err(|e| StorageError::VectorDb(format!("Invalid UTF-8 in markdown: {e}")))
}

/// Get the full filesystem path for a markdown file.
pub fn markdown_full_path(
    profiles_dir: &Path,
    profile_id: &Uuid,
    relative_path: &str,
) -> PathBuf {
    profiles_dir
        .join(profile_id.to_string())
        .join(relative_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::profile;

    fn test_session() -> (tempfile::TempDir, ProfileSession) {
        let dir = tempfile::tempdir().unwrap();
        let (info, _phrase) =
            profile::create_profile(dir.path(), "TestUser", "test_password_123", None, None).unwrap();
        let session = profile::open_profile(dir.path(), &info.id, "test_password_123").unwrap();
        (dir, session)
    }

    #[test]
    fn encrypt_and_decrypt_markdown() {
        let (dir, session) = test_session();
        let doc_id = Uuid::new_v4();
        let markdown = "## Medications\n\nMetformin 500mg twice daily\n\n## Lab Results\n\nHbA1c: 7.2%";

        let relative_path =
            save_encrypted_markdown(&session, dir.path(), &doc_id, markdown).unwrap();

        assert!(relative_path.starts_with("markdown/"));
        assert!(relative_path.ends_with(".md.enc"));

        let decrypted = read_encrypted_markdown(&session, dir.path(), &relative_path).unwrap();
        assert_eq!(decrypted, markdown);
    }

    #[test]
    fn encrypted_file_differs_from_plaintext() {
        let (dir, session) = test_session();
        let doc_id = Uuid::new_v4();
        let markdown = "Sensitive medical data here";

        let relative_path =
            save_encrypted_markdown(&session, dir.path(), &doc_id, markdown).unwrap();

        let full_path = markdown_full_path(dir.path(), &session.profile_id, &relative_path);
        let raw_bytes = std::fs::read(&full_path).unwrap();

        assert_ne!(raw_bytes, markdown.as_bytes());
    }

    #[test]
    fn overwrite_existing_markdown() {
        let (dir, session) = test_session();
        let doc_id = Uuid::new_v4();

        save_encrypted_markdown(&session, dir.path(), &doc_id, "version 1").unwrap();
        let path =
            save_encrypted_markdown(&session, dir.path(), &doc_id, "version 2").unwrap();

        let decrypted = read_encrypted_markdown(&session, dir.path(), &path).unwrap();
        assert_eq!(decrypted, "version 2");
    }

    #[test]
    fn empty_markdown_round_trip() {
        let (dir, session) = test_session();
        let doc_id = Uuid::new_v4();

        let path = save_encrypted_markdown(&session, dir.path(), &doc_id, "").unwrap();
        let decrypted = read_encrypted_markdown(&session, dir.path(), &path).unwrap();
        assert_eq!(decrypted, "");
    }

    #[test]
    fn read_nonexistent_file_errors() {
        let (dir, session) = test_session();
        let result = read_encrypted_markdown(&session, dir.path(), "markdown/nonexistent.md.enc");
        assert!(result.is_err());
    }

    #[test]
    fn markdown_full_path_builds_correctly() {
        let profile_id = Uuid::new_v4();
        let profiles_dir = Path::new("/data/profiles");
        let relative = "markdown/abc.md.enc";

        let full = markdown_full_path(profiles_dir, &profile_id, relative);
        assert!(full.to_str().unwrap().contains("markdown/abc.md.enc"));
        assert!(full.to_str().unwrap().contains(&profile_id.to_string()));
    }
}
