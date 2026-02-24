//! Pipeline diagnostic dump — writes intermediate artifacts to disk.
//!
//! Enables inspection of every pipeline stage: rendered pages, preprocessed images,
//! prompts, LLM responses, extraction results.
//!
//! **Activation**:
//! - Dev builds (`is_dev()`): auto-enabled, writes to `~/Coheara-dev/diagnostic/`
//! - Prod builds: disabled unless `COHEARA_DUMP_DIR` env var is set
//! - `COHEARA_DUMP_DIR` overrides the default in both modes
//!
//! **Output structure**:
//! ```text
//! {dump_dir}/{doc_id}/
//!   00-source-info.json
//!   01-rendered-page-0.png
//!   02-preprocessed-page-0.png
//!   02-preprocessed-page-0.json
//!   03-vision-ocr-prompt-page-0.txt
//!   03-vision-ocr-result-page-0.json
//!   04-extraction-result.json
//!   05-structuring-prompt-page-0.txt
//!   05-structuring-result-page-0.json
//!   06-final-result.json
//! ```

use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::config;

// ──────────────────────────────────────────────
// Dump directory resolution
// ──────────────────────────────────────────────

/// Diagnostic dump subdirectory name inside app data.
const DIAGNOSTIC_SUBDIR: &str = "diagnostic";

/// Resolve the base dump directory.
///
/// Priority:
/// 1. `COHEARA_DUMP_DIR` env var (explicit override, any build)
/// 2. `~/Coheara-dev/diagnostic/` in dev builds (auto-enabled)
/// 3. `None` in production (disabled by default)
fn resolve_base_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("COHEARA_DUMP_DIR") {
        return Some(PathBuf::from(dir));
    }

    if config::is_dev() {
        return Some(config::app_data_dir().join(DIAGNOSTIC_SUBDIR));
    }

    None
}

/// Returns the dump directory for a document, or `None` if diagnostics are disabled.
///
/// Creates the directory tree on first call. Returns `None` (with a warning) if
/// directory creation fails — never panics, never blocks the pipeline.
pub fn dump_dir_for(doc_id: &Uuid) -> Option<PathBuf> {
    let base = resolve_base_dir()?;
    let dir = base.join(doc_id.to_string());

    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::warn!(
            path = %dir.display(),
            error = %e,
            "Diagnostic dump: failed to create directory"
        );
        return None;
    }

    Some(dir)
}

// ──────────────────────────────────────────────
// Dump writers
// ──────────────────────────────────────────────

/// Write a binary artifact (PNG image, raw bytes).
///
/// Logs on success (debug) and failure (warn). Never panics.
pub fn dump_binary(dir: &Path, filename: &str, data: &[u8]) {
    let path = dir.join(filename);
    match std::fs::write(&path, data) {
        Ok(()) => tracing::debug!(
            path = %path.display(),
            size = data.len(),
            "Diagnostic dump: binary written"
        ),
        Err(e) => tracing::warn!(
            path = %path.display(),
            error = %e,
            "Diagnostic dump: failed to write binary"
        ),
    }
}

/// Write a JSON artifact (any serde-serializable value).
///
/// Uses pretty-printing for human readability. Never panics.
pub fn dump_json<T: serde::Serialize>(dir: &Path, filename: &str, value: &T) {
    let path = dir.join(filename);
    match serde_json::to_string_pretty(value) {
        Ok(json) => match std::fs::write(&path, json.as_bytes()) {
            Ok(()) => tracing::debug!(
                path = %path.display(),
                size = json.len(),
                "Diagnostic dump: JSON written"
            ),
            Err(e) => tracing::warn!(
                path = %path.display(),
                error = %e,
                "Diagnostic dump: failed to write JSON"
            ),
        },
        Err(e) => tracing::warn!(
            path = %path.display(),
            error = %e,
            "Diagnostic dump: failed to serialize JSON"
        ),
    }
}

