pub mod types;
pub mod prompt;
pub mod parser;
pub mod classify;
pub mod sanitize;
pub mod confidence;
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
pub use ollama::*;
pub use ollama_types::*;
pub use orchestrator::*;
pub use preferences::*;

use thiserror::Error;

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
