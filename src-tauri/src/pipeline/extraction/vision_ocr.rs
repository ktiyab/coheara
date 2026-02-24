//! R4: Vision OCR engine — extracts text from document images via Ollama.
//!
//! Bridges the `VisionClient` (structuring layer) to the `VisionOcrEngine` trait
//! (extraction layer). Uses MedGemma with a system prompt + extraction instruction.
//!
//! Appends a classification tag (`[DOCUMENT]` or `[MEDICAL_IMAGE]`) that the
//! orchestrator uses to route medical images to MedGemma interpretation.

use std::sync::Arc;

use base64::Engine as _;

use super::types::{
    ImageContentType, MedicalImageInterpreter, MedicalImageResult, VisionOcrEngine,
    VisionOcrResult,
};
use super::ExtractionError;
use crate::pipeline::structuring::types::VisionClient;

// ──────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────

/// Classification tag appended by the vision model to signal document type.
const DOCUMENT_TAG: &str = "[DOCUMENT]";
const MEDICAL_IMAGE_TAG: &str = "[MEDICAL_IMAGE]";

// ──────────────────────────────────────────────
// Language-aware prompt constants
// ──────────────────────────────────────────────

/// English system prompt — default.
const SYSTEM_PROMPT_EN: &str = "\
You are a medical document text extractor. Your task is to extract ALL visible text \
from the provided document image, preserving structure as Markdown. \
Output headers, tables, lists, and paragraphs. Be thorough and accurate.";

/// French system prompt — preserves accented characters and French medical terminology.
const SYSTEM_PROMPT_FR: &str = "\
You are a medical document text extractor. The document is likely in French. \
Extract ALL visible text, preserving structure as Markdown. \
Preserve French medical terminology, accented characters (é, è, ê, ë, à, ç, ù, ô, î), \
and units exactly as written. Output headers, tables, lists, and paragraphs.";

/// German system prompt — preserves umlauts and German medical terminology.
const SYSTEM_PROMPT_DE: &str = "\
You are a medical document text extractor. The document is likely in German. \
Extract ALL visible text, preserving structure as Markdown. \
Preserve German medical terminology, umlauts (ä, ö, ü, ß), \
and units exactly as written. Output headers, tables, lists, and paragraphs.";

/// English user prompt — default.
const USER_PROMPT_EN: &str = "\
Extract all visible text from this document image as structured Markdown. \
Preserve tables using Markdown table syntax. Preserve headers using # syntax. \
At the very end, on a new line, write exactly [DOCUMENT] if this is a text document, \
or [MEDICAL_IMAGE] if this is a medical image (X-ray, CT, MRI, radiograph, dermatology photo).";

/// French user prompt — targets French medical documents.
const USER_PROMPT_FR: &str = "\
Extract all visible text from this French medical document as structured Markdown. \
Preserve tables using Markdown table syntax. Preserve headers using # syntax. \
Preserve all French accented characters and medical terms exactly as written. \
At the very end, on a new line, write exactly [DOCUMENT] if this is a text document, \
or [MEDICAL_IMAGE] if this is a medical image (X-ray, CT, MRI, radiograph, dermatology photo).";

/// German user prompt — targets German medical documents.
const USER_PROMPT_DE: &str = "\
Extract all visible text from this German medical document as structured Markdown. \
Preserve tables using Markdown table syntax. Preserve headers using # syntax. \
Preserve all German umlauts and medical terms exactly as written. \
At the very end, on a new line, write exactly [DOCUMENT] if this is a text document, \
or [MEDICAL_IMAGE] if this is a medical image (X-ray, CT, MRI, radiograph, dermatology photo).";

// ──────────────────────────────────────────────
// OllamaVisionOcr
// ──────────────────────────────────────────────

// ──────────────────────────────────────────────
// Public prompt builders (reusable by any model)
// ──────────────────────────────────────────────

