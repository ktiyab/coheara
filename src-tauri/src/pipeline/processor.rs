//! E2E-B02: Document Processing Orchestrator.
//!
//! Single entry point that drives the full document pipeline:
//! import → extract → structure → (save pending review in command layer).
//!
//! Uses trait-based DI for all engines (VisionOcrEngine, LlmClient, etc.)
//! so the orchestrator remains fully testable with mock implementations.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;

use rusqlite::Connection;
use serde::Serialize;
use uuid::Uuid;

use crate::crypto::ProfileSession;
use crate::db::repository;
use crate::models::enums::{DocumentType, PipelineStatus};
use crate::pipeline::diagnostic;
use crate::pipeline::extraction::orchestrator::DocumentExtractor;
use crate::pipeline::extraction::types::TextExtractor;
use crate::pipeline::extraction::ExtractionError;
use crate::pipeline::import::importer::{import_file, ImportResult, ImportStatus};
use crate::pipeline::import::ImportError;
use crate::pipeline::structuring::orchestrator::DocumentStructurer;
use crate::pipeline::structuring::types::{
    ExtractedAllergy, ExtractedDiagnosis, ExtractedEntities, ExtractedInstruction,
    ExtractedLabResult, ExtractedMedication, ExtractedProcedure, ExtractedReferral,
    MedicalStructurer, StructuringResult,
};
use crate::pipeline::structuring::classify::{
    classify_document_type, classify_from_entities, parse_document_date,
};
use crate::pipeline::structuring::confidence::{
    apply_confidence_caps, compute_structuring_confidence, generate_confidence_warnings,
};
use crate::pipeline::structuring::extraction_strategy::StrategyOutput;
use crate::pipeline::structuring::sanitize::sanitize_markdown_output;
use crate::pipeline::structuring::validation::validate_extracted_entities;
use crate::pipeline::structuring::StructuringError;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// P.5: Patient-friendly error category for frontend display.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    /// File format not supported or corrupted
    UnsupportedFile,
    /// OCR engine not available or failed to initialize
    OcrUnavailable,
    /// AI model not reachable (Ollama down or model not pulled)
    AiUnavailable,
    /// AI produced unusable output (malformed response)
    AiOutputError,
    /// Internal database error
    DatabaseError,
    /// File could not be saved
    StorageError,
}

