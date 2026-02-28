//! R4: Document extraction orchestrator.
//!
//! Pipeline:
//! - pdfium-render for PDF → image conversion
//! - MedGemma vision model for image → structured Markdown
//!
//! All PDFs go through the same path: render pages → vision OCR.
//! Images go directly to vision OCR.
//! Plain text is read directly (no model needed).

use std::path::Path;

use base64::Engine;
use uuid::Uuid;

use super::confidence::compute_overall_confidence;
use super::preprocess::ImagePreprocessor;
use super::sanitize::sanitize_extracted_text;
use super::types::{
    ExtractionMethod, ExtractionResult, ImageContentType, MedicalImageInterpreter,
    PageExtraction, PdfPageRenderer, TextExtractor, VisionOcrEngine,
};
use super::vision_ocr::{build_system_prompt, build_user_prompt};
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

/// C4: Vision fallback configuration for hybrid OCR.
///
/// When monolithic OCR degenerates, the orchestrator falls back to iterative
/// vision Q&A using a `FallbackSession` and shared `VisionClient`.
///
/// Uses `FallbackSession` (no lifetime, no guard) instead of `ButlerSession`
/// because the caller (import_queue_worker) already holds the butler guard.
/// The fallback replicates the defense pipeline (sanitize + quality gate)
/// without acquiring a second lock (which would deadlock).
pub struct VisionFallback {
    /// Unguarded session — caller holds the butler guard separately.
    pub session: Box<dyn VisionSession>,
    /// Vision client for iterative Q&A calls (shared with normal OCR path).
    pub vision_client: Box<dyn VisionClient>,
    /// System prompt for vision Q&A.
    pub system_prompt: String,
}

/// R4+: Vision-based document extraction orchestrator.
///
/// Three required + two optional dependencies (trait objects for DI):
/// - `pdf_renderer`: Renders PDF pages to PNG images (PdfiumRenderer in prod)
/// - `vision_ocr`: Extracts text from images via vision model (OllamaVisionOcr in prod)
/// - `preprocessor`: Prepares images for optimal vision model input (PreprocessingPipeline in prod)
/// - `interpreter` (optional): Interprets medical images (OllamaMedicalImageInterpreter in prod)
/// - `vision_fallback` (optional): C4 hybrid fallback for OCR degeneration
pub struct DocumentExtractor {
    pdf_renderer: Box<dyn PdfPageRenderer>,
    vision_ocr: Box<dyn VisionOcrEngine>,
    preprocessor: Box<dyn ImagePreprocessor>,
    interpreter: Option<Box<dyn MedicalImageInterpreter>>,
    /// C4: Fallback for OCR degeneration — iterative vision Q&A.
    vision_fallback: Option<VisionFallback>,
    /// Language code for diagnostic prompt dumps (e.g., "en", "fr", "de").
    language: String,
}

impl DocumentExtractor {
    pub fn new(
        pdf_renderer: Box<dyn PdfPageRenderer>,
        vision_ocr: Box<dyn VisionOcrEngine>,
        preprocessor: Box<dyn ImagePreprocessor>,
    ) -> Self {
        Self {
            pdf_renderer,
            vision_ocr,
            preprocessor,
            interpreter: None,
            vision_fallback: None,
            language: "en".to_string(),
        }
    }

    /// Set the medical image interpreter for content-type routing.
    ///
    /// When set, pages classified as `MedicalImage` by the vision OCR model
    /// are routed to the interpreter for clinical findings instead of OCR text.
    pub fn with_interpreter(mut self, interpreter: Box<dyn MedicalImageInterpreter>) -> Self {
        self.interpreter = Some(interpreter);
        self
    }

    /// C4: Set the vision fallback for hybrid OCR.
    ///
    /// When monolithic OCR degenerates (repetition loops, quality gate failures),
    /// the orchestrator falls back to iterative vision Q&A that asks focused
    /// single-field questions per domain.
    pub fn with_vision_fallback(mut self, fallback: VisionFallback) -> Self {
        self.vision_fallback = Some(fallback);
        self
    }