/// Select the system prompt for the given language code.
///
/// Supports "en", "fr", "de". Falls back to English for unknown codes.
/// Public so other modules (e.g., chat extraction) can reuse prompt routing.
pub fn build_system_prompt(lang: &str) -> &'static str {
    match lang {
        "fr" => SYSTEM_PROMPT_FR,
        "de" => SYSTEM_PROMPT_DE,
        _ => SYSTEM_PROMPT_EN,
    }
}

/// Select the user prompt for the given language code.
///
/// Supports "en", "fr", "de". Falls back to English for unknown codes.
/// Public so other modules can reuse prompt routing.
pub fn build_user_prompt(lang: &str) -> &'static str {
    match lang {
        "fr" => USER_PROMPT_FR,
        "de" => USER_PROMPT_DE,
        _ => USER_PROMPT_EN,
    }
}

// ──────────────────────────────────────────────
// OllamaVisionOcr
// ──────────────────────────────────────────────

/// Production vision OCR engine backed by Ollama.
///
/// R4: Accepts any `VisionClient` implementation (OllamaClient or mock).
/// Model name is pre-resolved by the caller (via `ActiveModelResolver`).
/// Language configures prompt selection (EN/FR/DE).
pub struct OllamaVisionOcr {
    vision_client: Arc<dyn VisionClient>,
    model_name: String,
    language: String,
}

impl OllamaVisionOcr {
    pub fn new(vision_client: Arc<dyn VisionClient>, model_name: String) -> Self {
        Self {
            vision_client,
            model_name,
            language: "en".to_string(),
        }
    }

    /// Set the language for prompt selection (e.g., "fr", "de", "en").
    pub fn with_language(mut self, language: &str) -> Self {
        self.language = language.to_string();
        self
    }
}

impl VisionOcrEngine for OllamaVisionOcr {
    fn extract_text_from_image(
        &self,
        image_bytes: &[u8],
    ) -> Result<VisionOcrResult, ExtractionError> {
        let _span = tracing::info_span!(
            "vision_ocr_extract",
            model = %self.model_name,
            image_size = image_bytes.len(),
        )
        .entered();
        let start = std::time::Instant::now();

        // Encode image to base64 for Ollama API
        let base64_image = base64::engine::general_purpose::STANDARD.encode(image_bytes);
        let images = vec![base64_image];

        // R4+: Language-aware prompt selection
        let system_prompt = build_system_prompt(&self.language);
        let user_prompt = build_user_prompt(&self.language);

        let raw_response = self
            .vision_client
            .chat_with_images(
                &self.model_name,
                user_prompt,
                &images,
                Some(system_prompt),
            )
            .map_err(|e| ExtractionError::OcrProcessing(format!("Vision OCR failed: {e}")))?;

        // Parse classification tag from response
        let (text, content_type) = parse_classification_tag(&raw_response);

        // Compute heuristic confidence
        let confidence = compute_heuristic_confidence(&text);

        tracing::info!(
            model = %self.model_name,
            elapsed_ms = %start.elapsed().as_millis(),
            text_len = text.len(),
            confidence,
            content_type = ?content_type,
            "Vision OCR extraction complete"
        );

        Ok(VisionOcrResult {
            text,
            model_used: self.model_name.clone(),
            confidence,
            content_type,
        })
    }
}

// ──────────────────────────────────────────────
// Classification tag parsing
// ──────────────────────────────────────────────

/// Parse the classification tag from the model's response.
///
/// The model is instructed to append `[DOCUMENT]` or `[MEDICAL_IMAGE]`
/// at the end of its output. This function:
/// 1. Searches for the tag at the end of the response
/// 2. Strips the tag from the returned text
/// 3. Defaults to `Document` if no tag is found
fn parse_classification_tag(response: &str) -> (String, ImageContentType) {
    let trimmed = response.trim();

    if trimmed.ends_with(MEDICAL_IMAGE_TAG) {
        let text = trimmed[..trimmed.len() - MEDICAL_IMAGE_TAG.len()]
            .trim()
            .to_string();
        (text, ImageContentType::MedicalImage)
    } else if trimmed.ends_with(DOCUMENT_TAG) {
        let text = trimmed[..trimmed.len() - DOCUMENT_TAG.len()]
            .trim()
            .to_string();
        (text, ImageContentType::Document)
    } else {
        // No tag found — default to Document (conservative)
        (trimmed.to_string(), ImageContentType::Document)
    }
}