/// Errors that can occur during document processing.
#[derive(Debug, thiserror::Error)]
pub enum ProcessingError {
    #[error("Import failed: {0}")]
    Import(#[from] ImportError),

    #[error("Extraction failed: {0}")]
    Extraction(#[from] ExtractionError),

    #[error("Structuring failed: {0}")]
    Structuring(#[from] StructuringError),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Failed to persist for review: {0}")]
    PersistFailed(String),

    #[error("OCR engine initialization failed: {0}")]
    OcrInit(String),

    /// §21 Fix C: Processing cancelled by user via cooperative cancellation token.
    #[error("Processing cancelled by user")]
    Cancelled,
}

/// 12-ERC B3: Build StructuringResult from pre-extracted drill entities.
///
/// Applies all shared post-processing (steps 4-10 from structuring orchestrator).
/// Skips strategy.extract() LLM call — eliminates phantom entities.
/// INFALLIBLE: All operations are deterministic, no LLM, no I/O.
fn post_process_drill_output(
    document_id: &Uuid,
    output: StrategyOutput,
    ocr_confidence: f32,
) -> StructuringResult {
    // Step 4: Sanitize markdown (XSS)
    let markdown = sanitize_markdown_output(&output.markdown);

    // Step 5: Validate entities (count caps, injection, dose/lab plausibility)
    let validation = validate_extracted_entities(output.entities, Some(&document_id.to_string()));
    let mut entities = validation.entities;
    let mut warnings = validation.warnings;

    // Step 6: Cap confidence by OCR quality
    apply_confidence_caps(&mut entities, ocr_confidence);

    // Step 7: Classify document type from entities
    let mut doc_type = output
        .document_type
        .as_deref()
        .map(classify_document_type)
        .unwrap_or(DocumentType::Other);
    if matches!(doc_type, DocumentType::Other) {
        doc_type = classify_from_entities(&entities);
    }

    // Step 8: Parse document date
    let doc_date = output
        .document_date
        .as_deref()
        .and_then(parse_document_date);

    // Step 9: Compute structuring confidence
    let confidence = compute_structuring_confidence(ocr_confidence, &entities, warnings.len());

    // Step 10: Confidence warnings
    warnings.extend(generate_confidence_warnings(confidence));

    // Step 11: Raw responses for audit
    let raw = if output.raw_responses.is_empty() {
        None
    } else {
        Some(output.raw_responses.join("\n---\n"))
    };

    StructuringResult {
        document_id: *document_id,
        document_type: doc_type,
        document_date: doc_date,
        professional: output.professional,
        structured_markdown: markdown,
        extracted_entities: entities,
        structuring_confidence: confidence,
        markdown_file_path: None,
        validation_warnings: warnings,
        raw_llm_response: raw,
    }
}

/// R.1: Patient-friendly error with category, guidance, and sanitized message.
/// This is the single error type surfaced to the frontend for all processing errors.
#[derive(Debug, Clone, Serialize)]
pub struct PatientError {
    /// Short title for the error dialog
    pub title: String,
    /// Patient-friendly explanation (no technical details, no file paths, no PHI)
    pub message: String,
    /// Actionable suggestion for the patient
    pub suggestion: String,
    /// Error category for frontend styling/routing
    pub category: ErrorCategory,
    /// Whether retrying the operation might succeed
    pub retry_possible: bool,
}

/// R.4: Strip file paths, UUIDs, and technical details from error messages.
fn sanitize_error_message(raw: &str) -> String {
    let mut s = raw.to_string();

    // Strip absolute file paths (Unix + Windows)
    let path_re = regex::Regex::new(r"(/[^\s:]+|[A-Z]:\\[^\s:]+)").unwrap();
    s = path_re.replace_all(&s, "[file]").to_string();

    // Strip UUIDs
    let uuid_re =
        regex::Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
            .unwrap();
    s = uuid_re.replace_all(&s, "[id]").to_string();

    // Strip IP:port patterns
    let ip_re = regex::Regex::new(r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}(:\d+)?").unwrap();
    s = ip_re.replace_all(&s, "[server]").to_string();

    // Truncate if too long
    if s.len() > 200 {
        s.truncate(200);
        s.push_str("...");
    }

    s
}

impl ProcessingError {
    /// R.1: Convert to a patient-friendly error with full context.
    pub fn to_patient_error(&self) -> PatientError {
        // §21 Fix C: Cancelled is not a real error — neutral messaging.
        if matches!(self, ProcessingError::Cancelled) {
            return PatientError {
                title: "Import Cancelled".into(),
                message: "The import was cancelled.".into(),
                suggestion: "You can re-import this document at any time.".into(),
                category: ErrorCategory::UnsupportedFile,
                retry_possible: false,
            };
        }

        // Delegate to StructuringError's existing patient_message if available
        if let ProcessingError::Structuring(e) = self {
            let pm = e.patient_message();
            return PatientError {
                title: pm.title,
                message: pm.message,
                suggestion: pm.suggestion,
                category: self.category(),
                retry_possible: pm.retry_possible,
            };
        }

        match self {
            ProcessingError::Import(e) => match e {
                ImportError::UnsupportedFormat(_) => PatientError {
                    title: "Unsupported File".into(),
                    message: "This file type is not supported for medical document import.".into(),
                    suggestion: "Please use PDF, JPG, PNG, or plain text files.".into(),
                    category: ErrorCategory::UnsupportedFile,
                    retry_possible: false,
                },
                ImportError::FileTooLarge { size_mb, max_mb } => PatientError {
                    title: "File Too Large".into(),
                    message: format!("This file ({size_mb:.1} MB) exceeds the {max_mb} MB limit."),
                    suggestion: "Try scanning at a lower resolution or splitting the document."
                        .into(),
                    category: ErrorCategory::UnsupportedFile,
                    retry_possible: false,
                },
                ImportError::EncryptedPdf => PatientError {
                    title: "Protected PDF".into(),
                    message: "This PDF is password-protected and cannot be read.".into(),
                    suggestion:
                        "Please remove the PDF password protection first, then try again.".into(),
                    category: ErrorCategory::UnsupportedFile,
                    retry_possible: false,
                },
                _ => PatientError {
                    title: "Import Error".into(),
                    message: "The file could not be imported.".into(),
                    suggestion: "Please check the file is not corrupted and try again.".into(),
                    category: ErrorCategory::UnsupportedFile,
                    retry_possible: true,
                },
            },
            ProcessingError::Extraction(_) => PatientError {
                title: "Text Extraction Failed".into(),
                message: "Could not read text from this document.".into(),
                suggestion:
                    "Ensure the document is clear and readable. Scanned images need good contrast."
                        .into(),
                category: ErrorCategory::OcrUnavailable,
                retry_possible: true,
            },
            ProcessingError::Database(_) => PatientError {
                title: "Storage Error".into(),
                message: "A database error occurred while saving your document.".into(),
                suggestion: "Please try again. If the problem persists, check available disk space."
                    .into(),
                category: ErrorCategory::DatabaseError,
                retry_possible: false,
            },
            ProcessingError::PersistFailed(_) => PatientError {
                title: "Save Failed".into(),
                message: "Could not save the analysis results.".into(),
                suggestion: "Please check available disk space and try again.".into(),
                category: ErrorCategory::StorageError,
                retry_possible: true,
            },
            ProcessingError::OcrInit(_) => PatientError {
                title: "OCR Not Available".into(),
                message: "The text recognition engine could not start.".into(),
                suggestion: "Image-based documents cannot be processed. Try using a digital PDF instead.".into(),
                category: ErrorCategory::OcrUnavailable,
                retry_possible: false,
            },
            // Structuring and Cancelled are handled above via early return
            ProcessingError::Structuring(_) | ProcessingError::Cancelled => unreachable!(),
        }
    }

    /// R.4: Get a sanitized error string safe for frontend display.
    /// Strips file paths, UUIDs, and truncates.
    pub fn sanitized_message(&self) -> String {
        sanitize_error_message(&self.to_string())
    }

    /// P.5: Classify this error into a patient-friendly category.
    pub fn category(&self) -> ErrorCategory {
        match self {
            ProcessingError::Import(_) => ErrorCategory::UnsupportedFile,
            ProcessingError::Extraction(_) => ErrorCategory::OcrUnavailable,
            ProcessingError::Structuring(e) => {
                let msg = e.to_string().to_lowercase();
                if msg.contains("connection") || msg.contains("ollama") || msg.contains("model") {
                    ErrorCategory::AiUnavailable
                } else {
                    ErrorCategory::AiOutputError
                }
            }
            ProcessingError::Database(_) => ErrorCategory::DatabaseError,
            ProcessingError::PersistFailed(_) => ErrorCategory::StorageError,
            ProcessingError::OcrInit(_) => ErrorCategory::OcrUnavailable,
            ProcessingError::Cancelled => ErrorCategory::UnsupportedFile,
        }
    }

    /// P.5: Whether this error is retryable (transient vs permanent).
    pub fn is_retryable(&self) -> bool {
        match self {
            ProcessingError::Import(_) => false,
            ProcessingError::Extraction(_) => true,
            ProcessingError::Structuring(_) => true,
            ProcessingError::Database(_) => false,
            ProcessingError::PersistFailed(_) => true,
            ProcessingError::OcrInit(_) => false,
            ProcessingError::Cancelled => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// Summary returned to the frontend after processing a document.
#[derive(Debug, Clone, Serialize)]
pub struct ProcessingOutcome {
    pub document_id: Uuid,
    pub original_filename: String,
    pub import_status: ImportStatus,
    /// Populated only if extraction succeeded (i.e., file was staged).
    pub extraction: Option<ExtractionSummary>,
    /// Populated only if structuring succeeded.
    pub structuring: Option<StructuringSummary>,
}

/// Extraction stage summary.
#[derive(Debug, Clone, Serialize)]
pub struct ExtractionSummary {
    pub method: String,
    pub confidence: f32,
    pub page_count: usize,
    pub text_length: usize,
}

/// Structuring stage summary.
#[derive(Debug, Clone, Serialize)]
pub struct StructuringSummary {
    pub document_type: String,
    pub confidence: f32,
    pub entities_count: usize,
    pub has_professional: bool,
    pub document_date: Option<String>,
}

/// Full output including the raw StructuringResult for persistence.
pub struct ProcessingOutput {
    pub outcome: ProcessingOutcome,
    /// The full structuring result — used by the command layer to save
    /// the pending review file. None if import was not staged or
    /// structuring was skipped.
    pub structuring_result: Option<StructuringResult>,
}

// ---------------------------------------------------------------------------
// Stage tracking — shared between processor and command-layer heartbeat
// ---------------------------------------------------------------------------

/// Shared stage indicator for progress reporting.
/// Used by both `ProgressMonitor` (IPC commands) and `StageWatcher` (queue worker).
pub type StageTracker = Arc<AtomicU8>;

/// Stage constants matching the frontend's expected stage names.
pub const STAGE_IMPORTING: u8 = 0;
pub const STAGE_EXTRACTING: u8 = 1;
pub const STAGE_STRUCTURING: u8 = 2;

/// §22: Work-based progress tracker for the import queue path.
///
/// Wraps a `StageTracker` (shared with `DocumentProcessor`) and adds per-page
/// counters for real work-based progress. The queue worker's `StageWatcher`
/// reads all fields every 500ms to compute page-level progress.
///
/// `AtomicU8` for pages: max 255 covers >99.9% of medical documents.
/// For >255 pages, values clamp to 255 — progress still works, just coarser.
pub struct ProgressTracker {
    /// The stage atomic — shared with DocumentProcessor via `stage()`.
    pub stage: StageTracker,
    pub page_current: AtomicU8,
    pub page_total: AtomicU8,
}

impl ProgressTracker {
    pub fn new(initial_stage: u8) -> Self {
        Self {
            stage: Arc::new(AtomicU8::new(initial_stage)),
            page_current: AtomicU8::new(0),
            page_total: AtomicU8::new(0),
        }
    }

    /// Get the inner `StageTracker` to pass to `DocumentProcessor`.
    /// The processor writes stage transitions; the queue worker reads them.
    pub fn stage_tracker(&self) -> StageTracker {
        self.stage.clone()
    }
}

/// Map stage constant to frontend-expected stage name.
pub fn stage_name(stage: u8) -> &'static str {
    match stage {
        STAGE_IMPORTING => "importing",
        STAGE_EXTRACTING => "extracting",
        STAGE_STRUCTURING => "structuring",
        _ => "importing",
    }
}

/// §22: Canonical progress percentage range per stage (min, max).
/// Single source of truth — used by both processor and StageWatcher.
///
/// Extraction and structuring each get 40% width (roughly equal processing time
/// for vision-based workflows). Importing is instant (5% width).
pub fn stage_pct_range(stage: u8) -> (u8, u8) {
    match stage {
        STAGE_IMPORTING => (5, 10),
        STAGE_EXTRACTING => (10, 50),
        STAGE_STRUCTURING => (50, 90),
        _ => (5, 10),
    }
}

// ---------------------------------------------------------------------------
// Orchestrator
// ---------------------------------------------------------------------------

/// Minimum text length for a page to be worth structuring.
/// Pages below this threshold are blank separators or near-empty content.
/// Matches structuring/orchestrator.rs MIN_INPUT_LENGTH.
const MIN_PAGE_TEXT_LENGTH: usize = 10;

/// Orchestrates document processing: import → extract → structure.
///
/// Pure pipeline logic with trait-based DI. Does NOT perform IPC or Tauri
/// event emission — that responsibility belongs to the command layer.
pub struct DocumentProcessor {
    extractor: Box<dyn TextExtractor + Send + Sync>,
    structurer: Box<dyn MedicalStructurer + Send + Sync>,
    /// Stage tracker — shared atomic for stage transitions.
    /// Used by both IPC commands (ProgressMonitor) and queue worker (StageWatcher).
    stage_tracker: Option<StageTracker>,
    /// §22: Optional page-level progress tracker (queue worker path only).
    /// When set, the processor writes page_current/page_total during structuring.
    progress_tracker: Option<Arc<ProgressTracker>>,
    /// §22 G4: Hook called between extraction and structuring stages.
    /// Used by CPU swap strategy to unload vision model and warm LLM model.
    /// Returns Result so model swap failures propagate as clear error messages.
    between_stages_fn: Option<Box<dyn Fn() -> Result<(), String> + Send + Sync>>,
    /// §21 Fix C: Cooperative cancellation token shared with import queue.
    /// Set to `true` when user cancels; checked at processing checkpoints.
    cancellation_token: Option<Arc<AtomicBool>>,
}

impl DocumentProcessor {
    pub fn new(
        extractor: Box<dyn TextExtractor + Send + Sync>,
        structurer: Box<dyn MedicalStructurer + Send + Sync>,
    ) -> Self {
        Self {
            extractor,
            structurer,
            stage_tracker: None,
            progress_tracker: None,
            between_stages_fn: None,
            cancellation_token: None,
        }
    }

    /// Set a shared stage tracker for progress reporting.
    /// The command layer (ProgressMonitor) reads this from a heartbeat thread.
    pub fn set_stage_tracker(&mut self, tracker: StageTracker) {
        self.stage_tracker = Some(tracker);
    }

    /// §22: Set a work-based progress tracker (queue worker path).
    /// Also sets the stage tracker from the ProgressTracker's inner stage atomic.
    pub fn set_progress_tracker(&mut self, tracker: Arc<ProgressTracker>) {
        self.stage_tracker = Some(tracker.stage_tracker());
        self.progress_tracker = Some(tracker);
    }

    /// §22 G4: Set a hook called between extraction and structuring.
    /// Used by CPU swap strategy to unload vision model and warm LLM.
    /// Returns Result so model swap failures propagate clearly.
    pub fn set_between_stages_hook(&mut self, f: Box<dyn Fn() -> Result<(), String> + Send + Sync>) {
        self.between_stages_fn = Some(f);
    }

    /// §21 Fix C: Set a cooperative cancellation token.
    /// The import queue sets this to `true` when the user cancels.
    pub fn set_cancellation_token(&mut self, token: Arc<AtomicBool>) {
        self.cancellation_token = Some(token);
    }

    /// §21 Fix C: Check if cancellation has been requested.
    /// Returns `Err(ProcessingError::Cancelled)` if the token is set.
    fn check_cancellation(&self) -> Result<(), ProcessingError> {
        if let Some(ref token) = self.cancellation_token {
            if token.load(Ordering::Relaxed) {
                return Err(ProcessingError::Cancelled);
            }
        }
        Ok(())
    }

    /// Full pipeline from a source file path.
    ///
    /// 1. Import (format detect, hash, dedup, stage, DB insert)
    /// 2. If staged → extract text (OCR / PDF / plaintext)
    /// 3. If extracted → structure with LLM (entities, markdown)
    /// 4. Update document OCR confidence in DB
    ///
    /// Returns `ProcessingOutput` containing a summary for the frontend
    /// and the raw `StructuringResult` for persistence (pending review).
    pub fn process_file(
        &self,
        source_path: &Path,
        session: &ProfileSession,
        conn: &Connection,
    ) -> Result<ProcessingOutput, ProcessingError> {
        // Step 1: Import
        let import = import_file(source_path, session, conn)?;

        if import.status != ImportStatus::Staged {
            return Ok(ProcessingOutput {
                outcome: ProcessingOutcome {
                    document_id: import.document_id,
                    original_filename: import.original_filename,
                    import_status: import.status,
                    extraction: None,
                    structuring: None,
                },
                structuring_result: None,
            });
        }

        // Steps 2-4: extraction + structuring
        let (extraction_summary, structuring_summary, structuring_result) =
            match self.extract_and_structure(&import, session, conn) {
                Ok(result) => result,
                Err(e) => {
                    // O.5: Mark document as Failed on processing error
                    if let Err(status_err) = repository::update_pipeline_status(
                        conn,
                        &import.document_id,
                        &PipelineStatus::Failed,
                    ) {
                        tracing::warn!(
                            document_id = %import.document_id,
                            error = %status_err,
                            "Failed to set pipeline status to Failed"
                        );
                    }
                    return Err(e);
                }
            };

        Ok(ProcessingOutput {
            outcome: ProcessingOutcome {
                document_id: import.document_id,
                original_filename: import.original_filename,
                import_status: import.status,
                extraction: Some(extraction_summary),
                structuring: Some(structuring_summary),
            },
            structuring_result: Some(structuring_result),
        })
    }

    /// Process an already-imported document (e.g., from WiFi transfer).
    ///
    /// Same as steps 2-4 of `process_file`, but takes an existing
    /// `ImportResult` instead of a source path.
    pub fn process_imported(
        &self,
        import: &ImportResult,
        session: &ProfileSession,
        conn: &Connection,
    ) -> Result<ProcessingOutput, ProcessingError> {
        if import.status != ImportStatus::Staged {
            return Ok(ProcessingOutput {
                outcome: ProcessingOutcome {
                    document_id: import.document_id,
                    original_filename: import.original_filename.clone(),
                    import_status: import.status.clone(),
                    extraction: None,
                    structuring: None,
                },
                structuring_result: None,
            });
        }

        let (extraction_summary, structuring_summary, structuring_result) =
            match self.extract_and_structure(import, session, conn) {
                Ok(result) => result,
                Err(e) => {
                    // O.5: Mark document as Failed on processing error
                    if let Err(status_err) = repository::update_pipeline_status(
                        conn,
                        &import.document_id,
                        &PipelineStatus::Failed,
                    ) {
                        tracing::warn!(
                            document_id = %import.document_id,
                            error = %status_err,
                            "Failed to set pipeline status to Failed"
                        );
                    }
                    return Err(e);
                }
            };

        Ok(ProcessingOutput {
            outcome: ProcessingOutcome {
                document_id: import.document_id,
                original_filename: import.original_filename.clone(),
                import_status: import.status.clone(),
                extraction: Some(extraction_summary),
                structuring: Some(structuring_summary),
            },
            structuring_result: Some(structuring_result),
        })
    }

    /// Shared extraction + structuring logic.
    fn extract_and_structure(
        &self,
        import: &ImportResult,
        session: &ProfileSession,
        conn: &Connection,
    ) -> Result<(ExtractionSummary, StructuringSummary, StructuringResult), ProcessingError> {
        let staged_path = Path::new(&import.staged_path);

        // §21 Fix C: Checkpoint 1 — before extraction starts
        self.check_cancellation()?;

        // Update stage tracker → Extracting
        if let Some(ref tracker) = self.stage_tracker {
            tracker.store(STAGE_EXTRACTING, Ordering::Relaxed);
        }

        // O.5: Update pipeline status → Extracting
        if let Err(e) = repository::update_pipeline_status(
            conn,
            &import.document_id,
            &PipelineStatus::Extracting,
        ) {
            tracing::warn!(
                document_id = %import.document_id,
                error = %e,
                "Failed to set pipeline status to Extracting"
            );
        }

        // Step 2: Extract text
        tracing::info!(
            document_id = %import.document_id,
            "Processing: starting extraction"
        );
        // §22: Pass progress tracker to extractor for page-level extraction progress
        let progress_ref = self.progress_tracker.as_deref();
        let extraction = self
            .extractor
            .extract(&import.document_id, staged_path, &import.format, session, progress_ref)?;

        let extraction_summary = ExtractionSummary {
            method: format!("{:?}", extraction.method),
            confidence: extraction.overall_confidence,
            page_count: extraction.page_count,
            text_length: extraction.full_text.len(),
        };

        // Step 3: Update OCR confidence + page count in DB
        if let Err(e) =
            update_ocr_confidence(conn, &import.document_id, extraction.overall_confidence)
        {
            tracing::warn!(
                document_id = %import.document_id,
                error = %e,
                "Failed to update OCR confidence — continuing"
            );
        }
        if let Err(e) = repository::update_document_page_count(
            conn,
            &import.document_id,
            extraction.page_count as u32,
        ) {
            tracing::warn!(
                document_id = %import.document_id,
                error = %e,
                "Failed to update page count — continuing"
            );
        }

        // §21 Fix C: Checkpoint 2 — between extraction and structuring
        self.check_cancellation()?;

        // §22 G4: CPU swap between stages — errors now propagate
        if let Some(ref hook) = self.between_stages_fn {
            hook().map_err(|e| {
                ProcessingError::Extraction(ExtractionError::OcrProcessing(
                    format!("Model swap failed: {e}"),
                ))
            })?;
        }

        // Update stage tracker → Structuring
        if let Some(ref tracker) = self.stage_tracker {
            tracker.store(STAGE_STRUCTURING, Ordering::Relaxed);
        }

        // O.5: Update pipeline status → Structuring
        if let Err(e) = repository::update_pipeline_status(
            conn,
            &import.document_id,
            &PipelineStatus::Structuring,
        ) {
            tracing::warn!(
                document_id = %import.document_id,
                error = %e,
                "Failed to set pipeline status to Structuring"
            );
        }

        // Diagnostic dump directory (reuse same dir as extraction)
        let dump_dir = diagnostic::dump_dir_for(&import.document_id);

        // R4: Per-page structuring loop (D2 — page is the atomic unit)
        let total_pages = extraction.pages.len();
        let mut page_results: Vec<StructuringResult> = Vec::with_capacity(total_pages);

        // §22: Set page counters for work-based structuring progress
        if let Some(ref tracker) = self.progress_tracker {
            tracker.page_current.store(0, Ordering::Relaxed);
            tracker.page_total.store(total_pages.min(255) as u8, Ordering::Relaxed);
        }

        for (idx, page) in extraction.pages.iter().enumerate() {
            // §21 Fix C: Checkpoint 3 — per-page cancellation check
            self.check_cancellation()?;

            // Skip blank/short pages
            if page.text.trim().len() < MIN_PAGE_TEXT_LENGTH {
                tracing::debug!(
                    document_id = %import.document_id,
                    page = page.page_number,
                    text_len = page.text.trim().len(),
                    "Skipping short page"
                );
                continue;
            }

            tracing::info!(
                document_id = %import.document_id,
                page = page.page_number,
                total = total_pages,
                "Structuring page {}/{}",
                page.page_number, total_pages
            );

            if let Some(ref dir) = dump_dir {
                diagnostic::dump_text(dir, &format!("05-structuring-input-page-{idx}.txt"), &page.text);
            }

            // 12-ERC B3: Direct entity path — bypass structurer LLM when drill entities available
            if let Some(drill_output) = page.drill_output.clone() {
                tracing::info!(
                    document_id = %import.document_id,
                    page = page.page_number,
                    total = total_pages,
                    "Direct entity path — bypassing structurer LLM for page {}/{}",
                    page.page_number, total_pages
                );
                let result = post_process_drill_output(
                    &import.document_id,
                    drill_output,
                    page.confidence,
                );
                if let Some(ref dir) = dump_dir {
                    diagnostic::dump_json(
                        dir,
                        &format!("05-structuring-result-page-{idx}.json"),
                        &result,
                    );
                }
                page_results.push(result);
            } else {
                // Text-only path — structurer LLM (existing fault-tolerant match)
                match self.structurer.structure_document(
                    &import.document_id,
                    &page.text,
                    page.confidence,
                    session,
                ) {
                    Ok(result) => {
                        if let Some(ref dir) = dump_dir {
                            diagnostic::dump_json(dir, &format!("05-structuring-result-page-{idx}.json"), &result);
                        }
                        page_results.push(result);
                    }
                    Err(e) => {
                        if let Some(ref dir) = dump_dir {
                            diagnostic::dump_json(dir, &format!("05-structuring-error-page-{idx}.json"), &serde_json::json!({
                                "error": e.to_string(),
                                "page": page.page_number,
                            }));
                        }
                        tracing::warn!(
                            document_id = %import.document_id,
                            page = page.page_number,
                            error = %e,
                            "Page structuring failed — continuing to next page"
                        );
                    }
                }
            }

            // §22: Update page counter for work-based progress
            if let Some(ref tracker) = self.progress_tracker {
                tracker.page_current.store((idx + 1).min(255) as u8, Ordering::Relaxed);
            }
        }

        // All pages failed?
        if page_results.is_empty() {
            return Err(ProcessingError::Structuring(
                StructuringError::InputTooShort,
            ));
        }

        // R4: Merge per-page results (D3)
        let pages_processed = page_results.len();
        let merged = merge_page_results(&import.document_id, page_results);

        if let Some(ref dir) = dump_dir {
            diagnostic::dump_json(dir, "06-final-result.json", &merged);
        }

        let structuring_summary = StructuringSummary {
            document_type: merged.document_type.as_str().to_string(),
            confidence: merged.structuring_confidence,
            entities_count: count_entities(&merged.extracted_entities),
            has_professional: merged.professional.is_some(),
            document_date: merged.document_date.map(|d| d.to_string()),
        };

        tracing::info!(
            document_id = %import.document_id,
            pages_processed = pages_processed,
            pages_total = total_pages,
            document_type = merged.document_type.as_str(),
            entities = structuring_summary.entities_count,
            "Processing complete (per-page)"
        );

        Ok((extraction_summary, structuring_summary, merged))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn count_entities(e: &crate::pipeline::structuring::types::ExtractedEntities) -> usize {
    e.medications.len()
        + e.lab_results.len()
        + e.diagnoses.len()
        + e.allergies.len()
        + e.procedures.len()
        + e.referrals.len()
        + e.instructions.len()
}

fn update_ocr_confidence(
    conn: &Connection,
    document_id: &Uuid,
    confidence: f32,
) -> Result<(), ProcessingError> {
    let mut doc = repository::get_document(conn, document_id)
        .map_err(|e| ProcessingError::Database(e.to_string()))?
        .ok_or_else(|| {
            ProcessingError::Database(format!("Document not found: {document_id}"))
        })?;

    doc.ocr_confidence = Some(confidence);
    repository::update_document(conn, &doc)
        .map_err(|e| ProcessingError::Database(e.to_string()))
}

// ---------------------------------------------------------------------------
// R4: Per-page merge (D3)
// ---------------------------------------------------------------------------

/// Merge per-page structuring results into a single document-level result.
///
/// Single-page input returns the page result directly (no merge overhead).
///
/// Merge rules:
/// - Entities: concatenate all, deduplicate by type + normalized value
/// - Document type: first non-Other found (page scan order)
/// - Document date: earliest date across pages
/// - Professional: first non-None found
/// - Markdown: concatenate with page break separators
/// - Confidence: weighted average (weight = markdown length as text-size proxy)
/// - Warnings: union of all per-page warnings
fn merge_page_results(
    document_id: &Uuid,
    page_results: Vec<StructuringResult>,
) -> StructuringResult {
    // Fast path: single page — no allocation, no cloning
    if page_results.len() == 1 {
        return page_results.into_iter().next().unwrap();
    }

    // 1. Collect all entities
    let mut all_entities = ExtractedEntities::default();
    for result in &page_results {
        let e = &result.extracted_entities;
        all_entities.medications.extend(e.medications.iter().cloned());
        all_entities.lab_results.extend(e.lab_results.iter().cloned());
        all_entities.diagnoses.extend(e.diagnoses.iter().cloned());
        all_entities.allergies.extend(e.allergies.iter().cloned());
        all_entities.procedures.extend(e.procedures.iter().cloned());
        all_entities.referrals.extend(e.referrals.iter().cloned());
        all_entities.instructions.extend(e.instructions.iter().cloned());
    }

    // 2. Deduplicate entities
    dedup_medications(&mut all_entities.medications);
    dedup_lab_results(&mut all_entities.lab_results);
    dedup_diagnoses(&mut all_entities.diagnoses);
    dedup_allergies(&mut all_entities.allergies);
    dedup_procedures(&mut all_entities.procedures);
    dedup_referrals(&mut all_entities.referrals);
    dedup_instructions(&mut all_entities.instructions);

    // 3. Document type: first non-Other
    let document_type = page_results
        .iter()
        .map(|r| &r.document_type)
        .find(|t| **t != DocumentType::Other)
        .cloned()
        .unwrap_or(DocumentType::Other);

    // 4. Document date: earliest
    let document_date = page_results.iter().filter_map(|r| r.document_date).min();

    // 5. Professional: first non-None
    let professional = page_results.iter().find_map(|r| r.professional.clone());

    // 6. Markdown: concatenate with page breaks
    let structured_markdown = page_results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            if i == 0 {
                r.structured_markdown.clone()
            } else {
                format!("\n\n--- Page {} ---\n\n{}", i + 1, r.structured_markdown)
            }
        })
        .collect::<Vec<_>>()
        .join("");

    // 7. Confidence: weighted average (weight = markdown length as text-size proxy)
    let total_weight: f32 = page_results
        .iter()
        .map(|r| r.structured_markdown.len() as f32)
        .sum();
    let weighted_confidence = if total_weight > 0.0 {
        page_results
            .iter()
            .map(|r| r.structuring_confidence * r.structured_markdown.len() as f32)
            .sum::<f32>()
            / total_weight
    } else {
        page_results
            .iter()
            .map(|r| r.structuring_confidence)
            .sum::<f32>()
            / page_results.len() as f32
    };

    // 8. Warnings: union
    let validation_warnings: Vec<String> = page_results
        .iter()
        .flat_map(|r| r.validation_warnings.iter().cloned())
        .collect();

    StructuringResult {
        document_id: *document_id,
        document_type,
        document_date,
        professional,
        structured_markdown,
        extracted_entities: all_entities,
        structuring_confidence: weighted_confidence,
        markdown_file_path: None, // Set by command layer
        validation_warnings,
        raw_llm_response: None, // Per-page responses in tracing logs
    }
}

// ---------------------------------------------------------------------------
// R4: Entity deduplication helpers (D3)
// ---------------------------------------------------------------------------

/// Normalize text for dedup comparison: lowercase, collapse whitespace, trim.
fn normalize_for_dedup(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn dedup_medications(meds: &mut Vec<ExtractedMedication>) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut result: Vec<ExtractedMedication> = Vec::new();

    for med in meds.drain(..) {
        let name_key = normalize_for_dedup(
            med.generic_name
                .as_deref()
                .or(med.brand_name.as_deref())
                .unwrap_or(""),
        );
        let key = format!("{}|{}", name_key, normalize_for_dedup(&med.dose));

        if let Some(&idx) = seen.get(&key) {
            if med.confidence > result[idx].confidence {
                result[idx] = med;
            }
        } else {
            seen.insert(key, result.len());
            result.push(med);
        }
    }

    *meds = result;
}

fn dedup_lab_results(labs: &mut Vec<ExtractedLabResult>) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut result: Vec<ExtractedLabResult> = Vec::new();

    for lab in labs.drain(..) {
        let value_key = lab
            .value_text
            .as_deref()
            .map(|v| normalize_for_dedup(v))
            .or_else(|| lab.value.map(|v| v.to_string()))
            .unwrap_or_default();
        let key = format!("{}|{}", normalize_for_dedup(&lab.test_name), value_key);

        if let Some(&idx) = seen.get(&key) {
            if lab.confidence > result[idx].confidence {
                result[idx] = lab;
            }
        } else {
            seen.insert(key, result.len());
            result.push(lab);
        }
    }

    *labs = result;
}

fn dedup_diagnoses(diags: &mut Vec<ExtractedDiagnosis>) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut result: Vec<ExtractedDiagnosis> = Vec::new();

    for diag in diags.drain(..) {
        let key = normalize_for_dedup(&diag.name);

        if let Some(&idx) = seen.get(&key) {
            if diag.confidence > result[idx].confidence {
                result[idx] = diag;
            }
        } else {
            seen.insert(key, result.len());
            result.push(diag);
        }
    }

    *diags = result;
}

fn dedup_allergies(allergies: &mut Vec<ExtractedAllergy>) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut result: Vec<ExtractedAllergy> = Vec::new();

    for allergy in allergies.drain(..) {
        let key = normalize_for_dedup(&allergy.allergen);

        if let Some(&idx) = seen.get(&key) {
            if allergy.confidence > result[idx].confidence {
                result[idx] = allergy;
            }
        } else {
            seen.insert(key, result.len());
            result.push(allergy);
        }
    }

    *allergies = result;
}

fn dedup_procedures(procs: &mut Vec<ExtractedProcedure>) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut result: Vec<ExtractedProcedure> = Vec::new();

    for proc in procs.drain(..) {
        let key = format!(
            "{}|{}",
            normalize_for_dedup(&proc.name),
            proc.date.as_deref().unwrap_or("")
        );

        if let Some(&idx) = seen.get(&key) {
            if proc.confidence > result[idx].confidence {
                result[idx] = proc;
            }
        } else {
            seen.insert(key, result.len());
            result.push(proc);
        }
    }

    *procs = result;
}

fn dedup_referrals(refs: &mut Vec<ExtractedReferral>) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut result: Vec<ExtractedReferral> = Vec::new();

