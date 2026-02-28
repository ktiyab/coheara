//! R4+C4: Document extraction orchestrator.
//!
//! Pipeline:
//! - pdfium-render for PDF → image conversion
//! - VisionClassifier for image classification (Document vs MedicalImage)
//! - IterativeDrill with DomainContract focused prompts for document extraction
//! - MedicalImageInterpreter for medical imagery (X-ray, CT, MRI)
//!
//! C4-FIX: IterativeDrill is the PRIMARY extraction strategy for vision documents.
//! No monolithic "extract everything" prompt — focused single-question prompts
//! prevent degeneration by design.

use std::path::Path;

use base64::Engine;
use uuid::Uuid;

use super::confidence::compute_overall_confidence;
use super::preprocess::ImagePreprocessor;
use super::sanitize::sanitize_extracted_text;
use super::types::{
    ExtractionMethod, ExtractionResult, ImageContentType, MedicalImageInterpreter,
    PageExtraction, PdfPageRenderer, TextExtractor,
};
use super::vision_classifier::VisionClassifier;
use super::ExtractionError;
use crate::butler_service::VisionSession;
use crate::crypto::ProfileSession;
use crate::pipeline::diagnostic;
use crate::pipeline::extraction::types::ExtractionWarning;
use crate::pipeline::import::format::FileCategory;
use crate::pipeline::import::staging::read_staged_file;
use crate::pipeline::import::FormatDetection;
use crate::pipeline::structuring::strategy_iterative_drill::IterativeDrillStrategy;
use crate::pipeline::structuring::types::VisionClient;

// ──────────────────────────────────────────────
// Adaptive DPI selection
// ──────────────────────────────────────────────

/// DPI for scanned PDFs — higher resolution for dense text, tables, handwriting.
const SCANNED_PDF_DPI: u32 = 300;

/// DPI for digital PDFs and images — standard resolution, faster inference.
const STANDARD_DPI: u32 = 200;

/// Select rendering DPI based on document format.
///
/// Scanned PDFs get 300 DPI (dense text, tables, small fonts lose detail at 200).
/// Digital PDFs and images get 200 DPI (already sharp, lower res = faster inference).
///
/// Public so other modules can reuse the selection logic.
pub fn select_render_dpi(format: &FormatDetection) -> u32 {
    match format.category {
        FileCategory::ScannedPdf => SCANNED_PDF_DPI,
        _ => STANDARD_DPI,
    }
}

// ──────────────────────────────────────────────
// DocumentExtractor
// ──────────────────────────────────────────────

/// C4-FIX: Vision-based document extraction orchestrator.
///
/// Architecture: Classify → Extract
/// - VisionClassifier determines Document vs MedicalImage (1 lightweight call)
/// - Documents: IterativeDrill with DomainContract focused prompts (PRIMARY)
/// - Medical images: MedicalImageInterpreter for clinical findings
///
/// Dependencies (trait objects for DI):
/// - `pdf_renderer`: Renders PDF pages to PNG (PdfiumRenderer in prod)
/// - `classifier`: Classifies images as Document/MedicalImage (OllamaVisionClassifier)
/// - `vision_session`: Enforced defense pipeline for LLM calls (FallbackSession)
/// - `vision_client`: Ollama vision API client for drill calls
/// - `preprocessor`: Prepares images for vision model (PreprocessingPipeline)
/// - `interpreter` (optional): Medical image interpretation (MedGemma clinical mode)
pub struct DocumentExtractor {
    pdf_renderer: Box<dyn PdfPageRenderer>,
    classifier: Box<dyn VisionClassifier>,
    vision_session: Box<dyn VisionSession>,
    vision_client: Box<dyn VisionClient>,
    system_prompt: String,
    preprocessor: Box<dyn ImagePreprocessor>,
    interpreter: Option<Box<dyn MedicalImageInterpreter>>,
    /// Language code for prompts and diagnostics (e.g., "en", "fr", "de").
    language: String,
}

impl DocumentExtractor {
    pub fn new(
        pdf_renderer: Box<dyn PdfPageRenderer>,
        classifier: Box<dyn VisionClassifier>,
        vision_session: Box<dyn VisionSession>,
        vision_client: Box<dyn VisionClient>,
        system_prompt: String,
        preprocessor: Box<dyn ImagePreprocessor>,
    ) -> Self {
        Self {
            pdf_renderer,
            classifier,
            vision_session,
            vision_client,
            system_prompt,
            preprocessor,
            interpreter: None,
            language: "en".to_string(),
        }
    }

    /// Set the medical image interpreter for content-type routing.
    pub fn with_interpreter(mut self, interpreter: Box<dyn MedicalImageInterpreter>) -> Self {
        self.interpreter = Some(interpreter);
        self
    }

    /// Set the language for prompts and diagnostic dumps.
    pub fn with_language(mut self, language: &str) -> Self {
        self.language = language.to_string();
        self
    }

