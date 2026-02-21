//! Extraction-specific error types for the night batch pipeline.
//!
//! Separate from StructuringError to avoid coupling batch extraction
//! to the document structuring pipeline.

use thiserror::Error;

use crate::db::DatabaseError;
use crate::pipeline::structuring::StructuringError;

#[derive(Error, Debug)]
pub enum ExtractionError {
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("LLM error: {0}")]
    Llm(#[from] StructuringError),

    #[error("JSON parsing error: {0}")]
    JsonParsing(String),

    #[error("No eligible conversations for extraction")]
    NoEligibleConversations,

    #[error("Domain extractor not found: {0}")]
    ExtractorNotFound(String),

    #[error("Extraction cancelled")]
    Cancelled,

    #[error("Validation failed: {0}")]
    Validation(String),

    #[error("Configuration error: {0}")]
    Config(String),
}