// ──────────────────────────────────────────────
// Confidence heuristic
// ──────────────────────────────────────────────

/// Compute a heuristic confidence score based on text quality.
///
/// R3: Vision models don't provide per-word confidence like Tesseract.
/// Instead, we estimate confidence from output characteristics:
///
/// 1. **Text length** (primary signal):
///    - 0 chars → 0.0 (extraction failed)
///    - 1-49 chars → 0.2 (minimal content)
///    - 50-199 chars → 0.4 (short content)
///    - 200-499 chars → 0.6 (moderate content)
///    - 500+ chars → 0.8 (substantial content)
///
/// 2. **Structure markers** (bonus):
///    - Markdown headers `#` → +0.05
///    - Table pipes `|` → +0.05
///    - List markers `- ` or `* ` → +0.03
///
/// Capped at 0.95 (never claim certainty for heuristic scoring).
fn compute_heuristic_confidence(text: &str) -> f32 {
    if text.is_empty() {
        return 0.0;
    }

    let len = text.len();

    // Base confidence from text length
    let base: f32 = if len < 50 {
        0.2
    } else if len < 200 {
        0.4
    } else if len < 500 {
        0.6
    } else {
        0.8
    };

    // Structure bonuses
    let has_headers = text.lines().any(|l| l.starts_with('#'));
    let has_tables = text.lines().any(|l| l.contains('|') && l.matches('|').count() >= 2);
    let has_lists = text
        .lines()
        .any(|l| l.trim_start().starts_with("- ") || l.trim_start().starts_with("* "));

    let bonus: f32 = if has_headers { 0.05 } else { 0.0 }
        + if has_tables { 0.05 } else { 0.0 }
        + if has_lists { 0.03 } else { 0.0 };

    (base + bonus).min(0.95)
}

// ──────────────────────────────────────────────
// Medical image interpretation
// ──────────────────────────────────────────────

/// System prompt for medical image interpretation (MedGemma clinical mode).
const INTERPRET_SYSTEM_PROMPT: &str = "\
You are a medical image specialist. Analyze the provided medical image \
(X-ray, CT, MRI, dermatology photo, pathology slide) and describe your findings. \
Be thorough but factual. Do NOT provide diagnoses or treatment recommendations — \
describe anatomical structures and observations only.";

/// User prompt for medical image interpretation.
const INTERPRET_USER_PROMPT: &str = "\
Describe what you observe in this medical image. Include:\n\
- Image modality (X-ray, CT, MRI, dermatology, pathology, etc.)\n\
- Anatomical region\n\
- Key observations and findings\n\
- Any notable abnormalities or normal variants\n\
Output as structured Markdown.";

/// Production medical image interpreter backed by Ollama.
///
/// Uses MedGemma's clinical interpretation capability to describe
/// medical imagery (X-rays, CT, MRI, dermatology, histopathology).
pub struct OllamaMedicalImageInterpreter {
    vision_client: Arc<dyn VisionClient>,
    model_name: String,
}

impl OllamaMedicalImageInterpreter {
    pub fn new(vision_client: Arc<dyn VisionClient>, model_name: String) -> Self {
        Self {
            vision_client,
            model_name,
        }
    }
}