    /// Set the language for diagnostic prompt dumps.
    pub fn with_language(mut self, language: &str) -> Self {
        self.language = language.to_string();
        self
    }

    /// Extract text from a PDF: render each page, preprocess, then vision OCR.
    ///
    /// `dpi` is selected by `select_render_dpi()` based on document format.
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

            if let Some(ref dir) = dump_dir {
                diagnostic::dump_text(dir, &format!("03-vision-ocr-prompt-page-{page_idx}.txt"), &format!(
                    "=== SYSTEM PROMPT ===\n{}\n\n=== USER PROMPT ===\n{}",
                    build_system_prompt(&self.language),
                    build_user_prompt(&self.language),
                ));
            }

            let ocr_result = self.vision_ocr.extract_text_from_image(&prepared.png_bytes);

            if let Some(ref dir) = dump_dir {
                match &ocr_result {
                    Ok(result) => diagnostic::dump_json(dir, &format!("03-vision-ocr-result-page-{page_idx}.json"), &serde_json::json!({
                        "text": result.text,
                        "model_used": result.model_used,
                        "confidence": result.confidence,
                        "content_type": format!("{:?}", result.content_type),
                    })),
                    Err(e) => diagnostic::dump_json(dir, &format!("03-vision-ocr-result-page-{page_idx}.json"), &serde_json::json!({
                        "error": e.to_string(),
                    })),
                }
            }

            match ocr_result {
                Ok(result) => {
                    // Fast path: OCR succeeded
                    let (text, confidence, content_type) =
                        self.route_by_content_type(&prepared.png_bytes, &result);

                    pages.push(PageExtraction {
                        page_number: page_idx + 1,
                        text,
                        confidence,
                        regions: vec![],
                        warnings: prepared.warnings,
                        content_type: Some(content_type),
                    });
                }
                Err(ExtractionError::VisionDegeneration {
                    pattern,
                    tokens_before_abort: _,
                    partial_output: _,
                }) => {
                    // C4: OCR degenerated — try iterative vision fallback
                    let page = self.try_vision_fallback(
                        &prepared.png_bytes,
                        page_idx,
                        &pattern,
                        prepared.warnings,
                    )?;
                    pages.push(page);
                }
                Err(e) => return Err(e),
            }

