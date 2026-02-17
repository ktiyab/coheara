//! M1-05: Document upload endpoint — mobile camera → desktop pipeline.
//!
//! `POST /api/documents/upload` — receives photos from mobile, decodes base64,
//! stages as files, and runs L1-01 import pipeline.

use std::path::PathBuf;

use axum::extract::State;
use axum::Extension;
use axum::Json;
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::api::error::ApiError;
use crate::api::types::{ApiContext, DeviceContext};
use crate::db::sqlite::open_database;
use crate::pipeline::import::importer::{import_file, ImportStatus};
use crate::pipeline::import::staging::{write_encrypted_staging, decrypt_staging_to_temp};

/// Maximum pages per upload request.
const MAX_PAGES: usize = 10;
/// Maximum page size in bytes (4 MB).
const MAX_PAGE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize)]
pub struct UploadRequest {
    pub metadata: UploadMetadata,
    pub pages: Vec<UploadPage>,
}

#[derive(Deserialize)]
pub struct UploadMetadata {
    pub page_count: usize,
    pub device_name: String,
    pub captured_at: String,
}

#[derive(Deserialize)]
pub struct UploadPage {
    pub name: String,
    /// Base64 data URL (e.g., `data:image/jpeg;base64,/9j/...`)
    pub data: String,
    pub width: u32,
    pub height: u32,
    pub size_bytes: usize,
}

#[derive(Serialize)]
pub struct UploadResponse {
    pub document_id: String,
    pub status: &'static str,
    pub message: String,
}

/// `POST /api/documents/upload` — receive photos from mobile companion.
///
/// Decodes base64 data URLs, writes to staging directory, and runs L1-01
/// import pipeline on each page. Returns the first document ID.
pub async fn upload(
    State(ctx): State<ApiContext>,
    Extension(device): Extension<DeviceContext>,
    Json(payload): Json<UploadRequest>,
) -> Result<Json<UploadResponse>, ApiError> {
    // Validate page count
    if payload.pages.is_empty() {
        return Err(ApiError::BadRequest("No pages in upload".into()));
    }
    if payload.pages.len() > MAX_PAGES {
        return Err(ApiError::BadRequest(format!(
            "Maximum {} pages per upload",
            MAX_PAGES
        )));
    }

    // Acquire session — derive staging dir from db_path
    // Profile structure: profiles/{uuid}/database/profile.db
    // Staging dir: profiles/{uuid}/staging/mobile/
    let (db_path, staging_dir, db_key) = {
        let guard = ctx.core.read_session().map_err(ApiError::from)?;
        let session = guard.as_ref().ok_or(ApiError::NoActiveProfile)?;
        let profile_dir = session
            .db_path()
            .parent()
            .and_then(|db_dir| db_dir.parent())
            .ok_or_else(|| ApiError::Internal("Invalid profile path structure".into()))?;
        let staging: PathBuf = profile_dir.join("staging").join("mobile");
        (session.db_path().to_path_buf(), staging, *session.key_bytes())
    };

    // Create staging directory
    std::fs::create_dir_all(&staging_dir)
        .map_err(|e| ApiError::Internal(format!("Staging directory: {e}")))?;

    let conn =
        open_database(&db_path, Some(&db_key)).map_err(|e| ApiError::Internal(format!("Database: {e}")))?;

    // Decode and stage each page
    let mut staged_paths: Vec<PathBuf> = Vec::with_capacity(payload.pages.len());
    for page in &payload.pages {
        let image_bytes = decode_data_url(&page.data)
            .map_err(|e| ApiError::BadRequest(format!("Invalid image data: {e}")))?;

        if image_bytes.len() > MAX_PAGE_BYTES {
            // Clean up already-staged files on error
            cleanup_staged(&staged_paths);
            return Err(ApiError::BadRequest(format!(
                "Page '{}' exceeds 4 MB size limit ({} bytes)",
                page.name,
                image_bytes.len()
            )));
        }

        // Detect extension from magic bytes
        let ext = detect_extension(&image_bytes);
        let file_name = format!(
            "{}_{}.{}",
            chrono::Utc::now().timestamp_millis(),
            page.name,
            ext
        );
        let file_path = staging_dir.join(&file_name);

        // SEC-02-G03: Encrypt staging file so plaintext never hits disk
        write_encrypted_staging(&image_bytes, &file_path, &db_key)
            .map_err(|e| ApiError::Internal(format!("Failed to write staging file: {e}")))?;

        staged_paths.push(file_path);
    }

    // Import each staged file via L1-01
    let guard = ctx.core.read_session().map_err(ApiError::from)?;
    let session = guard.as_ref().ok_or(ApiError::NoActiveProfile)?;

    let mut document_ids: Vec<String> = Vec::new();
    for file_path in &staged_paths {
        // SEC-02-G03: Decrypt encrypted staging file to temp for import
        let temp_file = match decrypt_staging_to_temp(file_path, &db_key) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    device = %device.device_name,
                    "Mobile upload: staging decryption failed"
                );
                continue;
            }
        };
        match import_file(temp_file.path(), session, &conn) {
            Ok(result) => {
                if result.status == ImportStatus::Staged {
                    document_ids.push(result.document_id.to_string());
                }
                tracing::info!(
                    document_id = %result.document_id,
                    status = ?result.status,
                    device = %device.device_name,
                    "Mobile upload: page imported"
                );
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    device = %device.device_name,
                    "Mobile upload: import failed"
                );
            }
        }
        // temp_file dropped here → auto-deleted
    }

    // Log access for audit
    ctx.core.log_access(
        crate::core_state::AccessSource::MobileDevice {
            device_id: device.device_id.clone(),
        },
        "upload_document",
        &format!(
            "device:{} pages:{} imported:{}",
            device.device_id,
            payload.pages.len(),
            document_ids.len()
        ),
    );
    ctx.core.update_activity();

    let doc_id = document_ids.first().cloned().unwrap_or_default();
    let page_count = payload.pages.len();

    Ok(Json(UploadResponse {
        document_id: doc_id,
        status: "processing",
        message: format!(
            "{} page(s) received and queued for processing",
            page_count
        ),
    }))
}

