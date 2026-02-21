//! E2E-B02: Document Processing Orchestrator.
//!
//! Single entry point that drives the full document pipeline:
//! import → extract → structure → (save pending review in command layer).
//!
//! Uses trait-based DI for all engines (OcrEngine, LlmClient, etc.)
//! so the orchestrator remains fully testable with mock implementations.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use rusqlite::Connection;
use serde::Serialize;
use uuid::Uuid;

use crate::crypto::ProfileSession;
use crate::db::repository;
use crate::models::enums::PipelineStatus;
use crate::pipeline::extraction::orchestrator::DocumentExtractor;
use crate::pipeline::extraction::types::TextExtractor;
use crate::pipeline::extraction::ExtractionError;
use crate::pipeline::import::importer::{import_file, ImportResult, ImportStatus};
use crate::pipeline::import::ImportError;
use crate::pipeline::structuring::orchestrator::DocumentStructurer;
use crate::pipeline::structuring::types::{MedicalStructurer, StructuringResult};
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
            // Structuring is handled above via early return
            ProcessingError::Structuring(_) => unreachable!(),
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
pub type StageTracker = Arc<AtomicU8>;

/// Stage constants matching the frontend's expected stage names.
pub const STAGE_IMPORTING: u8 = 0;
pub const STAGE_EXTRACTING: u8 = 1;
pub const STAGE_STRUCTURING: u8 = 2;

/// Map stage constant to frontend-expected stage name.
pub fn stage_name(stage: u8) -> &'static str {
    match stage {
        STAGE_IMPORTING => "importing",
        STAGE_EXTRACTING => "extracting",
        STAGE_STRUCTURING => "structuring",
        _ => "importing",
    }
}

/// Progress percentage range per stage (min, max).
pub fn stage_pct_range(stage: u8) -> (u8, u8) {
    match stage {
        STAGE_IMPORTING => (5, 15),
        STAGE_EXTRACTING => (15, 40),
        STAGE_STRUCTURING => (40, 85),
        _ => (5, 15),
    }
}

// ---------------------------------------------------------------------------
// Orchestrator
// ---------------------------------------------------------------------------

