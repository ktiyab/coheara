//! E2E-B01: Direct file import IPC commands.
//!
//! Exposes the L1-01 document import pipeline directly to the desktop UI.
//! Users can import files via file picker without going through WiFi transfer.

use std::path::Path;
use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};

use crate::core_state::CoreState;
use crate::db::sqlite::open_database;
use crate::pipeline::import::importer::{import_file, ImportResult, ImportStatus};

/// Import a document from a local file path.
///
/// Validates the path, runs L1-01 format detection + staging + dedup,
/// and returns the import result for the frontend to handle.
#[tauri::command]
pub async fn import_document(
    state: State<'_, Arc<CoreState>>,
    app: AppHandle,
    file_path: String,
) -> Result<ImportResult, String> {
    let path = Path::new(&file_path);

    // Security: verify file exists and is a regular file
    if !path.exists() {
        return Err(format!("File not found: {}", file_path));
    }
    if !path.is_file() {
        return Err("Path is not a regular file".into());
    }

    // Acquire session and DB connection
    let (db_path,) = {
        let guard = state.read_session().map_err(|e| e.to_string())?;
        let session = guard
            .as_ref()
            .ok_or("No active profile. Unlock a profile first.")?;
        (session.db_path().to_path_buf(),)
    };

    // Emit progress event: started
    let _ = app.emit("import-progress", ImportProgressEvent {
        stage: "started".into(),
        file_name: path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string(),
        document_id: None,
        error: None,
    });

    // Re-acquire session for import_file (needs ProfileSession reference)
    let guard = state.read_session().map_err(|e| e.to_string())?;
    let session = guard
        .as_ref()
        .ok_or("No active profile. Unlock a profile first.")?;

    let conn = open_database(&db_path)
        .map_err(|e| format!("Database error: {e}"))?;

    let result = import_file(path, session, &conn)
        .map_err(|e| format!("Import failed: {e}"))?;

    // Emit progress event: result
    let stage = match &result.status {
        ImportStatus::Staged => "staged",
        ImportStatus::Duplicate => "duplicate",
        ImportStatus::Unsupported => "unsupported",
        ImportStatus::TooLarge => "too_large",
        ImportStatus::CorruptedFile => "corrupted",
    };
    let _ = app.emit("import-progress", ImportProgressEvent {
        stage: stage.into(),
        file_name: result.original_filename.clone(),
        document_id: Some(result.document_id.to_string()),
        error: None,
    });

    // Log access for audit
    state.log_access(
        crate::core_state::AccessSource::DesktopUi,
        "import_document",
        &format!("document:{}", result.document_id),
    );

    state.update_activity();

    tracing::info!(
        document_id = %result.document_id,
        status = ?result.status,
        file = %result.original_filename,
        "Document imported via file picker"
    );

    Ok(result)
}

/// Import multiple documents from local file paths (batch import).
///
/// Processes each file independently, collecting results.
/// Does not stop on individual file failures.
#[tauri::command]
pub async fn import_documents_batch(
    state: State<'_, Arc<CoreState>>,
    app: AppHandle,
    file_paths: Vec<String>,
) -> Result<Vec<ImportResult>, String> {
    if file_paths.is_empty() {
        return Ok(vec![]);
    }

    if file_paths.len() > 50 {
        return Err("Maximum 50 files per batch".into());
    }

    // Acquire session and DB
    let db_path = {
        let guard = state.read_session().map_err(|e| e.to_string())?;
        let session = guard
            .as_ref()
            .ok_or("No active profile. Unlock a profile first.")?;
        session.db_path().to_path_buf()
    };

    let guard = state.read_session().map_err(|e| e.to_string())?;
    let session = guard
        .as_ref()
        .ok_or("No active profile. Unlock a profile first.")?;

    let conn = open_database(&db_path)
        .map_err(|e| format!("Database error: {e}"))?;

    let total = file_paths.len();
    let mut results = Vec::with_capacity(total);

    for (i, file_path) in file_paths.iter().enumerate() {
        let path = Path::new(file_path);

        // Emit batch progress
        let _ = app.emit("import-batch-progress", ImportBatchProgressEvent {
            current: i + 1,
            total,
            file_name: path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string(),
        });

        if !path.exists() || !path.is_file() {
            tracing::warn!(path = %file_path, "Skipping non-existent or non-file path");
            continue;
        }

        match import_file(path, session, &conn) {
            Ok(result) => {
                tracing::info!(
                    document_id = %result.document_id,
                    status = ?result.status,
                    file = %result.original_filename,
                    "Batch import: file processed"
                );
                results.push(result);
            }
            Err(e) => {
                tracing::warn!(
                    path = %file_path,
                    error = %e,
                    "Batch import: file failed"
                );
            }
        }
    }

    state.log_access(
        crate::core_state::AccessSource::DesktopUi,
        "import_documents_batch",
        &format!("batch:{} files", results.len()),
    );

    state.update_activity();
    Ok(results)
}