/// Decode a base64 data URL to raw bytes.
///
/// Handles both `data:image/jpeg;base64,...` and raw base64 strings.
fn decode_data_url(data_url: &str) -> Result<Vec<u8>, String> {
    let base64_data = match data_url.find(',') {
        Some(idx) => &data_url[idx + 1..],
        None => data_url,
    };

    base64::engine::general_purpose::STANDARD
        .decode(base64_data)
        .map_err(|e| format!("Base64 decode failed: {e}"))
}

/// Detect file extension from magic bytes.
fn detect_extension(bytes: &[u8]) -> &'static str {
    if bytes.len() >= 3 && bytes[0..3] == [0xFF, 0xD8, 0xFF] {
        "jpg"
    } else if bytes.len() >= 8 && bytes[0..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
    {
        "png"
    } else if bytes.len() >= 4 && &bytes[0..4] == b"RIFF" && bytes.len() >= 12 && &bytes[8..12] == b"WEBP"
    {
        "webp"
    } else if bytes.len() >= 8 && &bytes[4..8] == b"ftyp" {
        "heic"
    } else if bytes.len() >= 5 && &bytes[0..5] == b"%PDF-" {
        "pdf"
    } else {
        "bin"
    }
}

/// Clean up staged files on error (SEC-02-G05: secure erasure).
fn cleanup_staged(paths: &[PathBuf]) {
    for path in paths {
        if let Err(e) = crate::crypto::secure_delete_file(path) {
            tracing::warn!("Failed to securely delete staging file: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_data_url_jpeg() {
        let data = "data:image/jpeg;base64,/9j/4AAQ";
        let bytes = decode_data_url(data).unwrap();
        assert!(!bytes.is_empty());
        assert_eq!(bytes[0], 0xFF); // JPEG magic byte
    }

    #[test]
    fn decode_data_url_raw_base64() {
        let raw = base64::engine::general_purpose::STANDARD.encode(b"hello");
        let bytes = decode_data_url(&raw).unwrap();
        assert_eq!(bytes, b"hello");
    }

    #[test]
    fn decode_data_url_invalid_base64() {
        let result = decode_data_url("not-valid-base64!!!");
        assert!(result.is_err());
    }

    #[test]
    fn detect_extension_jpeg() {
        assert_eq!(detect_extension(&[0xFF, 0xD8, 0xFF, 0xE0]), "jpg");
    }

    #[test]
    fn detect_extension_png() {
        assert_eq!(
            detect_extension(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]),
            "png"
        );
    }

    #[test]
    fn detect_extension_pdf() {
        assert_eq!(detect_extension(b"%PDF-1.4"), "pdf");
    }

    #[test]
    fn detect_extension_unknown() {
        assert_eq!(detect_extension(&[0x00, 0x01, 0x02]), "bin");
    }
}
