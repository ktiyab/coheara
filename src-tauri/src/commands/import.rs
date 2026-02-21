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
use crate::pipeline::processor::{stage_name, stage_pct_range, StageTracker, STAGE_IMPORTING};

/// Import a document from a local file path.
///
/// Validates the path, runs L1-01 format detection + staging + dedup,
/// and returns the import result for the frontend to handle.
/// Runs on a blocking thread via `spawn_blocking` to avoid freezing the UI.
#[tauri::command]
pub async fn import_document(
    state: State<'_, Arc<CoreState>>,
    app: AppHandle,
    file_path: String,
) -> Result<ImportResult, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
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

        let conn = open_database(&db_path, Some(session.key_bytes()))
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
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

/// Import multiple documents from local file paths (batch import).
///
/// Processes each file independently, collecting results.
/// Does not stop on individual file failures.
/// Runs on a blocking thread via `spawn_blocking` to avoid freezing the UI.
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

    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
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

        let conn = open_database(&db_path, Some(session.key_bytes()))
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
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

// ---------------------------------------------------------------------------
// E2E-B02: Document Processing Commands
// ---------------------------------------------------------------------------

/// RAII progress monitor that emits stage-aware progress events.
///
/// Uses `mpsc::recv_timeout` for efficient blocking — wakes instantly on shutdown
/// signal (via Drop) or after 500ms timeout to emit a progress event.
/// No manual cleanup needed: dropping the struct stops the thread.
struct ProgressMonitor {
    shutdown_tx: std::sync::mpsc::Sender<()>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl ProgressMonitor {
    /// Start a progress monitor thread that emits heartbeat events.
    fn start(app: &AppHandle, file_name: &str, tracker: StageTracker) -> Self {
        let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();
        let emit_app = app.clone();
        let emit_name = file_name.to_string();

        let handle = std::thread::spawn(move || {
            let mut last_stage: u8 = u8::MAX; // force initial transition
            let mut pct: u8 = 5;

            loop {
                match shutdown_rx.recv_timeout(std::time::Duration::from_millis(500)) {
                    // Shutdown signal or sender dropped — exit immediately
                    Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    // Timeout — emit a progress event
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        let current = tracker.load(std::sync::atomic::Ordering::Relaxed);
                        let (min_pct, max_pct) = stage_pct_range(current);

                        // Stage transition: jump to new stage's starting pct
                        if current != last_stage {
                            pct = min_pct;
                            last_stage = current;
                        }

                        let _ = emit_app.emit(
                            "processing-progress",
                            ProcessingProgressEvent {
                                stage: stage_name(current).into(),
                                file_name: emit_name.clone(),
                                progress_pct: Some(pct),
                                ..Default::default()
                            },
                        );

                        // Increment within range
                        if pct < max_pct {
                            pct = pct.saturating_add(3).min(max_pct);
                        }
                    }
                }
            }
        });

        Self {
            shutdown_tx,
            handle: Some(handle),
        }
    }
}

