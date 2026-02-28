//! BTL-10 C4: Import queue worker — background task processing queued jobs.
//!
//! Spawns a tokio task during Tauri setup. Loops: await notification →
//! drain all Queued jobs → run each through the import pipeline →
//! emit `import-queue-update` events per state change.
//!
//! Pattern: Signal JobManager (sequential processing, one job at a time).
//! The StageWatcher maps processor stages to queue states in real-time.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::core_state::CoreState;
use crate::import_queue::{ImportJob, JobState};
use crate::pipeline::processor::{
    stage_pct_range, ProgressTracker, STAGE_EXTRACTING, STAGE_IMPORTING, STAGE_STRUCTURING,
};

// ---------------------------------------------------------------------------
// Event payload
// ---------------------------------------------------------------------------

/// Event payload emitted to frontend on each queue state change.
#[derive(Debug, Clone, Serialize)]
pub struct ImportQueueEvent {
    pub job_id: String,
    pub state: JobState,
    pub progress_pct: u8,
    pub filename: String,
    pub document_id: Option<String>,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Worker entry point
// ---------------------------------------------------------------------------

/// Start the import queue worker as a background tokio task.
///
/// Call from Tauri `setup`. The task runs for the lifetime of the app.
/// It awaits `ImportQueueService::notifier()` for new jobs.
pub fn start_import_queue_worker(app_handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tracing::info!("Import queue worker started");
        worker_loop(&app_handle).await;
    });
}

async fn worker_loop(app: &AppHandle) {
    let state: tauri::State<'_, Arc<CoreState>> = app.state();
    let queue = state.import_queue();

    loop {
        // Wait for notification (enqueue/retry calls notify_one)
        queue.notifier().notified().await;

        // Drain all queued jobs (sequential — Ollama serves one request at a time)
        while let Some(job) = queue.next_queued() {
            queue.set_running(true);
            emit_job_snapshot(app, &job);
            process_job(app, job).await;
        }

        queue.set_running(false);
    }
}

// ---------------------------------------------------------------------------
// Event emission helpers
// ---------------------------------------------------------------------------

/// Emit an `import-queue-update` event from an ImportJob snapshot.
fn emit_job_snapshot(app: &AppHandle, job: &ImportJob) {
    let _ = app.emit(
        "import-queue-update",
        ImportQueueEvent {
            job_id: job.id.clone(),
            state: job.state.clone(),
            progress_pct: job.progress_pct,
            filename: job.filename.clone(),
            document_id: job.document_id.clone(),
            error: job.error.clone(),
        },
    );
}

/// Re-read a job from the queue and emit its current state.
fn emit_current_state(app: &AppHandle, state: &CoreState, job_id: &str) {
    if let Some(job) = state.import_queue().get_job(job_id) {
        emit_job_snapshot(app, &job);
    }
}

// ---------------------------------------------------------------------------
// Job processing
// ---------------------------------------------------------------------------

/// Process a single import job. Runs the heavy pipeline on a blocking thread.
///
/// §21 Fix C: Creates a cancellation token before spawn_blocking. The token is
/// shared with the processor for cooperative cancellation at page boundaries.
async fn process_job(app: &AppHandle, job: ImportJob) {
    let app_clone = app.clone();
    let job_id = job.id.clone();
    let job_id_for_error = job.id.clone();
    let file_path = job.file_path.clone();
    let filename = job.filename.clone();
    let is_recovery = job.document_id.is_some();

    // §21 Fix C: Create cancellation token before blocking work
    let state: tauri::State<'_, Arc<CoreState>> = app.state();
    let cancel_token = state.import_queue().create_cancellation_token(&job_id);
    let cancel_token_clone = cancel_token.clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        if is_recovery {
            process_recovery_job(&app_clone, &job_id, &file_path, &filename, cancel_token_clone)
        } else {
            process_fresh_job(&app_clone, &job_id, &file_path, &filename, cancel_token_clone)
        }
    })
    .await;

    let state: tauri::State<'_, Arc<CoreState>> = app.state();

    match result {
        Ok(Ok(())) => {
            // Success — job state already updated inside the blocking function
        }
        Ok(Err(err)) => {
            // §21 Fix C: Skip marking as Failed if already Cancelled by user
            let already_cancelled = state.import_queue()
                .get_job(&job_id_for_error)
                .map(|j| j.state == JobState::Cancelled)
                .unwrap_or(false);

            if !already_cancelled {
                let _ = state.import_queue().update_job_state(
                    &job_id_for_error,
                    JobState::Failed,
                    None,
                    None,
                    None,
                    Some(err),
                );
                emit_current_state(app, &state, &job_id_for_error);
            }
        }
        Err(join_err) => {
            let _ = state.import_queue().update_job_state(
                &job_id_for_error,
                JobState::Failed,
                None,
                None,
                None,
                Some(format!("Internal error: {join_err}")),
            );
            emit_current_state(app, &state, &job_id_for_error);
        }
    }

    // §21 Fix C: Always clean up cancellation token
    state.import_queue().remove_cancellation_token(&job_id_for_error);
}

