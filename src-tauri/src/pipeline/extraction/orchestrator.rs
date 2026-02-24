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

use uuid::Uuid;

use super::confidence::compute_overall_confidence;
use super::preprocess::ImagePreprocessor;
use super::sanitize::sanitize_extracted_text;
use super::types::{
    ExtractionMethod, ExtractionResult, ImageContentType, MedicalImageInterpreter,
    PageExtraction, PdfPageRenderer, TextExtractor, VisionOcrEngine,
};
use super::ExtractionError;
use crate::crypto::ProfileSession;
use crate::pipeline::import::format::FileCategory;
use crate::pipeline::import::staging::read_staged_file;
use crate::pipeline::import::FormatDetection;

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

/// R4+: Vision-based document extraction orchestrator.
///
/// Three required + one optional dependency (trait objects for DI):
/// - `pdf_renderer`: Renders PDF pages to PNG images (PdfiumRenderer in prod)
/// - `vision_ocr`: Extracts text from images via vision model (OllamaVisionOcr in prod)
/// - `preprocessor`: Prepares images for optimal vision model input (PreprocessingPipeline in prod)
/// - `interpreter` (optional): Interprets medical images (OllamaMedicalImageInterpreter in prod)
pub struct DocumentExtractor {
    pdf_renderer: Box<dyn PdfPageRenderer>,
    vision_ocr: Box<dyn VisionOcrEngine>,
    preprocessor: Box<dyn ImagePreprocessor>,
    interpreter: Option<Box<dyn MedicalImageInterpreter>>,
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

    /// Extract text from a PDF: render each page, preprocess, then vision OCR.
    ///
    /// `dpi` is selected by `select_render_dpi()` based on document format.
    fn extract_pdf(
        &self,
        pdf_bytes: &[u8],
        dpi: u32,
    ) -> Result<(ExtractionMethod, Vec<PageExtraction>), ExtractionError> {
        let num_pages = self.pdf_renderer.page_count(pdf_bytes)?;

        if num_pages == 0 {
            return Err(ExtractionError::EmptyDocument);
        }

        let mut pages = Vec::with_capacity(num_pages);

        for page_idx in 0..num_pages {
            let page_image = self
                .pdf_renderer
                .render_page(pdf_bytes, page_idx, dpi)?;

            let prepared = self.preprocessor.preprocess(&page_image)?;
            let ocr_result = self.vision_ocr.extract_text_from_image(&prepared.png_bytes)?;

            // Route medical images to interpreter if available
            let (text, confidence, content_type) =
                self.route_by_content_type(&prepared.png_bytes, &ocr_result);

            pages.push(PageExtraction {
                page_number: page_idx + 1, // 1-indexed for display
                text,
                confidence,
                regions: vec![],
                warnings: prepared.warnings,
                content_type: Some(content_type),
            });
        }

        Ok((ExtractionMethod::VisionOcr, pages))
    }

    /// Extract text from an image: preprocess, then vision OCR.
    fn extract_image(
        &self,
        image_bytes: &[u8],
    ) -> Result<(ExtractionMethod, Vec<PageExtraction>), ExtractionError> {
        let prepared = self.preprocessor.preprocess(image_bytes)?;
        let ocr_result = self.vision_ocr.extract_text_from_image(&prepared.png_bytes)?;

        // Route medical images to interpreter if available
        let (text, confidence, content_type) =
            self.route_by_content_type(&prepared.png_bytes, &ocr_result);

        let page = PageExtraction {
            page_number: 1,
            text,
            confidence,
            regions: vec![],
            warnings: prepared.warnings,
            content_type: Some(content_type),
        };

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
}

impl TextExtractor for DocumentExtractor {
    fn extract(
        &self,
        document_id: &Uuid,
        staged_path: &Path,
        format: &FormatDetection,
        session: &ProfileSession,
    ) -> Result<ExtractionResult, ExtractionError> {
        tracing::info!(
            document_id = %document_id,
            category = format.category.as_str(),
            "Starting text extraction"
        );

        // Step 1: Decrypt the staged file
        let decrypted_bytes = read_staged_file(staged_path, session)?;

        // Step 2: Extract based on format category
        let dpi = select_render_dpi(format);
        let (method, mut pages) = match &format.category {
            // All PDFs → pdfium render → vision OCR (adaptive DPI per format)
            FileCategory::DigitalPdf | FileCategory::ScannedPdf => {
                self.extract_pdf(&decrypted_bytes, dpi)?
            }
            // Images → vision OCR directly
            FileCategory::Image => self.extract_image(&decrypted_bytes)?,
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

        Ok(ExtractionResult {
            document_id: *document_id,
            method,
            pages,
            full_text,
            overall_confidence,
            language_detected: None, // R3: vision models handle multilingual natively
            page_count,
        })
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
            .extract(&doc_id, &staged_path, &format, &session)
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
            .extract(&doc_id, &staged_path, &format, &session)
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
            .extract(&doc_id, &staged_path, &format, &session)
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
            .extract(&doc_id, &staged_path, &format, &session)
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
            .extract(&doc_id, &staged_path, &format, &session)
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
            .extract(&doc_id, &staged_path, &format, &session)
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

        let result = extractor.extract(&doc_id, &staged_path, &format, &session);
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
            .extract(&doc_id, &staged_path, &format, &session)
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
            .extract(&doc_id, &staged_path, &format, &session)
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
            .extract(&doc_id, &staged_path, &format, &session)
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
            .extract(&doc_id, &staged_path, &format, &session)
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
            .extract(&doc_id, &staged_path, &format, &session)
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
            .extract(&doc_id, &staged_path, &format, &session)
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
            .extract(&doc_id, &staged_path, &format, &session)
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
            .extract(&doc_id, &staged_path, &format, &session)
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
            .extract(&doc_id, &staged_path, &format, &session)
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
            .extract(&doc_id, &staged_path, &format, &session)
            .unwrap();

        for page in &result.pages {
            assert_eq!(page.content_type, Some(ImageContentType::Document));
        }
    }
}
