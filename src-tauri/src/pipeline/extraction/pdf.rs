use super::types::{PageExtraction, PdfExtractor};
use super::ExtractionError;

/// PDF text extractor using the pdf-extract crate.
/// Handles digital PDFs with embedded text layers.
pub struct PdfTextExtractor;

impl PdfExtractor for PdfTextExtractor {
    fn extract_text(&self, pdf_bytes: &[u8]) -> Result<Vec<PageExtraction>, ExtractionError> {
        let page_texts = pdf_extract::extract_text_from_mem_by_pages(pdf_bytes)
            .map_err(|e| ExtractionError::PdfParsing(e.to_string()))?;

        let pages = page_texts
            .into_iter()
            .enumerate()
            .map(|(i, text)| {
                let confidence = if text.trim().len() > 10 { 0.95 } else { 0.0 };
                PageExtraction {
                    page_number: i + 1,
                    text,
                    confidence,
                    regions: vec![],
                    warnings: vec![],
                }
            })
            .collect();

        Ok(pages)
    }

    fn page_count(&self, pdf_bytes: &[u8]) -> Result<usize, ExtractionError> {
        let pages = pdf_extract::extract_text_from_mem_by_pages(pdf_bytes)
            .map_err(|e| ExtractionError::PdfParsing(e.to_string()))?;
        Ok(pages.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generate a valid PDF with text using lopdf (the library that pdf-extract uses internally).
    fn make_test_pdf(text: &str) -> Vec<u8> {
        use lopdf::dictionary;
        use lopdf::{Document, Object, Stream};

        let mut doc = Document::with_version("1.4");

        // Font dictionary
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        // Page content stream: BT /F1 12 Tf (text) Tj ET
        let content = format!("BT /F1 12 Tf 100 700 Td ({text}) Tj ET");
        let content_stream = Stream::new(dictionary! {}, content.into_bytes());
        let content_id = doc.add_object(content_stream);

        // Resources dictionary
        let resources = dictionary! {
            "Font" => dictionary! {
                "F1" => font_id,
            },
        };

        // Page
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => content_id,
            "Resources" => resources,
        });

        // Pages
        let pages_id = doc.add_object(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        });

        // Update page parent
        if let Ok(page) = doc.get_object_mut(page_id) {
            if let Object::Dictionary(ref mut dict) = page {
                dict.set("Parent", pages_id);
            }
        }

        // Catalog
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });

        doc.trailer.set("Root", catalog_id);

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    #[test]
    fn extract_text_from_digital_pdf() {
        let extractor = PdfTextExtractor;
        let pdf_bytes = make_test_pdf("Hello World from Coheara");
        let pages = extractor.extract_text(&pdf_bytes).unwrap();

        assert!(!pages.is_empty(), "Should extract at least one page");
        let full_text: String = pages.iter().map(|p| p.text.clone()).collect();
        assert!(
            full_text.contains("Hello") || full_text.contains("World"),
            "Expected text to contain 'Hello' or 'World', got: {full_text}"
        );
    }

    #[test]
    fn page_count_matches_extraction() {
        let extractor = PdfTextExtractor;
        let pdf_bytes = make_test_pdf("Test content");
        let count = extractor.page_count(&pdf_bytes).unwrap();
        let pages = extractor.extract_text(&pdf_bytes).unwrap();
        assert_eq!(count, pages.len());
    }

    #[test]
    fn invalid_pdf_returns_error() {
        let extractor = PdfTextExtractor;
        let result = extractor.extract_text(b"not a pdf");
        assert!(result.is_err());
    }

    #[test]
    fn confidence_high_for_pages_with_text() {
        let extractor = PdfTextExtractor;
        let pdf_bytes = make_test_pdf("Patient report with sufficient text content here");
        let pages = extractor.extract_text(&pdf_bytes).unwrap();

        for page in &pages {
            if page.text.trim().len() > 10 {
                assert!(
                    page.confidence > 0.90,
                    "Page with text should have high confidence, got {}",
                    page.confidence
                );
            }
        }
    }
}