    /// Extract text from a PDF: render each page, classify, then extract.
    fn extract_pdf(
        &self,
        pdf_bytes: &[u8],
        dpi: u32,
        dump_dir: &Option<std::path::PathBuf>,
        progress: Option<&crate::pipeline::processor::ProgressTracker>,
    ) -> Result<(ExtractionMethod, Vec<PageExtraction>), ExtractionError> {
        let num_pages = self.pdf_renderer.page_count(pdf_bytes)?;

        if num_pages == 0 {
            return Err(ExtractionError::EmptyDocument);
        }

        // §22: Set page total for work-based extraction progress
        if let Some(tracker) = progress {
            tracker.page_total.store(num_pages.min(255) as u8, std::sync::atomic::Ordering::Relaxed);
            tracker.page_current.store(0, std::sync::atomic::Ordering::Relaxed);
        }

        let mut pages = Vec::with_capacity(num_pages);

        for page_idx in 0..num_pages {
            let page_image = self
                .pdf_renderer
                .render_page(pdf_bytes, page_idx, dpi)?;

            if let Some(ref dir) = dump_dir {
                diagnostic::dump_binary(dir, &format!("01-rendered-page-{page_idx}.png"), &page_image);
            }

            let prepared = self.preprocessor.preprocess(&page_image)?;

            if let Some(ref dir) = dump_dir {
                diagnostic::dump_binary(dir, &format!("02-preprocessed-page-{page_idx}.png"), &prepared.png_bytes);
                diagnostic::dump_json(dir, &format!("02-preprocessed-page-{page_idx}.json"), &serde_json::json!({
                    "original_width": prepared.original_width,
                    "original_height": prepared.original_height,
                    "content_width": prepared.content_width,
                    "content_height": prepared.content_height,
                    "warnings": prepared.warnings,
                    "png_size": prepared.png_bytes.len(),
                }));
            }

            // C4-FIX: Classify first, then extract with focused prompts
            let content_type = self.classifier.classify_image(&prepared.png_bytes)?;

            if let Some(ref dir) = dump_dir {
                diagnostic::dump_json(dir, &format!("03-classify-result-page-{page_idx}.json"), &serde_json::json!({
                    "content_type": format!("{content_type:?}"),
                }));
            }

            let page = match content_type {
                ImageContentType::Document => {
                    self.extract_document_page(&prepared.png_bytes, page_idx, prepared.warnings, dump_dir)?
                }
                ImageContentType::MedicalImage => {
                    self.interpret_medical_page(&prepared.png_bytes, page_idx, prepared.warnings)?
                }
            };
            pages.push(page);

            // §22: Update extraction page progress
            if let Some(tracker) = progress {
                tracker.page_current.store((page_idx + 1).min(255) as u8, std::sync::atomic::Ordering::Relaxed);
            }
        }

        Ok((ExtractionMethod::VisionOcr, pages))
    }

    /// Extract text from an image: preprocess, classify, then extract.
    fn extract_image(
        &self,
        image_bytes: &[u8],
        dump_dir: &Option<std::path::PathBuf>,
        progress: Option<&crate::pipeline::processor::ProgressTracker>,
    ) -> Result<(ExtractionMethod, Vec<PageExtraction>), ExtractionError> {
        if let Some(ref dir) = dump_dir {
            diagnostic::dump_binary(dir, "01-raw-image-0.bin", image_bytes);
        }

        let prepared = self.preprocessor.preprocess(image_bytes)?;

        if let Some(ref dir) = dump_dir {
            diagnostic::dump_binary(dir, "02-preprocessed-page-0.png", &prepared.png_bytes);
            diagnostic::dump_json(dir, "02-preprocessed-page-0.json", &serde_json::json!({
                "original_width": prepared.original_width,
                "original_height": prepared.original_height,
                "content_width": prepared.content_width,
                "content_height": prepared.content_height,
                "warnings": prepared.warnings,
                "png_size": prepared.png_bytes.len(),
            }));
        }

        // C4-FIX: Classify first, then extract with focused prompts
        let content_type = self.classifier.classify_image(&prepared.png_bytes)?;

        if let Some(ref dir) = dump_dir {
            diagnostic::dump_json(dir, "03-classify-result-page-0.json", &serde_json::json!({
                "content_type": format!("{content_type:?}"),
            }));
        }

        let page = match content_type {
            ImageContentType::Document => {
                self.extract_document_page(&prepared.png_bytes, 0, prepared.warnings, dump_dir)?
            }
            ImageContentType::MedicalImage => {
                self.interpret_medical_page(&prepared.png_bytes, 0, prepared.warnings)?
            }
        };

        // §22: Single-page image — set progress as complete
        if let Some(tracker) = progress {
            tracker.page_total.store(1, std::sync::atomic::Ordering::Relaxed);
            tracker.page_current.store(1, std::sync::atomic::Ordering::Relaxed);
        }

        Ok((ExtractionMethod::VisionOcr, vec![page]))
    }

