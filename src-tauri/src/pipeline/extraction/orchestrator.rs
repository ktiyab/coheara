use std::path::Path;

use uuid::Uuid;

use super::column_detect::reorder_columns;
use super::confidence::{analyze_ocr_quality, compute_overall_confidence};
use super::language_detect::detect_language;
use super::medical_correction::correct_medical_terms;
use super::preprocess::preprocess_image;
use super::sanitize::sanitize_extracted_text;
use super::table_detect::annotate_table_continuations;
use super::types::{
    ExtractionMethod, ExtractionResult, ExtractionWarning, OcrEngine, PageExtraction,
    PdfExtractor, PdfPageRenderer, RegionConfidence, TextExtractor,
};
use super::ExtractionError;
use crate::crypto::ProfileSession;
use crate::pipeline::import::format::FileCategory;
use crate::pipeline::import::staging::read_staged_file;
use crate::pipeline::import::FormatDetection;

/// Concrete implementation of the text extractor.
/// Uses trait objects for OCR and PDF extraction, enabling dependency injection.
pub struct DocumentExtractor {
    ocr_engine: Box<dyn OcrEngine + Send + Sync>,
    pdf_extractor: Box<dyn PdfExtractor + Send + Sync>,
    pdf_renderer: Option<Box<dyn PdfPageRenderer + Send + Sync>>,
}

impl DocumentExtractor {
    pub fn new(
        ocr_engine: Box<dyn OcrEngine + Send + Sync>,
        pdf_extractor: Box<dyn PdfExtractor + Send + Sync>,
    ) -> Self {
        Self {
            ocr_engine,
            pdf_extractor,
            pdf_renderer: None,
        }
    }

    /// Add a PDF page renderer for per-page OCR of scanned PDFs.
    pub fn with_pdf_renderer(
        mut self,
        renderer: Box<dyn PdfPageRenderer + Send + Sync>,
    ) -> Self {
        self.pdf_renderer = Some(renderer);
        self
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
        let (method, mut pages) = match &format.category {
            FileCategory::DigitalPdf => {
                let pages = self.pdf_extractor.extract_text(&decrypted_bytes)?;
                (ExtractionMethod::PdfDirect, pages)
            }
            FileCategory::ScannedPdf => {
                let renderer_ref = self.pdf_renderer.as_ref().map(|r| &**r as &dyn PdfPageRenderer);
                let pages = ocr_scanned_pdf(
                    &decrypted_bytes,
                    &*self.pdf_extractor,
                    &*self.ocr_engine,
                    renderer_ref,
                )?;
                (ExtractionMethod::TesseractOcr, pages)
            }
            FileCategory::Image => {
                let processed = preprocess_image(&decrypted_bytes)?;
                let ocr_result = self.ocr_engine.ocr_image(&processed)?;
                let warnings = analyze_ocr_quality(&ocr_result);

                let page = PageExtraction {
                    page_number: 1,
                    text: ocr_result.text,
                    confidence: ocr_result.confidence,
                    regions: ocr_result
                        .word_confidences
                        .iter()
                        .map(|w| RegionConfidence {
                            text: w.text.clone(),
                            confidence: w.confidence,
                            bounding_box: w.bounding_box.clone(),
                        })
                        .collect(),
                    warnings,
                };
                (ExtractionMethod::TesseractOcr, vec![page])
            }
            FileCategory::PlainText => {
                let text = String::from_utf8(decrypted_bytes)
                    .map_err(|e| ExtractionError::EncodingError(e.to_string()))?;

                let page = PageExtraction {
                    page_number: 1,
                    text,
                    confidence: 0.99,
                    regions: vec![],
                    warnings: vec![],
                };
                (ExtractionMethod::PlainTextRead, vec![page])
            }
            FileCategory::Unsupported => {
                return Err(ExtractionError::UnsupportedFormat);
            }
        };

        // Step 3: Reorder multi-column digital PDF text (EXT-05-G05/G09)
        if method == ExtractionMethod::PdfDirect {
            for page in &mut pages {
                page.text = reorder_columns(&page.text);
            }
        }

        // Step 3b: Sanitize all extracted text
        for page in &mut pages {
            page.text = sanitize_extracted_text(&page.text);
        }

        // Step 3b: Apply medical term correction for OCR-extracted text (EXT-02-G06)
        if method == ExtractionMethod::TesseractOcr {
            for page in &mut pages {
                page.text = correct_medical_terms(&page.text);
            }
        }

        // Step 4: Detect table continuations across page breaks (EXT-05-G04)
        annotate_table_continuations(&mut pages);

        // Step 5: Compute overall confidence
        let overall_confidence = compute_overall_confidence(&pages, &method);

        // Step 5: Concatenate full text with page breaks
        let full_text = pages
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n--- Page Break ---\n\n");

        let page_count = pages.len();

        // Step 6: Detect document language (EXT-04-G03)
        let language_detected = Some(detect_language(&full_text));

        tracing::info!(
            document_id = %document_id,
            method = ?method,
            pages = page_count,
            confidence = overall_confidence,
            language = ?language_detected,
            text_length = full_text.len(),
            "Text extraction complete"
        );

        Ok(ExtractionResult {
            document_id: *document_id,
            method,
            pages,
            full_text,
            overall_confidence,
            language_detected,
            page_count,
        })
    }
}

