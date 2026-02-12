pub mod types;
pub mod chunker;
pub mod embedder;
pub mod vectordb;
pub mod entity_store;
pub mod markdown_store;
pub mod orchestrator;

use std::path::PathBuf;

use thiserror::Error;
use uuid::Uuid;

use crate::crypto::CryptoError;
use crate::db::DatabaseError;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("Encryption error: {0}")]
    Crypto(#[from] CryptoError),

    #[error("Vector DB error: {0}")]
    VectorDb(String),

    #[error("Embedding model not found: {0}")]
    ModelNotFound(PathBuf),

    #[error("Embedding model initialization: {0}")]
    ModelInit(String),

    #[error("Tokenization error: {0}")]
    Tokenization(String),

    #[error("Embedding generation failed: {0}")]
    Embedding(String),

    #[error("Document not found: {0}")]
    DocumentNotFound(Uuid),

    #[error("Chunking produced no results")]
    EmptyChunks,
}