// ---------------------------------------------------------------------------
// E2E-B02: Document Processing Commands
// ---------------------------------------------------------------------------

/// Process a document end-to-end: import → extract → structure → save pending review.
///
/// This is the primary command for the file picker flow. The user selects a file,
/// the system imports it, extracts text, structures with the LLM, and saves the
/// result for the patient to review before committing to storage.
#[tauri::command]
pub async fn process_document(
    state: State<'_, Arc<CoreState>>,
    app: AppHandle,
    file_path: String,
) -> Result<crate::pipeline::processor::ProcessingOutcome, String> {
    let path = Path::new(&file_path);

    if !path.exists() {
        return Err(format!("File not found: {}", file_path));
    }
    if !path.is_file() {
        return Err("Path is not a regular file".into());
    }

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Emit: processing started
    let _ = app.emit(
        "processing-progress",
        ProcessingProgressEvent {
            stage: "importing".into(),
            file_name: file_name.clone(),
            document_id: None,
            progress_pct: Some(10),
            error: None,
        },
    );

    // Build the document processor
    let processor = crate::pipeline::processor::build_processor()
        .map_err(|e| format!("Failed to initialize processor: {e}"))?;

    // Acquire session + DB
    let guard = state.read_session().map_err(|e| e.to_string())?;
    let session = guard
        .as_ref()
        .ok_or("No active profile. Unlock a profile first.")?;
    let conn = open_database(session.db_path()).map_err(|e| format!("Database error: {e}"))?;

    // Emit: extraction starting
    let _ = app.emit(
        "processing-progress",
        ProcessingProgressEvent {
            stage: "extracting".into(),
            file_name: file_name.clone(),
            document_id: None,
            progress_pct: Some(30),
            error: None,
        },
    );

    // Run the full pipeline (import → extract → structure)
    let output = match processor.process_file(path, session, &conn) {
        Ok(output) => output,
        Err(e) => {
            // E2E-B05: Emit failure event before returning error
            let _ = app.emit(
                "processing-progress",
                ProcessingProgressEvent {
                    stage: "failed".into(),
                    file_name: file_name.clone(),
                    document_id: None,
                    progress_pct: None,
                    error: Some(e.to_string()),
                },
            );
            return Err(format!("Processing failed: {e}"));
        }
    };

    // E2E-B05: Emit structuring stage (extraction complete, structuring done)
    let _ = app.emit(
        "processing-progress",
        ProcessingProgressEvent {
            stage: "structuring".into(),
            file_name: file_name.clone(),
            document_id: Some(output.outcome.document_id.to_string()),
            progress_pct: Some(60),
            error: None,
        },
    );

    // Save pending review if structuring succeeded
    if let Some(ref structuring) = output.structuring_result {
        // Emit: saving for review
        let _ = app.emit(
            "processing-progress",
            ProcessingProgressEvent {
                stage: "saving_review".into(),
                file_name: file_name.clone(),
                document_id: Some(output.outcome.document_id.to_string()),
                progress_pct: Some(90),
                error: None,
            },
        );

        crate::commands::review::save_pending_structuring(session, structuring)
            .map_err(|e| format!("Failed to save for review: {e}"))?;
    }

    // Emit: complete
    let _ = app.emit(
        "processing-progress",
        ProcessingProgressEvent {
            stage: "complete".into(),
            file_name: file_name.clone(),
            document_id: Some(output.outcome.document_id.to_string()),
            progress_pct: Some(100),
            error: None,
        },
    );

    state.log_access(
        crate::core_state::AccessSource::DesktopUi,
        "process_document",
        &format!("document:{}", output.outcome.document_id),
    );
    state.update_activity();

    tracing::info!(
        document_id = %output.outcome.document_id,
        import_status = ?output.outcome.import_status,
        has_structuring = output.outcome.structuring.is_some(),
        file = %output.outcome.original_filename,
        "Document processed end-to-end"
    );

    Ok(output.outcome)
}

