//! CT-01: Text-only extraction — no vision model required.
//!
//! Handles two file categories without any LLM:
//! - `PlainText`: UTF-8 read (same as DocumentExtractor's plain text path)
//! - `DigitalPdf`: PDFium native text layer extraction (no rendering, no OCR)
//!
//! Returns `ExtractionError::UnsupportedFormat` for `ScannedPdf`/`Image` —
//! those require a vision model and should use `DocumentExtractor` instead.

use std::path::Path;

use uuid::Uuid;

use super::confidence::compute_overall_confidence;
use super::pdfium::{load_pdfium, map_load_error};
use super::sanitize::sanitize_extracted_text;
use super::types::{
    ExtractionMethod, ExtractionResult, PageExtraction, TextExtractor,
};
use super::ExtractionError;
use crate::crypto::ProfileSession;
use crate::pipeline::import::format::FileCategory;
use crate::pipeline::import::staging::read_staged_file;
use crate::pipeline::import::FormatDetection;

/// Text-only extractor: handles PlainText and DigitalPdf without a vision model.
///
/// Used by the ModelRouter when no vision-capable model is available/enabled,
/// but the document type doesn't require one (digital PDF text layer, plain text).
pub struct PlainTextExtractor;

impl TextExtractor for PlainTextExtractor {
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
            "PlainTextExtractor: starting text-only extraction"
        );

        let decrypted_bytes = read_staged_file(staged_path, session)?;

        let (method, mut pages) = match &format.category {
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

            FileCategory::DigitalPdf => {
                extract_pdf_text_layer(&decrypted_bytes)?
            }

            FileCategory::ScannedPdf | FileCategory::Image => {
                return Err(ExtractionError::UnsupportedFormat);
            }

            FileCategory::Unsupported => {
                return Err(ExtractionError::UnsupportedFormat);
            }
        };

        // Sanitize extracted text
        for page in &mut pages {
            page.text = sanitize_extracted_text(&page.text);
        }

        let overall_confidence = compute_overall_confidence(&pages, &method);

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
            "PlainTextExtractor: extraction complete"
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

/// Extract text from a digital PDF using pdfium's native text layer.
///
/// No vision model or rendering needed — reads the embedded text directly.
/// Returns `PdfDirect` extraction method with 0.95 confidence (native text is reliable).
fn extract_pdf_text_layer(
    pdf_bytes: &[u8],
) -> Result<(ExtractionMethod, Vec<PageExtraction>), ExtractionError> {
    let pdfium = load_pdfium()?;
    let document = pdfium
        .load_pdf_from_byte_slice(pdf_bytes, None)
        .map_err(map_load_error)?;

    let page_count = document.pages().len();
    if page_count == 0 {
        return Err(ExtractionError::EmptyDocument);
    }

    let mut pages = Vec::with_capacity(page_count as usize);

    for (idx, page) in document.pages().iter().enumerate() {
        let text = page
            .text()
            .map(|t| t.all())
            .unwrap_or_default();

        pages.push(PageExtraction {
            page_number: idx + 1,
            text,
            confidence: 0.95,
            regions: vec![],
            warnings: vec![],
            content_type: None,
        });
    }

    Ok((ExtractionMethod::PdfDirect, pages))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::profile;
    use crate::pipeline::import::staging::stage_file;

    fn setup() -> (tempfile::TempDir, ProfileSession) {
        let dir = tempfile::tempdir().unwrap();
        let (info, _phrase) = profile::create_profile(
            dir.path(),
            "TextOnlyTest",
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

    fn stage_text(session: &ProfileSession, content: &str) -> (Uuid, std::path::PathBuf) {
        let source_dir = tempfile::tempdir().unwrap();
        let path = source_dir.path().join("test.txt");
        std::fs::write(&path, content).unwrap();
        let doc_id = Uuid::new_v4();
        let staged = stage_file(&path, &doc_id, session).unwrap();
        (doc_id, staged)
    }

    #[test]
    fn plain_text_extraction() {
        let (_dir, session) = setup();
        let content = "Potassium: 4.2 mmol/L";
        let (doc_id, staged_path) = stage_text(&session, content);

        let extractor = PlainTextExtractor;
        let format = FormatDetection {
            mime_type: "text/plain".into(),
            category: FileCategory::PlainText,
            is_digital_pdf: None,
            file_size_bytes: content.len() as u64,
        };

        let result = extractor.extract(&doc_id, &staged_path, &format, &session).unwrap();

        assert_eq!(result.method, ExtractionMethod::PlainTextRead);
        assert!(result.full_text.contains("Potassium"));
        assert!(result.overall_confidence > 0.95);
        assert_eq!(result.page_count, 1);
    }

    #[test]
    fn scanned_pdf_rejected() {
        let (_dir, session) = setup();
        let (doc_id, staged_path) = stage_text(&session, "fake pdf");

        let extractor = PlainTextExtractor;
        let format = FormatDetection {
            mime_type: "application/pdf".into(),
            category: FileCategory::ScannedPdf,
            is_digital_pdf: Some(false),
            file_size_bytes: 100,
        };

        let result = extractor.extract(&doc_id, &staged_path, &format, &session);
        assert!(matches!(result, Err(ExtractionError::UnsupportedFormat)));
    }

    #[test]
    fn image_rejected() {
        let (_dir, session) = setup();
        let (doc_id, staged_path) = stage_text(&session, "fake image");

        let extractor = PlainTextExtractor;
        let format = FormatDetection {
            mime_type: "image/jpeg".into(),
            category: FileCategory::Image,
            is_digital_pdf: None,
            file_size_bytes: 100,
        };

        let result = extractor.extract(&doc_id, &staged_path, &format, &session);
        assert!(matches!(result, Err(ExtractionError::UnsupportedFormat)));
    }

    #[test]
    fn plain_text_sanitized() {
        let (_dir, session) = setup();
        let content = "Patient\x00Name\x01: test";
        let (doc_id, staged_path) = stage_text(&session, content);

        let extractor = PlainTextExtractor;
        let format = FormatDetection {
            mime_type: "text/plain".into(),
            category: FileCategory::PlainText,
            is_digital_pdf: None,
            file_size_bytes: content.len() as u64,
        };

        let result = extractor.extract(&doc_id, &staged_path, &format, &session).unwrap();

        assert!(!result.full_text.contains('\x00'));
        assert!(!result.full_text.contains('\x01'));
        assert!(result.full_text.contains("test"));
    }
}
