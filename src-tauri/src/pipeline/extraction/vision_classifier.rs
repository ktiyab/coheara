//! C4-FIX: Lightweight vision image classifier.
//!
//! Determines whether a document image is a text document (lab report,
//! prescription, form) or a medical image (X-ray, CT, MRI, dermatology).
//!
//! Single focused call, ~5 token response. No degeneration risk because
//! the expected output is one word. Conservative default: Document.

use std::sync::Arc;

use base64::Engine as _;

use super::types::ImageContentType;
use super::ExtractionError;
use crate::pipeline::structuring::types::VisionClient;

// ──────────────────────────────────────────────
// Classification prompts
// ──────────────────────────────────────────────

const CLASSIFY_SYSTEM: &str = "You classify medical document images into exactly one category.";

const CLASSIFY_USER: &str = "\
Look at this image. Is it a text document (lab report, prescription, \
medical form, discharge summary, insurance document, letter) or a \
medical image (X-ray, CT scan, MRI, radiograph, dermatology photo, \
pathology slide, ultrasound)?\n\n\
Answer with exactly one word: DOCUMENT or IMAGE";

// ──────────────────────────────────────────────
// Trait
// ──────────────────────────────────────────────

/// Lightweight vision classifier — routes images before extraction.
///
/// Single focused call, ~5 token response. Used by the orchestrator to
/// decide between IterativeDrill (documents) and MedicalImageInterpreter
/// (medical imagery).
pub trait VisionClassifier: Send + Sync {
    fn classify_image(
        &self,
        image_bytes: &[u8],
    ) -> Result<ImageContentType, ExtractionError>;
}

// ──────────────────────────────────────────────
// Production implementation
// ──────────────────────────────────────────────

/// Production classifier backed by Ollama vision model.
pub struct OllamaVisionClassifier {
    vision_client: Arc<dyn VisionClient>,
    model_name: String,
}

impl OllamaVisionClassifier {
    pub fn new(vision_client: Arc<dyn VisionClient>, model_name: String) -> Self {
        Self {
            vision_client,
            model_name,
        }
    }
}

impl VisionClassifier for OllamaVisionClassifier {
    fn classify_image(
        &self,
        image_bytes: &[u8],
    ) -> Result<ImageContentType, ExtractionError> {
        let base64_image =
            base64::engine::general_purpose::STANDARD.encode(image_bytes);
        let images = vec![base64_image];

        let response = self
            .vision_client
            .chat_with_images(
                &self.model_name,
                CLASSIFY_USER,
                &images,
                Some(CLASSIFY_SYSTEM),
            )
            .map_err(|e| {
                ExtractionError::VisionOcrFailed(format!(
                    "Image classification failed: {e}"
                ))
            })?;

        Ok(parse_classification_response(&response))
    }
}

/// Parse the classifier response into a content type.
///
/// Looks for "IMAGE" (case-insensitive) in the response.
/// Conservative default: Document (most common case).
fn parse_classification_response(response: &str) -> ImageContentType {
    let lower = response.trim().to_lowercase();
    if lower.contains("image") {
        ImageContentType::MedicalImage
    } else {
        ImageContentType::Document
    }
}

// ──────────────────────────────────────────────
// Mock for testing
// ──────────────────────────────────────────────

/// Mock classifier that returns a fixed content type.
pub struct MockVisionClassifier {
    content_type: ImageContentType,
}

impl MockVisionClassifier {
    pub fn document() -> Self {
        Self {
            content_type: ImageContentType::Document,
        }
    }

    pub fn medical_image() -> Self {
        Self {
            content_type: ImageContentType::MedicalImage,
        }
    }
}

impl VisionClassifier for MockVisionClassifier {
    fn classify_image(
        &self,
        _image_bytes: &[u8],
    ) -> Result<ImageContentType, ExtractionError> {
        Ok(self.content_type)
    }
}

// ──────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::structuring::ollama::MockVisionClient;

    #[test]
    fn parse_document_response() {
        assert_eq!(
            parse_classification_response("DOCUMENT"),
            ImageContentType::Document,
        );
    }

    #[test]
    fn parse_image_response() {
        assert_eq!(
            parse_classification_response("IMAGE"),
            ImageContentType::MedicalImage,
        );
    }

    #[test]
    fn parse_image_case_insensitive() {
        assert_eq!(
            parse_classification_response("image"),
            ImageContentType::MedicalImage,
        );
        assert_eq!(
            parse_classification_response("Image"),
            ImageContentType::MedicalImage,
        );
    }

    #[test]
    fn parse_verbose_image_response() {
        assert_eq!(
            parse_classification_response("This is a medical IMAGE."),
            ImageContentType::MedicalImage,
        );
    }

    #[test]
    fn parse_unknown_defaults_to_document() {
        assert_eq!(
            parse_classification_response("I don't know"),
            ImageContentType::Document,
        );
        assert_eq!(
            parse_classification_response(""),
            ImageContentType::Document,
        );
    }

    #[test]
    fn parse_document_verbose() {
        assert_eq!(
            parse_classification_response("This looks like a DOCUMENT"),
            ImageContentType::Document,
        );
    }

    #[test]
    fn ollama_classifier_calls_chat_with_images() {
        let mock = Arc::new(MockVisionClient::new("DOCUMENT"));
        let classifier =
            OllamaVisionClassifier::new(mock, "medgemma:4b".to_string());

        let result = classifier.classify_image(b"fake-png-data").unwrap();
        assert_eq!(result, ImageContentType::Document);
    }

    #[test]
    fn ollama_classifier_detects_medical_image() {
        let mock = Arc::new(MockVisionClient::new("IMAGE"));
        let classifier =
            OllamaVisionClassifier::new(mock, "medgemma:4b".to_string());

        let result = classifier.classify_image(b"fake-xray-data").unwrap();
        assert_eq!(result, ImageContentType::MedicalImage);
    }

    #[test]
    fn mock_classifier_document() {
        let classifier = MockVisionClassifier::document();
        let result = classifier.classify_image(b"any").unwrap();
        assert_eq!(result, ImageContentType::Document);
    }

    #[test]
    fn mock_classifier_medical_image() {
        let classifier = MockVisionClassifier::medical_image();
        let result = classifier.classify_image(b"any").unwrap();
        assert_eq!(result, ImageContentType::MedicalImage);
    }

    #[test]
    fn classify_prompt_asks_one_word() {
        assert!(CLASSIFY_USER.contains("exactly one word"));
        assert!(CLASSIFY_USER.contains("DOCUMENT or IMAGE"));
    }
}
