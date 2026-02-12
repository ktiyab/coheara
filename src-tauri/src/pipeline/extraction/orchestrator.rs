use std::path::Path;

use uuid::Uuid;

use super::confidence::{analyze_ocr_quality, compute_overall_confidence};
use super::preprocess::preprocess_image;
use super::sanitize::sanitize_extracted_text;
use super::types::{
    ExtractionMethod, ExtractionResult, ExtractionWarning, OcrEngine, PageExtraction, PdfExtractor,
    RegionConfidence, TextExtractor,
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
}

impl DocumentExtractor {
    pub fn new(
        ocr_engine: Box<dyn OcrEngine + Send + Sync>,
        pdf_extractor: Box<dyn PdfExtractor + Send + Sync>,
    ) -> Self {
        Self {
            ocr_engine,
            pdf_extractor,
        }
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
                let pages =
                    ocr_scanned_pdf(&decrypted_bytes, &*self.pdf_extractor, &*self.ocr_engine)?;
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
                        .map(|(word, conf)| RegionConfidence {
                            text: word.clone(),
                            confidence: *conf,
                            bounding_box: None,
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
            language_detected: None,
            page_count,
        })
    }
}

/// OCR a scanned PDF page by page.
/// Each page is extracted as text (which will be empty/short for scanned PDFs),
/// then if text is insufficient, OCR is run on the page image.
fn ocr_scanned_pdf(
    pdf_bytes: &[u8],
    pdf_extractor: &dyn PdfExtractor,
    ocr_engine: &dyn OcrEngine,
) -> Result<Vec<PageExtraction>, ExtractionError> {
    // First try direct text extraction — scanned PDFs will yield little/no text
    let direct_pages = pdf_extractor.extract_text(pdf_bytes)?;

    let mut pages = Vec::with_capacity(direct_pages.len());

    for direct_page in &direct_pages {
        // If direct extraction found meaningful text, use it
        if direct_page.text.trim().len() > 20 {
            pages.push(direct_page.clone());
            continue;
        }

        // Otherwise, OCR the raw PDF bytes as an image
        // Note: full page rendering from PDF is complex; for MVP we OCR the
        // entire PDF as a single document. Proper page-by-page rendering
        // requires a PDF renderer (pdfium/mupdf), deferred to Phase 2.
        let ocr_result = ocr_engine.ocr_image(pdf_bytes)?;
        let warnings = analyze_ocr_quality(&ocr_result);

        pages.push(PageExtraction {
            page_number: direct_page.page_number,
            text: ocr_result.text,
            confidence: ocr_result.confidence,
            regions: ocr_result
                .word_confidences
                .iter()
                .map(|(word, conf)| RegionConfidence {
                    text: word.clone(),
                    confidence: *conf,
                    bounding_box: None,
                })
                .collect(),
            warnings,
        });

        // For MVP: only OCR once for scanned PDFs (all pages get same result)
        // Proper per-page rendering deferred to Phase 2
        break;
    }

    // If no pages were produced at all, return a single empty page
    if pages.is_empty() && !direct_pages.is_empty() {
        pages.push(PageExtraction {
            page_number: 1,
            text: String::new(),
            confidence: 0.0,
            regions: vec![],
            warnings: vec![ExtractionWarning::PartialExtraction {
                reason: "Scanned PDF page rendering not yet supported".into(),
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

    fn setup() -> (tempfile::TempDir, ProfileSession) {
        let dir = tempfile::tempdir().unwrap();
        let (info, _phrase) =
            profile::create_profile(dir.path(), "ExtractTest", "test_pass_123", None).unwrap();
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
}
