use super::types::{ExtractionMethod, ExtractionWarning, OcrPageResult, PageExtraction, RegionConfidence};

/// Confidence thresholds used by UI and pipeline
pub mod thresholds {
    /// Below this: extraction likely failed. Show strong warning.
    pub const VERY_LOW: f32 = 0.30;

    /// Below this: significant uncertainty. Flag all extracted fields.
    pub const LOW: f32 = 0.50;

    /// Below this: some uncertainty. Flag key medical fields.
    pub const MODERATE: f32 = 0.70;

    /// Above this: high confidence. No special flagging.
    pub const HIGH: f32 = 0.85;

    /// Above this: very high confidence. Extracted from digital source.
    pub const VERY_HIGH: f32 = 0.95;
}

/// Compute overall document confidence from per-page results
pub fn compute_overall_confidence(
    pages: &[PageExtraction],
    method: &ExtractionMethod,
) -> f32 {
    if pages.is_empty() {
        return 0.0;
    }

    // Digital PDFs: base 0.95, scaled by ratio of pages with text
    if *method == ExtractionMethod::PdfDirect {
        let pages_with_text = pages.iter().filter(|p| !p.text.trim().is_empty()).count();
        let ratio = pages_with_text as f32 / pages.len() as f32;
        return 0.95 * ratio;
    }

    // Plain text: always high confidence
    if *method == ExtractionMethod::PlainTextRead {
        return 0.99;
    }

    // OCR: weighted average by text length
    let total_chars: usize = pages.iter().map(|p| p.text.len()).sum();
    if total_chars == 0 {
        return 0.0;
    }

    let weighted_sum: f32 = pages
        .iter()
        .map(|p| p.confidence * p.text.len() as f32)
        .sum();

    weighted_sum / total_chars as f32
}

/// Analyze OCR result and generate warnings
pub fn analyze_ocr_quality(result: &OcrPageResult) -> Vec<ExtractionWarning> {
    let mut warnings = Vec::new();

    if result.confidence < thresholds::LOW {
        warnings.push(ExtractionWarning::BlurryImage);
    }

    // Check for signs of handwriting (majority of words below 0.40 confidence)
    let total_words = result.word_confidences.len().max(1);
    let low_conf_words = result
        .word_confidences
        .iter()
        .filter(|(_, c)| *c < 0.40)
        .count();
    if low_conf_words as f64 / total_words as f64 > 0.50 {
        warnings.push(ExtractionWarning::HandwritingDetected);
    }

    warnings
}