/// Write a text artifact (prompt, raw LLM response).
///
/// Never panics.
pub fn dump_text(dir: &Path, filename: &str, text: &str) {
    let path = dir.join(filename);
    match std::fs::write(&path, text.as_bytes()) {
        Ok(()) => tracing::debug!(
            path = %path.display(),
            size = text.len(),
            "Diagnostic dump: text written"
        ),
        Err(e) => tracing::warn!(
            path = %path.display(),
            error = %e,
            "Diagnostic dump: failed to write text"
        ),
    }
}

// ──────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dump_dir_for_creates_directory() {
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("COHEARA_DUMP_DIR", tmp.path());

        let doc_id = Uuid::new_v4();
        let dir = dump_dir_for(&doc_id).unwrap();

        assert!(dir.exists());
        assert!(dir.ends_with(doc_id.to_string()));

        std::env::remove_var("COHEARA_DUMP_DIR");
    }

    #[test]
    fn dump_dir_for_returns_some_in_dev() {
        // Remove env var to test dev fallback
        std::env::remove_var("COHEARA_DUMP_DIR");

        // In test builds, is_dev() == true, so should return Some
        let doc_id = Uuid::new_v4();
        let dir = dump_dir_for(&doc_id);

        assert!(dir.is_some());
        let dir = dir.unwrap();
        assert!(dir.exists());
        assert!(dir.to_string_lossy().contains("diagnostic"));

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dump_binary_writes_file() {
        let tmp = tempfile::tempdir().unwrap();
        let data = b"PNG fake image data";

        dump_binary(tmp.path(), "test.png", data);

        let written = std::fs::read(tmp.path().join("test.png")).unwrap();
        assert_eq!(written, data);
    }

    #[test]
    fn dump_json_writes_pretty_json() {
        let tmp = tempfile::tempdir().unwrap();

        #[derive(serde::Serialize)]
        struct Info {
            name: String,
            value: u32,
        }

        let info = Info {
            name: "test".to_string(),
            value: 42,
        };

        dump_json(tmp.path(), "info.json", &info);

        let content = std::fs::read_to_string(tmp.path().join("info.json")).unwrap();
        assert!(content.contains("\"name\": \"test\""));
        assert!(content.contains("\"value\": 42"));
        // Pretty-printed: contains newlines
        assert!(content.contains('\n'));
    }

    #[test]
    fn dump_text_writes_text() {
        let tmp = tempfile::tempdir().unwrap();
        let prompt = "Extract all visible text from this document";

        dump_text(tmp.path(), "prompt.txt", prompt);

        let content = std::fs::read_to_string(tmp.path().join("prompt.txt")).unwrap();
        assert_eq!(content, prompt);
    }

    #[test]
    fn dump_binary_handles_write_failure_gracefully() {
        // Non-existent directory — write should fail but not panic
        let bad_dir = Path::new("/nonexistent/path/that/does/not/exist");
        dump_binary(bad_dir, "test.png", b"data");
        // No panic = success
    }

    #[test]
    fn dump_json_handles_write_failure_gracefully() {
        let bad_dir = Path::new("/nonexistent/path");
        dump_json(bad_dir, "test.json", &"data");
        // No panic = success
    }

    #[test]
    fn dump_text_handles_write_failure_gracefully() {
        let bad_dir = Path::new("/nonexistent/path");
        dump_text(bad_dir, "test.txt", "data");
        // No panic = success
    }

    #[test]
    fn env_var_overrides_dev_default() {
        let tmp = tempfile::tempdir().unwrap();
        let custom = tmp.path().join("custom-dump");
        std::env::set_var("COHEARA_DUMP_DIR", &custom);

        let doc_id = Uuid::new_v4();
        let dir = dump_dir_for(&doc_id).unwrap();

        assert!(dir.starts_with(&custom));

        std::env::remove_var("COHEARA_DUMP_DIR");
    }
}