/// Process a fresh import job (no existing document — full pipeline).
fn process_fresh_job(
    app: &AppHandle,
    job_id: &str,
    file_path: &str,
    filename: &str,
    cancel_token: Arc<AtomicBool>,
) -> Result<(), String> {
    let state: tauri::State<'_, Arc<CoreState>> = app.state();
    let queue = state.import_queue();

    let path = std::path::Path::new(file_path);
    if !path.exists() {
        return Err(format!("File not found: {file_path}"));
    }
    if !path.is_file() {
        return Err("Path is not a regular file".into());
    }

    // Acquire session + DB
    let guard = state.read_session().map_err(|e| e.to_string())?;
    let session = guard
        .as_ref()
        .ok_or("No active profile. Unlock a profile first.")?;
    let conn = crate::db::sqlite::open_database(session.db_path(), Some(session.key_bytes()))
        .map_err(|e| format!("Database error: {e}"))?;

    // Resolve pipeline assignment
    let (assignment, mut ollama, pipeline_config) =
        resolve_pipeline_setup(&conn, &state, path)?;

    // Acquire Butler
    let primary_model = match &assignment.extraction {
        crate::pipeline::model_router::ExtractionStrategy::VisionOcr { model } => model.clone(),
        _ => assignment.structuring_model.clone(),
    };
    let _butler_guard = state
        .butler()
        .acquire(
            crate::ollama_service::OperationKind::DocumentOcr,
            &primary_model,
        )
        .map_err(|e| format!("Failed to acquire Ollama: {e}"))?;

    // §21 Fix B: Record model metadata without premature state transition.
    // StageWatcher handles Importing→Extracting when processor's stage_tracker changes.
    let _ = queue.update_job_progress(
        job_id,
        None,
        Some(primary_model.clone()),
    );
    emit_current_state(app, &state, job_id);

    // Warm models
    ollama.set_vision_num_ctx(pipeline_config.num_ctx);
    crate::commands::import::warm_assignment_models(state.butler(), &ollama, &assignment);

    // Build processor with C4 vision fallback
    let lang = state.get_profile_language();
    let fallback = build_vision_fallback(&state, &assignment, &pipeline_config, &lang);
    let mut processor = crate::pipeline::processor::build_processor_from_assignment_with_fallback(
        &assignment,
        &pipeline_config,
        &lang,
        fallback,
    )
    .map_err(|e| format!("Failed to initialize processor: {e}"))?;

    // CPU swap hook for BatchStages mode
    setup_cpu_swap_hook(&mut processor, &assignment);

    // §21 Fix C: Pass cancellation token to processor
    processor.set_cancellation_token(cancel_token);

    // §22: Work-based progress tracker (maps processor stages + page counters → events)
    let tracker = Arc::new(ProgressTracker::new(STAGE_IMPORTING));
    processor.set_progress_tracker(tracker.clone());
    let _watcher = StageWatcher::start(app, &state, job_id, filename, tracker);

    // Run the full pipeline
    let output = processor
        .process_file(path, session, &conn)
        .map_err(|e| {
            let patient_err = e.to_patient_error();
            format!("{}: {}", patient_err.title, patient_err.message)
        })?;

    // Explicitly drop watcher before final state update
    drop(_watcher);

    // Finalize
    finalize_job(app, &state, &conn, session, job_id, &output)?;

    state.log_access(
        crate::core_state::AccessSource::DesktopUi,
        "import_queue_process",
        &format!("document:{}", output.outcome.document_id),
    );
    state.update_activity();

    tracing::info!(
        job_id = %job_id,
        document_id = %output.outcome.document_id,
        file = %filename,
        "Queue worker: document processed"
    );

    Ok(())
}

