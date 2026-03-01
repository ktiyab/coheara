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
// User-provided classification (UC-01)
// ──────────────────────────────────────────────

/// Document type selected by the user at import time.
///
/// UC-01: The user always knows what they're importing.
/// One click, 2 seconds, 100% accurate — replaces LLM classifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserDocumentType {
    LabReport,
    Prescription,
    MedicalImage,
}

impl UserDocumentType {
    /// Map to the pipeline's content type for routing.
    pub fn to_content_type(self) -> ImageContentType {
        match self {
            Self::LabReport => ImageContentType::Document,
            Self::Prescription => ImageContentType::Document,
            Self::MedicalImage => ImageContentType::MedicalImage,
        }
    }

    /// Parse from the string sent over IPC (e.g. `"lab_report"`).
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "lab_report" => Some(Self::LabReport),
            "prescription" => Some(Self::Prescription),
            "medical_image" => Some(Self::MedicalImage),
            _ => None,
        }
    }
}

/// Classifier backed by user selection — no LLM call, instant, infallible.
pub struct UserProvidedClassifier {
    content_type: ImageContentType,
}

impl UserProvidedClassifier {
    pub fn new(doc_type: UserDocumentType) -> Self {
        Self {
            content_type: doc_type.to_content_type(),
        }
    }
}

impl VisionClassifier for UserProvidedClassifier {
    fn classify_image(
        &self,
        _image_bytes: &[u8],
    ) -> Result<ImageContentType, ExtractionError> {
        Ok(self.content_type)
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

    // -- UserDocumentType (UC-01) --

    #[test]
    fn user_doc_type_lab_report_maps_to_document() {
        assert_eq!(
            UserDocumentType::LabReport.to_content_type(),
            ImageContentType::Document,
        );
    }

    #[test]
    fn user_doc_type_prescription_maps_to_document() {
        assert_eq!(
            UserDocumentType::Prescription.to_content_type(),
            ImageContentType::Document,
        );
    }

    #[test]
    fn user_doc_type_medical_image_maps_to_medical_image() {
        assert_eq!(
            UserDocumentType::MedicalImage.to_content_type(),
            ImageContentType::MedicalImage,
        );
    }

    #[test]
    fn user_doc_type_from_str_parses_valid() {
        assert_eq!(UserDocumentType::from_str("lab_report"), Some(UserDocumentType::LabReport));
        assert_eq!(UserDocumentType::from_str("prescription"), Some(UserDocumentType::Prescription));
        assert_eq!(UserDocumentType::from_str("medical_image"), Some(UserDocumentType::MedicalImage));
        assert_eq!(UserDocumentType::from_str("unknown"), None);
        assert_eq!(UserDocumentType::from_str(""), None);
    }

    #[test]
    fn user_provided_classifier_returns_fixed_type() {
        let classifier = UserProvidedClassifier::new(UserDocumentType::LabReport);
        assert_eq!(classifier.classify_image(b"any").unwrap(), ImageContentType::Document);

        let classifier = UserProvidedClassifier::new(UserDocumentType::MedicalImage);
        assert_eq!(classifier.classify_image(b"any").unwrap(), ImageContentType::MedicalImage);
    }
}