    for referral in refs.drain(..) {
        let key = format!(
            "{}|{}",
            normalize_for_dedup(&referral.referred_to),
            normalize_for_dedup(referral.specialty.as_deref().unwrap_or(""))
        );

        if let Some(&idx) = seen.get(&key) {
            if referral.confidence > result[idx].confidence {
                result[idx] = referral;
            }
        } else {
            seen.insert(key, result.len());
            result.push(referral);
        }
    }

    *refs = result;
}

fn dedup_instructions(insts: &mut Vec<ExtractedInstruction>) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut result: Vec<ExtractedInstruction> = Vec::new();

    for inst in insts.drain(..) {
        let key = normalize_for_dedup(&inst.text);

        if seen.contains_key(&key) {
            // Instructions: keep first occurrence (no confidence field)
            continue;
        }
        seen.insert(key, result.len());
        result.push(inst);
    }

    *insts = result;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/// CT-01: Build a `DocumentProcessor` from a `PipelineAssignment`.
///
/// Dispatches on `ExtractionStrategy`:
/// - `VisionOcr { model }` → full vision pipeline (Classify → IterativeDrill)
/// - `PdfiumText` / `DirectText` → text-only extractor (no vision model needed)
///
/// The structuring model comes from `assignment.structuring_model`.
pub fn build_processor_from_assignment(
    assignment: &crate::pipeline::model_router::PipelineAssignment,
    config: &crate::pipeline_config::PipelineConfig,
    language: &str,
) -> Result<DocumentProcessor, ProcessingError> {
    build_processor_from_assignment_with_vision(assignment, config, language, false, None)
}