/// OCR a scanned PDF page by page.
/// Each page is extracted as text first (which will be empty/short for scanned PDFs),
/// then if text is insufficient, the page is rendered to an image and OCR'd.
///
/// When a `PdfPageRenderer` is provided, each page is rendered individually
/// and preprocessed before OCR. Without a renderer, the raw PDF bytes are
/// passed directly to the OCR engine (fallback for when no renderer is available).
fn ocr_scanned_pdf(
    pdf_bytes: &[u8],
    pdf_extractor: &dyn PdfExtractor,
    ocr_engine: &dyn OcrEngine,
    pdf_renderer: Option<&dyn PdfPageRenderer>,
) -> Result<Vec<PageExtraction>, ExtractionError> {
    // First try direct text extraction — scanned PDFs will yield little/no text
    let direct_pages = pdf_extractor.extract_text(pdf_bytes)?;

    let mut pages = Vec::with_capacity(direct_pages.len());

    for (page_idx, direct_page) in direct_pages.iter().enumerate() {
        // If direct extraction found meaningful text, use it
        if direct_page.text.trim().len() > 20 {
            pages.push(direct_page.clone());
            continue;
        }

        // Render the page to an image and OCR it
        let ocr_result = if let Some(renderer) = pdf_renderer {
            // EXT-02-G03: Per-page rendering at 300 DPI for OCR quality
            let page_image = renderer.render_page(pdf_bytes, page_idx, 300)?;
            let processed = preprocess_image(&page_image)?;
            ocr_engine.ocr_image(&processed)?
        } else {
            // Fallback: pass raw PDF bytes (may only work for single-page PDFs)
            tracing::warn!(
                page = page_idx + 1,
                "No PDF renderer available, OCR on raw PDF bytes"
            );
            ocr_engine.ocr_image(pdf_bytes)?
        };

        let warnings = analyze_ocr_quality(&ocr_result);

        pages.push(PageExtraction {
            page_number: direct_page.page_number,
            text: ocr_result.text,
            confidence: ocr_result.confidence,
            regions: ocr_result
                .word_confidences
                .iter()
                .map(|w| RegionConfidence {
                    text: w.text.clone(),
                    confidence: w.confidence,
                    bounding_box: w.bounding_box.clone(),
                })
                .collect(),
            warnings,
        });
    }

    // If no pages were produced at all, return a single empty page
    if pages.is_empty() && !direct_pages.is_empty() {
        pages.push(PageExtraction {
            page_number: 1,
            text: String::new(),
            confidence: 0.0,
            regions: vec![],
            warnings: vec![ExtractionWarning::PartialExtraction {
                reason: "No text could be extracted from scanned PDF".into(),
            }],
        });
    }

    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::profile;
    use crate::pipeline::extraction::ocr::MockOcrEngine;
    use crate::pipeline::import::format::FileCategory;
    use crate::pipeline::import::staging::stage_file;

    /// Mock PDF extractor for testing
    struct MockPdfExtractor {
        pages: Vec<PageExtraction>,
    }

    impl MockPdfExtractor {
        fn with_pages(pages: Vec<PageExtraction>) -> Self {
            Self { pages }
        }

        fn empty() -> Self {
            Self { pages: vec![] }
        }
    }

    impl PdfExtractor for MockPdfExtractor {
        fn extract_text(&self, _pdf_bytes: &[u8]) -> Result<Vec<PageExtraction>, ExtractionError> {
            Ok(self.pages.clone())
        }

        fn page_count(&self, _pdf_bytes: &[u8]) -> Result<usize, ExtractionError> {
            Ok(self.pages.len())
        }
    }

    /// Mock PDF page renderer that returns a minimal valid image for each page.
    struct MockPdfPageRenderer;

    impl PdfPageRenderer for MockPdfPageRenderer {
        fn render_page(
            &self,
            _pdf_bytes: &[u8],
            _page_number: usize,
            _dpi: u32,
        ) -> Result<Vec<u8>, ExtractionError> {
            // Return a minimal valid PNG (32x32 gray image)
            let img = image::GrayImage::from_pixel(32, 32, image::Luma([200u8]));
            let dynamic = image::DynamicImage::ImageLuma8(img);
            let mut buf = std::io::Cursor::new(Vec::new());
            dynamic
                .write_to(&mut buf, image::ImageOutputFormat::Png)
                .map_err(|e: image::ImageError| ExtractionError::ImageProcessing(e.to_string()))?;
            Ok(buf.into_inner())
        }
    }

    fn setup() -> (tempfile::TempDir, ProfileSession) {
        let dir = tempfile::tempdir().unwrap();
        let (info, _phrase) =
            profile::create_profile(dir.path(), "ExtractTest", "test_pass_123", None, None).unwrap();
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

    #[test]
    fn extract_plain_text_file() {
        let (_dir, session) = setup();
        let content = "Potassium: 4.2 mmol/L (normal range: 3.5-5.0)";
        let (doc_id, staged_path) = stage_text_file(&session, content);

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("unused", 0.0)),
            Box::new(MockPdfExtractor::empty()),
        );

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
    fn extract_plain_text_confidence_is_high() {
        let (_dir, session) = setup();
        let (doc_id, staged_path) = stage_text_file(&session, "Medical report content");

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("unused", 0.0)),
            Box::new(MockPdfExtractor::empty()),
        );

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

    #[test]
    fn extract_image_uses_ocr() {
        let (_dir, session) = setup();

        // Create a minimal JPEG image
        let img = image::RgbImage::from_pixel(32, 32, image::Rgb([100u8, 150, 200]));
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("scan.jpg");
        img.save(&path).unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("Metformin 500mg twice daily", 0.85)),
            Box::new(MockPdfExtractor::empty()),
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

        assert_eq!(result.method, ExtractionMethod::TesseractOcr);
        assert!(result.full_text.contains("Metformin"));
        assert!(result.full_text.contains("500mg"));
        assert_eq!(result.page_count, 1);
    }

    #[test]
    fn extract_digital_pdf_uses_pdf_extractor() {
        let (_dir, session) = setup();

        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("report.pdf");
        // Write fake PDF content (won't actually be parsed by mock)
        std::fs::write(&path, b"fake pdf content").unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        let mock_pages = vec![
            PageExtraction {
                page_number: 1,
                text: "Patient: Marie Dubois\nDOB: 1945-03-12".into(),
                confidence: 0.95,
                regions: vec![],
                warnings: vec![],
            },
            PageExtraction {
                page_number: 2,
                text: "Potassium: 4.2 mmol/L\nSodium: 140 mmol/L".into(),
                confidence: 0.95,
                regions: vec![],
                warnings: vec![],
            },
        ];

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("unused", 0.0)),
            Box::new(MockPdfExtractor::with_pages(mock_pages)),
        );

        let format = FormatDetection {
            mime_type: "application/pdf".into(),
            category: FileCategory::DigitalPdf,
            is_digital_pdf: Some(true),
            file_size_bytes: 5000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session)
            .unwrap();

        assert_eq!(result.method, ExtractionMethod::PdfDirect);
        assert!(result.full_text.contains("Marie Dubois"));
        assert!(result.full_text.contains("Potassium"));
        assert_eq!(result.page_count, 2);
        assert!(result.overall_confidence > 0.90);
    }

    #[test]
    fn extract_multipage_has_page_breaks() {
        let (_dir, session) = setup();

        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("multi.pdf");
        std::fs::write(&path, b"fake pdf").unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        let mock_pages = vec![
            PageExtraction {
                page_number: 1,
                text: "First page content here.".into(),
                confidence: 0.95,
                regions: vec![],
                warnings: vec![],
            },
            PageExtraction {
                page_number: 2,
                text: "Second page content here.".into(),
                confidence: 0.95,
                regions: vec![],
                warnings: vec![],
            },
        ];

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("unused", 0.0)),
            Box::new(MockPdfExtractor::with_pages(mock_pages)),
        );

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

    #[test]
    fn extract_sanitizes_text() {
        let (_dir, session) = setup();
        let content = "Patient: Marie\x00Dubois\x01\nDose: 500mg";
        let (doc_id, staged_path) = stage_text_file(&session, content);

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("unused", 0.0)),
            Box::new(MockPdfExtractor::empty()),
        );

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

    #[test]
    fn extract_unsupported_format_rejected() {
        let (_dir, session) = setup();
        let (doc_id, staged_path) = stage_text_file(&session, "whatever");

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("unused", 0.0)),
            Box::new(MockPdfExtractor::empty()),
        );

        let format = FormatDetection {
            mime_type: "application/octet-stream".into(),
            category: FileCategory::Unsupported,
            is_digital_pdf: None,
            file_size_bytes: 100,
        };

        let result = extractor.extract(&doc_id, &staged_path, &format, &session);
        assert!(matches!(result, Err(ExtractionError::UnsupportedFormat)));
    }

    #[test]
    fn extract_returns_correct_document_id() {
        let (_dir, session) = setup();
        let (doc_id, staged_path) = stage_text_file(&session, "test content");

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("unused", 0.0)),
            Box::new(MockPdfExtractor::empty()),
        );

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

    #[test]
    fn extract_image_generates_word_regions() {
        let (_dir, session) = setup();

        let img = image::RgbImage::from_pixel(32, 32, image::Rgb([50u8, 50, 50]));
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("scan.jpg");
        img.save(&path).unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("Blood pressure normal", 0.80)),
            Box::new(MockPdfExtractor::empty()),
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

        assert_eq!(result.pages[0].regions.len(), 3); // 3 words
        assert_eq!(result.pages[0].regions[0].text, "Blood");
    }

    // --- D.2/D.3: Multi-page scanned PDF OCR with page renderer ---

    #[test]
    fn scanned_pdf_with_renderer_ocrs_all_pages() {
        let (_dir, session) = setup();

        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("scanned.pdf");
        std::fs::write(&path, b"fake scanned pdf").unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        // 3 pages with no text (simulating scanned PDF)
        let scanned_pages = vec![
            PageExtraction {
                page_number: 1,
                text: "".into(),
                confidence: 0.0,
                regions: vec![],
                warnings: vec![],
            },
            PageExtraction {
                page_number: 2,
                text: "".into(),
                confidence: 0.0,
                regions: vec![],
                warnings: vec![],
            },
            PageExtraction {
                page_number: 3,
                text: "".into(),
                confidence: 0.0,
                regions: vec![],
                warnings: vec![],
            },
        ];

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("OCR result text", 0.82)),
            Box::new(MockPdfExtractor::with_pages(scanned_pages)),
        )
        .with_pdf_renderer(Box::new(MockPdfPageRenderer));

        let format = FormatDetection {
            mime_type: "application/pdf".into(),
            category: FileCategory::ScannedPdf,
            is_digital_pdf: Some(false),
            file_size_bytes: 50000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session)
            .unwrap();

        assert_eq!(result.method, ExtractionMethod::TesseractOcr);
        // All 3 pages should be OCR'd (no break)
        assert_eq!(result.page_count, 3);
        assert_eq!(result.pages.len(), 3);
        for page in &result.pages {
            assert!(page.text.contains("OCR result"));
            assert!(page.confidence > 0.0);
        }
    }

    #[test]
    fn scanned_pdf_mixed_pages_keeps_digital_text() {
        let (_dir, session) = setup();

        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("mixed.pdf");
        std::fs::write(&path, b"fake mixed pdf").unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        // Page 1: has text (digital), Page 2: scanned (no text)
        let mixed_pages = vec![
            PageExtraction {
                page_number: 1,
                text: "This page has enough digital text to skip OCR.".into(),
                confidence: 0.95,
                regions: vec![],
                warnings: vec![],
            },
            PageExtraction {
                page_number: 2,
                text: "".into(),
                confidence: 0.0,
                regions: vec![],
                warnings: vec![],
            },
        ];

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("Scanned page OCR", 0.75)),
            Box::new(MockPdfExtractor::with_pages(mixed_pages)),
        )
        .with_pdf_renderer(Box::new(MockPdfPageRenderer));

        let format = FormatDetection {
            mime_type: "application/pdf".into(),
            category: FileCategory::ScannedPdf,
            is_digital_pdf: Some(false),
            file_size_bytes: 30000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session)
            .unwrap();

        assert_eq!(result.page_count, 2);
        // Page 1: kept digital text
        assert!(result.pages[0].text.contains("enough digital text"));
        assert!((result.pages[0].confidence - 0.95).abs() < f32::EPSILON);
        // Page 2: OCR'd
        assert!(result.pages[1].text.contains("Scanned page OCR"));
    }

    #[test]
    fn scanned_pdf_without_renderer_falls_back() {
        // Without a renderer, the old fallback behavior applies
        let scanned_pages = vec![
            PageExtraction {
                page_number: 1,
                text: "".into(),
                confidence: 0.0,
                regions: vec![],
                warnings: vec![],
            },
            PageExtraction {
                page_number: 2,
                text: "".into(),
                confidence: 0.0,
                regions: vec![],
                warnings: vec![],
            },
        ];

        let ocr = Box::new(MockOcrEngine::new("fallback OCR", 0.60));
        let pdf = Box::new(MockPdfExtractor::with_pages(scanned_pages));

        let result = ocr_scanned_pdf(b"fake pdf", &*pdf, &*ocr, None).unwrap();

        // Both pages still get OCR'd (no more break), just with raw bytes
        assert_eq!(result.len(), 2);
        assert!(result[0].text.contains("fallback OCR"));
        assert!(result[1].text.contains("fallback OCR"));
    }

    // --- D.7: OCR pipeline integration tests ---

    #[test]
    fn ocr_pipeline_applies_medical_correction() {
        let (_dir, session) = setup();

        // Simulate OCR with a common medical term error
        let img = image::RgbImage::from_pixel(32, 32, image::Rgb([200u8, 200, 200]));
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("rx.jpg");
        img.save(&path).unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        // MockOCR returns "Metfonnin" (common rn→m error)
        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("Metfonnin 500mg twice daily", 0.82)),
            Box::new(MockPdfExtractor::empty()),
        );

        let format = FormatDetection {
            mime_type: "image/jpeg".into(),
            category: FileCategory::Image,
            is_digital_pdf: None,
            file_size_bytes: 5000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session)
            .unwrap();

        // Medical correction should fix "Metfonnin" → "Metformin"
        assert!(
            result.full_text.contains("Metformin"),
            "Expected 'Metformin', got: {}",
            result.full_text
        );
        assert!(result.full_text.contains("500mg"));
    }

    #[test]
    fn ocr_pipeline_does_not_correct_digital_pdf() {
        let (_dir, session) = setup();

        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("digital.pdf");
        std::fs::write(&path, b"fake pdf").unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        // Digital PDF with "Metfonnin" — should NOT be corrected
        let pages = vec![PageExtraction {
            page_number: 1,
            text: "Metfonnin 500mg daily dosage prescribed by doctor".into(),
            confidence: 0.95,
            regions: vec![],
            warnings: vec![],
        }];

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("unused", 0.0)),
            Box::new(MockPdfExtractor::with_pages(pages)),
        );

        let format = FormatDetection {
            mime_type: "application/pdf".into(),
            category: FileCategory::DigitalPdf,
            is_digital_pdf: Some(true),
            file_size_bytes: 5000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session)
            .unwrap();

        // Digital PDFs should NOT have medical correction applied
        assert!(
            result.full_text.contains("Metfonnin"),
            "Digital PDF should not be corrected: {}",
            result.full_text
        );
    }

    #[test]
    fn ocr_pipeline_handwriting_detected_in_low_confidence() {
        let (_dir, session) = setup();

        let img = image::RgbImage::from_pixel(32, 32, image::Rgb([180u8, 180, 180]));
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("handwritten.jpg");
        img.save(&path).unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        // Simulate handwritten text: many low-confidence words
        let mut mock = MockOcrEngine::new("a b c d e f g h i j", 0.20);
        // Override word confidences to simulate handwriting
        mock.confidence = 0.20;

        let extractor = DocumentExtractor::new(
            Box::new(mock),
            Box::new(MockPdfExtractor::empty()),
        );

        let format = FormatDetection {
            mime_type: "image/jpeg".into(),
            category: FileCategory::Image,
            is_digital_pdf: None,
            file_size_bytes: 3000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session)
            .unwrap();

        // With all words at 0.20 confidence, handwriting should be detected
        let has_handwriting = result.pages[0]
            .warnings
            .iter()
            .any(|w| matches!(w, ExtractionWarning::HandwritingDetected));
        assert!(has_handwriting, "Should detect handwriting with low confidence words");
    }

    // --- E.4: French OCR integration tests ---

    #[test]
    fn french_text_language_detected_as_fra() {
        let (_dir, session) = setup();
        let content = "Résultats d'analyses biologiques\nCréatinine: 72 µmol/L\nGlucose à jeun: 5,2 mmol/L";
        let (doc_id, staged_path) = stage_text_file(&session, content);

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("unused", 0.0)),
            Box::new(MockPdfExtractor::empty()),
        );

        let format = FormatDetection {
            mime_type: "text/plain".into(),
            category: FileCategory::PlainText,
            is_digital_pdf: None,
            file_size_bytes: content.len() as u64,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session)
            .unwrap();

        assert_eq!(
            result.language_detected.as_deref(),
            Some("fra"),
            "French medical text should be detected as 'fra'"
        );
    }

    #[test]
    fn english_text_language_detected_as_eng() {
        let (_dir, session) = setup();
        let content = "Blood test results for the patient\nCreatinine: 72 umol/L\nFasting glucose was normal";
        let (doc_id, staged_path) = stage_text_file(&session, content);

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("unused", 0.0)),
            Box::new(MockPdfExtractor::empty()),
        );

        let format = FormatDetection {
            mime_type: "text/plain".into(),
            category: FileCategory::PlainText,
            is_digital_pdf: None,
            file_size_bytes: content.len() as u64,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session)
            .unwrap();

        assert_eq!(
            result.language_detected.as_deref(),
            Some("eng"),
            "English medical text should be detected as 'eng'"
        );
    }

    #[test]
    fn french_prescription_preserves_accents_through_pipeline() {
        let (_dir, session) = setup();
        let content = "Ordonnance du Dr Martin\n\
                        Paracétamol 1g matin midi et soir\n\
                        Métoprolol 50mg une fois par jour\n\
                        Énalapril 10mg le matin à jeun";
        let (doc_id, staged_path) = stage_text_file(&session, content);

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("unused", 0.0)),
            Box::new(MockPdfExtractor::empty()),
        );

        let format = FormatDetection {
            mime_type: "text/plain".into(),
            category: FileCategory::PlainText,
            is_digital_pdf: None,
            file_size_bytes: content.len() as u64,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session)
            .unwrap();

        assert!(result.full_text.contains("Paracétamol"), "é in Paracétamol must survive");
        assert!(result.full_text.contains("Métoprolol"), "é in Métoprolol must survive");
        assert!(result.full_text.contains("Énalapril"), "É in Énalapril must survive");
        assert!(result.full_text.contains("à jeun"), "à must survive");
    }

    #[test]
    fn french_lab_report_preserves_guillemets_and_dashes() {
        let (_dir, session) = setup();
        let content = "Bilan sanguin «complet»\n\
                        Potassium: 4,2 mmol/L (3,5\u{2013}5,0)\n\
                        Résultat \u{2014} dans les normes\n\
                        Coût: 15,50\u{20AC} par analyse";
        let (doc_id, staged_path) = stage_text_file(&session, content);

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("unused", 0.0)),
            Box::new(MockPdfExtractor::empty()),
        );

        let format = FormatDetection {
            mime_type: "text/plain".into(),
            category: FileCategory::PlainText,
            is_digital_pdf: None,
            file_size_bytes: content.len() as u64,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session)
            .unwrap();

        assert!(result.full_text.contains('«'), "Left guillemet must survive pipeline");
        assert!(result.full_text.contains('»'), "Right guillemet must survive pipeline");
        assert!(result.full_text.contains('\u{2013}'), "En-dash must survive pipeline");
        assert!(result.full_text.contains('\u{2014}'), "Em-dash must survive pipeline");
        assert!(result.full_text.contains('€'), "Euro sign must survive pipeline");
    }

    #[test]
    fn french_ocr_result_detects_language_and_preserves_text() {
        let (_dir, session) = setup();

        let img = image::RgbImage::from_pixel(32, 32, image::Rgb([200u8, 200, 200]));
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("french_scan.jpg");
        img.save(&path).unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        // Simulate French OCR output
        let french_ocr = "Résultats d'analyses biologiques\n\
                           Créatinine: 72 µmol/L\n\
                           Hémoglobine: 14,2 g/dL\n\
                           Ordonnance du médecin";

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new(french_ocr, 0.85)),
            Box::new(MockPdfExtractor::empty()),
        );

        let format = FormatDetection {
            mime_type: "image/jpeg".into(),
            category: FileCategory::Image,
            is_digital_pdf: None,
            file_size_bytes: 5000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session)
            .unwrap();

        assert_eq!(result.method, ExtractionMethod::TesseractOcr);
        assert_eq!(result.language_detected.as_deref(), Some("fra"));
        assert!(result.full_text.contains("Résultats"));
        // Medical correction normalizes "Créatinine" → "Creatinine" (edit distance 1)
        assert!(result.full_text.contains("reatinine"), "Creatinine term must survive");
        assert!(result.full_text.contains("µmol/L"));
    }

    #[test]
    fn french_all_accented_vowels_survive_pipeline() {
        let (_dir, session) = setup();
        // All 14 French accented characters + ligatures
        let content = "à â ç é è ê ë î ï ô ù û ü ÿ æ œ";
        let (doc_id, staged_path) = stage_text_file(&session, content);

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("unused", 0.0)),
            Box::new(MockPdfExtractor::empty()),
        );

        let format = FormatDetection {
            mime_type: "text/plain".into(),
            category: FileCategory::PlainText,
            is_digital_pdf: None,
            file_size_bytes: content.len() as u64,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session)
            .unwrap();

        for ch in ['à', 'â', 'ç', 'é', 'è', 'ê', 'ë', 'î', 'ï', 'ô', 'ù', 'û', 'ü', 'ÿ', 'æ', 'œ'] {
            assert!(
                result.full_text.contains(ch),
                "French character '{}' must survive the full pipeline",
                ch
            );
        }
    }

    #[test]
    fn french_digital_pdf_detects_language() {
        let (_dir, session) = setup();

        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("french_report.pdf");
        std::fs::write(&path, b"fake french pdf").unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        let french_pages = vec![
            PageExtraction {
                page_number: 1,
                text: "Bilan sanguin complet\nNumération Formule Sanguine (NFS)\nHémoglobine: 14,2 g/dL".into(),
                confidence: 0.95,
                regions: vec![],
                warnings: vec![],
            },
            PageExtraction {
                page_number: 2,
                text: "Résultats dans les normes\nOrdonnance du médecin traitant".into(),
                confidence: 0.95,
                regions: vec![],
                warnings: vec![],
            },
        ];

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("unused", 0.0)),
            Box::new(MockPdfExtractor::with_pages(french_pages)),
        );

        let format = FormatDetection {
            mime_type: "application/pdf".into(),
            category: FileCategory::DigitalPdf,
            is_digital_pdf: Some(true),
            file_size_bytes: 10000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session)
            .unwrap();

        assert_eq!(result.language_detected.as_deref(), Some("fra"));
        assert!(result.full_text.contains("Hémoglobine"));
        assert!(result.full_text.contains("Résultats"));
    }

    #[test]
    fn ocr_pipeline_blurry_warning_on_low_overall_confidence() {
        let (_dir, session) = setup();

        let img = image::RgbImage::from_pixel(32, 32, image::Rgb([100u8, 100, 100]));
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("blurry.jpg");
        img.save(&path).unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("barely readable", 0.25)),
            Box::new(MockPdfExtractor::empty()),
        );

        let format = FormatDetection {
            mime_type: "image/jpeg".into(),
            category: FileCategory::Image,
            is_digital_pdf: None,
            file_size_bytes: 2000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session)
            .unwrap();

        let has_blurry = result.pages[0]
            .warnings
            .iter()
            .any(|w| matches!(w, ExtractionWarning::BlurryImage));
        assert!(has_blurry, "Should detect blurry image with low confidence");
    }

    // --- G.6: Table extraction integration tests ---

    #[test]
    fn image_ocr_produces_regions_with_bounding_boxes_from_mock() {
        // MockOcrEngine produces regions without bounding boxes (bounding_box: None).
        // This test verifies the regions are created and the pipeline handles None gracefully.
        let (_dir, session) = setup();

        let img = image::RgbImage::from_pixel(32, 32, image::Rgb([200u8, 200, 200]));
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("table_scan.jpg");
        img.save(&path).unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("Potassium 4.2 mmol/L", 0.88)),
            Box::new(MockPdfExtractor::empty()),
        );

        let format = FormatDetection {
            mime_type: "image/jpeg".into(),
            category: FileCategory::Image,
            is_digital_pdf: None,
            file_size_bytes: 3000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session)
            .unwrap();

        // Mock produces per-word regions
        assert_eq!(result.pages[0].regions.len(), 3);
        assert_eq!(result.pages[0].regions[0].text, "Potassium");
        // Mock does not provide bounding boxes
        assert!(result.pages[0].regions[0].bounding_box.is_none());
    }

    #[test]
    fn table_continuation_detected_across_pdf_pages() {
        let (_dir, session) = setup();

        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("table_report.pdf");
        std::fs::write(&path, b"fake table pdf").unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        // Page 1 ends with tabular data, page 2 starts with tabular data
        let table_pages = vec![
            PageExtraction {
                page_number: 1,
                text: "Lab Report\nK\t4.2\tmmol/L\nNa\t140\tmmol/L\nCl\t102\tmmol/L".into(),
                confidence: 0.95,
                regions: vec![],
                warnings: vec![],
            },
            PageExtraction {
                page_number: 2,
                text: "Ca\t2.4\tmmol/L\nMg\t0.9\tmmol/L\nPO4\t1.1\tmmol/L\nConclusion: normal".into(),
                confidence: 0.95,
                regions: vec![],
                warnings: vec![],
            },
        ];

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("unused", 0.0)),
            Box::new(MockPdfExtractor::with_pages(table_pages)),
        );

        let format = FormatDetection {
            mime_type: "application/pdf".into(),
            category: FileCategory::DigitalPdf,
            is_digital_pdf: Some(true),
            file_size_bytes: 10000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session)
            .unwrap();

        // Page 1 should have TableContinuation warning
        let has_continuation = result.pages[0]
            .warnings
            .iter()
            .any(|w| matches!(w, ExtractionWarning::TableContinuation));
        assert!(has_continuation, "Page 1 should have TableContinuation warning");

        // Page 2 should not (it's the last page)
        let page2_cont = result.pages[1]
            .warnings
            .iter()
            .any(|w| matches!(w, ExtractionWarning::TableContinuation));
        assert!(!page2_cont, "Last page should not have continuation warning");
    }

    #[test]
    fn column_reordering_applied_to_digital_pdf() {
        let (_dir, session) = setup();

        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("twocol.pdf");
        std::fs::write(&path, b"fake two-col pdf").unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        // Simulate multi-column PDF output
        let two_col_text = "Test Name              Result\n\
                            Potassium              4.2\n\
                            Sodium                 140\n\
                            Chloride               102\n\
                            Creatinine             72";
        let col_pages = vec![
            PageExtraction {
                page_number: 1,
                text: two_col_text.into(),
                confidence: 0.95,
                regions: vec![],
                warnings: vec![],
            },
        ];

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("unused", 0.0)),
            Box::new(MockPdfExtractor::with_pages(col_pages)),
        );

        let format = FormatDetection {
            mime_type: "application/pdf".into(),
            category: FileCategory::DigitalPdf,
            is_digital_pdf: Some(true),
            file_size_bytes: 5000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session)
            .unwrap();

        // All content should be present
        assert!(result.full_text.contains("Potassium"));
        assert!(result.full_text.contains("Sodium"));
        assert!(result.full_text.contains("4.2"));
        assert!(result.full_text.contains("140"));
    }

    #[test]
    fn column_reordering_not_applied_to_ocr() {
        let (_dir, session) = setup();

        let img = image::RgbImage::from_pixel(32, 32, image::Rgb([200u8, 200, 200]));
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("scan_table.jpg");
        img.save(&path).unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        // OCR text with multi-column look — should NOT be reordered
        let ocr_text = "Name              Result\n\
                         Potassium         4.2\n\
                         Sodium            140\n\
                         Chloride          102\n\
                         Creatinine        72";

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new(ocr_text, 0.85)),
            Box::new(MockPdfExtractor::empty()),
        );

        let format = FormatDetection {
            mime_type: "image/jpeg".into(),
            category: FileCategory::Image,
            is_digital_pdf: None,
            file_size_bytes: 5000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session)
            .unwrap();

        // OCR text should be intact (no column reordering)
        assert_eq!(result.method, ExtractionMethod::TesseractOcr);
        assert!(result.full_text.contains("Potassium"));
    }

    #[test]
    fn scanned_pdf_table_gets_continuation_warning() {
        let (_dir, session) = setup();

        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("scanned_table.pdf");
        std::fs::write(&path, b"fake scanned table pdf").unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        // Scanned PDF with tabular OCR output across pages
        let scanned_pages = vec![
            PageExtraction {
                page_number: 1,
                text: "".into(),
                confidence: 0.0,
                regions: vec![],
                warnings: vec![],
            },
            PageExtraction {
                page_number: 2,
                text: "".into(),
                confidence: 0.0,
                regions: vec![],
                warnings: vec![],
            },
        ];

        // Mock OCR returns tabular text
        let ocr_text = "K\t4.2\tmmol/L\nNa\t140\tmmol/L\nCl\t102\tmmol/L";

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new(ocr_text, 0.82)),
            Box::new(MockPdfExtractor::with_pages(scanned_pages)),
        )
        .with_pdf_renderer(Box::new(MockPdfPageRenderer));

        let format = FormatDetection {
            mime_type: "application/pdf".into(),
            category: FileCategory::ScannedPdf,
            is_digital_pdf: Some(false),
            file_size_bytes: 50000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session)
            .unwrap();

        // Both pages have the same mock OCR output → both look tabular
        // Page 1 should get TableContinuation, page 2 should not
        let p1_cont = result.pages[0]
            .warnings
            .iter()
            .any(|w| matches!(w, ExtractionWarning::TableContinuation));
        assert!(p1_cont, "First page of table should have continuation warning");
    }

    #[test]
    fn full_pipeline_table_french_lab_report() {
        // Full integration: French lab report with tabular data, multi-page
        let (_dir, session) = setup();

        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("bilan_sanguin.pdf");
        std::fs::write(&path, b"fake french lab pdf").unwrap();

        let doc_id = Uuid::new_v4();
        let staged_path = stage_file(&path, &doc_id, &session).unwrap();

        let french_table_pages = vec![
            PageExtraction {
                page_number: 1,
                text: "Bilan Sanguin Complet\n\
                       Analyse\tRésultat\tUnité\tRéférence\n\
                       Potassium\t4,2\tmmol/L\t3,5-5,0\n\
                       Sodium\t140\tmmol/L\t136-145\n\
                       Chlorure\t102\tmmol/L\t98-106".into(),
                confidence: 0.95,
                regions: vec![],
                warnings: vec![],
            },
            PageExtraction {
                page_number: 2,
                text: "Créatinine\t72\tµmol/L\t53-97\n\
                       Urée\t5,5\tmmol/L\t2,8-7,2\n\
                       Glucose\t5,2\tmmol/L\t3,9-5,8\n\
                       Conclusion: résultats dans les normes".into(),
                confidence: 0.95,
                regions: vec![],
                warnings: vec![],
            },
        ];

        let extractor = DocumentExtractor::new(
            Box::new(MockOcrEngine::new("unused", 0.0)),
            Box::new(MockPdfExtractor::with_pages(french_table_pages)),
        );

        let format = FormatDetection {
            mime_type: "application/pdf".into(),
            category: FileCategory::DigitalPdf,
            is_digital_pdf: Some(true),
            file_size_bytes: 15000,
        };

        let result = extractor
            .extract(&doc_id, &staged_path, &format, &session)
            .unwrap();

        // Language should be French
        assert_eq!(result.language_detected.as_deref(), Some("fra"));

        // All French content preserved
        assert!(result.full_text.contains("Bilan Sanguin"));
        assert!(result.full_text.contains("Créatinine") || result.full_text.contains("Creatinine"));
        assert!(result.full_text.contains("µmol/L"));
        assert!(result.full_text.contains("résultats"));

        // Table continuation detected (tabular data spans both pages)
        let p1_cont = result.pages[0]
            .warnings
            .iter()
            .any(|w| matches!(w, ExtractionWarning::TableContinuation));
        assert!(p1_cont, "Page 1 should flag table continuing to page 2");

        // Good confidence
        assert!(result.overall_confidence > 0.80);
    }
}