/// Identify low-confidence regions for highlighting in the review screen
pub fn flag_low_confidence_regions(
    page: &PageExtraction,
    threshold: f32,
) -> Vec<RegionConfidence> {
    page.regions
        .iter()
        .filter(|r| r.confidence < threshold)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_page(text: &str, confidence: f32) -> PageExtraction {
        PageExtraction {
            page_number: 1,
            text: text.to_string(),
            confidence,
            regions: vec![],
            warnings: vec![],
        }
    }

    #[test]
    fn digital_pdf_high_confidence() {
        let pages = vec![
            make_page("Page one with text.", 0.95),
            make_page("Page two with more text.", 0.95),
        ];
        let conf = compute_overall_confidence(&pages, &ExtractionMethod::PdfDirect);
        assert!(conf > 0.90, "Expected > 0.90, got {conf}");
    }

    #[test]
    fn digital_pdf_empty_page_lowers_confidence() {
        let pages = vec![
            make_page("Page with text.", 0.95),
            make_page("", 0.0),
        ];
        let conf = compute_overall_confidence(&pages, &ExtractionMethod::PdfDirect);
        // One of two pages has text → 0.95 * 0.5 = 0.475
        assert!((conf - 0.475).abs() < 0.01, "Expected ~0.475, got {conf}");
    }

    #[test]
    fn ocr_weighted_by_text_length() {
        let pages = vec![
            PageExtraction {
                page_number: 1,
                text: "Clear text on page one ".repeat(10),
                confidence: 0.85,
                regions: vec![],
                warnings: vec![],
            },
            PageExtraction {
                page_number: 2,
                text: "Blurry".to_string(),
                confidence: 0.30,
                regions: vec![],
                warnings: vec![],
            },
        ];
        let conf = compute_overall_confidence(&pages, &ExtractionMethod::TesseractOcr);
        // Long page dominates: should be close to 0.85
        assert!(conf > 0.70, "Expected > 0.70, got {conf}");
    }

    #[test]
    fn empty_pages_returns_zero() {
        let conf = compute_overall_confidence(&[], &ExtractionMethod::TesseractOcr);
        assert_eq!(conf, 0.0);
    }

    #[test]
    fn ocr_all_empty_text_returns_zero() {
        let pages = vec![make_page("", 0.0), make_page("", 0.0)];
        let conf = compute_overall_confidence(&pages, &ExtractionMethod::TesseractOcr);
        assert_eq!(conf, 0.0);
    }

    #[test]
    fn plain_text_always_high() {
        let pages = vec![make_page("Any text", 0.99)];
        let conf = compute_overall_confidence(&pages, &ExtractionMethod::PlainTextRead);
        assert!((conf - 0.99).abs() < f32::EPSILON);
    }

    #[test]
    fn blurry_image_warning_on_low_confidence() {
        let result = OcrPageResult {
            text: "blurry text".into(),
            confidence: 0.25,
            word_confidences: vec![("blurry".into(), 0.20), ("text".into(), 0.30)],
        };
        let warnings = analyze_ocr_quality(&result);
        assert!(warnings
            .iter()
            .any(|w| matches!(w, ExtractionWarning::BlurryImage)));
    }

    #[test]
    fn handwriting_warning_on_many_low_confidence_words() {
        let result = OcrPageResult {
            text: "a b c d e".into(),
            confidence: 0.35,
            word_confidences: vec![
                ("a".into(), 0.10),
                ("b".into(), 0.15),
                ("c".into(), 0.20),
                ("d".into(), 0.60),
                ("e".into(), 0.30),
            ],
        };
        let warnings = analyze_ocr_quality(&result);
        assert!(warnings
            .iter()
            .any(|w| matches!(w, ExtractionWarning::HandwritingDetected)));
    }

    #[test]
    fn no_handwriting_warning_on_clear_text() {
        let result = OcrPageResult {
            text: "clear text".into(),
            confidence: 0.90,
            word_confidences: vec![("clear".into(), 0.92), ("text".into(), 0.88)],
        };
        let warnings = analyze_ocr_quality(&result);
        assert!(!warnings
            .iter()
            .any(|w| matches!(w, ExtractionWarning::HandwritingDetected)));
    }

    #[test]
    fn flag_regions_below_threshold() {
        let page = PageExtraction {
            page_number: 1,
            text: "test".into(),
            confidence: 0.80,
            regions: vec![
                RegionConfidence {
                    text: "clear".into(),
                    confidence: 0.90,
                    bounding_box: None,
                },
                RegionConfidence {
                    text: "blurry".into(),
                    confidence: 0.40,
                    bounding_box: None,
                },
                RegionConfidence {
                    text: "medium".into(),
                    confidence: 0.65,
                    bounding_box: None,
                },
            ],
            warnings: vec![],
        };
        let flagged = flag_low_confidence_regions(&page, thresholds::MODERATE);
        assert_eq!(flagged.len(), 2); // 0.40 and 0.65 are below 0.70
        assert_eq!(flagged[0].text, "blurry");
        assert_eq!(flagged[1].text, "medium");
    }

    #[test]
    fn threshold_constants_are_ordered() {
        assert!(thresholds::VERY_LOW < thresholds::LOW);
        assert!(thresholds::LOW < thresholds::MODERATE);
        assert!(thresholds::MODERATE < thresholds::HIGH);
        assert!(thresholds::HIGH < thresholds::VERY_HIGH);
    }
}