impl MedicalImageInterpreter for OllamaMedicalImageInterpreter {
    fn interpret_medical_image(
        &self,
        image_bytes: &[u8],
    ) -> Result<MedicalImageResult, ExtractionError> {
        let _span = tracing::info_span!(
            "medical_image_interpret",
            model = %self.model_name,
            image_size = image_bytes.len(),
        )
        .entered();
        let start = std::time::Instant::now();

        let base64_image = base64::engine::general_purpose::STANDARD.encode(image_bytes);
        let images = vec![base64_image];

        let raw_response = self
            .vision_client
            .chat_with_images(
                &self.model_name,
                INTERPRET_USER_PROMPT,
                &images,
                Some(INTERPRET_SYSTEM_PROMPT),
            )
            .map_err(|e| {
                ExtractionError::OcrProcessing(format!("Medical image interpretation failed: {e}"))
            })?;

        let findings = raw_response.trim().to_string();
        let confidence = compute_heuristic_confidence(&findings);

        tracing::info!(
            model = %self.model_name,
            elapsed_ms = %start.elapsed().as_millis(),
            findings_len = findings.len(),
            confidence,
            "Medical image interpretation complete"
        );

        Ok(MedicalImageResult {
            findings,
            model_used: self.model_name.clone(),
            confidence,
        })
    }
}

/// Mock medical image interpreter for testing.
pub struct MockMedicalImageInterpreter {
    findings: String,
    model_name: String,
    confidence: f32,
}

impl MockMedicalImageInterpreter {
    pub fn new(findings: &str, model_name: &str, confidence: f32) -> Self {
        Self {
            findings: findings.to_string(),
            model_name: model_name.to_string(),
            confidence,
        }
    }
}

impl MedicalImageInterpreter for MockMedicalImageInterpreter {
    fn interpret_medical_image(
        &self,
        _image_bytes: &[u8],
    ) -> Result<MedicalImageResult, ExtractionError> {
        Ok(MedicalImageResult {
            findings: self.findings.clone(),
            model_used: self.model_name.clone(),
            confidence: self.confidence,
        })
    }
}

// ──────────────────────────────────────────────
// MockVisionOcr (testing)
// ──────────────────────────────────────────────

/// Mock vision OCR engine for testing.
///
/// Returns a configurable response text, model name, and content type.
/// Use `with_confidence()` to override the heuristic confidence score.
pub struct MockVisionOcr {
    response_text: String,
    model_name: String,
    content_type: ImageContentType,
    confidence_override: Option<f32>,
}

impl MockVisionOcr {
    pub fn new(response_text: &str, model_name: &str) -> Self {
        Self {
            response_text: response_text.to_string(),
            model_name: model_name.to_string(),
            content_type: ImageContentType::Document,
            confidence_override: None,
        }
    }

    pub fn with_content_type(mut self, content_type: ImageContentType) -> Self {
        self.content_type = content_type;
        self
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence_override = Some(confidence);
        self
    }
}

impl VisionOcrEngine for MockVisionOcr {
    fn extract_text_from_image(
        &self,
        _image_bytes: &[u8],
    ) -> Result<VisionOcrResult, ExtractionError> {
        let confidence = self
            .confidence_override
            .unwrap_or_else(|| compute_heuristic_confidence(&self.response_text));
        Ok(VisionOcrResult {
            text: self.response_text.clone(),
            model_used: self.model_name.clone(),
            confidence,
            content_type: self.content_type,
        })
    }
}

