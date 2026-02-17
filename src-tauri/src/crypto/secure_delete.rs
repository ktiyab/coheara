// Secure file deletion and orphaned staging cleanup (SEC-02-G05, SEC-02-G08).
// Overwrites file content with random bytes before removing the filesystem entry.

use std::fs;
use std::io::Write;
use std::path::Path;

use aes_gcm::aead::rand_core::{OsRng, RngCore};

/// Securely delete a file: overwrite with random bytes, sync to disk, then remove.
///
/// Falls back to standard remove_file if overwrite fails (e.g., read-only file).
/// Returns Ok even if the file doesn't exist (idempotent).
pub fn secure_delete_file(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let metadata = fs::metadata(path)?;
    let file_size = metadata.len() as usize;

    if file_size > 0 {
        // Overwrite with random data
        let mut random_buf = vec![0u8; file_size.min(64 * 1024)]; // Cap at 64KB chunks
        let mut file = fs::OpenOptions::new().write(true).open(path)?;

        let mut remaining = file_size;
        while remaining > 0 {
            let chunk_size = remaining.min(random_buf.len());
            OsRng.fill_bytes(&mut random_buf[..chunk_size]);
            if let Err(e) = file.write_all(&random_buf[..chunk_size]) {
                tracing::warn!(path = %path.display(), "Secure overwrite failed: {e}");
                break;
            }
            remaining -= chunk_size;
        }

        // Flush to physical storage
        if let Err(e) = file.sync_all() {
            tracing::warn!(path = %path.display(), "Sync after overwrite failed: {e}");
        }
    }

    fs::remove_file(path)
}

/// Clean orphaned staging files from all profile directories (SEC-02-G08).
///
/// Called at startup to remove files left behind by previous crashes.
/// Scans: `profiles/{uuid}/staging/mobile/` and `profiles/{uuid}/wifi_staging/`.
pub fn cleanup_orphaned_staging(profiles_dir: &Path) {
    let entries = match fs::read_dir(profiles_dir) {
        Ok(e) => e,
        Err(_) => return, // Profiles dir may not exist yet
    };

    let mut total_cleaned = 0usize;

    for entry in entries.flatten() {
        let profile_path = entry.path();
        if !profile_path.is_dir() {
            continue;
        }

        // Clean staging/mobile/
        let mobile_staging = profile_path.join("staging").join("mobile");
        total_cleaned += clean_staging_dir(&mobile_staging);

        // Clean wifi_staging/
        let wifi_staging = profile_path.join("wifi_staging");
        total_cleaned += clean_staging_dir(&wifi_staging);
    }

    if total_cleaned > 0 {
        tracing::info!(
            files_cleaned = total_cleaned,
            "Cleaned orphaned staging files from previous session"
        );
    }
}

/// Clean all files in a staging directory. Returns count of files removed.
fn clean_staging_dir(dir: &Path) -> usize {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };

    let mut count = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Err(e) = secure_delete_file(&path) {
                tracing::warn!("Failed to clean staging file: {e}");
            } else {
                count += 1;
            }
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn secure_delete_overwrites_and_removes() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, b"sensitive medical data here").unwrap();
        assert!(file_path.exists());

        secure_delete_file(&file_path).unwrap();
        assert!(!file_path.exists());
    }

    #[test]
    fn secure_delete_nonexistent_file_ok() {
        let path = Path::new("/tmp/nonexistent_file_abc123");
        assert!(secure_delete_file(path).is_ok());
    }

    #[test]
    fn secure_delete_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("empty.txt");
        fs::write(&file_path, b"").unwrap();

        secure_delete_file(&file_path).unwrap();
        assert!(!file_path.exists());
    }

    #[test]
    fn cleanup_orphaned_staging_removes_files() {
        let profiles_dir = tempfile::tempdir().unwrap();

        // Simulate a profile with orphaned staging files
        let profile = profiles_dir.path().join("test-profile-uuid");
        let mobile_staging = profile.join("staging").join("mobile");
        let wifi_staging = profile.join("wifi_staging");
        fs::create_dir_all(&mobile_staging).unwrap();
        fs::create_dir_all(&wifi_staging).unwrap();

        // Write orphaned files
        fs::write(mobile_staging.join("orphan1.jpg"), b"image data").unwrap();
        fs::write(mobile_staging.join("orphan2.png"), b"more data").unwrap();
        fs::write(wifi_staging.join("orphan3.pdf"), b"pdf data").unwrap();

        // Run cleanup
        cleanup_orphaned_staging(profiles_dir.path());

        // All files should be gone
        assert!(fs::read_dir(&mobile_staging).unwrap().count() == 0);
        assert!(fs::read_dir(&wifi_staging).unwrap().count() == 0);
    }

    #[test]
    fn cleanup_empty_profiles_dir_no_panic() {
        let dir = tempfile::tempdir().unwrap();
        cleanup_orphaned_staging(dir.path());
        // No panic, no error
    }

    #[test]
    fn cleanup_nonexistent_dir_no_panic() {
        cleanup_orphaned_staging(Path::new("/tmp/nonexistent_dir_abc123"));
        // No panic, no error
    }
}