impl Drop for ProgressMonitor {
    fn drop(&mut self) {
        // Send shutdown signal (ignore error if receiver already dropped)
        let _ = self.shutdown_tx.send(());
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// Process a document end-to-end: import → extract → structure → save pending review.
///
/// This is the primary command for the file picker flow. The user selects a file,
/// the system imports it, extracts text, structures with the LLM, and saves the
/// result for the patient to review before committing to storage.
/// Runs on a blocking thread via `spawn_blocking` — the pipeline uses
/// `reqwest::blocking::Client` internally (for Ollama HTTP calls), which is safe
/// inside `spawn_blocking` (non-async thread) but would panic in async context.
#[tauri::command]
pub async fn process_document(
    state: State<'_, Arc<CoreState>>,
    app: AppHandle,
    file_path: String,
) -> Result<crate::pipeline::processor::ProcessingOutcome, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
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

        // Acquire session + DB
        let guard = state.read_session().map_err(|e| e.to_string())?;
        let session = guard
            .as_ref()
            .ok_or("No active profile. Unlock a profile first.")?;
        let conn = open_database(session.db_path(), Some(session.key_bytes()))
            .map_err(|e| format!("Database error: {e}"))?;

        // Resolve active model via preferences (L6-04)
        let ollama = crate::pipeline::structuring::ollama::OllamaClient::default_local();
        let resolved = state
            .resolver()
            .resolve(&conn, &ollama)
            .map_err(|e| format!("No AI model available: {e}"))?;

        // Build the document processor with the resolved model + stage tracker
        let mut processor = crate::pipeline::processor::build_processor(&resolved.name)
            .map_err(|e| format!("Failed to initialize processor: {e}"))?;

        let tracker: StageTracker =
            std::sync::Arc::new(std::sync::atomic::AtomicU8::new(STAGE_IMPORTING));
        processor.set_stage_tracker(tracker.clone());

        // Emit initial "importing" stage
        let _ = app.emit(
            "processing-progress",
            ProcessingProgressEvent {
                stage: "importing".into(),
                file_name: file_name.clone(),
                progress_pct: Some(5),
                ..Default::default()
            },
        );

        // Start RAII progress monitor — Drop stops the thread automatically
        let _monitor = ProgressMonitor::start(&app, &file_name, tracker);

        // Run the full pipeline (import → extract → structure)
        let output = match processor.process_file(path, session, &conn) {
            Ok(output) => output,
            Err(e) => {
                drop(_monitor); // explicit drop for clarity (would drop at scope end anyway)
                // R.1+R.4: Emit failure event with patient-friendly error
                let patient_err = e.to_patient_error();
                let _ = app.emit(
                    "processing-progress",
                    ProcessingProgressEvent {
                        stage: "failed".into(),
                        file_name: file_name.clone(),
                        error: Some(e.sanitized_message()),
                        error_category: Some(patient_err.category.clone()),
                        is_retryable: Some(patient_err.retry_possible),
                        ..Default::default()
                    },
                );
                return Err(format!("{}: {}", patient_err.title, patient_err.message));
            }
        };

        let doc_id_str = output.outcome.document_id.to_string();

        // Save pending review if structuring succeeded
        if let Some(ref structuring) = output.structuring_result {
            let _ = app.emit(
                "processing-progress",
                ProcessingProgressEvent {
                    stage: "saving_review".into(),
                    file_name: file_name.clone(),
                    document_id: Some(doc_id_str.clone()),
                    progress_pct: Some(90),
                    ..Default::default()
                },
            );

            crate::commands::review::save_pending_structuring(session, structuring)
                .map_err(|e| format!("Failed to save for review: {e}"))?;

            // O.5: Update pipeline status → PendingReview
            if let Err(e) = crate::db::repository::update_pipeline_status(
                &conn,
                &output.outcome.document_id,
                &crate::models::enums::PipelineStatus::PendingReview,
            ) {
                tracing::warn!(
                    document_id = %output.outcome.document_id,
                    error = %e,
                    "Failed to set pipeline status to PendingReview"
                );
            }
        }

        // Emit: complete (only AFTER all post-processing is done)
        let _ = app.emit(
            "processing-progress",
            ProcessingProgressEvent {
                stage: "complete".into(),
                file_name: file_name.clone(),
                document_id: Some(doc_id_str),
                progress_pct: Some(100),
                ..Default::default()
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
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

/// Process multiple documents end-to-end (batch).
///
/// Each file goes through the full pipeline independently.
/// Failures on individual files do not stop the batch.
/// Runs on a blocking thread via `spawn_blocking` to avoid freezing the UI.
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

    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let guard = state.read_session().map_err(|e| e.to_string())?;
        let session = guard
            .as_ref()
            .ok_or("No active profile. Unlock a profile first.")?;
        let conn = open_database(session.db_path(), Some(session.key_bytes()))
            .map_err(|e| format!("Database error: {e}"))?;

        // Resolve active model via preferences (L6-04)
        let ollama = crate::pipeline::structuring::ollama::OllamaClient::default_local();
        let resolved = state
            .resolver()
            .resolve(&conn, &ollama)
            .map_err(|e| format!("No AI model available: {e}"))?;

        // Build processor once for the batch
        let mut processor = crate::pipeline::processor::build_processor(&resolved.name)
            .map_err(|e| format!("Failed to initialize processor: {e}"))?;

        let total = file_paths.len();
        let mut results = Vec::with_capacity(total);

        for (i, file_path) in file_paths.iter().enumerate() {
            let path = Path::new(file_path);
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            if !path.exists() || !path.is_file() {
                tracing::warn!(path = %file_path, "Skipping non-existent or non-file path");
                continue;
            }

            // Per-file stage tracker + heartbeat
            let tracker: StageTracker =
                std::sync::Arc::new(std::sync::atomic::AtomicU8::new(STAGE_IMPORTING));
            processor.set_stage_tracker(tracker.clone());

            // Emit batch progress with correct stage
            let _ = app.emit(
                "processing-batch-progress",
                ProcessingBatchProgressEvent {
                    current: i + 1,
                    total,
                    file_name: file_name.clone(),
                    stage: "importing".into(),
                },
            );

            // Emit initial per-file progress
            let _ = app.emit(
                "processing-progress",
                ProcessingProgressEvent {
                    stage: "importing".into(),
                    file_name: file_name.clone(),
                    progress_pct: Some(5),
                    ..Default::default()
                },
            );

            let _monitor = ProgressMonitor::start(&app, &file_name, tracker);

            match processor.process_file(path, session, &conn) {
                Ok(output) => {
                    drop(_monitor);

                    // Save pending review
                    if let Some(ref structuring) = output.structuring_result {
                        if let Err(e) = crate::commands::review::save_pending_structuring(
                            session,
                            structuring,
                        ) {
                            tracing::warn!(
                                document_id = %output.outcome.document_id,
                                error = %e,
                                "Failed to save pending review"
                            );
                        } else {
                            // O.5: Update pipeline status → PendingReview
                            if let Err(e) = crate::db::repository::update_pipeline_status(
                                &conn,
                                &output.outcome.document_id,
                                &crate::models::enums::PipelineStatus::PendingReview,
                            ) {
                                tracing::warn!(
                                    document_id = %output.outcome.document_id,
                                    error = %e,
                                    "Failed to set pipeline status to PendingReview"
                                );
                            }
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
                    drop(_monitor);
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
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

// ---------------------------------------------------------------------------
// P.4: Delete Document IPC
// ---------------------------------------------------------------------------

/// Delete a document and all its child entities, vector chunks, and pending review files.
///
/// Uses the cascade delete from O.2 to ensure complete cleanup.
/// Also removes any pending review file for this document.
/// Runs on a blocking thread via `spawn_blocking` to avoid freezing the UI.
#[tauri::command]
pub async fn delete_document(
    document_id: String,
    state: State<'_, Arc<CoreState>>,
    app: AppHandle,
) -> Result<(), String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let doc_id = uuid::Uuid::parse_str(&document_id)
            .map_err(|e| format!("Invalid document ID: {e}"))?;

        let guard = state.read_session().map_err(|e| e.to_string())?;
        let session = guard
            .as_ref()
            .ok_or("No active profile. Unlock a profile first.")?;
        let conn = open_database(session.db_path(), Some(session.key_bytes()))
            .map_err(|e| format!("Database error: {e}"))?;

        // Delete from database (entities, chunks, document)
        crate::db::repository::delete_document_cascade(&conn, &doc_id)
            .map_err(|e| format!("Delete failed: {e}"))?;

        // Clean up pending review file if it exists
        let _ = crate::commands::review::remove_pending_structuring_pub(session, &doc_id);

        let _ = app.emit("document-deleted", doc_id.to_string());

        state.log_access(
            crate::core_state::AccessSource::DesktopUi,
            "delete_document",
            &format!("document:{doc_id}"),
        );
        state.update_activity();

        tracing::info!(document_id = %doc_id, "Document deleted via IPC");
        Ok(())
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

// ---------------------------------------------------------------------------
// P.1: Reprocess Document IPC
// ---------------------------------------------------------------------------

/// Reprocess an existing document: re-run extraction + structuring.
///
/// P.7: Skips duplicate detection since the document is already in the DB.
/// P.3: Entity store is idempotent (clears before insert).
///
/// The document must exist and be in a reprocessable state
/// (Imported, Failed, or PendingReview).
/// Runs on a blocking thread via `spawn_blocking` to avoid freezing the UI.
#[tauri::command]
pub async fn reprocess_document(
    document_id: String,
    state: State<'_, Arc<CoreState>>,
    app: AppHandle,
) -> Result<crate::pipeline::processor::ProcessingOutcome, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let doc_id = uuid::Uuid::parse_str(&document_id)
            .map_err(|e| format!("Invalid document ID: {e}"))?;

        let guard = state.read_session().map_err(|e| e.to_string())?;
        let session = guard
            .as_ref()
            .ok_or("No active profile. Unlock a profile first.")?;
        let conn = open_database(session.db_path(), Some(session.key_bytes()))
            .map_err(|e| format!("Database error: {e}"))?;

        // Fetch the existing document
        let doc = crate::db::repository::get_document(&conn, &doc_id)
            .map_err(|e| format!("Database error: {e}"))?
            .ok_or_else(|| format!("Document not found: {doc_id}"))?;

        // Validate the document is in a reprocessable state
        let reprocessable = matches!(
            doc.pipeline_status,
            crate::models::enums::PipelineStatus::Imported
                | crate::models::enums::PipelineStatus::Failed
                | crate::models::enums::PipelineStatus::PendingReview
        );
        if !reprocessable {
            return Err(format!(
                "Document cannot be reprocessed in '{}' state",
                doc.pipeline_status.as_str()
            ));
        }

        // P.7: Build ImportResult from existing document (skips duplicate detection).
        let import_result = ImportResult {
            document_id: doc.id,
            original_filename: doc.title.clone(),
            staged_path: doc.source_file.clone(),
            format: crate::pipeline::import::format::FormatDetection {
                mime_type: "application/octet-stream".into(),
                category: crate::pipeline::import::format::FileCategory::PlainText,
                is_digital_pdf: None,
                file_size_bytes: 0,
            },
            duplicate_of: None,
            status: ImportStatus::Staged,
        };

        // Resolve model + build processor
        let ollama = crate::pipeline::structuring::ollama::OllamaClient::default_local();
        let resolved = state
            .resolver()
            .resolve(&conn, &ollama)
            .map_err(|e| format!("No AI model available: {e}"))?;

        let mut processor = crate::pipeline::processor::build_processor(&resolved.name)
            .map_err(|e| format!("Failed to initialize processor: {e}"))?;

        // Reset pipeline status to Imported before reprocessing
        if let Err(e) = crate::db::repository::update_pipeline_status(
            &conn,
            &doc_id,
            &crate::models::enums::PipelineStatus::Imported,
        ) {
            tracing::warn!(document_id = %doc_id, error = %e, "Failed to reset pipeline status");
        }

        // Reprocessing skips import stage — starts at extracting
        let tracker: StageTracker = std::sync::Arc::new(
            std::sync::atomic::AtomicU8::new(crate::pipeline::processor::STAGE_EXTRACTING),
        );
        processor.set_stage_tracker(tracker.clone());

        let _ = app.emit(
            "processing-progress",
            ProcessingProgressEvent {
                stage: "extracting".into(),
                file_name: doc.title.clone(),
                document_id: Some(doc_id.to_string()),
                progress_pct: Some(15),
                ..Default::default()
            },
        );

        // Start RAII progress monitor — Drop stops the thread automatically
        let _monitor = ProgressMonitor::start(&app, &doc.title, tracker);

        // Process using the imported document path
        let output = match processor.process_imported(&import_result, session, &conn) {
            Ok(output) => output,
            Err(e) => {
                drop(_monitor);
                // R.1+R.4: Emit failure with patient-friendly error
                let patient_err = e.to_patient_error();
                let _ = app.emit(
                    "processing-progress",
                    ProcessingProgressEvent {
                        stage: "failed".into(),
                        file_name: doc.title.clone(),
                        document_id: Some(doc_id.to_string()),
                        error: Some(e.sanitized_message()),
                        error_category: Some(patient_err.category.clone()),
                        is_retryable: Some(patient_err.retry_possible),
                        ..Default::default()
                    },
                );
                return Err(format!("{}: {}", patient_err.title, patient_err.message));
            }
        };

        // Save pending review if structuring succeeded
        if let Some(ref structuring) = output.structuring_result {
            crate::commands::review::save_pending_structuring(session, structuring)
                .map_err(|e| format!("Failed to save for review: {e}"))?;

            if let Err(e) = crate::db::repository::update_pipeline_status(
                &conn,
                &doc_id,
                &crate::models::enums::PipelineStatus::PendingReview,
            ) {
                tracing::warn!(document_id = %doc_id, error = %e, "Failed to set PendingReview status");
            }
        }

        let _ = app.emit(
            "processing-progress",
            ProcessingProgressEvent {
                stage: "complete".into(),
                file_name: doc.title.clone(),
                document_id: Some(doc_id.to_string()),
                progress_pct: Some(100),
                ..Default::default()
            },
        );

        state.log_access(
            crate::core_state::AccessSource::DesktopUi,
            "reprocess_document",
            &format!("document:{doc_id}"),
        );
        state.update_activity();

        tracing::info!(document_id = %doc_id, "Document reprocessed via IPC");
        Ok(output.outcome)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
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
#[derive(Debug, Clone, Default, serde::Serialize)]
struct ProcessingProgressEvent {
    stage: String,
    file_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    document_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    progress_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    /// P.5: Patient-friendly error category (only set when stage is "failed").
    #[serde(skip_serializing_if = "Option::is_none")]
    error_category: Option<crate::pipeline::processor::ErrorCategory>,
    /// P.5: Whether the error is retryable.
    #[serde(skip_serializing_if = "Option::is_none")]
    is_retryable: Option<bool>,
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
            ..Default::default()
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

    #[test]
    fn processing_progress_with_error_category_serializes() {
        let event = ProcessingProgressEvent {
            stage: "failed".into(),
            file_name: "broken.pdf".into(),
            error: Some("AI model not found".into()),
            error_category: Some(crate::pipeline::processor::ErrorCategory::AiUnavailable),
            is_retryable: Some(true),
            ..Default::default()
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"error_category\":\"ai_unavailable\""));
        assert!(json.contains("\"is_retryable\":true"));
        assert!(!json.contains("progress_pct")); // skip_serializing_if works
    }
}
