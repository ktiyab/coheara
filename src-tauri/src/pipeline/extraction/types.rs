use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ExtractionError;
use crate::crypto::ProfileSession;
use crate::pipeline::import::FormatDetection;

/// Result of text extraction from a single document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub document_id: Uuid,
    pub method: ExtractionMethod,
    pub pages: Vec<PageExtraction>,
    pub full_text: String,
    pub overall_confidence: f32,
    pub language_detected: Option<String>,
    pub page_count: usize,
}

/// How text was extracted
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExtractionMethod {
    PdfDirect,
    TesseractOcr,
    PlainTextRead,
}

/// Per-page extraction result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageExtraction {
    pub page_number: usize,
    pub text: String,
    pub confidence: f32,
    pub regions: Vec<RegionConfidence>,
    pub warnings: Vec<ExtractionWarning>,
}

/// Confidence for a specific region of a page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionConfidence {
    pub text: String,
    pub confidence: f32,
    pub bounding_box: Option<BoundingBox>,
}

/// Bounding box for a text region (for highlighting in review screen)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Warnings about extraction quality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExtractionWarning {
    LowConfidencePage { page: usize, confidence: f32 },
    BlurryImage,
    SkewedDocument { angle_degrees: f32 },
    PoorContrast,
    HandwritingDetected,
    PartialExtraction { reason: String },
}

/// Raw OCR result from the engine
#[derive(Debug)]
pub struct OcrPageResult {
    pub text: String,
    pub confidence: f32,
    pub word_confidences: Vec<(String, f32)>,
}

/// Image quality assessment result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageQuality {
    pub resolution: String,
    pub contrast: String,
    pub estimated_confidence: f32,
}

/// OCR engine abstraction (allows mocking for tests)
pub trait OcrEngine {
    fn ocr_image(&self, image_bytes: &[u8]) -> Result<OcrPageResult, ExtractionError>;

    fn ocr_image_with_lang(
        &self,
        image_bytes: &[u8],
        lang: &str,
    ) -> Result<OcrPageResult, ExtractionError>;
}

/// PDF text extraction abstraction
pub trait PdfExtractor {
    fn extract_text(&self, pdf_bytes: &[u8]) -> Result<Vec<PageExtraction>, ExtractionError>;

    fn page_count(&self, pdf_bytes: &[u8]) -> Result<usize, ExtractionError>;
}

/// Main extraction orchestrator trait
pub trait TextExtractor {
    fn extract(
        &self,
        document_id: &Uuid,
        staged_path: &std::path::Path,
        format: &FormatDetection,
        session: &ProfileSession,
    ) -> Result<ExtractionResult, ExtractionError>;
}
