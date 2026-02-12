pub mod format;
pub mod hash;
pub mod staging;
pub mod importer;

pub use format::*;
pub use hash::*;
pub use staging::*;
pub use importer::*;

use thiserror::Error;
use uuid::Uuid;

use crate::crypto::CryptoError;
use crate::db::DatabaseError;

#[derive(Error, Debug)]
pub enum ImportError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Unsupported file format")]
    UnsupportedFormat,

    #[error("File too large (max 100MB)")]
    FileTooLarge,

    #[error("Could not read file: {0}")]
    FileReadError(String),

    #[error("Image processing error: {0}")]
    ImageProcessing(String),

    #[error("Hash computation failed")]
    HashComputation,

    #[error("Document not found: {0}")]
    DocumentNotFound(Uuid),

    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("Encryption error: {0}")]
    Crypto(#[from] CryptoError),

    #[error("No active profile session")]
    NoActiveSession,
}