/// Orchestrates document processing: import → extract → structure.
///
/// Pure pipeline logic with trait-based DI. Does NOT perform IPC or Tauri
/// event emission — that responsibility belongs to the command layer.
pub struct DocumentProcessor {
    extractor: Box<dyn TextExtractor + Send + Sync>,
    structurer: Box<dyn MedicalStructurer + Send + Sync>,
    stage_tracker: Option<StageTracker>,
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
        }
    }

    /// Set a shared stage tracker for progress reporting.
    /// The command layer reads this from the heartbeat thread.
    pub fn set_stage_tracker(&mut self, tracker: StageTracker) {
        self.stage_tracker = Some(tracker);
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
        let extraction = self
            .extractor
            .extract(&import.document_id, staged_path, &import.format, session)?;

        let extraction_summary = ExtractionSummary {
            method: format!("{:?}", extraction.method),
            confidence: extraction.overall_confidence,
            page_count: extraction.page_count,
            text_length: extraction.full_text.len(),
        };

        // Step 3: Update OCR confidence in DB
        if let Err(e) =
            update_ocr_confidence(conn, &import.document_id, extraction.overall_confidence)
        {
            tracing::warn!(
                document_id = %import.document_id,
                error = %e,
                "Failed to update OCR confidence — continuing"
            );
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

        // Step 4: Structure with LLM
        tracing::info!(
            document_id = %import.document_id,
            confidence = extraction.overall_confidence,
            text_length = extraction.full_text.len(),
            "Processing: starting structuring"
        );
        let structuring = self.structurer.structure_document(
            &import.document_id,
            &extraction.full_text,
            extraction.overall_confidence,
            session,
        )?;

        let entities = &structuring.extracted_entities;
        let structuring_summary = StructuringSummary {
            document_type: structuring.document_type.as_str().to_string(),
            confidence: structuring.structuring_confidence,
            entities_count: count_entities(entities),
            has_professional: structuring.professional.is_some(),
            document_date: structuring.document_date.map(|d| d.to_string()),
        };

        tracing::info!(
            document_id = %import.document_id,
            document_type = structuring.document_type.as_str(),
            entities = structuring_summary.entities_count,
            "Processing complete"
        );

        Ok((extraction_summary, structuring_summary, structuring))
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
// Factory
// ---------------------------------------------------------------------------

/// Build a `DocumentProcessor` with production implementations.
///
/// - OCR: `BundledTesseract` (feature-gated) or `MockOcrEngine`
/// - PDF: `PdfTextExtractor`
/// - LLM: `OllamaClient` → `DocumentStructurer`
///
/// Returns an error if required services are unavailable (no Ollama, no model).
pub fn build_processor(model: &str) -> Result<DocumentProcessor, ProcessingError> {
    let ocr = build_ocr_engine()?;
    let pdf = Box::new(crate::pipeline::extraction::pdf::PdfTextExtractor);
    let extractor = Box::new(DocumentExtractor::new(ocr, pdf));

    let ollama = crate::pipeline::structuring::ollama::OllamaClient::default_local();
    tracing::info!(model = %model, "Document processor using LLM model");

    let structurer = Box::new(DocumentStructurer::new(Box::new(ollama), model));

    Ok(DocumentProcessor::new(extractor, structurer))
}

/// Build the OCR engine, respecting feature flags.
fn build_ocr_engine(
) -> Result<Box<dyn crate::pipeline::extraction::types::OcrEngine + Send + Sync>, ProcessingError>
{
    #[cfg(feature = "ocr")]
    {
        if let Ok(tessdata) = find_tessdata_dir() {
            let mut engine = crate::pipeline::extraction::ocr::BundledTesseract::new(&tessdata)
                .map_err(|e| ProcessingError::OcrInit(e.to_string()))?;

            // EXT-02-G04: Load medical wordlist for improved OCR accuracy
            let wordlist_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("resources")
                .join("medical_wordlist.txt");
            engine = engine.with_medical_wordlist(&wordlist_path);

            tracing::info!(tessdata = %tessdata.display(), "Tesseract OCR initialized");
            return Ok(Box::new(engine));
        }
        tracing::warn!("Tesseract data not found — images will not be OCR'd");
    }

    // Fallback: mock OCR (digital PDFs and plaintext still work)
    tracing::info!("Using mock OCR engine — image OCR unavailable");
    Ok(Box::new(
        crate::pipeline::extraction::ocr::MockOcrEngine::new("[OCR not available]", 0.0),
    ))
}

/// Locate tessdata directory from environment or system paths.
#[cfg(feature = "ocr")]
fn find_tessdata_dir() -> Result<PathBuf, ProcessingError> {
    // 1. Check TESSDATA_PREFIX environment variable (cross-platform)
    if let Ok(path) = std::env::var("TESSDATA_PREFIX") {
        let p = PathBuf::from(&path);
        if p.join("eng.traineddata").exists() {
            return Ok(p);
        }
    }

    // 2. Check VCPKG_ROOT for non-default vcpkg locations (Windows)
    if let Ok(vcpkg_root) = std::env::var("VCPKG_ROOT") {
        let p = PathBuf::from(&vcpkg_root)
            .join("installed")
            .join("x64-windows-static-md")
            .join("share")
            .join("tessdata");
        if p.join("eng.traineddata").exists() {
            return Ok(p);
        }
    }

    // 3. Try platform-specific system paths
    let candidates: &[&str] = if cfg!(target_os = "windows") {
        &[
            r"C:\vcpkg\installed\x64-windows-static-md\share\tessdata",
            r"C:\Program Files\Tesseract-OCR\tessdata",
            r"C:\Program Files (x86)\Tesseract-OCR\tessdata",
        ]
    } else {
        &[
            "/usr/share/tesseract-ocr/5/tessdata",
            "/usr/share/tesseract-ocr/4.00/tessdata",
            "/usr/share/tessdata",
            "/usr/local/share/tessdata",
            "/opt/homebrew/share/tessdata",
        ]
    };

    for path in candidates {
        let p = PathBuf::from(path);
        if p.join("eng.traineddata").exists() {
            return Ok(p);
        }
    }

    Err(ProcessingError::OcrInit(
        "Tesseract data directory not found. Set TESSDATA_PREFIX or install tesseract-ocr-eng"
            .into(),
    ))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::profile;
    use crate::db::sqlite::open_database;
    use crate::pipeline::extraction::ocr::MockOcrEngine;
    use crate::pipeline::extraction::types::{PageExtraction, PdfExtractor};
    use crate::pipeline::structuring::ollama::MockLlmClient;
    use crate::pipeline::structuring::types::ExtractedEntities;

    // -- Mock PDF extractor (not exported from extraction module) -----------

    struct TestPdfExtractor;

    impl PdfExtractor for TestPdfExtractor {
        fn extract_text(&self, _pdf_bytes: &[u8]) -> Result<Vec<PageExtraction>, ExtractionError> {
            Ok(vec![])
        }

        fn page_count(&self, _pdf_bytes: &[u8]) -> Result<usize, ExtractionError> {
            Ok(0)
        }
    }

    // -- Helpers -----------------------------------------------------------

    fn test_session() -> (tempfile::TempDir, ProfileSession) {
        let dir = tempfile::tempdir().unwrap();
        let (info, _phrase) =
            profile::create_profile(dir.path(), "ProcessorTest", "test_pass_123", None, None, None, None).unwrap();
        let session = profile::open_profile(dir.path(), &info.id, "test_pass_123").unwrap();
        (dir, session)
    }

    fn mock_llm_response() -> String {
        r#"```json
{
  "document_type": "prescription",
  "document_date": "2024-01-15",
  "professional": {"name": "Dr. Chen", "specialty": "GP", "institution": null},
  "medications": [
    {
      "generic_name": "Metformin",
      "brand_name": "Glucophage",
      "dose": "500mg",
      "frequency": "twice daily",
      "frequency_type": "scheduled",
      "route": "oral",
      "reason": "Type 2 diabetes",
      "instructions": ["Take with food"],
      "is_compound": false,
      "compound_ingredients": [],
      "tapering_steps": [],
      "max_daily_dose": "2000mg",
      "condition": "Type 2 diabetes",
      "confidence": 0.92
    }
  ],
  "lab_results": [],
  "diagnoses": [
    {
      "name": "Type 2 Diabetes",
      "icd_code": "E11",
      "date": "2024-01-15",
      "status": "active",
      "confidence": 0.90
    }
  ],
  "allergies": [],
  "procedures": [],
  "referrals": [],
  "instructions": []
}
```

# Prescription — Dr. Chen, GP
**Date:** January 15, 2024

## Medications
- **Metformin (Glucophage)** 500mg — twice daily, oral
  - Take with food
  - For: Type 2 diabetes"#
            .to_string()
    }

    fn build_test_processor() -> DocumentProcessor {
        let ocr = Box::new(MockOcrEngine::new("Metformin 500mg twice daily", 0.85));
        let pdf = Box::new(TestPdfExtractor);
        let extractor = Box::new(DocumentExtractor::new(ocr, pdf));

        let llm = Box::new(MockLlmClient::new(&mock_llm_response()));
        let structurer = Box::new(DocumentStructurer::new(llm, "medgemma:latest"));

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

        let ocr = Box::new(MockOcrEngine::new("Some medical text here", 0.85));
        let pdf = Box::new(TestPdfExtractor);
        let extractor = Box::new(DocumentExtractor::new(ocr, pdf));
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
}
