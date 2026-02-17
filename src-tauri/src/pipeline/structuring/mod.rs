pub mod types;
pub mod prompt;
pub mod parser;
pub mod classify;
pub mod sanitize;
pub mod confidence;
pub mod validation;
pub mod ollama;
pub mod ollama_types;
pub mod orchestrator;
pub mod preferences;

pub use types::*;
pub use prompt::*;
pub use parser::*;
pub use classify::*;
pub use sanitize::*;
pub use confidence::*;
pub use validation::*;
pub use ollama::*;
pub use ollama_types::*;
pub use orchestrator::*;
pub use preferences::*;

#[cfg(test)]
mod security_tests;

use thiserror::Error;

use serde::{Deserialize, Serialize};

use crate::crypto::CryptoError;

#[derive(Error, Debug)]
pub enum StructuringError {
    #[error("Ollama is not running at {0}")]
    OllamaConnection(String),

    #[error("Ollama returned error (status {status}): {body}")]
    OllamaError { status: u16, body: String },

    #[error("No compatible MedGemma model available")]
    NoModelAvailable,

    #[error("HTTP client error: {0}")]
    HttpClient(String),

    #[error("Malformed MedGemma response: {0}")]
    MalformedResponse(String),

    #[error("JSON parsing error: {0}")]
    JsonParsing(String),

    #[error("Response parsing error: {0}")]
    ResponseParsing(String),

    #[error("Input text too short for structuring (< 10 characters)")]
    InputTooShort,

    #[error("Encryption error: {0}")]
    Crypto(#[from] CryptoError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Patient-friendly error message (K.5).
/// Maps technical errors to actionable, non-technical guidance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientMessage {
    pub title: String,
    pub message: String,
    pub suggestion: String,
    pub retry_possible: bool,
}

impl StructuringError {
    /// Convert this error to a patient-friendly message (K.5 — STR-05-G03).
    pub fn patient_message(&self) -> PatientMessage {
        match self {
            StructuringError::OllamaConnection(_) => PatientMessage {
                title: "AI Service Unavailable".into(),
                message: "The AI analysis service is not running on your computer.".into(),
                suggestion: "Please start Ollama and try again. If the problem persists, check the AI Setup in Settings.".into(),
                retry_possible: true,
            },
            StructuringError::OllamaError { status, .. } => PatientMessage {
                title: "AI Service Error".into(),
                message: format!("The AI service encountered a problem (code {status})."),
                suggestion: "Please try again. If the error repeats, restart Ollama or check Settings.".into(),
                retry_possible: true,
            },
            StructuringError::NoModelAvailable => PatientMessage {
                title: "No AI Model Installed".into(),
                message: "No compatible medical AI model was found on your computer.".into(),
                suggestion: "Open Settings > AI Setup to download and configure a medical model.".into(),
                retry_possible: false,
            },
            StructuringError::HttpClient(_) => PatientMessage {
                title: "Connection Problem".into(),
                message: "Could not communicate with the AI service.".into(),
                suggestion: "Please check that Ollama is running and try again.".into(),
                retry_possible: true,
            },
            StructuringError::MalformedResponse(_) | StructuringError::JsonParsing(_) | StructuringError::ResponseParsing(_) => PatientMessage {
                title: "Analysis Incomplete".into(),
                message: "The AI returned a response that could not be fully understood.".into(),
                suggestion: "Please try again. The AI may produce a clearer response on retry.".into(),
                retry_possible: true,
            },
            StructuringError::InputTooShort => PatientMessage {
                title: "Document Too Short".into(),
                message: "The extracted text is too short to analyze meaningfully.".into(),
                suggestion: "Please check that the document was scanned clearly and contains readable text.".into(),
                retry_possible: false,
            },
            StructuringError::Crypto(_) => PatientMessage {
                title: "Security Error".into(),
                message: "An encryption error occurred while processing your document.".into(),
                suggestion: "Please try again. If the problem persists, your profile may need to be re-authenticated.".into(),
                retry_possible: true,
            },
            StructuringError::Io(_) => PatientMessage {
                title: "File Access Error".into(),
                message: "Could not read or write a file needed for analysis.".into(),
                suggestion: "Please check available disk space and try again.".into(),
                retry_possible: true,
            },
        }
    }
}

#[cfg(test)]
mod patient_message_tests {
    use super::*;

    #[test]
    fn ollama_connection_is_retryable() {
        let err = StructuringError::OllamaConnection("localhost:11434".into());
        let msg = err.patient_message();
        assert!(msg.retry_possible);
        assert!(msg.title.contains("Unavailable"));
        assert!(msg.suggestion.contains("Ollama"));
    }

    #[test]
    fn no_model_is_not_retryable() {
        let err = StructuringError::NoModelAvailable;
        let msg = err.patient_message();
        assert!(!msg.retry_possible);
        assert!(msg.suggestion.contains("Settings"));
    }

    #[test]
    fn malformed_response_is_retryable() {
        let err = StructuringError::MalformedResponse("bad json".into());
        let msg = err.patient_message();
        assert!(msg.retry_possible);
        assert!(msg.title.contains("Incomplete"));
    }

    #[test]
    fn json_parsing_maps_to_analysis_incomplete() {
        let err = StructuringError::JsonParsing("unexpected token".into());
        let msg = err.patient_message();
        assert_eq!(msg.title, "Analysis Incomplete");
    }

    #[test]
    fn response_parsing_maps_to_analysis_incomplete() {
        let err = StructuringError::ResponseParsing("missing field".into());
        let msg = err.patient_message();
        assert_eq!(msg.title, "Analysis Incomplete");
    }

    #[test]
    fn input_too_short_not_retryable() {
        let err = StructuringError::InputTooShort;
        let msg = err.patient_message();
        assert!(!msg.retry_possible);
        assert!(msg.title.contains("Short"));
    }

    #[test]
    fn ollama_error_includes_status_code() {
        let err = StructuringError::OllamaError { status: 500, body: "internal".into() };
        let msg = err.patient_message();
        assert!(msg.message.contains("500"));
        assert!(msg.retry_possible);
    }

    #[test]
    fn http_client_is_retryable() {
        let err = StructuringError::HttpClient("timeout".into());
        let msg = err.patient_message();
        assert!(msg.retry_possible);
        assert!(msg.title.contains("Connection"));
    }

    #[test]
    fn io_error_is_retryable() {
        let err = StructuringError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"));
        let msg = err.patient_message();
        assert!(msg.retry_possible);
        assert!(msg.title.contains("File"));
    }

    #[test]
    fn all_variants_have_non_empty_fields() {
        let errors: Vec<StructuringError> = vec![
            StructuringError::OllamaConnection("localhost".into()),
            StructuringError::OllamaError { status: 500, body: "err".into() },
            StructuringError::NoModelAvailable,
            StructuringError::HttpClient("err".into()),
            StructuringError::MalformedResponse("err".into()),
            StructuringError::JsonParsing("err".into()),
            StructuringError::ResponseParsing("err".into()),
            StructuringError::InputTooShort,
            StructuringError::Io(std::io::Error::new(std::io::ErrorKind::Other, "err")),
        ];
        for err in &errors {
            let msg = err.patient_message();
            assert!(!msg.title.is_empty(), "Empty title for {:?}", err);
            assert!(!msg.message.is_empty(), "Empty message for {:?}", err);
            assert!(!msg.suggestion.is_empty(), "Empty suggestion for {:?}", err);
        }
    }
}
