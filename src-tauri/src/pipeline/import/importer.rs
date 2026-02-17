use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::crypto::ProfileSession;
use crate::db::repository;
use crate::models::document::Document;
use crate::models::enums::{DocumentType, PipelineStatus};
use super::format::{detect_format, sanitize_filename, FileCategory, FormatDetection};
use super::hash::{compute_hash, hash_similarity};
use super::staging::stage_file;
use super::ImportError;

/// Import result returned to the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub document_id: Uuid,
    pub original_filename: String,
    pub format: FormatDetection,
    pub staged_path: String,
    pub duplicate_of: Option<Uuid>,
    pub status: ImportStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ImportStatus {
    Staged,
    Duplicate,
    Unsupported,
    TooLarge,
    CorruptedFile,
}

/// Duplicate detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateCheck {
    pub is_duplicate: bool,
    pub existing_document_id: Option<Uuid>,
    pub similarity_score: f64,
    pub hash: String,
}

const DUPLICATE_SIMILARITY_THRESHOLD: f64 = 0.85;

/// Import a single file into a profile
pub fn import_file(
    source_path: &Path,
    session: &ProfileSession,
    conn: &Connection,
) -> Result<ImportResult, ImportError> {
    let original_filename = sanitize_filename(
        source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown"),
    );

    tracing::info!(file = %original_filename, "Starting document import");

    // Step 1: Format detection
    let format = detect_format(source_path)?;

    if !format.category.is_supported() {
        return Ok(ImportResult {
            document_id: Uuid::new_v4(),
            original_filename,
            format,
            staged_path: String::new(),
            duplicate_of: None,
            status: ImportStatus::Unsupported,
        });
    }

    // Step 2: Size check (100MB)
    if format.file_size_bytes > 100 * 1024 * 1024 {
        return Ok(ImportResult {
            document_id: Uuid::new_v4(),
            original_filename,
            format,
            staged_path: String::new(),
            duplicate_of: None,
            status: ImportStatus::TooLarge,
        });
    }

    // Step 3: Compute hash for duplicate detection
    let hash = compute_hash(source_path, &format.category)?;

    // Step 4: Check for duplicates
    let dup_check = check_duplicate_in_db(&hash, &format.category, conn)?;

    let document_id = Uuid::new_v4();

    if dup_check.is_duplicate {
        tracing::info!(
            file = %original_filename,
            duplicate_of = ?dup_check.existing_document_id,
            similarity = dup_check.similarity_score,
            "Duplicate document detected"
        );

        return Ok(ImportResult {
            document_id,
            original_filename,
            format,
            staged_path: String::new(),
            duplicate_of: dup_check.existing_document_id,
            status: ImportStatus::Duplicate,
        });
    }

    // Step 5: Stage file (copy to profile directory, encrypted)
    let staged_path = stage_file(source_path, &document_id, session)?;

    // Step 6: Create document record in SQLite
    let doc = Document {
        id: document_id,
        doc_type: DocumentType::Other, // Will be classified by L1-03
        title: original_filename.clone(),
        document_date: None,
        ingestion_date: chrono::Local::now().naive_local(),
        professional_id: None,
        source_file: staged_path.to_string_lossy().to_string(),
        markdown_file: None,
        ocr_confidence: None,
        verified: false,
        source_deleted: false,
        perceptual_hash: Some(hash),
        notes: None,
        pipeline_status: PipelineStatus::Imported,
    };
    repository::insert_document(conn, &doc)?;

    tracing::info!(
        document_id = %document_id,
        file = %original_filename,
        category = format.category.as_str(),
        "Document imported and staged"
    );

    Ok(ImportResult {
        document_id,
        original_filename,
        format,
        staged_path: staged_path.to_string_lossy().to_string(),
        duplicate_of: None,
        status: ImportStatus::Staged,
    })
}

