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

    #[error("Unsupported file format: {0}")]
    UnsupportedFormat(String),

    #[error("File too large: {size_mb:.1}MB exceeds {max_mb}MB limit")]
    FileTooLarge { size_mb: f64, max_mb: u64 },

    #[error("PDF is password-protected — please decrypt it first")]
    EncryptedPdf,

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