    /// C4-FIX: PRIMARY document extraction — IterativeDrill with focused prompts.
    ///
    /// Encodes the preprocessed PNG as base64 and runs 7-domain enumerate +
    /// per-field drill through the VisionSession (with defense pipeline).
    fn extract_document_page(
        &self,
        png_bytes: &[u8],
        page_idx: usize,
        warnings: Vec<ExtractionWarning>,
        dump_dir: &Option<std::path::PathBuf>,
    ) -> Result<PageExtraction, ExtractionError> {
        let base64_image =
            base64::engine::general_purpose::STANDARD.encode(png_bytes);
        let images = vec![base64_image];

        let drill = IterativeDrillStrategy::new(1);
        let drill_result = drill
            .extract_from_image(
                self.vision_session.as_ref(),
                self.vision_client.as_ref(),
                &images,
                &self.system_prompt,
            )
            .map_err(|e| {
                ExtractionError::VisionOcrFailed(format!("Vision extraction failed: {e}"))
            })?;

        if let Some(ref dir) = dump_dir {
            diagnostic::dump_json(dir, &format!("03-drill-result-page-{page_idx}.json"), &serde_json::json!({
                "entity_count": drill_result.entities.lab_results.len()
                    + drill_result.entities.medications.len()
                    + drill_result.entities.diagnoses.len()
                    + drill_result.entities.allergies.len()
                    + drill_result.entities.procedures.len()
                    + drill_result.entities.referrals.len()
                    + drill_result.entities.instructions.len(),
                "lab_results": drill_result.entities.lab_results.len(),
                "medications": drill_result.entities.medications.len(),
                "diagnoses": drill_result.entities.diagnoses.len(),
                "has_markdown": !drill_result.markdown.is_empty(),
                "raw_response_count": drill_result.raw_responses.len(),
            }));
        }

        let page_text = format_entities_as_markdown(&drill_result);
        let confidence = compute_drill_confidence(&drill_result);

        Ok(PageExtraction {
            page_number: page_idx + 1,
            text: page_text,
            confidence,
            regions: vec![],
            warnings,
            content_type: Some(ImageContentType::Document),
        })
    }

    /// Route medical images to the interpreter, or fall back to drill extraction.
    fn interpret_medical_page(
        &self,
        image_bytes: &[u8],
        page_idx: usize,
        warnings: Vec<ExtractionWarning>,
    ) -> Result<PageExtraction, ExtractionError> {
        if let Some(ref interpreter) = self.interpreter {
            match interpreter.interpret_medical_image(image_bytes) {
                Ok(findings) => {
                    tracing::info!(
                        model = %findings.model_used,
                        findings_len = findings.findings.len(),
                        "Medical image interpreted"
                    );
                    return Ok(PageExtraction {
                        page_number: page_idx + 1,
                        text: findings.findings,
                        confidence: findings.confidence,
                        regions: vec![],
                        warnings,
                        content_type: Some(ImageContentType::MedicalImage),
                    });
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "Medical image interpretation failed — falling back to drill extraction"
                    );
                }
            }
        }

        // No interpreter or interpretation failed — try drill extraction
        self.extract_document_page(image_bytes, page_idx, warnings, &None)
    }
}

/// Format extracted entities from iterative drill as structured markdown.
///
/// Produces human-readable text that can feed into downstream structuring
/// or be stored as page text.
fn format_entities_as_markdown(
    output: &crate::pipeline::structuring::extraction_strategy::StrategyOutput,
) -> String {
    if output.markdown.is_empty() {
        // Build from typed entities
        let mut parts = Vec::new();
        let e = &output.entities;
        if !e.lab_results.is_empty() {
            let items: Vec<String> = e.lab_results.iter().map(|r| {
                let val = r.value.map_or_else(
                    || r.value_text.clone().unwrap_or_default(),
                    |v| v.to_string(),
                );
                let unit = r.unit.as_deref().unwrap_or("");
                format!("- {}: {} {}", r.test_name, val, unit)
            }).collect();
            parts.push(format!("## Lab Results\n{}", items.join("\n")));
        }
        if !e.medications.is_empty() {
            let items: Vec<String> = e.medications.iter().map(|m| {
                let name = m.generic_name.as_deref().unwrap_or("unknown");
                format!("- {} {}", name, m.dose)
            }).collect();
            parts.push(format!("## Medications\n{}", items.join("\n")));
        }
        if !e.diagnoses.is_empty() {
            let items: Vec<String> = e.diagnoses.iter().map(|d| {
                format!("- {}", d.name)
            }).collect();
            parts.push(format!("## Diagnoses\n{}", items.join("\n")));
        }
        if !e.allergies.is_empty() {
            let items: Vec<String> = e.allergies.iter().map(|a| {
                format!("- {}", a.allergen)
            }).collect();
            parts.push(format!("## Allergies\n{}", items.join("\n")));
        }
        parts.join("\n\n")
    } else {
        output.markdown.clone()
    }
}

