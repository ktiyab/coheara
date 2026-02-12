pub mod types;
pub mod classify;
pub mod retrieval;
pub mod context;
pub mod prompt;
pub mod citation;
pub mod conversation;
pub mod orchestrator;

use thiserror::Error;
use uuid::Uuid;

use crate::crypto::CryptoError;
use crate::db::DatabaseError;

#[derive(Error, Debug)]
pub enum RagError {
    #[error("Ollama connection failed: {0}")]
    OllamaConnection(String),

    #[error("No model available")]
    NoModel,

    #[error("Streaming error: {0}")]
    StreamingError(String),

    #[error("Response parsing error: {0}")]
    ResponseParsing(String),

    #[error("Embedding generation failed: {0}")]
    EmbeddingFailed(String),

    #[error("Vector search failed: {0}")]
    VectorSearch(String),

    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("No relevant context found")]
    NoContext,

    #[error("Conversation not found: {0}")]
    ConversationNotFound(Uuid),

    #[error("Encryption error: {0}")]
    Crypto(#[from] CryptoError),
}
