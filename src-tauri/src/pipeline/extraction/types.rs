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
    /// R3: Vision model read page image → structured Markdown.
    VisionOcr,
    /// Plain text file — direct read, no model needed.
    PlainTextRead,
    /// Legacy: Digital PDF text operators (kept for deserialization compat).
    PdfDirect,
    /// Legacy: Tesseract OCR from image (kept for deserialization compat).
    TesseractOcr,
}

/// Per-page extraction result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageExtraction {
    pub page_number: usize,
    pub text: String,
    pub confidence: f32,
    pub regions: Vec<RegionConfidence>,
    pub warnings: Vec<ExtractionWarning>,
    /// Content classification from vision model.
    /// `None` for plain text (no vision model involved).
    /// `Some(Document)` for text documents, `Some(MedicalImage)` for medical imagery.
    #[serde(default)]
    pub content_type: Option<ImageContentType>,
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
    /// Table appears to continue on the next page
    TableContinuation,
}

/// PDF page-to-image renderer abstraction.
/// Renders individual PDF pages to image bytes for vision model OCR.
///
/// Production: `PdfiumRenderer` (pdfium-render via PDFium).
/// Testing: `MockPdfPageRenderer` (returns minimal PNG).
pub trait PdfPageRenderer: Send + Sync {
    /// Render a single PDF page to image bytes (PNG format).
    /// `page_number` is 0-indexed.
    /// `dpi` controls resolution (200 for vision model, was 300 for Tesseract).
    fn render_page(
        &self,
        pdf_bytes: &[u8],
        page_number: usize,
        dpi: u32,
    ) -> Result<Vec<u8>, ExtractionError>;

    /// Count pages in a PDF document.
    fn page_count(&self, pdf_bytes: &[u8]) -> Result<usize, ExtractionError>;
}

// ──────────────────────────────────────────────
// R3: Vision OCR types
// ──────────────────────────────────────────────

/// Classification signal from the vision model's extraction pass.
///
/// MedGemma is prompted to append a classification tag.
/// The orchestrator uses this to route medical images to MedGemma interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageContentType {
    /// Text document: lab report, prescription, form, letter, insurance doc.
    Document,
    /// Medical image: X-ray, radiograph, CT scan, dermatology photo, pathology slide.
    MedicalImage,
}

/// R3: Result from vision model OCR extraction.
#[derive(Debug, Clone)]
pub struct VisionOcrResult {
    /// Extracted text as structured Markdown.
    pub text: String,
    /// Which model performed the extraction (e.g., "medgemma:4b").
    pub model_used: String,
    /// Heuristic confidence based on text length and structure quality.
    pub confidence: f32,
    /// Image classification from the extraction prompt.
    /// `MedicalImage` signals the orchestrator to route to MedGemma interpretation.
    pub content_type: ImageContentType,
}

/// R3: Result from medical image interpretation (MedGemma).
///
/// Used when an image contains medical imagery (radiographs, dermatology, pathology)
/// rather than text documents. MedGemma interprets the image content.
#[derive(Debug, Clone)]
pub struct MedicalImageResult {
    /// Medical findings as structured text.
    pub findings: String,
    /// Which model performed the interpretation.
    pub model_used: String,
    /// Confidence in the interpretation.
    pub confidence: f32,
}

/// R4: Vision OCR engine — extracts TEXT from document images.
///
/// MedGemma vision extraction with structured Markdown output.
///
/// Implementations: `OllamaVisionOcr` (production), `MockVisionOcr` (testing).
pub trait VisionOcrEngine: Send + Sync {
    /// Extract text from a document image using a vision model.
    /// Returns structured Markdown for documents with text content.
    fn extract_text_from_image(
        &self,
        image_bytes: &[u8],
    ) -> Result<VisionOcrResult, ExtractionError>;
}

/// R3: Medical image interpreter — UNDERSTANDS medical imagery.
///
/// MedGemma 1.5 is fine-tuned on X-rays, CT, MRI, dermatology, histopathology.
/// Used when vision OCR yields low text confidence (suggesting the image
/// is a medical image, not a text document).
///
/// The orchestrator routes: if OCR confidence < threshold → MedGemma interpretation.
pub trait MedicalImageInterpreter: Send + Sync {
    /// Interpret a medical image and return findings.
    fn interpret_medical_image(
        &self,
        image_bytes: &[u8],
    ) -> Result<MedicalImageResult, ExtractionError>;
}

/// Main extraction orchestrator trait
pub trait TextExtractor: Send + Sync {
    fn extract(
        &self,
        document_id: &Uuid,
        staged_path: &std::path::Path,
        format: &FormatDetection,
        session: &ProfileSession,
    ) -> Result<ExtractionResult, ExtractionError>;
}
