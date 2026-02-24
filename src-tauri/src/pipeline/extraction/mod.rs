pub mod types;
pub mod confidence;
pub mod sanitize;
pub mod pdfium;
pub mod preprocess;
pub mod orchestrator;
pub mod vision_ocr;

pub use types::*;
pub use confidence::*;
pub use sanitize::*;
pub use orchestrator::*;

use thiserror::Error;
use uuid::Uuid;

use crate::crypto::CryptoError;
use crate::db::DatabaseError;
use crate::pipeline::import::ImportError;

#[derive(Error, Debug)]
pub enum ExtractionError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("OCR processing failed: {0}")]
    OcrProcessing(String),

    #[error("Image processing error: {0}")]
    ImageProcessing(String),

    #[error("Text encoding error: {0}")]
    EncodingError(String),

    #[error("Staged file not found for document: {0}")]
    StagedFileNotFound(Uuid),

    #[error("Unsupported format for extraction")]
    UnsupportedFormat,

    // ── R3: Vision-based extraction errors ──

    /// pdfium-render failed to render a PDF page to image.
    #[error("PDF rendering failed for page {page}: {reason}")]
    PdfRendering { page: usize, reason: String },

    /// PDF is encrypted and cannot be rendered without a password.
    #[error("PDF is encrypted — please provide an unencrypted document")]
    PdfEncrypted,

    /// No vision-capable model is installed for OCR extraction.
    #[error("No vision model available for document extraction — install MedGemma")]
    NoVisionModel,

    /// Vision OCR extraction failed for a specific reason.
    #[error("Vision OCR failed: {0}")]
    VisionOcrFailed(String),

    /// Vision OCR timed out (model inference took too long).
    #[error("Vision OCR timed out after {0} seconds")]
    VisionOcrTimeout(u64),

    /// Document page rendered but contained no extractable content.
    #[error("Empty page — no text content detected on page {0}")]
    EmptyPage(usize),

    /// Document produced no extractable content across all pages.
    #[error("Document appears empty — no text could be extracted")]
    EmptyDocument,

    // ── Cross-cutting errors ──

    #[error("Encryption error: {0}")]
    Crypto(#[from] CryptoError),

    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("Import error: {0}")]
    Import(#[from] ImportError),
}