/// Import multiple files (batch)
pub fn import_files(
    source_paths: &[PathBuf],
    session: &ProfileSession,
    conn: &Connection,
) -> Result<Vec<ImportResult>, ImportError> {
    let mut results = Vec::with_capacity(source_paths.len());
    for path in source_paths {
        match import_file(path, session, conn) {
            Ok(result) => results.push(result),
            Err(e) => {
                tracing::warn!(file = %path.display(), error = %e, "Failed to import file");
                results.push(ImportResult {
                    document_id: Uuid::new_v4(),
                    original_filename: sanitize_filename(
                        path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown"),
                    ),
                    format: FormatDetection {
                        mime_type: "unknown".into(),
                        category: FileCategory::Unsupported,
                        is_digital_pdf: None,
                        file_size_bytes: 0,
                    },
                    staged_path: String::new(),
                    duplicate_of: None,
                    status: ImportStatus::CorruptedFile,
                });
            }
        }
    }
    Ok(results)
}

/// Check if a hash matches any existing document in the database
fn check_duplicate_in_db(
    hash: &str,
    category: &FileCategory,
    conn: &Connection,
) -> Result<DuplicateCheck, ImportError> {
    // Exact hash match (fast path)
    if let Some(existing) = repository::get_document_by_hash(conn, hash)? {
        return Ok(DuplicateCheck {
            is_duplicate: true,
            existing_document_id: Some(existing.id),
            similarity_score: 1.0,
            hash: hash.to_string(),
        });
    }

    // For images: fuzzy perceptual hash comparison (Hamming distance)
    if matches!(category, FileCategory::Image | FileCategory::ScannedPdf) {
        if let Some((existing_id, score)) = find_similar_hash(hash, conn)? {
            if score > DUPLICATE_SIMILARITY_THRESHOLD {
                return Ok(DuplicateCheck {
                    is_duplicate: true,
                    existing_document_id: Some(existing_id),
                    similarity_score: score,
                    hash: hash.to_string(),
                });
            }
        }
    }

    Ok(DuplicateCheck {
        is_duplicate: false,
        existing_document_id: None,
        similarity_score: 0.0,
        hash: hash.to_string(),
    })
}

