pub mod types;
pub mod confidence;
pub mod sanitize;
pub mod preprocess;
pub mod pdf;
pub mod ocr;
pub mod language_detect;
pub mod medical_correction;
pub mod column_detect;
pub mod table_detect;
pub mod pdf_renderer;
pub mod orchestrator;

pub use types::*;
pub use confidence::*;
pub use sanitize::*;
pub use preprocess::*;
pub use pdf::*;
pub use ocr::*;
pub use orchestrator::*;

use std::path::PathBuf;

use thiserror::Error;
use uuid::Uuid;

use crate::crypto::CryptoError;
use crate::db::DatabaseError;
use crate::pipeline::import::ImportError;

#[derive(Error, Debug)]
pub enum ExtractionError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Tesseract OCR initialization failed: {0}")]
    OcrInit(String),

    #[error("Tesseract OCR configuration error: {0}")]
    OcrConfig(String),

    #[error("OCR processing failed: {0}")]
    OcrProcessing(String),

    #[error("PDF parsing failed: {0}")]
    PdfParsing(String),

    #[error("Image processing error: {0}")]
    ImageProcessing(String),

    #[error("Text encoding error: {0}")]
    EncodingError(String),

    #[error("Tessdata not found at: {0}")]
    TessdataNotFound(PathBuf),

    #[error("Staged file not found for document: {0}")]
    StagedFileNotFound(Uuid),

    #[error("Unsupported format for extraction")]
    UnsupportedFormat,

    #[error("Encryption error: {0}")]
    Crypto(#[from] CryptoError),

    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("Import error: {0}")]
    Import(#[from] ImportError),
}