            // §22: Update extraction page progress
            if let Some(tracker) = progress {
                tracker.page_current.store((page_idx + 1).min(255) as u8, std::sync::atomic::Ordering::Relaxed);
            }
        }

        Ok((ExtractionMethod::VisionOcr, pages))
    }

    /// Extract text from an image: preprocess, then vision OCR.
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

        if let Some(ref dir) = dump_dir {
            diagnostic::dump_text(dir, "03-vision-ocr-prompt-page-0.txt", &format!(
                "=== SYSTEM PROMPT ===\n{}\n\n=== USER PROMPT ===\n{}",
                build_system_prompt(&self.language),
                build_user_prompt(&self.language),
            ));
        }

        let ocr_result = self.vision_ocr.extract_text_from_image(&prepared.png_bytes);

        if let Some(ref dir) = dump_dir {
            match &ocr_result {
                Ok(result) => diagnostic::dump_json(dir, "03-vision-ocr-result-page-0.json", &serde_json::json!({
                    "text": result.text,
                    "model_used": result.model_used,
                    "confidence": result.confidence,
                    "content_type": format!("{:?}", result.content_type),
                })),
                Err(e) => diagnostic::dump_json(dir, "03-vision-ocr-result-page-0.json", &serde_json::json!({
                    "error": e.to_string(),
                })),
            }
        }

        let page = match ocr_result {
            Ok(result) => {
                let (text, confidence, content_type) =
                    self.route_by_content_type(&prepared.png_bytes, &result);

                PageExtraction {
                    page_number: 1,
                    text,
                    confidence,
                    regions: vec![],
                    warnings: prepared.warnings,
                    content_type: Some(content_type),
                }
            }
            Err(ExtractionError::VisionDegeneration {
                pattern,
                tokens_before_abort: _,
                partial_output: _,
            }) => {
                // C4: OCR degenerated — try iterative vision fallback
                self.try_vision_fallback(
                    &prepared.png_bytes,
                    0,
                    &pattern,
                    prepared.warnings,
                )?
            }
            Err(e) => return Err(e),
        };

        // §22: Single-page image — set progress as complete
        if let Some(tracker) = progress {
            tracker.page_total.store(1, std::sync::atomic::Ordering::Relaxed);
            tracker.page_current.store(1, std::sync::atomic::Ordering::Relaxed);
        }

        Ok((ExtractionMethod::VisionOcr, vec![page]))
    }

    /// Route page based on content type classification.
    ///
    /// - Document → return OCR text as-is
    /// - MedicalImage + interpreter → interpret original image → clinical findings
    /// - MedicalImage + no interpreter → fallback to OCR text
    fn route_by_content_type(
        &self,
        image_bytes: &[u8],
        ocr_result: &super::types::VisionOcrResult,
    ) -> (String, f32, ImageContentType) {
        if ocr_result.content_type == ImageContentType::MedicalImage {
            if let Some(ref interpreter) = self.interpreter {
                match interpreter.interpret_medical_image(image_bytes) {
                    Ok(findings) => {
                        tracing::info!(
                            model = %findings.model_used,
                            findings_len = findings.findings.len(),
                            "Medical image interpreted — replacing OCR text with findings"
                        );
                        return (
                            findings.findings,
                            findings.confidence,
                            ImageContentType::MedicalImage,
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "Medical image interpretation failed — falling back to OCR text"
                        );
                        // Fall through to return OCR text
                    }
                }
            }
        }
        (
            ocr_result.text.clone(),
            ocr_result.confidence,
            ocr_result.content_type,
        )
    }

    /// C4: Attempt iterative vision Q&A fallback for a degenerated page.
    ///
    /// Encodes the preprocessed PNG as base64 and runs
    /// `IterativeDrillStrategy::extract_from_image()` to extract entities
    /// with focused single-question prompts via the fallback session.
    fn try_vision_fallback(
        &self,
        png_bytes: &[u8],
        page_idx: usize,
        pattern: &str,
        mut warnings: Vec<ExtractionWarning>,
    ) -> Result<PageExtraction, ExtractionError> {
        let fallback = self.vision_fallback.as_ref().ok_or_else(|| {
            ExtractionError::VisionOcrFailed(format!(
                "OCR degenerated ({pattern}) and no fallback configured"
            ))
        })?;

        tracing::warn!(
            page = page_idx,
            pattern = %pattern,
            "C4: OCR degenerated — falling back to iterative vision Q&A"
        );

        let base64_image =
            base64::engine::general_purpose::STANDARD.encode(png_bytes);
        let images = vec![base64_image];

        let drill = IterativeDrillStrategy::new(1);
        let drill_result = drill
            .extract_from_image(
                fallback.session.as_ref(),
                fallback.vision_client.as_ref(),
                &images,
                &fallback.system_prompt,
            )
            .map_err(|e| {
                ExtractionError::VisionOcrFailed(format!("Fallback vision Q&A failed: {e}"))
            })?;

        let page_text = format_entities_as_markdown(&drill_result);

        warnings.push(ExtractionWarning::FallbackUsed {
            reason: format!("OCR degenerated: {pattern}"),
        });

        Ok(PageExtraction {
            page_number: page_idx + 1,
            text: page_text,
            confidence: 0.7,
            regions: vec![],
            warnings,
            content_type: Some(ImageContentType::Document),
        })
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
        // Fallback: list entities by domain
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
            // All PDFs → pdfium render → vision OCR (adaptive DPI per format)
            FileCategory::DigitalPdf | FileCategory::ScannedPdf => {
                self.extract_pdf(&decrypted_bytes, dpi, &dump_dir, progress)?
            }
            // Images → vision OCR directly
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
    use crate::crypto::profile;
    use crate::pipeline::extraction::pdfium::MockPdfPageRenderer;
    use crate::pipeline::extraction::preprocess::MockImagePreprocessor;
    use crate::pipeline::extraction::types::ImageContentType;
    use crate::pipeline::extraction::vision_ocr::{
        MockMedicalImageInterpreter, MockVisionOcr,
    };
    use crate::pipeline::import::staging::stage_file;

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

    fn make_extractor(page_count: usize, ocr_text: &str, confidence: f32) -> DocumentExtractor {
        DocumentExtractor::new(
            Box::new(MockPdfPageRenderer::new(page_count)),
            Box::new(
                MockVisionOcr::new(ocr_text, "mock-vision").with_confidence(confidence),
            ),
            Box::new(MockImagePreprocessor::new()),
        )
    }

    // ── Plain text extraction ──

    #[test]
    fn extract_plain_text_file() {
        let (_dir, session) = setup();
        let content = "Potassium: 4.2 mmol/L (normal range: 3.5-5.0)";
        let (doc_id, staged_path) = stage_text_file(&session, content);

        let extractor = make_extractor(0, "unused", 0.0);

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

        let extractor = make_extractor(0, "unused", 0.0);

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

    // ── Image extraction via vision OCR ──

    #[test]
    fn extract_image_uses_vision_ocr() {
        let (_dir, session) = setup();

        let img = image::RgbImage::from_pixel(32, 32, image::Rgb([100u8, 150, 200]));
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("scan.jpg");
        img.save(&path).unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        let extractor = make_extractor(0, "# Lab Report\n\nPotassium: 4.2 mmol/L", 0.85);

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
        assert!(result.full_text.contains("Potassium"));
        assert!(result.full_text.contains("4.2 mmol/L"));
        assert_eq!(result.page_count, 1);
    }

    // ── PDF extraction ──

    #[test]
    fn extract_pdf_renders_and_ocrs_all_pages() {
        let (_dir, session) = setup();
        let (doc_id, staged_path) = stage_bytes_file(&session, "report.pdf", b"fake pdf content");

        let extractor = make_extractor(3, "Extracted page content", 0.82);

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
            assert!(page.text.contains("Extracted page content"));
            assert!((page.confidence - 0.82).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn extract_scanned_pdf_same_path_as_digital() {
        let (_dir, session) = setup();
        let (doc_id, staged_path) =
            stage_bytes_file(&session, "scanned.pdf", b"fake scanned pdf");

        let extractor = make_extractor(2, "OCR result from vision model", 0.78);

        let format = FormatDetection {
            mime_type: "application/pdf".into(),
            category: FileCategory::ScannedPdf,
            is_digital_pdf: Some(false),
            file_size_bytes: 50000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session, None)
            .unwrap();

        // R3: Both DigitalPdf and ScannedPdf use the same VisionOcr path
        assert_eq!(result.method, ExtractionMethod::VisionOcr);
        assert_eq!(result.page_count, 2);
        assert!(result.full_text.contains("OCR result from vision model"));
    }

    #[test]
    fn extract_multipage_has_page_breaks() {
        let (_dir, session) = setup();
        let (doc_id, staged_path) = stage_bytes_file(&session, "multi.pdf", b"fake multi pdf");

        let extractor = make_extractor(2, "Page content", 0.90);

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

        let extractor = make_extractor(0, "unused", 0.0);

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

        let extractor = make_extractor(0, "unused", 0.0);

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

        let extractor = make_extractor(0, "unused", 0.0);

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
    fn pdf_confidence_computed_from_pages() {
        let (_dir, session) = setup();
        let (doc_id, staged_path) = stage_bytes_file(&session, "test.pdf", b"fake pdf");

        let extractor = make_extractor(2, "Some extracted content here", 0.80);

        let format = FormatDetection {
            mime_type: "application/pdf".into(),
            category: FileCategory::DigitalPdf,
            is_digital_pdf: Some(true),
            file_size_bytes: 5000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session, None)
            .unwrap();

        // VisionOcr confidence uses weighted average — both pages have same text/confidence
        assert!(
            (result.overall_confidence - 0.80).abs() < 0.01,
            "Expected ~0.80, got {}",
            result.overall_confidence
        );
    }

    // ── French text preservation ──

    #[test]
    fn french_accents_survive_pipeline() {
        let (_dir, session) = setup();
        let content =
            "Résultats d'analyses biologiques\nCréatinine: 72 µmol/L\nÉnalapril 10mg à jeun";
        let (doc_id, staged_path) = stage_text_file(&session, content);

        let extractor = make_extractor(0, "unused", 0.0);

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

    // ── R3: No language detection (vision models handle multilingual natively) ──

    #[test]
    fn language_detected_is_none() {
        let (_dir, session) = setup();
        let (doc_id, staged_path) = stage_text_file(&session, "English text for testing");

        let extractor = make_extractor(0, "unused", 0.0);

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

    // ── Content-type routing ──

    #[test]
    fn document_content_type_propagated() {
        let (_dir, session) = setup();

        let img = image::RgbImage::from_pixel(32, 32, image::Rgb([100u8, 150, 200]));
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("doc.jpg");
        img.save(&path).unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        // MockVisionOcr defaults to Document content type
        let extractor = make_extractor(0, "# Lab Report\nContent", 0.85);

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
    fn medical_image_with_interpreter_uses_findings() {
        let (_dir, session) = setup();

        let img = image::RgbImage::from_pixel(32, 32, image::Rgb([50u8, 50, 50]));
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("xray.jpg");
        img.save(&path).unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        let extractor = DocumentExtractor::new(
            Box::new(MockPdfPageRenderer::new(0)),
            Box::new(
                MockVisionOcr::new("Some OCR text", "mock-vision")
                    .with_content_type(ImageContentType::MedicalImage)
                    .with_confidence(0.3),
            ),
            Box::new(MockImagePreprocessor::new()),
        )
        .with_interpreter(Box::new(MockMedicalImageInterpreter::new(
            "## Chest X-ray\n\nBilateral infiltrates observed",
            "medgemma:4b",
            0.80,
        )));

        let format = FormatDetection {
            mime_type: "image/jpeg".into(),
            category: FileCategory::Image,
            is_digital_pdf: None,
            file_size_bytes: 1000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session, None)
            .unwrap();

        // Interpreter findings replace OCR text
        assert!(
            result.full_text.contains("Bilateral infiltrates"),
            "Expected findings, got: {}",
            result.full_text
        );
        assert!(
            !result.full_text.contains("Some OCR text"),
            "OCR text should be replaced"
        );
        assert_eq!(
            result.pages[0].content_type,
            Some(ImageContentType::MedicalImage)
        );
        // Confidence from interpreter, not OCR
        assert!((result.pages[0].confidence - 0.80).abs() < f32::EPSILON);
    }

    #[test]
    fn medical_image_without_interpreter_falls_back_to_ocr() {
        let (_dir, session) = setup();

        let img = image::RgbImage::from_pixel(32, 32, image::Rgb([50u8, 50, 50]));
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("xray.jpg");
        img.save(&path).unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        // No interpreter set — should fall back to OCR text
        let extractor = DocumentExtractor::new(
            Box::new(MockPdfPageRenderer::new(0)),
            Box::new(
                MockVisionOcr::new("OCR fallback text for X-ray", "mock-vision")
                    .with_content_type(ImageContentType::MedicalImage)
                    .with_confidence(0.35),
            ),
            Box::new(MockImagePreprocessor::new()),
        );

        let format = FormatDetection {
            mime_type: "image/jpeg".into(),
            category: FileCategory::Image,
            is_digital_pdf: None,
            file_size_bytes: 1000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session, None)
            .unwrap();

        // Falls back to OCR text
        assert!(
            result.full_text.contains("OCR fallback text"),
            "Expected OCR fallback, got: {}",
            result.full_text
        );
        assert_eq!(
            result.pages[0].content_type,
            Some(ImageContentType::MedicalImage)
        );
    }

    #[test]
    fn plain_text_has_no_content_type() {
        let (_dir, session) = setup();
        let (doc_id, staged_path) = stage_text_file(&session, "Some medical text");

        let extractor = make_extractor(0, "unused", 0.0);

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
    fn pdf_pages_get_content_type() {
        let (_dir, session) = setup();
        let (doc_id, staged_path) = stage_bytes_file(&session, "report.pdf", b"fake pdf");

        let extractor = make_extractor(2, "Page text", 0.85);

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

    // ── C4: Vision fallback tests ──

    use crate::pipeline::extraction::types::VisionOcrResult;
    use crate::pipeline::structuring::ollama_types::OllamaError;

    /// Mock VisionOcr that always returns VisionDegeneration.
    struct DegeneratingVisionOcr;

    impl VisionOcrEngine for DegeneratingVisionOcr {
        fn extract_text_from_image(
            &self,
            _image_bytes: &[u8],
        ) -> Result<VisionOcrResult, ExtractionError> {
            Err(ExtractionError::VisionDegeneration {
                pattern: "low_diversity(0.09)".into(),
                tokens_before_abort: 1370,
                partial_output: "Titre Titre Titre".into(),
            })
        }
    }

    /// Mock VisionClient that responds to iterative drill prompts.
    struct FallbackVisionClient;

    impl crate::pipeline::structuring::types::VisionClient for FallbackVisionClient {
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

    fn make_extractor_with_fallback() -> DocumentExtractor {
        use crate::butler_service::FallbackSession;
        use crate::pipeline::strategy::ContextType;

        let session = FallbackSession::new("medgemma:4b", ContextType::VisionOcr, false);

        let fallback = VisionFallback {
            session: Box::new(session),
            vision_client: Box::new(FallbackVisionClient),
            system_prompt: "You are a medical document extractor.".into(),
        };

        DocumentExtractor::new(
            Box::new(MockPdfPageRenderer::new(1)),
            Box::new(DegeneratingVisionOcr),
            Box::new(MockImagePreprocessor::new()),
        )
        .with_vision_fallback(fallback)
    }

    #[test]
    fn ocr_degeneration_triggers_fallback() {
        let extractor = make_extractor_with_fallback();
        let prepared = extractor.preprocessor.preprocess(&[0u8; 10]).unwrap();

        let page = extractor
            .try_vision_fallback(&prepared.png_bytes, 0, "low_diversity(0.09)", vec![])
            .unwrap();

        assert!(page.text.contains("Hemoglobine"));
        assert!(page.text.contains("Leucocytes"));
        assert_eq!(page.confidence, 0.7);
        assert_eq!(page.content_type, Some(ImageContentType::Document));
    }

    #[test]
    fn fallback_warning_includes_reason() {
        let extractor = make_extractor_with_fallback();
        let prepared = extractor.preprocessor.preprocess(&[0u8; 10]).unwrap();

        let page = extractor
            .try_vision_fallback(&prepared.png_bytes, 0, "low_diversity(0.09)", vec![])
            .unwrap();

        let has_fallback_warning = page.warnings.iter().any(|w| {
            matches!(w, ExtractionWarning::FallbackUsed { reason } if reason.contains("low_diversity"))
        });
        assert!(has_fallback_warning);
    }

    #[test]
    fn no_fallback_propagates_degeneration_error() {
        // Extractor WITHOUT fallback configured
        let extractor = DocumentExtractor::new(
            Box::new(MockPdfPageRenderer::new(1)),
            Box::new(DegeneratingVisionOcr),
            Box::new(MockImagePreprocessor::new()),
        );

        let result = extractor.try_vision_fallback(&[0u8; 10], 0, "test", vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn extract_image_uses_fallback_on_degeneration() {
        let extractor = make_extractor_with_fallback();

        let result = extractor.extract_image(&[0u8; 10], &None, None);
        assert!(result.is_ok());

        let (method, pages) = result.unwrap();
        assert_eq!(method, ExtractionMethod::VisionOcr);
        assert_eq!(pages.len(), 1);
        assert!(pages[0].text.contains("Hemoglobine"));
        assert_eq!(pages[0].confidence, 0.7);
    }

    #[test]
    fn fallback_entities_formatted_as_markdown() {
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
    fn fallback_uses_markdown_when_available() {
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
}