/// Find the most similar perceptual hash among existing documents
fn find_similar_hash(
    new_hash: &str,
    conn: &Connection,
) -> Result<Option<(Uuid, f64)>, ImportError> {
    // Query all documents with hashes
    let mut stmt = conn
        .prepare("SELECT id, perceptual_hash FROM documents WHERE perceptual_hash IS NOT NULL")
        .map_err(crate::db::DatabaseError::from)?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
            ))
        })
        .map_err(crate::db::DatabaseError::from)?;

    let mut best_match: Option<(Uuid, f64)> = None;

    for row in rows {
        let (id_str, existing_hash) = row.map_err(crate::db::DatabaseError::from)?;
        if let Some(similarity) = hash_similarity(new_hash, &existing_hash) {
            let dominated = match &best_match {
                Some((_, best_score)) => similarity > *best_score,
                None => true,
            };
            if dominated {
                if let Ok(id) = uuid::Uuid::parse_str(&id_str) {
                    best_match = Some((id, similarity));
                }
            }
        }
    }

    Ok(best_match)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::profile;
    use crate::db::sqlite::open_memory_database;

    fn setup() -> (tempfile::TempDir, ProfileSession, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let (info, _phrase) =
            profile::create_profile(dir.path(), "ImportTest", "test_pass_123", None).unwrap();
        let session = profile::open_profile(dir.path(), &info.id, "test_pass_123").unwrap();
        let conn = open_memory_database().unwrap();
        (dir, session, conn)
    }

    #[test]
    fn import_text_file_staged() {
        let (_dir, session, conn) = setup();
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("report.txt");
        std::fs::write(&path, "Medical report: Patient is stable.").unwrap();

        let result = import_file(&path, &session, &conn).unwrap();
        assert_eq!(result.status, ImportStatus::Staged);
        assert_eq!(result.original_filename, "report.txt");
        assert_eq!(result.format.category, FileCategory::PlainText);
        assert!(result.duplicate_of.is_none());
    }

    #[test]
    fn import_unsupported_format() {
        let (_dir, session, conn) = setup();
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("binary.exe");
        std::fs::write(&path, &[0x4D, 0x5A, 0x90, 0x00, 0x03, 0x00]).unwrap();

        let result = import_file(&path, &session, &conn).unwrap();
        assert_eq!(result.status, ImportStatus::Unsupported);
    }

    #[test]
    fn import_creates_document_record() {
        let (_dir, session, conn) = setup();
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("lab_result.txt");
        std::fs::write(&path, "Potassium: 4.2 mmol/L (normal range: 3.5-5.0)").unwrap();

        let result = import_file(&path, &session, &conn).unwrap();
        assert_eq!(result.status, ImportStatus::Staged);

        // Verify document exists in DB
        let doc = repository::get_document(&conn, &result.document_id)
            .unwrap()
            .unwrap();
        assert_eq!(doc.title, "lab_result.txt");
        assert!(doc.perceptual_hash.is_some());
    }

    #[test]
    fn import_same_file_twice_detects_duplicate() {
        let (_dir, session, conn) = setup();
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("prescription.txt");
        std::fs::write(&path, "Metformin 500mg twice daily").unwrap();

        let result1 = import_file(&path, &session, &conn).unwrap();
        assert_eq!(result1.status, ImportStatus::Staged);

        let result2 = import_file(&path, &session, &conn).unwrap();
        assert_eq!(result2.status, ImportStatus::Duplicate);
        assert_eq!(result2.duplicate_of, Some(result1.document_id));
    }

    #[test]
    fn import_batch_processes_all_files() {
        let (_dir, session, conn) = setup();
        let source_dir = tempfile::tempdir().unwrap();

        let paths: Vec<PathBuf> = (0..3)
            .map(|i| {
                let path = source_dir.path().join(format!("doc_{i}.txt"));
                std::fs::write(&path, format!("Document content {i}")).unwrap();
                path
            })
            .collect();

        let results = import_files(&paths, &session, &conn).unwrap();
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.status == ImportStatus::Staged));
    }

    #[test]
    fn import_batch_handles_mixed_results() {
        let (_dir, session, conn) = setup();
        let source_dir = tempfile::tempdir().unwrap();

        let good_path = source_dir.path().join("valid.txt");
        std::fs::write(&good_path, "Valid medical content").unwrap();

        let bad_path = source_dir.path().join("binary.exe");
        std::fs::write(&bad_path, &[0x4D, 0x5A, 0x90, 0x00]).unwrap();

        let results = import_files(&[good_path, bad_path], &session, &conn).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].status, ImportStatus::Staged);
        assert_eq!(results[1].status, ImportStatus::Unsupported);
    }

    #[test]
    fn staged_file_is_encrypted() {
        let (_dir, session, conn) = setup();
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("secret.txt");
        let content = "Confidential patient information";
        std::fs::write(&path, content).unwrap();

        let result = import_file(&path, &session, &conn).unwrap();
        assert_eq!(result.status, ImportStatus::Staged);

        // Read the staged file — it should NOT be the plaintext
        let staged_bytes = std::fs::read(&result.staged_path).unwrap();
        assert_ne!(staged_bytes, content.as_bytes());
    }

    #[test]
    fn staged_file_decrypts_to_original() {
        let (_dir, session, conn) = setup();
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("roundtrip.txt");
        let content = b"Full round-trip encryption test content";
        std::fs::write(&path, content).unwrap();

        let result = import_file(&path, &session, &conn).unwrap();
        assert_eq!(result.status, ImportStatus::Staged);

        let decrypted =
            super::super::staging::read_staged_file(Path::new(&result.staged_path), &session)
                .unwrap();
        assert_eq!(decrypted, content);
    }

    #[test]
    fn import_image_creates_perceptual_hash() {
        let (_dir, session, conn) = setup();
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("scan.jpg");

        // Create a valid JPEG image (using image v0.23 via img_hash re-export)
        let img = img_hash::image::RgbImage::from_pixel(
            32,
            32,
            img_hash::image::Rgb([100u8, 150, 200]),
        );
        img.save(&path).unwrap();

        let result = import_file(&path, &session, &conn).unwrap();
        assert_eq!(result.status, ImportStatus::Staged);
        assert_eq!(result.format.category, FileCategory::Image);

        // Verify hash stored in DB
        let doc = repository::get_document(&conn, &result.document_id)
            .unwrap()
            .unwrap();
        assert!(doc.perceptual_hash.is_some());
    }
}