/// Compute confidence from drill results based on entity and field completeness.
fn compute_drill_confidence(
    output: &crate::pipeline::structuring::extraction_strategy::StrategyOutput,
) -> f32 {
    let e = &output.entities;
    let total = e.lab_results.len()
        + e.medications.len()
        + e.diagnoses.len()
        + e.allergies.len()
        + e.procedures.len()
        + e.referrals.len()
        + e.instructions.len();

    if total == 0 {
        return 0.3; // Empty extraction — low confidence
    }

    // Base confidence from entity count
    let base: f32 = match total {
        1..=2 => 0.6,
        3..=5 => 0.75,
        _ => 0.85,
    };

    // Bonus for lab results with complete fields (value + unit)
    let complete_labs = e.lab_results.iter().filter(|r| {
        r.value.is_some() && r.unit.is_some()
    }).count();
    let lab_bonus = if !e.lab_results.is_empty() {
        0.1 * (complete_labs as f32 / e.lab_results.len() as f32)
    } else {
        0.0
    };

    (base + lab_bonus).min(0.95)
}

impl TextExtractor for DocumentExtractor {
    fn extract(
        &self,
        document_id: &Uuid,
        staged_path: &Path,
        format: &FormatDetection,
        session: &ProfileSession,
        progress: Option<&crate::pipeline::processor::ProgressTracker>,
    ) -> Result<ExtractionResult, ExtractionError> {
        tracing::info!(
            document_id = %document_id,
            category = format.category.as_str(),
            "Starting text extraction"
        );

        // Diagnostic dump directory (auto in dev, COHEARA_DUMP_DIR in prod)
        let dump_dir = diagnostic::dump_dir_for(document_id);

        // Step 1: Decrypt the staged file
        let decrypted_bytes = read_staged_file(staged_path, session)?;

        // Step 2: Extract based on format category
        let dpi = select_render_dpi(format);

        if let Some(ref dir) = dump_dir {
            diagnostic::dump_json(dir, "00-source-info.json", &serde_json::json!({
                "document_id": document_id.to_string(),
                "mime_type": format.mime_type,
                "category": format.category.as_str(),
                "is_digital_pdf": format.is_digital_pdf,
                "file_size_bytes": format.file_size_bytes,
                "dpi_selected": dpi,
            }));
        }

        let (method, mut pages) = match &format.category {
            // All PDFs → pdfium render → classify → extract
            FileCategory::DigitalPdf | FileCategory::ScannedPdf => {
                self.extract_pdf(&decrypted_bytes, dpi, &dump_dir, progress)?
            }
            // Images → classify → extract
            FileCategory::Image => self.extract_image(&decrypted_bytes, &dump_dir, progress)?,
            // Plain text → UTF-8 read (no model needed)
            FileCategory::PlainText => {
                let text = String::from_utf8(decrypted_bytes)
                    .map_err(|e| ExtractionError::EncodingError(e.to_string()))?;
                let page = PageExtraction {
                    page_number: 1,
                    text,
                    confidence: 0.99,
                    regions: vec![],
                    warnings: vec![],
                    content_type: None,
                };
                (ExtractionMethod::PlainTextRead, vec![page])
            }
            FileCategory::Unsupported => {
                return Err(ExtractionError::UnsupportedFormat);
            }
        };

        // Step 3: Sanitize all extracted text
        for page in &mut pages {
            page.text = sanitize_extracted_text(&page.text);
        }

        // Step 4: Compute overall confidence
        let overall_confidence = compute_overall_confidence(&pages, &method);

        // Step 5: Concatenate full text with page breaks
        let full_text = pages
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n--- Page Break ---\n\n");

        let page_count = pages.len();

        tracing::info!(
            document_id = %document_id,
            method = ?method,
            pages = page_count,
            confidence = overall_confidence,
            text_length = full_text.len(),
            "Text extraction complete"
        );

        let result = ExtractionResult {
            document_id: *document_id,
            method,
            pages,
            full_text,
            overall_confidence,
            language_detected: None, // R3: vision models handle multilingual natively
            page_count,
        };

        if let Some(ref dir) = dump_dir {
            diagnostic::dump_json(dir, "04-extraction-result.json", &result);
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::butler_service::FallbackSession;
    use crate::crypto::profile;
    use crate::pipeline::extraction::pdfium::MockPdfPageRenderer;
    use crate::pipeline::extraction::preprocess::MockImagePreprocessor;
    use crate::pipeline::extraction::types::ImageContentType;
    use crate::pipeline::extraction::vision_classifier::MockVisionClassifier;
    use crate::pipeline::extraction::vision_ocr::MockMedicalImageInterpreter;
    use crate::pipeline::import::staging::stage_file;
    use crate::pipeline::strategy::ContextType;
    use crate::pipeline::structuring::ollama_types::OllamaError;

    fn setup() -> (tempfile::TempDir, ProfileSession) {
        let dir = tempfile::tempdir().unwrap();
        let (info, _phrase) = profile::create_profile(
            dir.path(),
            "ExtractTest",
            "test_pass_123",
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let session = profile::open_profile(dir.path(), &info.id, "test_pass_123").unwrap();
        (dir, session)
    }

    fn stage_text_file(session: &ProfileSession, content: &str) -> (Uuid, std::path::PathBuf) {
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("test.txt");
        std::fs::write(&path, content).unwrap();
        let doc_id = Uuid::new_v4();
        let staged = stage_file(&path, &doc_id, session).unwrap();
        (doc_id, staged)
    }

    fn stage_bytes_file(
        session: &ProfileSession,
        filename: &str,
        content: &[u8],
    ) -> (Uuid, std::path::PathBuf) {
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join(filename);
        std::fs::write(&path, content).unwrap();
        let doc_id = Uuid::new_v4();
        let staged = stage_file(&path, &doc_id, session).unwrap();
        (doc_id, staged)
    }

    /// Mock VisionClient that responds to iterative drill prompts.
    struct DrillVisionClient;

    impl VisionClient for DrillVisionClient {
        fn generate_with_images(
            &self,
            _: &str,
            _: &str,
            _: &[String],
            _: Option<&str>,
        ) -> Result<String, OllamaError> {
            Ok(String::new())
        }

        fn chat_with_images(
            &self,
            _: &str,
            prompt: &str,
            _: &[String],
            _: Option<&str>,
        ) -> Result<String, OllamaError> {
            let lower = prompt.to_lowercase();

            // Enumerate: return lab tests for "tests" domain
            if lower.contains("are visible") {
                if lower.contains("tests") {
                    return Ok("- Hemoglobine\n- Leucocytes".into());
                }
                return Ok("None".into());
            }

            // Drill
            if lower.contains("what is the") {
                if lower.contains("hemoglobine") {
                    if lower.contains("result value") {
                        return Ok("11.2".into());
                    }
                    if lower.contains("unit") {
                        return Ok("g/dL".into());
                    }
                    if lower.contains("whether the result") {
                        return Ok("low".into());
                    }
                }
                if lower.contains("leucocytes") {
                    if lower.contains("result value") {
                        return Ok("7.2".into());
                    }
                    if lower.contains("unit") {
                        return Ok("10^9/L".into());
                    }
                }
                return Ok("not specified".into());
            }

            Ok(String::new())
        }
    }

    fn make_document_extractor() -> DocumentExtractor {
        let session = FallbackSession::new("medgemma:4b", ContextType::VisionOcr, false);

        DocumentExtractor::new(
            Box::new(MockPdfPageRenderer::new(1)),
            Box::new(MockVisionClassifier::document()),
            Box::new(session),
            Box::new(DrillVisionClient),
            "You are a medical document extractor.".into(),
            Box::new(MockImagePreprocessor::new()),
        )
    }

    fn make_document_extractor_pages(page_count: usize) -> DocumentExtractor {
        let session = FallbackSession::new("medgemma:4b", ContextType::VisionOcr, false);

        DocumentExtractor::new(
            Box::new(MockPdfPageRenderer::new(page_count)),
            Box::new(MockVisionClassifier::document()),
            Box::new(session),
            Box::new(DrillVisionClient),
            "You are a medical document extractor.".into(),
            Box::new(MockImagePreprocessor::new()),
        )
    }

    fn make_medical_image_extractor() -> DocumentExtractor {
        let session = FallbackSession::new("medgemma:4b", ContextType::VisionOcr, false);

        DocumentExtractor::new(
            Box::new(MockPdfPageRenderer::new(0)),
            Box::new(MockVisionClassifier::medical_image()),
            Box::new(session),
            Box::new(DrillVisionClient),
            "You are a medical document extractor.".into(),
            Box::new(MockImagePreprocessor::new()),
        )
        .with_interpreter(Box::new(MockMedicalImageInterpreter::new(
            "## Chest X-ray\n\nBilateral infiltrates observed",
            "medgemma:4b",
            0.80,
        )))
    }

    // ── Plain text extraction (unchanged) ──

    #[test]
    fn extract_plain_text_file() {
        let (_dir, session) = setup();
        let content = "Potassium: 4.2 mmol/L (normal range: 3.5-5.0)";
        let (doc_id, staged_path) = stage_text_file(&session, content);

        let extractor = make_document_extractor();

        let format = FormatDetection {
            mime_type: "text/plain".into(),
            category: FileCategory::PlainText,
            is_digital_pdf: None,
            file_size_bytes: content.len() as u64,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session, None)
            .unwrap();

        assert_eq!(result.method, ExtractionMethod::PlainTextRead);
        assert!(result.full_text.contains("Potassium"));
        assert!(result.full_text.contains("4.2 mmol/L"));
        assert!(result.overall_confidence > 0.95);
        assert_eq!(result.page_count, 1);
    }

    #[test]
    fn plain_text_has_high_confidence() {
        let (_dir, session) = setup();
        let (doc_id, staged_path) = stage_text_file(&session, "Medical report content");

        let extractor = make_document_extractor();

        let format = FormatDetection {
            mime_type: "text/plain".into(),
            category: FileCategory::PlainText,
            is_digital_pdf: None,
            file_size_bytes: 100,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session, None)
            .unwrap();

        assert!((result.overall_confidence - 0.99).abs() < 0.01);
    }

    // ── C4-FIX: Image extraction via classify → drill ──

    #[test]
    fn extract_image_uses_drill_primary() {
        let (_dir, session) = setup();

        let img = image::RgbImage::from_pixel(32, 32, image::Rgb([100u8, 150, 200]));
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("scan.jpg");
        img.save(&path).unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        let extractor = make_document_extractor();

        let format = FormatDetection {
            mime_type: "image/jpeg".into(),
            category: FileCategory::Image,
            is_digital_pdf: None,
            file_size_bytes: 1000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session, None)
            .unwrap();

        assert_eq!(result.method, ExtractionMethod::VisionOcr);
        assert!(result.full_text.contains("Hemoglobine"));
        assert!(result.full_text.contains("Leucocytes"));
        assert_eq!(result.page_count, 1);
        assert_eq!(result.pages[0].content_type, Some(ImageContentType::Document));
    }

    // ── PDF extraction via classify → drill ──

    #[test]
    fn extract_pdf_renders_and_drills_all_pages() {
        let (_dir, session) = setup();
        let (doc_id, staged_path) = stage_bytes_file(&session, "report.pdf", b"fake pdf content");

        let extractor = make_document_extractor_pages(3);

        let format = FormatDetection {
            mime_type: "application/pdf".into(),
            category: FileCategory::DigitalPdf,
            is_digital_pdf: Some(true),
            file_size_bytes: 5000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session, None)
            .unwrap();

        assert_eq!(result.method, ExtractionMethod::VisionOcr);
        assert_eq!(result.page_count, 3);
        assert_eq!(result.pages.len(), 3);
        for (i, page) in result.pages.iter().enumerate() {
            assert_eq!(page.page_number, i + 1);
            assert!(page.text.contains("Hemoglobine"));
        }
    }

    #[test]
    fn extract_scanned_pdf_same_path_as_digital() {
        let (_dir, session) = setup();
        let (doc_id, staged_path) =
            stage_bytes_file(&session, "scanned.pdf", b"fake scanned pdf");

        let extractor = make_document_extractor_pages(2);

        let format = FormatDetection {
            mime_type: "application/pdf".into(),
            category: FileCategory::ScannedPdf,
            is_digital_pdf: Some(false),
            file_size_bytes: 50000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session, None)
            .unwrap();

        assert_eq!(result.method, ExtractionMethod::VisionOcr);
        assert_eq!(result.page_count, 2);
        assert!(result.full_text.contains("Hemoglobine"));
    }

    #[test]
    fn extract_multipage_has_page_breaks() {
        let (_dir, session) = setup();
        let (doc_id, staged_path) = stage_bytes_file(&session, "multi.pdf", b"fake multi pdf");

        let extractor = make_document_extractor_pages(2);

        let format = FormatDetection {
            mime_type: "application/pdf".into(),
            category: FileCategory::DigitalPdf,
            is_digital_pdf: Some(true),
            file_size_bytes: 5000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session, None)
            .unwrap();

        assert!(
            result.full_text.contains("--- Page Break ---"),
            "Multi-page text should contain page break markers"
        );
    }

    // ── Error handling ──

    #[test]
    fn extract_unsupported_format_rejected() {
        let (_dir, session) = setup();
        let (doc_id, staged_path) = stage_text_file(&session, "whatever");

        let extractor = make_document_extractor();

        let format = FormatDetection {
            mime_type: "application/octet-stream".into(),
            category: FileCategory::Unsupported,
            is_digital_pdf: None,
            file_size_bytes: 100,
        };

        let result = extractor.extract(&doc_id, &staged_path, &format, &session, None);
        assert!(matches!(result, Err(ExtractionError::UnsupportedFormat)));
    }

    // ── Sanitization ──

    #[test]
    fn extract_sanitizes_text() {
        let (_dir, session) = setup();
        let content = "Patient: Marie\x00Dubois\x01\nDose: 500mg";
        let (doc_id, staged_path) = stage_text_file(&session, content);

        let extractor = make_document_extractor();

        let format = FormatDetection {
            mime_type: "text/plain".into(),
            category: FileCategory::PlainText,
            is_digital_pdf: None,
            file_size_bytes: 100,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session, None)
            .unwrap();

        assert!(!result.full_text.contains('\x00'));
        assert!(!result.full_text.contains('\x01'));
        assert!(result.full_text.contains("500mg"));
    }

    // ── Document ID propagation ──

    #[test]
    fn extract_returns_correct_document_id() {
        let (_dir, session) = setup();
        let (doc_id, staged_path) = stage_text_file(&session, "test content");

        let extractor = make_document_extractor();

        let format = FormatDetection {
            mime_type: "text/plain".into(),
            category: FileCategory::PlainText,
            is_digital_pdf: None,
            file_size_bytes: 100,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session, None)
            .unwrap();

        assert_eq!(result.document_id, doc_id);
    }

    // ── Confidence ──

    #[test]
    fn drill_confidence_computed_from_entities() {
        let extractor = make_document_extractor();
        let prepared = extractor.preprocessor.preprocess(&[0u8; 10]).unwrap();

        let page = extractor
            .extract_document_page(&prepared.png_bytes, 0, vec![], &None)
            .unwrap();

        // 2 lab results with value+unit = base 0.6 + lab bonus
        assert!(page.confidence > 0.5, "Confidence: {}", page.confidence);
        assert!(page.confidence <= 0.95);
    }

    // ── French text preservation ──

    #[test]
    fn french_accents_survive_pipeline() {
        let (_dir, session) = setup();
        let content =
            "Résultats d'analyses biologiques\nCréatinine: 72 µmol/L\nÉnalapril 10mg à jeun";
        let (doc_id, staged_path) = stage_text_file(&session, content);

        let extractor = make_document_extractor();

        let format = FormatDetection {
            mime_type: "text/plain".into(),
            category: FileCategory::PlainText,
            is_digital_pdf: None,
            file_size_bytes: content.len() as u64,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session, None)
            .unwrap();

        assert!(result.full_text.contains("Résultats"));
        assert!(result.full_text.contains("Créatinine"));
        assert!(result.full_text.contains("µmol/L"));
        assert!(result.full_text.contains("Énalapril"));
        assert!(result.full_text.contains("à jeun"));
    }

    #[test]
    fn language_detected_is_none() {
        let (_dir, session) = setup();
        let (doc_id, staged_path) = stage_text_file(&session, "English text for testing");

        let extractor = make_document_extractor();

        let format = FormatDetection {
            mime_type: "text/plain".into(),
            category: FileCategory::PlainText,
            is_digital_pdf: None,
            file_size_bytes: 100,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session, None)
            .unwrap();

        assert!(
            result.language_detected.is_none(),
            "R3: language_detected should be None (vision models are multilingual)"
        );
    }

    // ── Adaptive DPI selection ──

    #[test]
    fn dpi_scanned_pdf_is_300() {
        let format = FormatDetection {
            mime_type: "application/pdf".into(),
            category: FileCategory::ScannedPdf,
            is_digital_pdf: Some(false),
            file_size_bytes: 50000,
        };
        assert_eq!(select_render_dpi(&format), 300);
    }

    #[test]
    fn dpi_digital_pdf_is_200() {
        let format = FormatDetection {
            mime_type: "application/pdf".into(),
            category: FileCategory::DigitalPdf,
            is_digital_pdf: Some(true),
            file_size_bytes: 5000,
        };
        assert_eq!(select_render_dpi(&format), 200);
    }

    #[test]
    fn dpi_image_is_200() {
        let format = FormatDetection {
            mime_type: "image/jpeg".into(),
            category: FileCategory::Image,
            is_digital_pdf: None,
            file_size_bytes: 1000,
        };
        assert_eq!(select_render_dpi(&format), 200);
    }

    #[test]
    fn dpi_plain_text_is_200() {
        let format = FormatDetection {
            mime_type: "text/plain".into(),
            category: FileCategory::PlainText,
            is_digital_pdf: None,
            file_size_bytes: 100,
        };
        assert_eq!(select_render_dpi(&format), 200);
    }

    // ── C4-FIX: Content-type routing via classifier ──

    #[test]
    fn classify_document_routes_to_drill() {
        let extractor = make_document_extractor();
        let prepared = extractor.preprocessor.preprocess(&[0u8; 10]).unwrap();

        let page = extractor
            .extract_document_page(&prepared.png_bytes, 0, vec![], &None)
            .unwrap();

        assert!(page.text.contains("Hemoglobine"));
        assert!(page.text.contains("Leucocytes"));
        assert_eq!(page.content_type, Some(ImageContentType::Document));
    }

    #[test]
    fn classify_medical_image_routes_to_interpreter() {
        let (_dir, session) = setup();

        let img = image::RgbImage::from_pixel(32, 32, image::Rgb([50u8, 50, 50]));
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("xray.jpg");
        img.save(&path).unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        let extractor = make_medical_image_extractor();

        let format = FormatDetection {
            mime_type: "image/jpeg".into(),
            category: FileCategory::Image,
            is_digital_pdf: None,
            file_size_bytes: 1000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session, None)
            .unwrap();

        assert!(
            result.full_text.contains("Bilateral infiltrates"),
            "Expected findings, got: {}",
            result.full_text
        );
        assert_eq!(
            result.pages[0].content_type,
            Some(ImageContentType::MedicalImage)
        );
        assert!((result.pages[0].confidence - 0.80).abs() < f32::EPSILON);
    }

    #[test]
    fn classify_medical_no_interpreter_falls_back_to_drill() {
        let session = FallbackSession::new("medgemma:4b", ContextType::VisionOcr, false);

        // Classifier says MedicalImage but NO interpreter set
        let extractor = DocumentExtractor::new(
            Box::new(MockPdfPageRenderer::new(0)),
            Box::new(MockVisionClassifier::medical_image()),
            Box::new(session),
            Box::new(DrillVisionClient),
            "You are a medical document extractor.".into(),
            Box::new(MockImagePreprocessor::new()),
        );

        let prepared = extractor.preprocessor.preprocess(&[0u8; 10]).unwrap();
        let page = extractor
            .interpret_medical_page(&prepared.png_bytes, 0, vec![])
            .unwrap();

        // Falls back to drill extraction since no interpreter
        assert!(
            page.text.contains("Hemoglobine"),
            "Expected drill fallback, got: {}",
            page.text
        );
    }

    #[test]
    fn document_content_type_propagated() {
        let (_dir, session) = setup();

        let img = image::RgbImage::from_pixel(32, 32, image::Rgb([100u8, 150, 200]));
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("doc.jpg");
        img.save(&path).unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        let extractor = make_document_extractor();

        let format = FormatDetection {
            mime_type: "image/jpeg".into(),
            category: FileCategory::Image,
            is_digital_pdf: None,
            file_size_bytes: 1000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session, None)
            .unwrap();

        assert_eq!(
            result.pages[0].content_type,
            Some(ImageContentType::Document)
        );
    }

    #[test]
    fn plain_text_has_no_content_type() {
        let (_dir, session) = setup();
        let (doc_id, staged_path) = stage_text_file(&session, "Some medical text");

        let extractor = make_document_extractor();

        let format = FormatDetection {
            mime_type: "text/plain".into(),
            category: FileCategory::PlainText,
            is_digital_pdf: None,
            file_size_bytes: 100,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session, None)
            .unwrap();

        assert_eq!(result.pages[0].content_type, None);
    }

    #[test]
    fn pdf_pages_get_document_content_type() {
        let (_dir, session) = setup();
        let (doc_id, staged_path) = stage_bytes_file(&session, "report.pdf", b"fake pdf");

        let extractor = make_document_extractor_pages(2);

        let format = FormatDetection {
            mime_type: "application/pdf".into(),
            category: FileCategory::DigitalPdf,
            is_digital_pdf: Some(true),
            file_size_bytes: 5000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session, None)
            .unwrap();

        for page in &result.pages {
            assert_eq!(page.content_type, Some(ImageContentType::Document));
        }
    }

    // ── format_entities_as_markdown ──

    #[test]
    fn entities_formatted_as_markdown() {
        use crate::pipeline::structuring::extraction_strategy::StrategyOutput;
        use crate::pipeline::structuring::types::{ExtractedEntities, ExtractedLabResult};

        let mut entities = ExtractedEntities::default();
        entities.lab_results.push(ExtractedLabResult {
            test_name: "Hemoglobine".into(),
            test_code: None,
            value: Some(11.2),
            value_text: None,
            unit: Some("g/dL".into()),
            reference_range_low: None,
            reference_range_high: None,
            reference_range_text: None,
            abnormal_flag: None,
            collection_date: None,
            confidence: 0.0,
        });

        let output = StrategyOutput {
            entities,
            markdown: String::new(),
            document_type: None,
            document_date: None,
            professional: None,
            raw_responses: vec![],
        };

        let text = format_entities_as_markdown(&output);
        assert!(text.contains("## Lab Results"));
        assert!(text.contains("Hemoglobine"));
        assert!(text.contains("11.2"));
        assert!(text.contains("g/dL"));
    }

    #[test]
    fn entities_use_markdown_when_available() {
        use crate::pipeline::structuring::extraction_strategy::StrategyOutput;
        use crate::pipeline::structuring::types::ExtractedEntities;

        let output = StrategyOutput {
            entities: ExtractedEntities::default(),
            markdown: "## Lab Results\n- Hemoglobine: 11.2 g/dL".into(),
            document_type: None,
            document_date: None,
            professional: None,
            raw_responses: vec![],
        };

        let text = format_entities_as_markdown(&output);
        assert_eq!(text, "## Lab Results\n- Hemoglobine: 11.2 g/dL");
    }

    // ── compute_drill_confidence ──

    #[test]
    fn drill_confidence_empty_is_low() {
        use crate::pipeline::structuring::extraction_strategy::StrategyOutput;
        use crate::pipeline::structuring::types::ExtractedEntities;

        let output = StrategyOutput {
            entities: ExtractedEntities::default(),
            markdown: String::new(),
            document_type: None,
            document_date: None,
            professional: None,
            raw_responses: vec![],
        };

        let conf = compute_drill_confidence(&output);
        assert!((conf - 0.3).abs() < f32::EPSILON, "Empty: {conf}");
    }

    #[test]
    fn drill_confidence_many_entities_is_high() {
        use crate::pipeline::structuring::extraction_strategy::StrategyOutput;
        use crate::pipeline::structuring::types::{ExtractedEntities, ExtractedLabResult};

        let mut entities = ExtractedEntities::default();
        for i in 0..10 {
            entities.lab_results.push(ExtractedLabResult {
                test_name: format!("Test{i}"),
                test_code: None,
                value: Some(i as f64),
                value_text: None,
                unit: Some("mg/dL".into()),
                reference_range_low: None,
                reference_range_high: None,
                reference_range_text: None,
                abnormal_flag: None,
                collection_date: None,
                confidence: 0.0,
            });
        }

        let output = StrategyOutput {
            entities,
            markdown: String::new(),
            document_type: None,
            document_date: None,
            professional: None,
            raw_responses: vec![],
        };

        let conf = compute_drill_confidence(&output);
        assert!(conf > 0.85, "10 complete labs: {conf}");
        assert!(conf <= 0.95);
    }
}
