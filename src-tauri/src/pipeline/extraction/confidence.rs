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

    // Digital PDFs: dynamic confidence based on text quality (EXT-01-G06)
    if *method == ExtractionMethod::PdfDirect {
        return compute_digital_pdf_confidence(pages);
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

/// EXT-01-G06: Dynamic confidence for digital PDFs.
/// Scores based on three quality signals:
/// 1. Page coverage: ratio of pages with extractable text
/// 2. Text density: average characters per page (higher = richer extraction)
/// 3. Encoding validity: ratio of valid UTF-8 characters (non-replacement)
fn compute_digital_pdf_confidence(pages: &[PageExtraction]) -> f32 {
    if pages.is_empty() {
        return 0.0;
    }

    let non_empty_pages = pages.iter().filter(|p| !p.text.trim().is_empty()).collect::<Vec<_>>();
    let page_coverage = non_empty_pages.len() as f32 / pages.len() as f32;

    if non_empty_pages.is_empty() {
        return 0.0;
    }

    // Text density: average chars per non-empty page, capped contribution
    // < 10 chars/page = likely garbage; > 50 chars/page = good extraction
    let avg_chars: f32 = non_empty_pages.iter().map(|p| p.text.len() as f32).sum::<f32>()
        / non_empty_pages.len() as f32;
    let density_score = (avg_chars / 50.0).min(1.0);

    // Encoding validity: ratio of non-replacement characters
    // U+FFFD = replacement character from bad encoding
    let total_chars: usize = non_empty_pages.iter().map(|p| p.text.len()).sum();
    let replacement_chars: usize = non_empty_pages
        .iter()
        .map(|p| p.text.chars().filter(|c| *c == '\u{FFFD}').count())
        .sum();
    let encoding_score = if total_chars > 0 {
        1.0 - (replacement_chars as f32 / total_chars as f32)
    } else {
        0.0
    };

    // Weighted combination: coverage 40%, density 30%, encoding 30%
    // Base of 0.60 ensures digital PDFs generally score above OCR
    let quality = page_coverage * 0.40 + density_score * 0.30 + encoding_score * 0.30;
    // Scale to 0.60..0.99 range
    0.60 + quality * 0.39
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
        .filter(|w| w.confidence < 0.40)
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
    use super::super::types::OcrWordResult;

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
    fn digital_pdf_high_confidence_with_rich_text() {
        let long_text = "This is a well-extracted digital PDF page with plenty of medical content. ".repeat(5);
        let pages = vec![
            make_page(&long_text, 0.95),
            make_page(&long_text, 0.95),
        ];
        let conf = compute_overall_confidence(&pages, &ExtractionMethod::PdfDirect);
        // Full coverage, high density, clean encoding → near max
        assert!(conf > 0.90, "Expected > 0.90, got {conf}");
    }

    #[test]
    fn digital_pdf_empty_page_lowers_confidence() {
        let long_text = "Page with decent text content for scoring. ".repeat(5);
        let all_full = vec![
            make_page(&long_text, 0.95),
            make_page(&long_text, 0.95),
        ];
        let half_empty = vec![
            make_page(&long_text, 0.95),
            make_page("", 0.0),
        ];
        let full_conf = compute_overall_confidence(&all_full, &ExtractionMethod::PdfDirect);
        let half_conf = compute_overall_confidence(&half_empty, &ExtractionMethod::PdfDirect);
        // Half empty → lower confidence than all full
        assert!(half_conf < full_conf, "Half empty {half_conf} should be < full {full_conf}");
        assert!(half_conf > 0.60, "Expected > 0.60, got {half_conf}");
    }

    #[test]
    fn digital_pdf_sparse_text_lower_confidence() {
        // Very short text per page → low density score
        let pages = vec![
            make_page("OK", 0.95),
            make_page("Hi", 0.95),
        ];
        let conf = compute_overall_confidence(&pages, &ExtractionMethod::PdfDirect);
        // Full coverage but low density → moderate confidence
        assert!(conf > 0.60, "Expected > 0.60, got {conf}");
        assert!(conf < 0.95, "Expected < 0.95, got {conf}");
    }

    #[test]
    fn digital_pdf_encoding_errors_lower_confidence() {
        // Text with replacement characters (bad encoding)
        let bad_text = "Metformin \u{FFFD}\u{FFFD}\u{FFFD} 500mg daily \u{FFFD}";
        let pages = vec![make_page(bad_text, 0.95)];
        let good_pages = vec![make_page("Metformin hydrochloride 500mg daily dose", 0.95)];
        let bad_conf = compute_overall_confidence(&pages, &ExtractionMethod::PdfDirect);
        let good_conf = compute_overall_confidence(&good_pages, &ExtractionMethod::PdfDirect);
        assert!(bad_conf < good_conf, "Bad encoding {bad_conf} should be < good {good_conf}");
    }

    #[test]
    fn digital_pdf_all_empty_pages_zero() {
        let pages = vec![make_page("", 0.0), make_page("   ", 0.0)];
        let conf = compute_overall_confidence(&pages, &ExtractionMethod::PdfDirect);
        assert_eq!(conf, 0.0);
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
            word_confidences: vec![
                OcrWordResult { text: "blurry".into(), confidence: 0.20, bounding_box: None },
                OcrWordResult { text: "text".into(), confidence: 0.30, bounding_box: None },
            ],
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
                OcrWordResult { text: "a".into(), confidence: 0.10, bounding_box: None },
                OcrWordResult { text: "b".into(), confidence: 0.15, bounding_box: None },
                OcrWordResult { text: "c".into(), confidence: 0.20, bounding_box: None },
                OcrWordResult { text: "d".into(), confidence: 0.60, bounding_box: None },
                OcrWordResult { text: "e".into(), confidence: 0.30, bounding_box: None },
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
            word_confidences: vec![
                OcrWordResult { text: "clear".into(), confidence: 0.92, bounding_box: None },
                OcrWordResult { text: "text".into(), confidence: 0.88, bounding_box: None },
            ],
        };
        let warnings = analyze_ocr_quality(&result);
        assert!(!warnings
            .iter()
            .any(|w| matches!(w, ExtractionWarning::HandwritingDetected)));
    }

    #[test]
    fn handwriting_detected_with_zero_confidence_words() {
        // Tesseract returns -1 (mapped to 0.0) for unrecognizable handwriting
        let result = OcrPageResult {
            text: "scrawl illegible marks here now".into(),
            confidence: 0.15,
            word_confidences: vec![
                OcrWordResult { text: "scrawl".into(), confidence: 0.0, bounding_box: None },
                OcrWordResult { text: "illegible".into(), confidence: 0.0, bounding_box: None },
                OcrWordResult { text: "marks".into(), confidence: 0.0, bounding_box: None },
                OcrWordResult { text: "here".into(), confidence: 0.0, bounding_box: None },
                OcrWordResult { text: "now".into(), confidence: 0.45, bounding_box: None },
            ],
        };
        let warnings = analyze_ocr_quality(&result);
        assert!(warnings.iter().any(|w| matches!(w, ExtractionWarning::HandwritingDetected)));
    }

    #[test]
    fn no_handwriting_warning_with_empty_word_confidences() {
        let result = OcrPageResult {
            text: "".into(),
            confidence: 0.0,
            word_confidences: vec![],
        };
        let warnings = analyze_ocr_quality(&result);
        // Empty text: should not trigger handwriting warning
        assert!(!warnings.iter().any(|w| matches!(w, ExtractionWarning::HandwritingDetected)));
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