/// Process a recovery job (document exists in DB — reprocess only).
fn process_recovery_job(
    app: &AppHandle,
    job_id: &str,
    file_path: &str,
    _filename: &str,
    cancel_token: Arc<AtomicBool>,
) -> Result<(), String> {
    let state: tauri::State<'_, Arc<CoreState>> = app.state();
    let queue = state.import_queue();

    // Get existing document_id from job
    let doc_id_str = queue
        .get_job(job_id)
        .and_then(|j| j.document_id.clone())
        .ok_or("Recovery job missing document_id")?;

    let doc_id = uuid::Uuid::parse_str(&doc_id_str)
        .map_err(|e| format!("Invalid document ID: {e}"))?;

    // Acquire session + DB
    let guard = state.read_session().map_err(|e| e.to_string())?;
    let session = guard
        .as_ref()
        .ok_or("No active profile. Unlock a profile first.")?;
    let conn = crate::db::sqlite::open_database(session.db_path(), Some(session.key_bytes()))
        .map_err(|e| format!("Database error: {e}"))?;

    // Fetch document
    let doc = crate::db::repository::get_document(&conn, &doc_id)
        .map_err(|e| format!("Database error: {e}"))?
        .ok_or_else(|| format!("Document not found: {doc_id}"))?;

    // §21 Fix A: Re-detect format from staged file (magic bytes, not extension).
    // This ensures recovery jobs use the correct extraction strategy (e.g. VisionOCR for PDFs).
    let staged_path = std::path::Path::new(&doc.source_file);
    let format = crate::pipeline::import::format::detect_format(staged_path)
        .unwrap_or_else(|e| {
            tracing::warn!(
                document_id = %doc_id,
                error = %e,
                "Recovery: format detection failed, falling back to PlainText"
            );
            crate::pipeline::import::format::FormatDetection {
                mime_type: "application/octet-stream".into(),
                category: crate::pipeline::import::format::FileCategory::PlainText,
                is_digital_pdf: None,
                file_size_bytes: 0,
            }
        });

    let import_result = crate::pipeline::import::importer::ImportResult {
        document_id: doc.id,
        original_filename: doc.title.clone(),
        staged_path: doc.source_file.clone(),
        format,
        duplicate_of: None,
        status: crate::pipeline::import::importer::ImportStatus::Staged,
    };

    // Resolve pipeline
    let path = std::path::Path::new(file_path);
    let (assignment, mut ollama, pipeline_config) =
        resolve_pipeline_setup(&conn, &state, path)?;

    // Acquire Butler
    let primary_model = match &assignment.extraction {
        crate::pipeline::model_router::ExtractionStrategy::VisionOcr { model } => model.clone(),
        _ => assignment.structuring_model.clone(),
    };
    let _butler_guard = state
        .butler()
        .acquire(
            crate::ollama_service::OperationKind::DocumentOcr,
            &primary_model,
        )
        .map_err(|e| format!("Failed to acquire Ollama: {e}"))?;

    // §21 Fix B: Record model metadata without premature state transition.
    let _ = queue.update_job_progress(
        job_id,
        None,
        Some(primary_model.clone()),
    );
    emit_current_state(app, &state, job_id);

    // Warm models
    ollama.set_vision_num_ctx(pipeline_config.num_ctx);
    crate::commands::import::warm_assignment_models(state.butler(), &ollama, &assignment);

    // Build processor with C4 vision fallback
    let lang = state.get_profile_language();
    let fallback = build_vision_fallback(&state, &assignment, &pipeline_config, &lang);
    let mut processor = crate::pipeline::processor::build_processor_from_assignment_with_fallback(
        &assignment,
        &pipeline_config,
        &lang,
        fallback,
    )
    .map_err(|e| format!("Failed to initialize processor: {e}"))?;

    setup_cpu_swap_hook(&mut processor, &assignment);

    // §21 Fix C: Pass cancellation token to processor
    processor.set_cancellation_token(cancel_token);

    // §22: Work-based progress tracker starts at extracting (import already done)
    let tracker = Arc::new(ProgressTracker::new(STAGE_EXTRACTING));
    processor.set_progress_tracker(tracker.clone());
    let _watcher = StageWatcher::start(app, &state, job_id, &doc.title, tracker);

    // Reset DB pipeline status before reprocessing
    if let Err(e) = crate::db::repository::update_pipeline_status(
        &conn,
        &doc_id,
        &crate::models::enums::PipelineStatus::Imported,
    ) {
        tracing::warn!(document_id = %doc_id, error = %e, "Failed to reset pipeline status");
    }

    // Process
    let output = processor
        .process_imported(&import_result, session, &conn)
        .map_err(|e| {
            let patient_err = e.to_patient_error();
            format!("{}: {}", patient_err.title, patient_err.message)
        })?;

    drop(_watcher);

    // Finalize
    finalize_job(app, &state, &conn, session, job_id, &output)?;

    state.log_access(
        crate::core_state::AccessSource::DesktopUi,
        "import_queue_recovery",
        &format!("document:{doc_id}"),
    );
    state.update_activity();

    tracing::info!(
        job_id = %job_id,
        document_id = %doc_id,
        "Queue worker: recovery job processed"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Shared pipeline helpers
// ---------------------------------------------------------------------------

/// Resolve pipeline assignment, Ollama client, and hardware config.
fn resolve_pipeline_setup(
    conn: &rusqlite::Connection,
    state: &CoreState,
    path: &std::path::Path,
) -> Result<
    (
        crate::pipeline::model_router::PipelineAssignment,
        crate::pipeline::structuring::ollama::OllamaClient,
        crate::pipeline_config::PipelineConfig,
    ),
    String,
> {
    let ollama = crate::ollama_service::OllamaService::client();
    let file_category = crate::pipeline::import::format::detect_format(path)
        .map(|f| f.category)
        .unwrap_or(crate::pipeline::import::format::FileCategory::DigitalPdf);

    let mut assignment = crate::pipeline::model_router::resolve_pipeline(
        conn,
        state.resolver(),
        &ollama,
        &file_category,
    )
    .map_err(|e| format!("No AI model available: {e}"))?;

    // STR-01: Resolve extraction strategy
    let variant =
        crate::pipeline::strategy::detect_model_variant(&assignment.structuring_model);
    assignment.prompt_strategy = Some(crate::pipeline::strategy::resolve_strategy(
        crate::pipeline::strategy::ContextType::DocumentExtraction,
        variant,
    ));

    let pipeline_config = crate::commands::import::detect_pipeline_config(&ollama);

    Ok((assignment, ollama, pipeline_config))
}

/// C4: Build vision fallback for OCR degeneration recovery.
///
/// Creates a `FallbackSession` (unguarded — caller already holds butler guard)
/// and a `VisionClient` for iterative vision Q&A. Returns `None` for
/// non-vision extraction strategies (plain text, pdfium text).
fn build_vision_fallback(
    state: &CoreState,
    assignment: &crate::pipeline::model_router::PipelineAssignment,
    pipeline_config: &crate::pipeline_config::PipelineConfig,
    language: &str,
) -> Option<crate::pipeline::extraction::orchestrator::VisionFallback> {
    use crate::butler_service::FallbackSession;
    use crate::pipeline::extraction::orchestrator::VisionFallback;
    use crate::pipeline::extraction::vision_ocr::build_system_prompt;
    use crate::pipeline::strategy::ContextType;

    let model = match &assignment.extraction {
        crate::pipeline::model_router::ExtractionStrategy::VisionOcr { model } => model.clone(),
        _ => return None, // No vision fallback needed for non-vision strategies
    };

    let has_gpu = state
        .butler()
        .hardware_tier()
        .map_or(false, |t| {
            matches!(
                t,
                crate::hardware::GpuTier::FullGpu
                    | crate::hardware::GpuTier::PartialGpu
            )
        });

    let session = FallbackSession::new(&model, ContextType::VisionOcr, has_gpu);

    let mut vision_client = crate::ollama_service::OllamaService::client();
    vision_client.set_vision_num_ctx(pipeline_config.num_ctx);

    let system_prompt = build_system_prompt(language).to_string();

    Some(VisionFallback {
        session: Box::new(session),
        vision_client: Box::new(vision_client),
        system_prompt,
    })
}

/// Set up CPU swap hook for BatchStages mode.
fn setup_cpu_swap_hook(
    processor: &mut crate::pipeline::processor::DocumentProcessor,
    assignment: &crate::pipeline::model_router::PipelineAssignment,
) {
    if assignment.processing_mode == crate::pipeline::model_router::ProcessingMode::BatchStages {
        if let crate::pipeline::model_router::ExtractionStrategy::VisionOcr { ref model } =
            assignment.extraction
        {
            let swap_vision = model.clone();
            let swap_llm = assignment.structuring_model.clone();
            processor.set_between_stages_hook(Box::new(move || {
                let client = crate::ollama_service::OllamaService::client();
                tracing::info!("Queue worker CPU swap: unloading vision model, warming LLM");
                client.unload_model(&swap_vision)
                    .map_err(|e| format!("Unload vision model failed: {e}"))?;
                client.warm_model(&swap_llm)
                    .map_err(|e| format!("Warm LLM model failed: {e}"))?;
                Ok(())
            }));
        }
    }
}

/// Finalize a completed job: save pending review, update queue state, emit event.
fn finalize_job(
    app: &AppHandle,
    state: &CoreState,
    conn: &rusqlite::Connection,
    session: &crate::crypto::ProfileSession,
    job_id: &str,
    output: &crate::pipeline::processor::ProcessingOutput,
) -> Result<(), String> {
    let queue = state.import_queue();
    let doc_id_str = output.outcome.document_id.to_string();

    if let Some(ref structuring) = output.structuring_result {
        crate::commands::review::save_pending_structuring(session, structuring)
            .map_err(|e| format!("Failed to save for review: {e}"))?;

        // Update pipeline status → PendingReview
        if let Err(e) = crate::db::repository::update_pipeline_status(
            conn,
            &output.outcome.document_id,
            &crate::models::enums::PipelineStatus::PendingReview,
        ) {
            tracing::warn!(
                document_id = %output.outcome.document_id,
                error = %e,
                "Failed to set pipeline status to PendingReview"
            );
        }

        // Queue: Structuring → PendingReview
        let _ = queue.update_job_state(
            job_id,
            JobState::PendingReview,
            Some(95),
            Some(doc_id_str.clone()),
            None,
            None,
        );
        emit_current_state(app, state, job_id);

        // Queue: PendingReview → Done
        let _ = queue.update_job_state(
            job_id,
            JobState::Done,
            Some(100),
            Some(doc_id_str),
            None,
            None,
        );
    } else {
        // No structuring result — mark Done directly
        let _ = queue.update_job_state(
            job_id,
            JobState::Done,
            Some(100),
            Some(doc_id_str),
            None,
            None,
        );
    }

    emit_current_state(app, state, job_id);
    Ok(())
}

// ---------------------------------------------------------------------------
// StageWatcher — maps processor stages to queue states in real-time
// ---------------------------------------------------------------------------

/// §22: RAII stage watcher that polls the processor's ProgressTracker and computes
/// work-based progress from page counters. Updates queue state + emits events.
///
/// Pattern: ProgressMonitor (commands/import.rs) — uses mpsc recv_timeout
/// for efficient blocking with responsive shutdown.
struct StageWatcher {
    shutdown_tx: std::sync::mpsc::Sender<()>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl StageWatcher {
    /// §22: Start watching processor stages and computing work-based progress.
    ///
    /// Reads `ProgressTracker.stage` for stage transitions, and
    /// `page_current / page_total` for real page-level progress within each stage.
    fn start(
        app: &AppHandle,
        _state: &CoreState,
        job_id: &str,
        _filename: &str,
        tracker: Arc<ProgressTracker>,
    ) -> Self {
        let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();
        let emit_app = app.clone();
        let watcher_job_id = job_id.to_string();
        let state_ptr = app.state::<Arc<CoreState>>().inner().clone();

        let handle = std::thread::spawn(move || {
            let mut last_stage: u8 = u8::MAX;
            let mut pct: u8 = 5;

            loop {
                match shutdown_rx.recv_timeout(std::time::Duration::from_millis(500)) {
                    Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        let current = tracker.stage.load(Ordering::Relaxed);

                        // Stage changed → update queue state + base progress
                        if current != last_stage {
                            let queue = state_ptr.import_queue();
                            let new_state = match current {
                                STAGE_IMPORTING => JobState::Importing,
                                STAGE_EXTRACTING => JobState::Extracting,
                                STAGE_STRUCTURING => JobState::Structuring,
                                _ => continue,
                            };

                            if let Some(job) = queue.get_job(&watcher_job_id) {
                                if job.state != new_state {
                                    let (min_pct, _) = stage_pct_range(current);
                                    let _ = queue.update_job_state(
                                        &watcher_job_id,
                                        new_state,
                                        Some(min_pct),
                                        None,
                                        None,
                                        None,
                                    );
                                    pct = min_pct;
                                    emit_current_state(
                                        &emit_app,
                                        &state_ptr,
                                        &watcher_job_id,
                                    );
                                }
                            }

                            last_stage = current;
                        } else {
                            // §22: Same stage — compute work-based progress from page counters.
                            let (min_pct, max_pct) = stage_pct_range(current);
                            let page_cur = tracker.page_current.load(Ordering::Relaxed);
                            let page_tot = tracker.page_total.load(Ordering::Relaxed);

                            let new_pct = if page_tot > 0 {
                                let ratio = page_cur as f32 / page_tot as f32;
                                let range = (max_pct - min_pct) as f32;
                                (min_pct as f32 + ratio * range).min(max_pct as f32) as u8
                            } else {
                                // No page count yet — hold at stage base
                                min_pct
                            };

                            if new_pct != pct {
                                pct = new_pct;
                                let queue = state_ptr.import_queue();
                                let _ = queue.update_job_progress(
                                    &watcher_job_id,
                                    Some(pct),
                                    None,
                                );
                                emit_current_state(
                                    &emit_app,
                                    &state_ptr,
                                    &watcher_job_id,
                                );
                            }
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

impl Drop for StageWatcher {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(());
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_queue_event_serializes() {
        let event = ImportQueueEvent {
            job_id: "abc-123".into(),
            state: JobState::Extracting,
            progress_pct: 35,
            filename: "scan.pdf".into(),
            document_id: Some("doc-456".into()),
            error: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("abc-123"));
        assert!(json.contains("Extracting"));
        assert!(json.contains("35"));
        assert!(json.contains("scan.pdf"));
        assert!(json.contains("doc-456"));
    }

    #[test]
    fn import_queue_event_with_error() {
        let event = ImportQueueEvent {
            job_id: "abc-123".into(),
            state: JobState::Failed,
            progress_pct: 0,
            filename: "broken.pdf".into(),
            document_id: None,
            error: Some("OCR timeout".into()),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("Failed"));
        assert!(json.contains("OCR timeout"));
    }

    #[test]
    fn import_queue_event_all_states() {
        for state in [
            JobState::Queued,
            JobState::Importing,
            JobState::Extracting,
            JobState::Structuring,
            JobState::PendingReview,
            JobState::Done,
            JobState::Failed,
            JobState::Cancelled,
        ] {
            let event = ImportQueueEvent {
                job_id: "test".into(),
                state: state.clone(),
                progress_pct: 50,
                filename: "test.pdf".into(),
                document_id: None,
                error: None,
            };
            let json = serde_json::to_string(&event).unwrap();
            assert!(
                json.contains(&format!("{:?}", state)),
                "State {:?} not found in JSON: {}",
                state,
                json
            );
        }
    }
}