/// Process multiple documents end-to-end (batch).
///
/// Each file goes through the full pipeline independently.
/// Failures on individual files do not stop the batch.
#[tauri::command]
pub async fn process_documents_batch(
    state: State<'_, Arc<CoreState>>,
    app: AppHandle,
    file_paths: Vec<String>,
) -> Result<Vec<crate::pipeline::processor::ProcessingOutcome>, String> {
    if file_paths.is_empty() {
        return Ok(vec![]);
    }
    if file_paths.len() > 20 {
        return Err("Maximum 20 files per processing batch (LLM is slow per file)".into());
    }

    // Build processor once for the batch
    let processor = crate::pipeline::processor::build_processor()
        .map_err(|e| format!("Failed to initialize processor: {e}"))?;

    let guard = state.read_session().map_err(|e| e.to_string())?;
    let session = guard
        .as_ref()
        .ok_or("No active profile. Unlock a profile first.")?;
    let conn = open_database(session.db_path()).map_err(|e| format!("Database error: {e}"))?;

    let total = file_paths.len();
    let mut results = Vec::with_capacity(total);

    for (i, file_path) in file_paths.iter().enumerate() {
        let path = Path::new(file_path);
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Emit batch progress
        let _ = app.emit(
            "processing-batch-progress",
            ProcessingBatchProgressEvent {
                current: i + 1,
                total,
                file_name: file_name.clone(),
                stage: "processing".into(),
            },
        );

        if !path.exists() || !path.is_file() {
            tracing::warn!(path = %file_path, "Skipping non-existent or non-file path");
            continue;
        }

        match processor.process_file(path, session, &conn) {
            Ok(output) => {
                // Save pending review
                if let Some(ref structuring) = output.structuring_result {
                    if let Err(e) =
                        crate::commands::review::save_pending_structuring(session, structuring)
                    {
                        tracing::warn!(
                            document_id = %output.outcome.document_id,
                            error = %e,
                            "Failed to save pending review"
                        );
                    }
                }

                tracing::info!(
                    document_id = %output.outcome.document_id,
                    import_status = ?output.outcome.import_status,
                    "Batch processing: file completed"
                );
                results.push(output.outcome);
            }
            Err(e) => {
                tracing::warn!(
                    path = %file_path,
                    error = %e,
                    "Batch processing: file failed"
                );
            }
        }
    }

    state.log_access(
        crate::core_state::AccessSource::DesktopUi,
        "process_documents_batch",
        &format!("batch:{} files", results.len()),
    );
    state.update_activity();

    Ok(results)
}

// ---------------------------------------------------------------------------
// Event types
// ---------------------------------------------------------------------------

/// Progress event emitted during document import.
#[derive(Debug, Clone, serde::Serialize)]
struct ImportProgressEvent {
    stage: String,
    file_name: String,
    document_id: Option<String>,
    error: Option<String>,
}

/// Batch progress event emitted during multi-file import.
#[derive(Debug, Clone, serde::Serialize)]
struct ImportBatchProgressEvent {
    current: usize,
    total: usize,
    file_name: String,
}

/// Progress event emitted during document processing (E2E-B02).
#[derive(Debug, Clone, serde::Serialize)]
struct ProcessingProgressEvent {
    stage: String,
    file_name: String,
    document_id: Option<String>,
    progress_pct: Option<u8>,
    error: Option<String>,
}

/// Batch progress event emitted during multi-file processing (E2E-B02).
#[derive(Debug, Clone, serde::Serialize)]
struct ProcessingBatchProgressEvent {
    current: usize,
    total: usize,
    file_name: String,
    stage: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_progress_event_serializes() {
        let event = ImportProgressEvent {
            stage: "started".into(),
            file_name: "test.pdf".into(),
            document_id: None,
            error: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("started"));
        assert!(json.contains("test.pdf"));
    }

    #[test]
    fn import_batch_progress_serializes() {
        let event = ImportBatchProgressEvent {
            current: 3,
            total: 10,
            file_name: "scan.jpg".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"current\":3"));
        assert!(json.contains("\"total\":10"));
    }

    #[test]
    fn processing_progress_event_serializes() {
        let event = ProcessingProgressEvent {
            stage: "extracting".into(),
            file_name: "report.pdf".into(),
            document_id: Some("abc-123".into()),
            progress_pct: Some(30),
            error: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("extracting"));
        assert!(json.contains("report.pdf"));
        assert!(json.contains("30"));
    }

    #[test]
    fn processing_batch_progress_serializes() {
        let event = ProcessingBatchProgressEvent {
            current: 2,
            total: 5,
            file_name: "scan.jpg".into(),
            stage: "processing".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"current\":2"));
        assert!(json.contains("\"total\":5"));
        assert!(json.contains("processing"));
    }
}