// ──────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::structuring::ollama::MockVisionClient;
    use crate::pipeline::structuring::ollama_types::OllamaError;

    // ── parse_classification_tag ──

    #[test]
    fn parse_document_tag() {
        let (text, ct) = parse_classification_tag("# Lab Report\nSome text\n[DOCUMENT]");
        assert_eq!(ct, ImageContentType::Document);
        assert_eq!(text, "# Lab Report\nSome text");
    }

    #[test]
    fn parse_medical_image_tag() {
        let (text, ct) = parse_classification_tag("Chest X-ray findings...\n[MEDICAL_IMAGE]");
        assert_eq!(ct, ImageContentType::MedicalImage);
        assert_eq!(text, "Chest X-ray findings...");
    }

    #[test]
    fn parse_no_tag_defaults_to_document() {
        let (text, ct) = parse_classification_tag("Just some text without a tag");
        assert_eq!(ct, ImageContentType::Document);
        assert_eq!(text, "Just some text without a tag");
    }

    #[test]
    fn parse_empty_response() {
        let (text, ct) = parse_classification_tag("");
        assert_eq!(ct, ImageContentType::Document);
        assert!(text.is_empty());
    }

    #[test]
    fn parse_tag_with_trailing_whitespace() {
        let (text, ct) = parse_classification_tag("Content here  \n  [DOCUMENT]  ");
        assert_eq!(ct, ImageContentType::Document);
        assert_eq!(text, "Content here");
    }

    // ── compute_heuristic_confidence ──

    #[test]
    fn confidence_empty_text_is_zero() {
        assert_eq!(compute_heuristic_confidence(""), 0.0);
    }

    #[test]
    fn confidence_short_text_is_low() {
        let c = compute_heuristic_confidence("Hello");
        assert!((c - 0.2).abs() < f32::EPSILON, "Short text: {c}");
    }

    #[test]
    fn confidence_moderate_text() {
        let text = "x".repeat(100);
        let c = compute_heuristic_confidence(&text);
        assert!((c - 0.4).abs() < f32::EPSILON, "100 chars: {c}");
    }

    #[test]
    fn confidence_long_text() {
        let text = "x".repeat(600);
        let c = compute_heuristic_confidence(&text);
        assert!((c - 0.8).abs() < f32::EPSILON, "600 chars: {c}");
    }

    #[test]
    fn confidence_with_structure_bonuses() {
        let text = format!(
            "# Lab Results\n\n| Test | Value |\n|------|-------|\n| WBC | 7.2 |\n\n- Normal range\n{}",
            "x".repeat(500)
        );
        let c = compute_heuristic_confidence(&text);
        // 0.8 (length) + 0.05 (headers) + 0.05 (tables) + 0.03 (lists) = 0.93
        assert!((c - 0.93).abs() < 0.01, "Structured text: {c}");
    }

    #[test]
    fn confidence_capped_at_0_95() {
        // Even with all bonuses on long text, can't exceed 0.95
        let text = format!(
            "# H1\n## H2\n| a | b |\n|---|---|\n| 1 | 2 |\n- item\n* item\n{}",
            "x".repeat(1000)
        );
        let c = compute_heuristic_confidence(&text);
        assert!(c <= 0.95, "Should be capped: {c}");
    }

    // ── OllamaVisionOcr ──

    #[test]
    fn extract_with_document_tag() {
        let response = "# Blood Test Results\n\n| Test | Value |\n|------|-------|\n| WBC | 7.2 |\n\n[DOCUMENT]";
        let mock = Arc::new(MockVisionClient::new(response));
        let ocr = OllamaVisionOcr::new(mock, "medgemma:4b".to_string());

        let result = ocr.extract_text_from_image(b"fake-png-data").unwrap();
        assert_eq!(result.content_type, ImageContentType::Document);
        assert!(result.text.contains("Blood Test Results"));
        assert!(!result.text.contains("[DOCUMENT]"));
        assert_eq!(result.model_used, "medgemma:4b");
    }

    #[test]
    fn extract_with_medical_image_tag() {
        let response = "Chest X-ray shows bilateral infiltrates\n[MEDICAL_IMAGE]";
        let mock = Arc::new(MockVisionClient::new(response));
        let ocr = OllamaVisionOcr::new(mock, "medgemma:4b".to_string());

        let result = ocr.extract_text_from_image(b"fake-xray-data").unwrap();
        assert_eq!(result.content_type, ImageContentType::MedicalImage);
        assert!(result.text.contains("bilateral infiltrates"));
    }

    #[test]
    fn extract_empty_response_zero_confidence() {
        let mock = Arc::new(MockVisionClient::new(""));
        let ocr = OllamaVisionOcr::new(mock, "medgemma:4b".to_string());

        let result = ocr.extract_text_from_image(b"blank-page").unwrap();
        assert_eq!(result.confidence, 0.0);
        assert!(result.text.is_empty());
    }

    // ── MockVisionOcr ──

    #[test]
    fn mock_returns_configured_response() {
        let mock = MockVisionOcr::new("# Extracted text", "test-model");
        let result = mock.extract_text_from_image(b"any-bytes").unwrap();
        assert_eq!(result.text, "# Extracted text");
        assert_eq!(result.model_used, "test-model");
    }

    #[test]
    fn mock_with_content_type() {
        let mock = MockVisionOcr::new("X-ray findings", "medgemma:4b")
            .with_content_type(ImageContentType::MedicalImage);
        let result = mock.extract_text_from_image(b"xray").unwrap();
        assert_eq!(result.content_type, ImageContentType::MedicalImage);
    }

    // ── Prompt builders ──

    #[test]
    fn prompt_en_is_default() {
        assert_eq!(build_system_prompt("en"), SYSTEM_PROMPT_EN);
        assert_eq!(build_user_prompt("en"), USER_PROMPT_EN);
    }

    #[test]
    fn prompt_fr_selected() {
        assert_eq!(build_system_prompt("fr"), SYSTEM_PROMPT_FR);
        assert_eq!(build_user_prompt("fr"), USER_PROMPT_FR);
        assert!(SYSTEM_PROMPT_FR.contains("French"));
        assert!(USER_PROMPT_FR.contains("French"));
    }

    #[test]
    fn prompt_de_selected() {
        assert_eq!(build_system_prompt("de"), SYSTEM_PROMPT_DE);
        assert_eq!(build_user_prompt("de"), USER_PROMPT_DE);
        assert!(SYSTEM_PROMPT_DE.contains("German"));
        assert!(USER_PROMPT_DE.contains("German"));
    }

    #[test]
    fn prompt_unknown_falls_back_to_en() {
        assert_eq!(build_system_prompt("ja"), SYSTEM_PROMPT_EN);
        assert_eq!(build_user_prompt("ja"), USER_PROMPT_EN);
        assert_eq!(build_system_prompt(""), SYSTEM_PROMPT_EN);
    }

    #[test]
    fn all_user_prompts_contain_classification_tags() {
        for lang in &["en", "fr", "de"] {
            let prompt = build_user_prompt(lang);
            assert!(prompt.contains("[DOCUMENT]"), "Missing [DOCUMENT] in {lang}");
            assert!(
                prompt.contains("[MEDICAL_IMAGE]"),
                "Missing [MEDICAL_IMAGE] in {lang}"
            );
        }
    }

    #[test]
    fn fr_system_prompt_mentions_accented_chars() {
        assert!(SYSTEM_PROMPT_FR.contains("é"));
        assert!(SYSTEM_PROMPT_FR.contains("ç"));
    }

    #[test]
    fn de_system_prompt_mentions_umlauts() {
        assert!(SYSTEM_PROMPT_DE.contains("ä"));
        assert!(SYSTEM_PROMPT_DE.contains("ß"));
    }

    #[test]
    fn with_language_builder_works() {
        let mock = Arc::new(MockVisionClient::new("Text\n[DOCUMENT]"));
        let ocr = OllamaVisionOcr::new(mock, "medgemma:4b".to_string()).with_language("fr");
        assert_eq!(ocr.language, "fr");
        // Extraction should succeed (language just affects prompt selection)
        let result = ocr.extract_text_from_image(b"fake-png").unwrap();
        assert!(result.text.contains("Text"));
    }

    #[test]
    fn default_language_is_en() {
        let mock = Arc::new(MockVisionClient::new("ok\n[DOCUMENT]"));
        let ocr = OllamaVisionOcr::new(mock, "test".to_string());
        assert_eq!(ocr.language, "en");
    }

    // ── Error propagation ──

    #[test]
    fn vision_client_error_maps_to_extraction_error() {
        // Use a client that always returns an error by connecting to unreachable port
        struct FailingVisionClient;
        impl VisionClient for FailingVisionClient {
            fn generate_with_images(
                &self,
                _model: &str,
                _prompt: &str,
                _images: &[String],
                _system: Option<&str>,
            ) -> Result<String, OllamaError> {
                Err(OllamaError::NotReachable)
            }
            fn chat_with_images(
                &self,
                _model: &str,
                _user_prompt: &str,
                _images: &[String],
                _system: Option<&str>,
            ) -> Result<String, OllamaError> {
                Err(OllamaError::NotReachable)
            }
        }

        let ocr = OllamaVisionOcr::new(
            Arc::new(FailingVisionClient),
            "medgemma:4b".to_string(),
        );
        let result = ocr.extract_text_from_image(b"data");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Vision OCR failed"), "Error: {err}");
    }

    #[test]
    fn medgemma_uses_chat_endpoint() {
        // MedGemma should call chat_with_images, not generate_with_images.
        // We verify by using a client that fails on generate but succeeds on chat.
        struct ChatOnlyVisionClient;
        impl VisionClient for ChatOnlyVisionClient {
            fn generate_with_images(
                &self,
                _model: &str,
                _prompt: &str,
                _images: &[String],
                _system: Option<&str>,
            ) -> Result<String, OllamaError> {
                Err(OllamaError::ApiError {
                    status: 500,
                    message: "generate not supported for this model".into(),
                })
            }
            fn chat_with_images(
                &self,
                _model: &str,
                _user_prompt: &str,
                _images: &[String],
                _system: Option<&str>,
            ) -> Result<String, OllamaError> {
                Ok("# Prescription\nMetformin 500mg\n[DOCUMENT]".into())
            }
        }

        let ocr = OllamaVisionOcr::new(
            Arc::new(ChatOnlyVisionClient),
            "MedAIBase/MedGemma1.5:4b".to_string(),
        );
        let result = ocr.extract_text_from_image(b"fake-pdf-page").unwrap();
        assert_eq!(result.content_type, ImageContentType::Document);
        assert!(result.text.contains("Metformin 500mg"));
    }

    // ── OllamaMedicalImageInterpreter ──

    #[test]
    fn interpreter_returns_findings() {
        let response = "## Chest X-ray\n\n- Bilateral infiltrates\n- No pneumothorax";
        let mock = Arc::new(MockVisionClient::new(response));
        let interp = OllamaMedicalImageInterpreter::new(mock, "medgemma:4b".to_string());

        let result = interp.interpret_medical_image(b"fake-xray").unwrap();
        assert!(result.findings.contains("Bilateral infiltrates"));
        assert_eq!(result.model_used, "medgemma:4b");
    }

    #[test]
    fn interpreter_error_propagates() {
        struct FailClient;
        impl VisionClient for FailClient {
            fn generate_with_images(
                &self,
                _model: &str,
                _prompt: &str,
                _images: &[String],
                _system: Option<&str>,
            ) -> Result<String, OllamaError> {
                Err(OllamaError::NotReachable)
            }
            fn chat_with_images(
                &self,
                _model: &str,
                _user_prompt: &str,
                _images: &[String],
                _system: Option<&str>,
            ) -> Result<String, OllamaError> {
                Err(OllamaError::NotReachable)
            }
        }

        let interp =
            OllamaMedicalImageInterpreter::new(Arc::new(FailClient), "medgemma:4b".to_string());
        let err = interp.interpret_medical_image(b"data").unwrap_err();
        assert!(
            err.to_string().contains("Medical image interpretation failed"),
            "Error: {err}"
        );
    }

    #[test]
    fn interpret_prompt_mentions_modality() {
        assert!(INTERPRET_USER_PROMPT.contains("modality"));
        assert!(INTERPRET_SYSTEM_PROMPT.contains("medical image"));
    }

    // ── MockMedicalImageInterpreter ──

    #[test]
    fn mock_interpreter_returns_configured() {
        let mock = MockMedicalImageInterpreter::new("Findings here", "test-model", 0.75);
        let result = mock.interpret_medical_image(b"image").unwrap();
        assert_eq!(result.findings, "Findings here");
        assert_eq!(result.model_used, "test-model");
        assert!((result.confidence - 0.75).abs() < f32::EPSILON);
    }
}