/// C4-FIX + 09-CAE: Build a `DocumentProcessor` with vision extraction config.
///
/// When `has_gpu` is true, the VisionSession uses GPU-optimized strategy.
/// UC-01: When `user_doc_type` is Some, uses `UserProvidedClassifier`
/// instead of the LLM-based `OllamaVisionClassifier`.
/// 09-CAE: `UserDocumentType` is threaded to `DocumentExtractor` for domain
/// filtering and category-aware prompts (not just classifier bypass).
pub fn build_processor_from_assignment_with_vision(
    assignment: &crate::pipeline::model_router::PipelineAssignment,
    config: &crate::pipeline_config::PipelineConfig,
    language: &str,
    has_gpu: bool,
    user_doc_type: Option<crate::pipeline::extraction::vision_classifier::UserDocumentType>,
) -> Result<DocumentProcessor, ProcessingError> {
    use crate::pipeline::model_router::ExtractionStrategy;

    let extractor: Box<dyn TextExtractor + Send + Sync> = match &assignment.extraction {
        ExtractionStrategy::VisionOcr { model } => {
            use crate::butler_service::FallbackSession;
            use crate::ollama_service::OllamaService;
            use crate::pipeline::extraction::pdfium::PdfiumRenderer;
            use crate::pipeline::extraction::preprocess::{ImagePreprocessor, PreprocessingPipeline};
            // 10-LDC: build_system_prompt removed — IterativeDrill uses PromptLocale.system_prompt
            use crate::pipeline::extraction::vision_ocr::OllamaMedicalImageInterpreter;
            use crate::pipeline::strategy::ContextType;

            let pdf_renderer = PdfiumRenderer::new()
                .map_err(|e| ProcessingError::OcrInit(format!("PDFium init failed: {e}")))?;

            // Shared vision client for classification, drill, and interpretation
            let mut vision_client = OllamaService::client();
            vision_client.set_vision_num_ctx(config.num_ctx);
            let vision_client: Arc<dyn crate::pipeline::structuring::types::VisionClient> =
                Arc::new(vision_client);

            // UC-01 + 09-CAE: Use user-provided classifier when available, else LLM classifier
            let classifier: Box<dyn crate::pipeline::extraction::vision_classifier::VisionClassifier> =
                if let Some(dt) = user_doc_type {
                    use crate::pipeline::extraction::vision_classifier::UserProvidedClassifier;
                    Box::new(UserProvidedClassifier::new(dt))
                } else {
                    use crate::pipeline::extraction::vision_classifier::OllamaVisionClassifier;
                    Box::new(OllamaVisionClassifier::new(Arc::clone(&vision_client), model.clone()))
                };

            // C4-FIX: VisionSession for IterativeDrill (primary extraction)
            let session = FallbackSession::new(model, ContextType::VisionOcr, has_gpu);

            // Drill client (separate instance to avoid shared mutable state)
            let mut drill_client = OllamaService::client();
            drill_client.set_vision_num_ctx(config.num_ctx);

            let interpreter = Box::new(OllamaMedicalImageInterpreter::new(
                Arc::clone(&vision_client),
                model.clone(),
            ));
            let preprocessor: Box<dyn ImagePreprocessor> =
                Box::new(PreprocessingPipeline::medgemma_gpu());

            let mut doc_extractor = DocumentExtractor::new(
                Box::new(pdf_renderer),
                classifier,
                Box::new(session),
                Box::new(drill_client),
                preprocessor,
            )
            .with_interpreter(interpreter)
            .with_language(language);

            // 09-CAE: Thread UserDocumentType for domain filtering + category prompts
            if let Some(dt) = user_doc_type {
                doc_extractor = doc_extractor.with_user_document_type(dt);
            }

            Box::new(doc_extractor)
        }
        ExtractionStrategy::PdfiumText | ExtractionStrategy::DirectText => {
            Box::new(crate::pipeline::extraction::text_only::PlainTextExtractor)
        }
    };

    // LLM structuring — strategy-aware (STR-01)
    let mut structuring_opts = crate::pipeline::structuring::ollama_types::GenerationOptions::default();
    structuring_opts.num_ctx = Some(config.num_ctx);
    let structuring_client = crate::ollama_service::OllamaService::client().with_options(structuring_opts);

    // STR-01: Resolve extraction strategy from PipelineAssignment.
    // Strategy must be resolved by caller (import.rs) before building processor.
    let prompt_strategy = assignment.prompt_strategy.clone().unwrap_or_else(|| {
        tracing::warn!("STR-01: prompt_strategy not set, falling back to MarkdownList");
        crate::pipeline::strategy::resolve_strategy(
            crate::pipeline::strategy::ContextType::DocumentExtraction,
            crate::pipeline::strategy::detect_model_variant(&assignment.structuring_model),
        )
    });
    let extraction_strategy = crate::pipeline::structuring::extraction_strategy::build_strategy(&prompt_strategy);

    tracing::info!(
        extraction = ?assignment.extraction,
        structuring_model = %assignment.structuring_model,
        processing_mode = ?assignment.processing_mode,
        strategy = extraction_strategy.name(),
        "STR-01: Document processor built with strategy"
    );
    let structurer = Box::new(DocumentStructurer::new(
        Box::new(structuring_client),
        &assignment.structuring_model,
        extraction_strategy,
    ));

    Ok(DocumentProcessor::new(extractor, structurer))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use chrono::NaiveDate;

    use crate::butler_service::FallbackSession;
    use crate::crypto::profile;
    use crate::db::sqlite::open_database;
    use crate::pipeline::extraction::pdfium::MockPdfPageRenderer;
    use crate::pipeline::extraction::preprocess::MockImagePreprocessor;
    use crate::pipeline::extraction::vision_classifier::MockVisionClassifier;
    use crate::pipeline::strategy::ContextType;
    use crate::pipeline::structuring::extraction_strategy::{MockExtractionStrategy, StrategyOutput};
    use crate::pipeline::structuring::ollama::{MockLlmClient, MockVisionClient};
    use crate::pipeline::structuring::types::{
        ExtractedDiagnosis, ExtractedEntities, ExtractedMedication, ExtractedProfessional,
    };

    // -- Helpers -----------------------------------------------------------

    fn test_session() -> (tempfile::TempDir, ProfileSession) {
        let dir = tempfile::tempdir().unwrap();
        let (info, _phrase) =
            profile::create_profile(dir.path(), "ProcessorTest", "test_pass_123", None, None, None, None).unwrap();
        let session = profile::open_profile(dir.path(), &info.id, "test_pass_123").unwrap();
        (dir, session)
    }

    /// Standard strategy output for processor tests.
    fn mock_strategy_output() -> StrategyOutput {
        let mut entities = ExtractedEntities::default();
        entities.medications.push(ExtractedMedication {
            generic_name: Some("Metformin".into()),
            brand_name: Some("Glucophage".into()),
            dose: "500mg".into(),
            frequency: "twice daily".into(),
            frequency_type: "scheduled".into(),
            route: "oral".into(),
            reason: Some("Type 2 diabetes".into()),
            instructions: vec!["Take with food".into()],
            is_compound: false,
            compound_ingredients: vec![],
            tapering_steps: vec![],
            max_daily_dose: Some("2000mg".into()),
            condition: Some("Type 2 diabetes".into()),
            confidence: 0.92,
        });
        entities.diagnoses.push(ExtractedDiagnosis {
            name: "Type 2 Diabetes".into(),
            icd_code: Some("E11".into()),
            date: Some("2024-01-15".into()),
            status: "active".into(),
            confidence: 0.90,
        });

        StrategyOutput {
            entities,
            markdown: "# Prescription — Dr. Chen, GP\n**Date:** January 15, 2024\n\n## Medications\n- **Metformin (Glucophage)** 500mg — twice daily, oral\n  - Take with food\n  - For: Type 2 diabetes".into(),
            document_type: Some("prescription".into()),
            document_date: Some("2024-01-15".into()),
            professional: Some(ExtractedProfessional {
                name: "Dr. Chen".into(),
                specialty: Some("GP".into()),
                institution: None,
            }),
            raw_responses: vec!["mock strategy response".into()],
        }
    }

    /// Build a DocumentExtractor with mock classifier + drill mocks for processor tests.
    fn build_test_extractor(page_count: usize) -> DocumentExtractor {
        let session = FallbackSession::new("medgemma:4b", ContextType::VisionOcr, false);
        DocumentExtractor::new(
            Box::new(MockPdfPageRenderer::new(page_count)),
            Box::new(MockVisionClassifier::document()),
            Box::new(session),
            Box::new(MockVisionClient::new("mock drill response")),
            Box::new(MockImagePreprocessor::new()),
        )
    }

    fn build_test_processor() -> DocumentProcessor {
        let extractor = Box::new(build_test_extractor(1));

        let strategy = Box::new(MockExtractionStrategy::with_output(mock_strategy_output()));
        let llm = Box::new(MockLlmClient::new("unused"));
        let structurer = Box::new(DocumentStructurer::new(llm, "medgemma:latest", strategy));

        DocumentProcessor::new(extractor, structurer)
    }

    fn create_test_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    // -- Tests -------------------------------------------------------------

    #[test]
    fn process_text_file_full_pipeline() {
        let (_dir, session) = test_session();
        let conn = open_database(session.db_path(), Some(session.key_bytes())).unwrap();
        let processor = build_test_processor();

        let tmp = tempfile::tempdir().unwrap();
        let file = create_test_file(
            tmp.path(),
            "prescription.txt",
            "Metformin 500mg twice daily for type 2 diabetes management. \
             Dr. Chen prescribed this on January 15, 2024.",
        );

        let output = processor.process_file(&file, &session, &conn).unwrap();

        assert_eq!(output.outcome.import_status, ImportStatus::Staged);
        assert!(output.outcome.extraction.is_some());
        assert!(output.outcome.structuring.is_some());
        assert!(output.structuring_result.is_some());

        let ext = output.outcome.extraction.unwrap();
        assert!(ext.confidence > 0.9);
        assert!(ext.text_length > 0);
        assert_eq!(ext.page_count, 1);

        let stru = output.outcome.structuring.unwrap();
        assert_eq!(stru.document_type, "prescription");
        assert!(stru.entities_count >= 2); // 1 med + 1 diagnosis
        assert!(stru.has_professional);
    }

    #[test]
    fn process_unsupported_file_skips_extraction() {
        let (_dir, session) = test_session();
        let conn = open_database(session.db_path(), Some(session.key_bytes())).unwrap();
        let processor = build_test_processor();

        let tmp = tempfile::tempdir().unwrap();
        // .exe magic bytes → unsupported format
        let file = create_test_file(tmp.path(), "program.exe", "MZ\x00\x00");

        let output = processor.process_file(&file, &session, &conn).unwrap();

        assert_eq!(output.outcome.import_status, ImportStatus::Unsupported);
        assert!(output.outcome.extraction.is_none());
        assert!(output.outcome.structuring.is_none());
        assert!(output.structuring_result.is_none());
    }

    #[test]
    fn process_duplicate_file_skips_extraction() {
        let (_dir, session) = test_session();
        let conn = open_database(session.db_path(), Some(session.key_bytes())).unwrap();
        let processor = build_test_processor();

        let tmp = tempfile::tempdir().unwrap();
        let content = "Exact same content for duplicate detection test. \
                        This must be long enough to pass format detection.";
        let file1 = create_test_file(tmp.path(), "report1.txt", content);
        let file2 = create_test_file(tmp.path(), "report2.txt", content);

        // First import succeeds
        let output1 = processor.process_file(&file1, &session, &conn).unwrap();
        assert_eq!(output1.outcome.import_status, ImportStatus::Staged);

        // Second import detected as duplicate
        let output2 = processor.process_file(&file2, &session, &conn).unwrap();
        assert_eq!(output2.outcome.import_status, ImportStatus::Duplicate);
        assert!(output2.outcome.extraction.is_none());
    }

    #[test]
    fn process_file_updates_ocr_confidence() {
        let (_dir, session) = test_session();
        let conn = open_database(session.db_path(), Some(session.key_bytes())).unwrap();
        let processor = build_test_processor();

        let tmp = tempfile::tempdir().unwrap();
        let file = create_test_file(
            tmp.path(),
            "lab_results.txt",
            "HbA1c: 7.2% — above target of 7.0%. Follow up in 3 months.",
        );

        let output = processor.process_file(&file, &session, &conn).unwrap();
        let doc_id = output.outcome.document_id;

        // Verify OCR confidence was written to DB
        let doc = repository::get_document(&conn, &doc_id).unwrap().unwrap();
        assert!(doc.ocr_confidence.is_some());
        assert!(doc.ocr_confidence.unwrap() > 0.9); // plaintext = 0.99
    }

    #[test]
    fn process_imported_document() {
        let (_dir, session) = test_session();
        let conn = open_database(session.db_path(), Some(session.key_bytes())).unwrap();

        // First: import only
        let tmp = tempfile::tempdir().unwrap();
        let file = create_test_file(
            tmp.path(),
            "notes.txt",
            "Patient presents with fatigue. Ordered CBC and metabolic panel.",
        );
        let import_result = import_file(&file, &session, &conn).unwrap();
        assert_eq!(import_result.status, ImportStatus::Staged);

        // Then: process the imported document
        let processor = build_test_processor();
        let output = processor
            .process_imported(&import_result, &session, &conn)
            .unwrap();

        assert_eq!(output.outcome.import_status, ImportStatus::Staged);
        assert!(output.outcome.extraction.is_some());
        assert!(output.outcome.structuring.is_some());
    }

    #[test]
    fn between_stages_hook_is_called() {
        let (_dir, session) = test_session();
        let conn = open_database(session.db_path(), Some(session.key_bytes())).unwrap();

        let mut processor = build_test_processor();
        let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let called_clone = called.clone();
        processor.set_between_stages_hook(Box::new(move || {
            called_clone.store(true, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        }));

        let tmp = tempfile::tempdir().unwrap();
        let file = create_test_file(
            tmp.path(),
            "hook_test.txt",
            "Metformin 500mg twice daily for type 2 diabetes.",
        );

        let output = processor.process_file(&file, &session, &conn).unwrap();
        assert_eq!(output.outcome.import_status, ImportStatus::Staged);
        assert!(
            called.load(std::sync::atomic::Ordering::Relaxed),
            "Between-stages hook should have been called"
        );
    }

    #[test]
    fn count_entities_helper() {
        let entities = ExtractedEntities::default();
        assert_eq!(count_entities(&entities), 0);
    }

    #[test]
    fn process_file_updates_pipeline_status() {
        let (_dir, session) = test_session();
        let conn = open_database(session.db_path(), Some(session.key_bytes())).unwrap();
        let processor = build_test_processor();

        let tmp = tempfile::tempdir().unwrap();
        let file = create_test_file(
            tmp.path(),
            "status_test.txt",
            "Metformin 500mg twice daily for type 2 diabetes management.",
        );

        let output = processor.process_file(&file, &session, &conn).unwrap();
        let doc_id = output.outcome.document_id;

        // After successful processing, status should be Structuring (last stage set by processor)
        // The command layer would then set PendingReview, but the processor stops at Structuring.
        let doc = repository::get_document(&conn, &doc_id).unwrap().unwrap();
        assert_eq!(
            doc.pipeline_status,
            PipelineStatus::Structuring,
            "After processing, status should be Structuring (command layer sets PendingReview)"
        );
    }

    #[test]
    fn process_file_sets_failed_on_structuring_error() {
        use crate::pipeline::structuring::types::MedicalStructurer;

        // LLM that returns invalid JSON → structuring will fail
        struct FailingStructurer;
        impl MedicalStructurer for FailingStructurer {
            fn structure_document(
                &self,
                _document_id: &Uuid,
                _text: &str,
                _ocr_confidence: f32,
                _session: &crate::crypto::ProfileSession,
            ) -> Result<StructuringResult, StructuringError> {
                Err(StructuringError::MalformedResponse("Mock LLM failure".into()))
            }
        }

        let (_dir, session) = test_session();
        let conn = open_database(session.db_path(), Some(session.key_bytes())).unwrap();

        let extractor = Box::new(build_test_extractor(1));
        let processor = DocumentProcessor::new(extractor, Box::new(FailingStructurer));

        let tmp = tempfile::tempdir().unwrap();
        let file = create_test_file(tmp.path(), "fail_test.txt", "Some medical content for test.");

        let result = processor.process_file(&file, &session, &conn);
        assert!(result.is_err(), "Processing should fail with bad structurer");

        // The document should have been created (import succeeded) with Failed status
        // Find the document by checking recent documents
        let docs = repository::get_documents_by_pipeline_status(&conn, &PipelineStatus::Failed)
            .unwrap();
        assert_eq!(docs.len(), 1, "Exactly one document should be in Failed status");
    }

    #[test]
    fn processing_outcome_serializes() {
        let outcome = ProcessingOutcome {
            document_id: Uuid::nil(),
            original_filename: "test.txt".into(),
            import_status: ImportStatus::Staged,
            extraction: Some(ExtractionSummary {
                method: "PlainTextRead".into(),
                confidence: 0.99,
                page_count: 1,
                text_length: 100,
            }),
            structuring: Some(StructuringSummary {
                document_type: "prescription".into(),
                confidence: 0.87,
                entities_count: 3,
                has_professional: true,
                document_date: Some("2024-01-15".into()),
            }),
        };

        let json = serde_json::to_string(&outcome).unwrap();
        assert!(json.contains("prescription"));
        assert!(json.contains("PlainTextRead"));
    }

    #[test]
    fn error_category_import_is_unsupported_file() {
        let err = ProcessingError::Import(ImportError::UnsupportedFormat("bad".into()));
        assert_eq!(err.category(), ErrorCategory::UnsupportedFile);
        assert!(!err.is_retryable());
    }

    #[test]
    fn error_category_structuring_connection() {
        let err = ProcessingError::Structuring(StructuringError::OllamaConnection(
            "connection refused".into(),
        ));
        assert_eq!(err.category(), ErrorCategory::AiUnavailable);
        assert!(err.is_retryable());
    }

    #[test]
    fn error_category_structuring_malformed() {
        let err = ProcessingError::Structuring(StructuringError::MalformedResponse(
            "bad json".into(),
        ));
        assert_eq!(err.category(), ErrorCategory::AiOutputError);
        assert!(err.is_retryable());
    }

    #[test]
    fn error_category_serializes_snake_case() {
        let json = serde_json::to_string(&ErrorCategory::AiUnavailable).unwrap();
        assert_eq!(json, "\"ai_unavailable\"");
        let json = serde_json::to_string(&ErrorCategory::OcrUnavailable).unwrap();
        assert_eq!(json, "\"ocr_unavailable\"");
    }

    // -- R.1: PatientError tests --

    #[test]
    fn patient_error_from_unsupported_format() {
        let err = ProcessingError::Import(ImportError::UnsupportedFormat("docx".into()));
        let pe = err.to_patient_error();
        assert_eq!(pe.title, "Unsupported File");
        assert!(!pe.retry_possible);
        assert_eq!(pe.category, ErrorCategory::UnsupportedFile);
    }

    #[test]
    fn patient_error_from_file_too_large() {
        let err = ProcessingError::Import(ImportError::FileTooLarge { size_mb: 25.5, max_mb: 20 });
        let pe = err.to_patient_error();
        assert!(pe.message.contains("25.5"));
        assert!(pe.message.contains("20"));
        assert!(!pe.retry_possible);
    }

    #[test]
    fn patient_error_from_encrypted_pdf() {
        let err = ProcessingError::Import(ImportError::EncryptedPdf);
        let pe = err.to_patient_error();
        assert_eq!(pe.title, "Protected PDF");
        assert!(!pe.retry_possible);
    }

    #[test]
    fn patient_error_from_structuring_delegates() {
        let err = ProcessingError::Structuring(StructuringError::OllamaConnection("localhost".into()));
        let pe = err.to_patient_error();
        // Should delegate to StructuringError::patient_message()
        assert_eq!(pe.title, "AI Service Unavailable");
        assert!(pe.retry_possible);
        assert_eq!(pe.category, ErrorCategory::AiUnavailable);
    }

    #[test]
    fn patient_error_from_extraction() {
        let err = ProcessingError::Extraction(ExtractionError::OcrProcessing("failed".into()));
        let pe = err.to_patient_error();
        assert_eq!(pe.title, "Text Extraction Failed");
        assert!(pe.retry_possible);
    }

    #[test]
    fn patient_error_from_ocr_init() {
        let err = ProcessingError::OcrInit("tessdata not found".into());
        let pe = err.to_patient_error();
        assert_eq!(pe.title, "OCR Not Available");
        assert!(!pe.retry_possible);
    }

    #[test]
    fn patient_error_serializes() {
        let err = ProcessingError::Import(ImportError::EncryptedPdf);
        let pe = err.to_patient_error();
        let json = serde_json::to_string(&pe).unwrap();
        assert!(json.contains("\"title\":\"Protected PDF\""));
        assert!(json.contains("\"category\":\"unsupported_file\""));
    }

    // -- R.4: Sanitization tests --

    #[test]
    fn sanitize_strips_unix_paths() {
        let msg = "Failed to read /home/user/profile/data/documents/abc.pdf";
        let clean = sanitize_error_message(msg);
        assert!(!clean.contains("/home/user"));
        assert!(clean.contains("[file]"));
    }

    #[test]
    fn sanitize_strips_uuids() {
        let msg = "Document not found: 550e8400-e29b-41d4-a716-446655440000";
        let clean = sanitize_error_message(msg);
        assert!(!clean.contains("550e8400"));
        assert!(clean.contains("[id]"));
    }

    #[test]
    fn sanitize_strips_ip_addresses() {
        let msg = "Connection refused at 127.0.0.1:11434";
        let clean = sanitize_error_message(msg);
        assert!(!clean.contains("127.0.0.1"));
        assert!(clean.contains("[server]"));
    }

    #[test]
    fn sanitize_truncates_long_messages() {
        let msg = "A".repeat(300);
        let clean = sanitize_error_message(&msg);
        assert!(clean.len() <= 210); // 200 + "..."
        assert!(clean.ends_with("..."));
    }

    // -- R4: Per-page structuring tests (D2) --

    fn make_structuring_result(
        doc_id: Uuid,
        doc_type: DocumentType,
        date: Option<NaiveDate>,
        professional: Option<ExtractedProfessional>,
        markdown: &str,
        entities: ExtractedEntities,
        confidence: f32,
        warnings: Vec<String>,
    ) -> StructuringResult {
        StructuringResult {
            document_id: doc_id,
            document_type: doc_type,
            document_date: date,
            professional,
            structured_markdown: markdown.to_string(),
            extracted_entities: entities,
            structuring_confidence: confidence,
            markdown_file_path: None,
            validation_warnings: warnings,
            raw_llm_response: None,
        }
    }

    #[test]
    fn merge_single_page_passthrough() {
        let doc_id = Uuid::new_v4();
        let result = make_structuring_result(
            doc_id,
            DocumentType::Prescription,
            Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()),
            Some(ExtractedProfessional {
                name: "Dr. Chen".into(),
                specialty: Some("GP".into()),
                institution: None,
            }),
            "# Prescription",
            ExtractedEntities::default(),
            0.90,
            vec![],
        );

        let merged = merge_page_results(&doc_id, vec![result]);

        assert_eq!(merged.document_type, DocumentType::Prescription);
        assert_eq!(merged.structured_markdown, "# Prescription");
        assert!((merged.structuring_confidence - 0.90).abs() < f32::EPSILON);
        assert!(merged.professional.is_some());
    }

    #[test]
    fn merge_multi_page_entities_combined() {
        let doc_id = Uuid::new_v4();

        let mut entities_p1 = ExtractedEntities::default();
        entities_p1.medications.push(ExtractedMedication {
            generic_name: Some("Metformin".into()),
            brand_name: None,
            dose: "500mg".into(),
            frequency: "twice daily".into(),
            frequency_type: "scheduled".into(),
            route: "oral".into(),
            reason: None,
            instructions: vec![],
            is_compound: false,
            compound_ingredients: vec![],
            tapering_steps: vec![],
            max_daily_dose: None,
            condition: None,
            confidence: 0.90,
        });

        let mut entities_p2 = ExtractedEntities::default();
        entities_p2.diagnoses.push(ExtractedDiagnosis {
            name: "Type 2 Diabetes".into(),
            icd_code: Some("E11".into()),
            date: None,
            status: "active".into(),
            confidence: 0.88,
        });

        let mut entities_p3 = ExtractedEntities::default();
        entities_p3.allergies.push(ExtractedAllergy {
            allergen: "Penicillin".into(),
            reaction: Some("rash".into()),
            severity: Some("moderate".into()),
            confidence: 0.85,
        });

        let results = vec![
            make_structuring_result(
                doc_id,
                DocumentType::Prescription,
                None,
                None,
                "Page 1 content",
                entities_p1,
                0.90,
                vec![],
            ),
            make_structuring_result(
                doc_id,
                DocumentType::Other,
                None,
                None,
                "Page 2 content",
                entities_p2,
                0.85,
                vec![],
            ),
            make_structuring_result(
                doc_id,
                DocumentType::Other,
                None,
                None,
                "Page 3 content",
                entities_p3,
                0.80,
                vec![],
            ),
        ];

        let merged = merge_page_results(&doc_id, results);

        assert_eq!(merged.extracted_entities.medications.len(), 1);
        assert_eq!(merged.extracted_entities.diagnoses.len(), 1);
        assert_eq!(merged.extracted_entities.allergies.len(), 1);
        assert_eq!(count_entities(&merged.extracted_entities), 3);
    }

    #[test]
    fn merge_dedup_same_medication() {
        let doc_id = Uuid::new_v4();

        let med = ExtractedMedication {
            generic_name: Some("Metformin".into()),
            brand_name: None,
            dose: "500mg".into(),
            frequency: "twice daily".into(),
            frequency_type: "scheduled".into(),
            route: "oral".into(),
            reason: None,
            instructions: vec![],
            is_compound: false,
            compound_ingredients: vec![],
            tapering_steps: vec![],
            max_daily_dose: None,
            condition: None,
            confidence: 0.80,
        };

        let med_higher_conf = ExtractedMedication {
            confidence: 0.95,
            ..med.clone()
        };

        let mut entities_p1 = ExtractedEntities::default();
        entities_p1.medications.push(med);

        let mut entities_p3 = ExtractedEntities::default();
        entities_p3.medications.push(med_higher_conf);

        let results = vec![
            make_structuring_result(
                doc_id,
                DocumentType::Prescription,
                None,
                None,
                "Page 1",
                entities_p1,
                0.85,
                vec![],
            ),
            make_structuring_result(
                doc_id,
                DocumentType::Other,
                None,
                None,
                "Page 3",
                entities_p3,
                0.90,
                vec![],
            ),
        ];

        let merged = merge_page_results(&doc_id, results);

        // Same med+dose → deduplicated, higher confidence kept
        assert_eq!(merged.extracted_entities.medications.len(), 1);
        assert!((merged.extracted_entities.medications[0].confidence - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn merge_dedup_different_dose_kept_separate() {
        let doc_id = Uuid::new_v4();

        let med_500 = ExtractedMedication {
            generic_name: Some("Metformin".into()),
            brand_name: None,
            dose: "500mg".into(),
            frequency: "twice daily".into(),
            frequency_type: "scheduled".into(),
            route: "oral".into(),
            reason: None,
            instructions: vec![],
            is_compound: false,
            compound_ingredients: vec![],
            tapering_steps: vec![],
            max_daily_dose: None,
            condition: None,
            confidence: 0.90,
        };

        let med_1000 = ExtractedMedication {
            dose: "1000mg".into(),
            ..med_500.clone()
        };

        let mut entities_p1 = ExtractedEntities::default();
        entities_p1.medications.push(med_500);

        let mut entities_p2 = ExtractedEntities::default();
        entities_p2.medications.push(med_1000);

        let results = vec![
            make_structuring_result(
                doc_id,
                DocumentType::Prescription,
                None,
                None,
                "Page 1",
                entities_p1,
                0.90,
                vec![],
            ),
            make_structuring_result(
                doc_id,
                DocumentType::Other,
                None,
                None,
                "Page 2",
                entities_p2,
                0.85,
                vec![],
            ),
        ];

        let merged = merge_page_results(&doc_id, results);

        // Different dose → two separate entries
        assert_eq!(merged.extracted_entities.medications.len(), 2);
    }

    #[test]
    fn merge_document_type_first_non_other() {
        let doc_id = Uuid::new_v4();

        let results = vec![
            make_structuring_result(
                doc_id,
                DocumentType::Other,
                None,
                None,
                "Page 1",
                ExtractedEntities::default(),
                0.80,
                vec![],
            ),
            make_structuring_result(
                doc_id,
                DocumentType::LabResult,
                None,
                None,
                "Page 2",
                ExtractedEntities::default(),
                0.85,
                vec![],
            ),
            make_structuring_result(
                doc_id,
                DocumentType::Other,
                None,
                None,
                "Page 3",
                ExtractedEntities::default(),
                0.80,
                vec![],
            ),
        ];

        let merged = merge_page_results(&doc_id, results);
        assert_eq!(merged.document_type, DocumentType::LabResult);
    }

    #[test]
    fn merge_document_date_earliest() {
        let doc_id = Uuid::new_v4();

        let results = vec![
            make_structuring_result(
                doc_id,
                DocumentType::Other,
                Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()),
                None,
                "Page 1",
                ExtractedEntities::default(),
                0.80,
                vec![],
            ),
            make_structuring_result(
                doc_id,
                DocumentType::Other,
                Some(NaiveDate::from_ymd_opt(2024, 1, 10).unwrap()),
                None,
                "Page 2",
                ExtractedEntities::default(),
                0.85,
                vec![],
            ),
            make_structuring_result(
                doc_id,
                DocumentType::Other,
                None,
                None,
                "Page 3",
                ExtractedEntities::default(),
                0.80,
                vec![],
            ),
        ];

        let merged = merge_page_results(&doc_id, results);
        assert_eq!(
            merged.document_date,
            Some(NaiveDate::from_ymd_opt(2024, 1, 10).unwrap())
        );
    }

    #[test]
    fn merge_markdown_page_breaks() {
        let doc_id = Uuid::new_v4();

        let results = vec![
            make_structuring_result(
                doc_id,
                DocumentType::Other,
                None,
                None,
                "First page",
                ExtractedEntities::default(),
                0.80,
                vec![],
            ),
            make_structuring_result(
                doc_id,
                DocumentType::Other,
                None,
                None,
                "Second page",
                ExtractedEntities::default(),
                0.85,
                vec![],
            ),
            make_structuring_result(
                doc_id,
                DocumentType::Other,
                None,
                None,
                "Third page",
                ExtractedEntities::default(),
                0.80,
                vec![],
            ),
        ];

        let merged = merge_page_results(&doc_id, results);

        assert!(merged.structured_markdown.contains("First page"));
        assert!(merged.structured_markdown.contains("--- Page 2 ---"));
        assert!(merged.structured_markdown.contains("Second page"));
        assert!(merged.structured_markdown.contains("--- Page 3 ---"));
        assert!(merged.structured_markdown.contains("Third page"));
    }

    #[test]
    fn merge_confidence_weighted_average() {
        let doc_id = Uuid::new_v4();

        // Page 1: 10 chars markdown, 0.90 confidence → weight 10
        // Page 2: 30 chars markdown, 0.70 confidence → weight 30
        // Expected: (10*0.90 + 30*0.70) / 40 = (9.0 + 21.0) / 40 = 0.75
        let results = vec![
            make_structuring_result(
                doc_id,
                DocumentType::Other,
                None,
                None,
                "0123456789", // 10 chars
                ExtractedEntities::default(),
                0.90,
                vec![],
            ),
            make_structuring_result(
                doc_id,
                DocumentType::Other,
                None,
                None,
                "012345678901234567890123456789", // 30 chars
                ExtractedEntities::default(),
                0.70,
                vec![],
            ),
        ];

        let merged = merge_page_results(&doc_id, results);

        assert!(
            (merged.structuring_confidence - 0.75).abs() < 0.01,
            "Expected ~0.75, got {}",
            merged.structuring_confidence
        );
    }

    #[test]
    fn merge_warnings_union() {
        let doc_id = Uuid::new_v4();

        let results = vec![
            make_structuring_result(
                doc_id,
                DocumentType::Other,
                None,
                None,
                "Page 1",
                ExtractedEntities::default(),
                0.80,
                vec!["Warning A".into()],
            ),
            make_structuring_result(
                doc_id,
                DocumentType::Other,
                None,
                None,
                "Page 2",
                ExtractedEntities::default(),
                0.85,
                vec!["Warning B".into(), "Warning C".into()],
            ),
        ];

        let merged = merge_page_results(&doc_id, results);
        assert_eq!(merged.validation_warnings.len(), 3);
    }

    #[test]
    fn merge_professional_first_non_none() {
        let doc_id = Uuid::new_v4();

        let results = vec![
            make_structuring_result(
                doc_id,
                DocumentType::Other,
                None,
                None, // No professional on page 1
                "Page 1",
                ExtractedEntities::default(),
                0.80,
                vec![],
            ),
            make_structuring_result(
                doc_id,
                DocumentType::Other,
                None,
                Some(ExtractedProfessional {
                    name: "Dr. Chen".into(),
                    specialty: Some("GP".into()),
                    institution: None,
                }),
                "Page 2",
                ExtractedEntities::default(),
                0.85,
                vec![],
            ),
            make_structuring_result(
                doc_id,
                DocumentType::Other,
                None,
                Some(ExtractedProfessional {
                    name: "Dr. Smith".into(),
                    specialty: None,
                    institution: None,
                }),
                "Page 3",
                ExtractedEntities::default(),
                0.80,
                vec![],
            ),
        ];

        let merged = merge_page_results(&doc_id, results);
        assert_eq!(merged.professional.unwrap().name, "Dr. Chen");
    }

    // -- R4: Per-page pipeline integration tests (D2) --

    #[test]
    fn per_page_progress_tracker_atomics() {
        let (_dir, session) = test_session();
        let conn = open_database(session.db_path(), Some(session.key_bytes())).unwrap();

        let mut processor = build_test_processor();

        // §22: ProgressTracker wraps StageTracker + adds page counters
        let tracker = Arc::new(ProgressTracker::new(STAGE_IMPORTING));
        processor.set_progress_tracker(tracker.clone());

        let tmp = tempfile::tempdir().unwrap();
        let file = create_test_file(
            tmp.path(),
            "progress_test.txt",
            "Metformin 500mg twice daily for type 2 diabetes.",
        );

        let output = processor.process_file(&file, &session, &conn).unwrap();
        assert_eq!(output.outcome.import_status, ImportStatus::Staged);

        // After processing: stage atomic should be at structuring (last stage set)
        assert_eq!(tracker.stage.load(Ordering::Relaxed), STAGE_STRUCTURING);
        // Single-page text file → page_current=1, page_total=1
        assert_eq!(tracker.page_current.load(Ordering::Relaxed), 1);
        assert_eq!(tracker.page_total.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn per_page_fault_tolerance_partial_failure() {
        use crate::pipeline::structuring::types::MedicalStructurer;

        // Structurer that fails on every other call
        struct AlternatingStructurer {
            call_count: std::sync::atomic::AtomicUsize,
        }

        impl MedicalStructurer for AlternatingStructurer {
            fn structure_document(
                &self,
                document_id: &Uuid,
                text: &str,
                ocr_confidence: f32,
                _session: &ProfileSession,
            ) -> Result<StructuringResult, StructuringError> {
                let call = self
                    .call_count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if call % 2 == 1 {
                    // Fail on calls 1, 3, 5...
                    return Err(StructuringError::MalformedResponse(
                        "Simulated page failure".into(),
                    ));
                }
                // Use MockExtractionStrategy for successful calls
                let strategy = Box::new(MockExtractionStrategy::with_output(mock_strategy_output()));
                let llm = Box::new(MockLlmClient::new("unused"));
                let structurer = DocumentStructurer::new(llm, "medgemma:latest", strategy);
                structurer.structure_document(document_id, text, ocr_confidence, _session)
            }
        }

        let (_dir, session) = test_session();
        let conn = open_database(session.db_path(), Some(session.key_bytes())).unwrap();

        let extractor = Box::new(build_test_extractor(3));

        let structurer = Box::new(AlternatingStructurer {
            call_count: std::sync::atomic::AtomicUsize::new(0),
        });

        let processor = DocumentProcessor::new(extractor, structurer);

        let tmp = tempfile::tempdir().unwrap();
        // Use a PDF-detectable file so it goes through multi-page path
        let file_path = tmp.path().join("multi_page.pdf");
        std::fs::write(&file_path, b"fake pdf content").unwrap();

        let import_result = crate::pipeline::import::importer::import_file(
            &file_path, &session, &conn,
        )
        .unwrap();

        // Process the imported file — pages 0 (success), 1 (fail), 2 (success)
        let output = processor
            .process_imported(&import_result, &session, &conn)
            .unwrap();

        assert!(output.structuring_result.is_some());
        // 2 of 3 pages succeed
        let result = output.structuring_result.unwrap();
        assert!(count_entities(&result.extracted_entities) >= 2);
    }

    #[test]
    fn per_page_all_fail_returns_error() {
        use crate::pipeline::structuring::types::MedicalStructurer;

        struct AlwaysFailStructurer;

        impl MedicalStructurer for AlwaysFailStructurer {
            fn structure_document(
                &self,
                _document_id: &Uuid,
                _text: &str,
                _ocr_confidence: f32,
                _session: &ProfileSession,
            ) -> Result<StructuringResult, StructuringError> {
                Err(StructuringError::MalformedResponse(
                    "Always fails".into(),
                ))
            }
        }

        let (_dir, session) = test_session();
        let conn = open_database(session.db_path(), Some(session.key_bytes())).unwrap();

        let extractor = Box::new(build_test_extractor(1));
        let processor = DocumentProcessor::new(extractor, Box::new(AlwaysFailStructurer));

        let tmp = tempfile::tempdir().unwrap();
        let file = create_test_file(
            tmp.path(),
            "all_fail.txt",
            "This text is long enough to pass the minimum length check.",
        );

        let result = processor.process_file(&file, &session, &conn);
        assert!(result.is_err());
    }

    #[test]
    fn per_page_skip_short_page() {
        use crate::pipeline::extraction::types::{TextExtractor, ExtractionResult, ExtractionMethod, PageExtraction};
        use crate::pipeline::import::FormatDetection;

        // Custom extractor that returns pages with varying text lengths
        struct MultiPageExtractor;

        impl TextExtractor for MultiPageExtractor {
            fn extract(
                &self,
                document_id: &Uuid,
                _staged_path: &Path,
                _format: &FormatDetection,
                _session: &ProfileSession,
                _progress: Option<&ProgressTracker>,
            ) -> Result<ExtractionResult, ExtractionError> {
                Ok(ExtractionResult {
                    document_id: *document_id,
                    method: ExtractionMethod::VisionOcr,
                    pages: vec![
                        PageExtraction {
                            page_number: 1,
                            text: "Metformin 500mg twice daily for diabetes management".into(),
                            confidence: 0.90,
                            regions: vec![],
                            warnings: vec![],
                            content_type: None,
                            drill_output: None,
                        },
                        PageExtraction {
                            page_number: 2,
                            text: "---".into(), // Too short — should be skipped
                            confidence: 0.50,
                            regions: vec![],
                            warnings: vec![],
                            content_type: None,
                            drill_output: None,
                        },
                        PageExtraction {
                            page_number: 3,
                            text: "Aspirin 100mg daily for cardiac prevention".into(),
                            confidence: 0.88,
                            regions: vec![],
                            warnings: vec![],
                            content_type: None,
                            drill_output: None,
                        },
                    ],
                    full_text: "page1\n\npage2\n\npage3".into(),
                    overall_confidence: 0.85,
                    language_detected: None,
                    page_count: 3,
                })
            }
        }

        let (_dir, session) = test_session();
        let conn = open_database(session.db_path(), Some(session.key_bytes())).unwrap();

        let strategy = Box::new(MockExtractionStrategy::with_output(mock_strategy_output()));
        let llm = Box::new(MockLlmClient::new("unused"));
        let structurer = Box::new(DocumentStructurer::new(llm, "medgemma:latest", strategy));
        let processor = DocumentProcessor::new(Box::new(MultiPageExtractor), structurer);

        let tmp = tempfile::tempdir().unwrap();
        let file = create_test_file(
            tmp.path(),
            "short_page.txt",
            "Placeholder text to make the import succeed with enough length.",
        );

        let output = processor.process_file(&file, &session, &conn).unwrap();
        assert!(output.structuring_result.is_some());
        // Page 2 (3 chars) should be skipped; pages 1 and 3 processed
    }

    // -- R4: Dedup unit tests --

    #[test]
    fn normalize_for_dedup_basic() {
        assert_eq!(normalize_for_dedup("  Metformin  "), "metformin");
        assert_eq!(normalize_for_dedup("Type 2  Diabetes"), "type 2 diabetes");
        assert_eq!(normalize_for_dedup(""), "");
    }

    #[test]
    fn dedup_medications_keeps_higher_confidence() {
        let mut meds = vec![
            ExtractedMedication {
                generic_name: Some("Metformin".into()),
                brand_name: None,
                dose: "500mg".into(),
                frequency: String::new(),
                frequency_type: String::new(),
                route: String::new(),
                reason: None,
                instructions: vec![],
                is_compound: false,
                compound_ingredients: vec![],
                tapering_steps: vec![],
                max_daily_dose: None,
                condition: None,
                confidence: 0.80,
            },
            ExtractedMedication {
                generic_name: Some("metformin".into()), // lowercase — should dedup
                brand_name: None,
                dose: "500mg".into(),
                frequency: String::new(),
                frequency_type: String::new(),
                route: String::new(),
                reason: None,
                instructions: vec![],
                is_compound: false,
                compound_ingredients: vec![],
                tapering_steps: vec![],
                max_daily_dose: None,
                condition: None,
                confidence: 0.95,
            },
        ];

        dedup_medications(&mut meds);

        assert_eq!(meds.len(), 1);
        assert!((meds[0].confidence - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn dedup_lab_results_same_test_same_value() {
        let mut labs = vec![
            ExtractedLabResult {
                test_name: "HbA1c".into(),
                test_code: None,
                value: Some(7.2),
                value_text: None,
                unit: Some("%".into()),
                reference_range_low: None,
                reference_range_high: None,
                reference_range_text: None,
                abnormal_flag: None,
                collection_date: None,
                confidence: 0.80,
            },
            ExtractedLabResult {
                test_name: "hba1c".into(), // case-insensitive
                test_code: None,
                value: Some(7.2),
                value_text: None,
                unit: Some("%".into()),
                reference_range_low: None,
                reference_range_high: None,
                reference_range_text: None,
                abnormal_flag: None,
                collection_date: None,
                confidence: 0.90,
            },
        ];

        dedup_lab_results(&mut labs);

        assert_eq!(labs.len(), 1);
        assert!((labs[0].confidence - 0.90).abs() < f32::EPSILON);
    }

    #[test]
    fn dedup_instructions_keeps_first() {
        let mut insts = vec![
            ExtractedInstruction {
                text: "Take with food".into(),
                category: "dietary".into(),
            },
            ExtractedInstruction {
                text: "take with food".into(), // case-insensitive dupe
                category: "general".into(),
            },
            ExtractedInstruction {
                text: "Avoid alcohol".into(),
                category: "lifestyle".into(),
            },
        ];

        dedup_instructions(&mut insts);

        assert_eq!(insts.len(), 2);
        assert_eq!(insts[0].text, "Take with food");
        assert_eq!(insts[0].category, "dietary"); // First one kept
        assert_eq!(insts[1].text, "Avoid alcohol");
    }

    // --- 12-ERC Brick 3: post_process_drill_output ---

    #[test]
    fn drill_entities_bypass_structurer() {
        // Page with drill_output should NOT invoke the structurer
        let output = mock_strategy_output();
        let doc_id = Uuid::new_v4();
        let result = post_process_drill_output(&doc_id, output, 0.85);
        assert_eq!(result.document_id, doc_id);
        assert_eq!(result.document_type, DocumentType::Prescription);
        assert!(!result.extracted_entities.medications.is_empty());
    }

    #[test]
    fn post_process_validates_entities() {
        use crate::pipeline::structuring::types::ExtractedLabResult;
        // Feed > 100 lab results — validation caps them
        let mut output = mock_strategy_output();
        output.entities.lab_results = (0..150).map(|i| ExtractedLabResult {
            test_name: format!("Lab{i}"),
            test_code: None,
            value: Some(i as f64),
            value_text: None,
            unit: Some("mg".into()),
            reference_range_low: None,
            reference_range_high: None,
            reference_range_text: None,
            abnormal_flag: None,
            collection_date: None,
            confidence: 0.9,
        }).collect();
        let result = post_process_drill_output(&Uuid::new_v4(), output, 0.85);
        // Validation caps entity counts (MAX_LAB_RESULTS = 100)
        assert!(result.extracted_entities.lab_results.len() <= 100);
    }

    #[test]
    fn post_process_classifies_lab_report() {
        use crate::pipeline::structuring::types::ExtractedLabResult;
        let mut output = mock_strategy_output();
        output.document_type = None; // Force classification from entities
        output.entities.medications.clear();
        output.entities.diagnoses.clear();
        output.entities.lab_results = vec![
            ExtractedLabResult {
                test_name: "Glucose".into(),
                test_code: None,
                value: Some(5.6),
                value_text: None,
                unit: Some("mmol/L".into()),
                reference_range_low: None,
                reference_range_high: None,
                reference_range_text: None,
                abnormal_flag: None,
                collection_date: None,
                confidence: 0.9,
            },
            ExtractedLabResult {
                test_name: "HbA1c".into(),
                test_code: None,
                value: Some(7.2),
                value_text: None,
                unit: Some("%".into()),
                reference_range_low: None,
                reference_range_high: None,
                reference_range_text: None,
                abnormal_flag: None,
                collection_date: None,
                confidence: 0.9,
            },
        ];
        let result = post_process_drill_output(&Uuid::new_v4(), output, 0.85);
        assert_eq!(result.document_type, DocumentType::LabResult);
    }

    #[test]
    fn text_only_path_post_process_no_drill() {
        // Without drill_output, post_process_drill_output is NOT called.
        // Verify that the function itself works with minimal entities (no meds).
        let output = StrategyOutput {
            entities: ExtractedEntities::default(),
            markdown: "# Empty page".into(),
            document_type: None,
            document_date: None,
            professional: None,
            raw_responses: vec![],
        };
        let result = post_process_drill_output(&Uuid::new_v4(), output, 0.7);
        assert_eq!(result.document_type, DocumentType::Other);
        assert!(result.extracted_entities.medications.is_empty());
        assert!(result.professional.is_none());
        assert!(result.raw_llm_response.is_none());
    }

    #[test]
    fn mixed_pages_drill_and_text() {
        // Ensure post_process handles variety: professional, date, entities all pass through
        let output = StrategyOutput {
            entities: ExtractedEntities {
                medications: vec![ExtractedMedication {
                    generic_name: Some("Aspirin".into()),
                    brand_name: None,
                    dose: "100mg".into(),
                    frequency: "daily".into(),
                    frequency_type: "regular".into(),
                    route: "oral".into(),
                    reason: None,
                    instructions: vec![],
                    is_compound: false,
                    compound_ingredients: vec![],
                    tapering_steps: vec![],
                    max_daily_dose: None,
                    condition: None,
                    confidence: 0.85,
                }],
                ..Default::default()
            },
            markdown: "# Mixed page test".into(),
            document_type: Some("prescription".into()),
            document_date: Some("2024-03-15".into()),
            professional: Some(ExtractedProfessional {
                name: "Dr. Mixed".into(),
                specialty: Some("GP".into()),
                institution: None,
            }),
            raw_responses: vec!["response1".into(), "response2".into()],
        };
        let result = post_process_drill_output(&Uuid::new_v4(), output, 0.90);
        assert_eq!(result.professional.as_ref().unwrap().name, "Dr. Mixed");
        assert!(result.document_date.is_some());
        assert!(!result.extracted_entities.medications.is_empty());
        assert!(result.raw_llm_response.is_some());
        assert!(result.raw_llm_response.unwrap().contains("---"));
    }
}
