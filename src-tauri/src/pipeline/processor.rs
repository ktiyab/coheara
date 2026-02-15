//! E2E-B02: Document Processing Orchestrator.
//!
//! Single entry point that drives the full document pipeline:
//! import → extract → structure → (save pending review in command layer).
//!
//! Uses trait-based DI for all engines (OcrEngine, LlmClient, etc.)
//! so the orchestrator remains fully testable with mock implementations.

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::Serialize;
use uuid::Uuid;

use crate::crypto::ProfileSession;
use crate::db::repository;
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
// Orchestrator
// ---------------------------------------------------------------------------

/// Orchestrates document processing: import → extract → structure.
///
/// Pure pipeline logic with trait-based DI. Does NOT perform IPC or Tauri
/// event emission — that responsibility belongs to the command layer.
pub struct DocumentProcessor {
    extractor: Box<dyn TextExtractor + Send + Sync>,
    structurer: Box<dyn MedicalStructurer + Send + Sync>,
}

impl DocumentProcessor {
    pub fn new(
        extractor: Box<dyn TextExtractor + Send + Sync>,
        structurer: Box<dyn MedicalStructurer + Send + Sync>,
    ) -> Self {
        Self {
            extractor,
            structurer,
        }
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
            self.extract_and_structure(&import, session, conn)?;

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
            self.extract_and_structure(import, session, conn)?;

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
            let engine = crate::pipeline::extraction::ocr::BundledTesseract::new(&tessdata)
                .map_err(|e| ProcessingError::OcrInit(e.to_string()))?;
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
    // 1. Check TESSDATA_PREFIX environment variable
    if let Ok(path) = std::env::var("TESSDATA_PREFIX") {
        let p = PathBuf::from(&path);
        if p.join("eng.traineddata").exists() {
            return Ok(p);
        }
    }

    // 2. Try common system paths
    let candidates = [
        "/usr/share/tesseract-ocr/5/tessdata",
        "/usr/share/tesseract-ocr/4.00/tessdata",
        "/usr/share/tessdata",
        "/usr/local/share/tessdata",
        "/opt/homebrew/share/tessdata",
    ];

    for path in &candidates {
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
            profile::create_profile(dir.path(), "ProcessorTest", "test_pass_123", None).unwrap();
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
        let conn = open_database(session.db_path()).unwrap();
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
        let conn = open_database(session.db_path()).unwrap();
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
        let conn = open_database(session.db_path()).unwrap();
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
        let conn = open_database(session.db_path()).unwrap();
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
        let conn = open_database(session.db_path()).unwrap();

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
}
