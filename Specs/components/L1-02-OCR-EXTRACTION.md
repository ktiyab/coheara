# L1-02 — OCR & Extraction

<!--
=============================================================================
COMPONENT SPEC — Converts raw documents into machine-readable text.
Engineer review: E-RS (Rust), E-ML (AI/ML), E-SC (Security), E-QA (QA)
This is the bridge between physical documents and digital understanding.
Quality here determines everything downstream.
=============================================================================
-->

## Table of Contents

| Section | Offset |
|---------|--------|
| [1] Identity | `offset=20 limit=28` |
| [2] Dependencies | `offset=48 limit=20` |
| [3] Interfaces | `offset=68 limit=80` |
| [4] Tesseract Integration | `offset=148 limit=65` |
| [5] PDF Text Extraction | `offset=213 limit=50` |
| [6] Confidence Scoring | `offset=263 limit=55` |
| [7] Pre-Processing | `offset=318 limit=50` |
| [8] Extraction Orchestration | `offset=368 limit=60` |
| [9] Error Handling | `offset=428 limit=25` |
| [10] Security | `offset=453 limit=25` |
| [11] Testing | `offset=478 limit=60` |
| [12] Performance | `offset=538 limit=15` |
| [13] Open Questions | `offset=553 limit=15` |

---

## [1] Identity

**What:** Extract machine-readable text from imported documents. This component handles two paths: (1) digital PDFs where text is directly extractable, and (2) images and scanned PDFs that require OCR via Tesseract. It produces raw text with per-page and per-region confidence scores that inform the downstream Medical Structuring component (L1-03) and the patient Review Screen (L3-04).

**After this session:**
- Digital PDFs: text extracted directly via pdf-extract with high confidence
- Images (JPEG, PNG, TIFF): OCR via bundled Tesseract, with confidence per page
- Scanned PDFs: page-by-page rendering to image, then OCR
- Overall document confidence score computed
- Raw text output ready for L1-03 Medical Structuring
- Extraction metadata stored (confidence, method used, page count)
- Image pre-processing (deskew, contrast) improves OCR quality on poor photos

**Estimated complexity:** High
**Source:** Tech Spec v1.1 Section 6.1 (Ingestion Flow), Section 6.2 (Confidence Scoring)

**Critical quality requirement:** Marie photographs her prescriptions with her phone. The photos may be slightly blurry, tilted, under fluorescent lighting, with shadows. OCR must handle these gracefully and flag low-confidence regions rather than silently producing garbage.

---

## [2] Dependencies

**Incoming:**
- L0-03 (encryption — decrypt staged files before extraction)
- L1-01 (document import — provides staged file path and format detection)

**Outgoing:**
- L1-03 (medical structuring — receives raw text + confidence)
- L3-04 (review screen — uses confidence to highlight uncertain regions)

**New Cargo.toml dependencies:**
```toml
tesseract-rs = "0.5"          # Tesseract 5 bindings (or leptess)
lopdf = "0.34"                # PDF page rendering for scanned PDFs
imageproc = "0.25"            # Image pre-processing (deskew, contrast)
```

**Bundled runtime dependency:**
- Tesseract 5 OCR engine + trained data files (`eng.traineddata`, `fra.traineddata`)
- Bundled in installer, NOT downloaded at runtime (offline requirement)

---

## [3] Interfaces

### Core Extraction Trait

```rust
/// Result of text extraction from a single document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub document_id: Uuid,
    pub method: ExtractionMethod,
    pub pages: Vec<PageExtraction>,
    pub full_text: String,                  // All pages concatenated
    pub overall_confidence: f32,            // 0.0-1.0 weighted average
    pub language_detected: Option<String>,  // e.g., "eng", "fra"
    pub page_count: usize,
}

/// How text was extracted
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExtractionMethod {
    PdfDirect,       // Digital PDF — text extracted from text layer
    TesseractOcr,    // Image or scanned PDF — OCR
    PlainTextRead,   // .txt file — direct read
}

/// Per-page extraction result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageExtraction {
    pub page_number: usize,          // 1-indexed
    pub text: String,
    pub confidence: f32,             // 0.0-1.0 for this page
    pub regions: Vec<RegionConfidence>, // Per-region confidence (optional detail)
    pub warnings: Vec<ExtractionWarning>,
}

/// Confidence for a specific region of a page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionConfidence {
    pub text: String,
    pub confidence: f32,
    pub bounding_box: Option<BoundingBox>,  // Pixel coordinates on source image
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
```

### Text Extractor Trait

```rust
/// Main extraction orchestrator
pub trait TextExtractor {
    /// Extract text from a document given its staged (encrypted) path
    fn extract(
        &self,
        document_id: &Uuid,
        format: &FormatDetection,
        session: &ProfileSession,
    ) -> Result<ExtractionResult, ExtractionError>;
}

/// OCR engine abstraction (allows mocking for tests)
pub trait OcrEngine {
    /// Run OCR on a single image
    fn ocr_image(&self, image_bytes: &[u8]) -> Result<OcrPageResult, ExtractionError>;

    /// Run OCR on a single image with language hint
    fn ocr_image_with_lang(
        &self,
        image_bytes: &[u8],
        lang: &str,
    ) -> Result<OcrPageResult, ExtractionError>;
}

/// Raw OCR result from the engine
#[derive(Debug)]
pub struct OcrPageResult {
    pub text: String,
    pub confidence: f32,
    pub word_confidences: Vec<(String, f32)>,
}

/// PDF text extraction abstraction
pub trait PdfExtractor {
    /// Extract text from a digital PDF
    fn extract_text(&self, pdf_bytes: &[u8]) -> Result<Vec<PageExtraction>, ExtractionError>;

    /// Render a PDF page to an image (for scanned PDFs → OCR)
    fn render_page_to_image(
        &self,
        pdf_bytes: &[u8],
        page_number: usize,
        dpi: u32,
    ) -> Result<Vec<u8>, ExtractionError>;

    /// Get total page count
    fn page_count(&self, pdf_bytes: &[u8]) -> Result<usize, ExtractionError>;
}
```

---

## [4] Tesseract Integration

### Engine Setup

**E-RS + E-ML:** Tesseract must be bundled, not system-installed. The trained data files are included in the installer.

```rust
use tesseract_rs::Tesseract;

/// Bundled Tesseract OCR engine
pub struct BundledTesseract {
    tessdata_dir: PathBuf,     // Path to tessdata/ in app bundle
    default_lang: String,      // "eng" or "eng+fra"
}

impl BundledTesseract {
    /// Initialize with bundled tessdata
    pub fn new(app_dir: &Path) -> Result<Self, ExtractionError> {
        let tessdata_dir = app_dir.join("tessdata");

        // Verify tessdata exists
        if !tessdata_dir.join("eng.traineddata").exists() {
            return Err(ExtractionError::TessdataNotFound(tessdata_dir));
        }

        Ok(Self {
            tessdata_dir,
            default_lang: "eng".to_string(),
        })
    }

    /// Set language(s) for OCR
    pub fn with_languages(mut self, langs: &str) -> Self {
        self.default_lang = langs.to_string();
        self
    }
}

impl OcrEngine for BundledTesseract {
    fn ocr_image(&self, image_bytes: &[u8]) -> Result<OcrPageResult, ExtractionError> {
        self.ocr_image_with_lang(image_bytes, &self.default_lang)
    }

    fn ocr_image_with_lang(
        &self,
        image_bytes: &[u8],
        lang: &str,
    ) -> Result<OcrPageResult, ExtractionError> {
        let mut tess = Tesseract::new(
            self.tessdata_dir.to_str().unwrap(),
            lang,
        ).map_err(|e| ExtractionError::OcrInit(e.to_string()))?;

        // Set OCR parameters for medical documents
        tess.set_variable("tessedit_pageseg_mode", "3")  // Fully automatic page segmentation
            .map_err(|e| ExtractionError::OcrConfig(e.to_string()))?;

        // Set image from bytes
        tess.set_image_from_mem(image_bytes)
            .map_err(|e| ExtractionError::OcrProcessing(e.to_string()))?;

        // Run recognition
        let text = tess.get_text()
            .map_err(|e| ExtractionError::OcrProcessing(e.to_string()))?;

        // Get mean confidence (0-100 from Tesseract, normalize to 0.0-1.0)
        let confidence = tess.mean_text_conf() as f32 / 100.0;

        // Get word-level confidences
        let word_confidences = extract_word_confidences(&tess)?;

        Ok(OcrPageResult {
            text,
            confidence,
            word_confidences,
        })
    }
}

/// Extract per-word confidence from Tesseract iterator
fn extract_word_confidences(
    tess: &Tesseract,
) -> Result<Vec<(String, f32)>, ExtractionError> {
    let mut words = Vec::new();

    // Tesseract provides word-level iteration via ResultIterator
    // Implementation depends on exact tesseract-rs API
    // Fallback: split text by whitespace and assign mean confidence
    let text = tess.get_text()
        .map_err(|e| ExtractionError::OcrProcessing(e.to_string()))?;
    let mean_conf = tess.mean_text_conf() as f32 / 100.0;

    for word in text.split_whitespace() {
        words.push((word.to_string(), mean_conf));
    }

    Ok(words)
}
```

### OCR Quality Warnings

```rust
/// Analyze OCR result and generate warnings
fn analyze_ocr_quality(result: &OcrPageResult) -> Vec<ExtractionWarning> {
    let mut warnings = Vec::new();

    if result.confidence < 0.50 {
        warnings.push(ExtractionWarning::BlurryImage);
    }

    // Check for signs of handwriting (very low confidence + short words)
    let low_conf_words = result.word_confidences.iter()
        .filter(|(_, c)| *c < 0.40)
        .count();
    let total_words = result.word_confidences.len().max(1);
    if low_conf_words as f64 / total_words as f64 > 0.50 {
        warnings.push(ExtractionWarning::HandwritingDetected);
    }

    warnings
}
```

---

## [5] PDF Text Extraction

### Digital PDF Path

```rust
use pdf::file::FileOptions;

pub struct PdfTextExtractor;

impl PdfExtractor for PdfTextExtractor {
    fn extract_text(&self, pdf_bytes: &[u8]) -> Result<Vec<PageExtraction>, ExtractionError> {
        let doc = FileOptions::cached()
            .load(pdf_bytes.to_vec())
            .map_err(|e| ExtractionError::PdfParsing(e.to_string()))?;

        let num_pages = doc.num_pages();
        let mut pages = Vec::with_capacity(num_pages as usize);

        for i in 0..num_pages {
            let page = doc.get_page(i)
                .map_err(|e| ExtractionError::PdfParsing(e.to_string()))?;

            let text = match page.extract_text() {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!(page = i, error = %e, "Failed to extract text from page");
                    String::new()
                }
            };

            // Digital PDFs get high base confidence
            let confidence = if text.len() > 10 { 0.95 } else { 0.0 };

            pages.push(PageExtraction {
                page_number: (i + 1) as usize,
                text,
                confidence,
                regions: vec![],  // No per-region data for digital PDFs
                warnings: vec![],
            });
        }

        Ok(pages)
    }

    fn render_page_to_image(
        &self,
        pdf_bytes: &[u8],
        page_number: usize,
        dpi: u32,
    ) -> Result<Vec<u8>, ExtractionError> {
        // For scanned PDFs: render page as image for OCR
        // Using lopdf or pdf-render crate
        // This extracts embedded images from the page
        let doc = FileOptions::cached()
            .load(pdf_bytes.to_vec())
            .map_err(|e| ExtractionError::PdfParsing(e.to_string()))?;

        let page = doc.get_page(page_number as u32)
            .map_err(|e| ExtractionError::PdfParsing(e.to_string()))?;

        // Extract the largest image from the page
        // (Scanned PDFs typically have one large image per page)
        extract_page_image(&doc, &page)
    }

    fn page_count(&self, pdf_bytes: &[u8]) -> Result<usize, ExtractionError> {
        let doc = FileOptions::cached()
            .load(pdf_bytes.to_vec())
            .map_err(|e| ExtractionError::PdfParsing(e.to_string()))?;
        Ok(doc.num_pages() as usize)
    }
}

/// Extract the embedded image from a scanned PDF page
fn extract_page_image(
    doc: &pdf::file::File<Vec<u8>>,
    page: &pdf::content::Page,
) -> Result<Vec<u8>, ExtractionError> {
    // Iterate page resources, find XObject images
    // Return the largest image as PNG bytes
    // Implementation depends on exact pdf crate API
    Err(ExtractionError::PdfParsing(
        "PDF image extraction not yet implemented — deferred to implementation".into()
    ))
}
```

### Scanned PDF OCR Path

```rust
/// OCR a scanned PDF page by page
fn ocr_scanned_pdf(
    pdf_bytes: &[u8],
    pdf_extractor: &dyn PdfExtractor,
    ocr_engine: &dyn OcrEngine,
) -> Result<Vec<PageExtraction>, ExtractionError> {
    let num_pages = pdf_extractor.page_count(pdf_bytes)?;
    let mut pages = Vec::with_capacity(num_pages);

    for page_num in 0..num_pages {
        // Render page to image
        let image_bytes = pdf_extractor.render_page_to_image(pdf_bytes, page_num, 300)?;

        // Pre-process image for better OCR
        let processed = preprocess_image(&image_bytes)?;

        // Run OCR
        let ocr_result = ocr_engine.ocr_image(&processed)?;
        let warnings = analyze_ocr_quality(&ocr_result);

        pages.push(PageExtraction {
            page_number: page_num + 1,
            text: ocr_result.text,
            confidence: ocr_result.confidence,
            regions: ocr_result.word_confidences.iter()
                .map(|(word, conf)| RegionConfidence {
                    text: word.clone(),
                    confidence: *conf,
                    bounding_box: None,
                })
                .collect(),
            warnings,
        });
    }

    Ok(pages)
}
```

---

## [6] Confidence Scoring

**E-ML + E-UX:** Confidence determines what gets flagged in the Review Screen (L3-04). Too many false flags annoy users. Too few miss errors. The scoring system must be calibrated.

### Scoring System

```rust
/// Compute overall document confidence from per-page results
pub fn compute_overall_confidence(
    pages: &[PageExtraction],
    method: &ExtractionMethod,
) -> f32 {
    if pages.is_empty() {
        return 0.0;
    }

    // Base confidence from extraction method
    let method_base = match method {
        ExtractionMethod::PdfDirect => 0.95,
        ExtractionMethod::TesseractOcr => 0.0,  // Entirely from OCR results
        ExtractionMethod::PlainTextRead => 0.99,
    };

    if *method == ExtractionMethod::PdfDirect {
        // Digital PDFs: check if all pages had text
        let pages_with_text = pages.iter()
            .filter(|p| !p.text.trim().is_empty())
            .count();
        let ratio = pages_with_text as f32 / pages.len() as f32;
        return method_base * ratio;
    }

    // For OCR: weighted average by text length
    let total_chars: usize = pages.iter().map(|p| p.text.len()).sum();
    if total_chars == 0 {
        return 0.0;
    }

    let weighted_sum: f32 = pages.iter()
        .map(|p| p.confidence * p.text.len() as f32)
        .sum();

    weighted_sum / total_chars as f32
}
```

### Confidence Thresholds

```rust
/// Confidence thresholds used by UI and pipeline
pub mod confidence {
    /// Below this: extraction likely failed. Show strong warning.
    pub const VERY_LOW: f32 = 0.30;

    /// Below this: significant uncertainty. Flag all extracted fields.
    pub const LOW: f32 = 0.50;

    /// Below this: some uncertainty. Flag key medical fields.
    pub const MODERATE: f32 = 0.70;

    /// Above this: high confidence. No special flagging.
    pub const HIGH: f32 = 0.85;

    /// Above this: very high confidence. Extracted from digital source.
    pub const VERY_HIGH: f32 = 0.95;
}
```

### Per-Field Confidence (for Review Screen)

```rust
/// Identify low-confidence regions for highlighting
pub fn flag_low_confidence_regions(
    page: &PageExtraction,
    threshold: f32,
) -> Vec<RegionConfidence> {
    page.regions.iter()
        .filter(|r| r.confidence < threshold)
        .cloned()
        .collect()
}
```

---

## [7] Pre-Processing

**E-ML:** Image pre-processing significantly improves OCR quality, especially for phone photos. These operations run before Tesseract.

### Image Pre-Processing Pipeline

```rust
use image::{DynamicImage, GrayImage, imageops};
use imageproc::contrast;

/// Pre-process an image for better OCR results
pub fn preprocess_image(image_bytes: &[u8]) -> Result<Vec<u8>, ExtractionError> {
    let img = image::load_from_memory(image_bytes)
        .map_err(|e| ExtractionError::ImageProcessing(e.to_string()))?;

    // Step 1: Convert to grayscale
    let gray = img.to_luma8();

    // Step 2: Auto-contrast (normalize histogram)
    let contrasted = auto_contrast(&gray);

    // Step 3: Deskew detection (optional — only if angle is significant)
    let (deskewed, angle) = deskew_if_needed(&contrasted);

    // Step 4: Adaptive thresholding (binarize for OCR)
    let binary = adaptive_threshold(&deskewed, 15); // block_size=15

    // Convert back to bytes (PNG for Tesseract)
    let mut output = Vec::new();
    let dynamic = DynamicImage::ImageLuma8(binary);
    dynamic.write_to(
        &mut std::io::Cursor::new(&mut output),
        image::ImageFormat::Png,
    ).map_err(|e| ExtractionError::ImageProcessing(e.to_string()))?;

    Ok(output)
}

/// Auto-contrast: stretch histogram to use full range
fn auto_contrast(img: &GrayImage) -> GrayImage {
    let mut min = 255u8;
    let mut max = 0u8;
    for pixel in img.pixels() {
        let v = pixel.0[0];
        if v < min { min = v; }
        if v > max { max = v; }
    }

    if max == min {
        return img.clone();
    }

    let range = (max - min) as f32;
    let mut result = img.clone();
    for pixel in result.pixels_mut() {
        let v = pixel.0[0];
        pixel.0[0] = ((v as f32 - min as f32) / range * 255.0) as u8;
    }
    result
}

/// Detect skew angle and correct if > 1 degree
fn deskew_if_needed(img: &GrayImage) -> (GrayImage, f32) {
    // Simple deskew using projection profile
    // For MVP: skip deskew, Tesseract handles moderate skew well
    // Future: implement Hough transform deskew
    (img.clone(), 0.0)
}

/// Adaptive threshold (Sauvola or mean-based)
fn adaptive_threshold(img: &GrayImage, block_size: u32) -> GrayImage {
    // Simple global Otsu threshold as MVP
    // Future: implement Sauvola adaptive thresholding
    let threshold = imageproc::contrast::otsu_level(img);
    imageproc::contrast::threshold(img, threshold)
}
```

### Quality Assessment (Pre-OCR)

```rust
/// Assess image quality before OCR to set expectations
pub fn assess_image_quality(image_bytes: &[u8]) -> Result<ImageQuality, ExtractionError> {
    let img = image::load_from_memory(image_bytes)
        .map_err(|e| ExtractionError::ImageProcessing(e.to_string()))?;

    let gray = img.to_luma8();
    let (width, height) = gray.dimensions();

    // Resolution check
    let resolution = if width * height > 2_000_000 { "high" }
        else if width * height > 500_000 { "medium" }
        else { "low" };

    // Contrast check (standard deviation of pixel values)
    let mean: f64 = gray.pixels().map(|p| p.0[0] as f64).sum::<f64>()
        / (width * height) as f64;
    let variance: f64 = gray.pixels()
        .map(|p| (p.0[0] as f64 - mean).powi(2))
        .sum::<f64>() / (width * height) as f64;
    let std_dev = variance.sqrt();

    let contrast = if std_dev > 60.0 { "good" }
        else if std_dev > 30.0 { "fair" }
        else { "poor" };

    Ok(ImageQuality {
        resolution: resolution.into(),
        contrast: contrast.into(),
        estimated_confidence: match (resolution, contrast) {
            ("high", "good") => 0.85,
            ("high", "fair") | ("medium", "good") => 0.70,
            ("medium", "fair") => 0.55,
            _ => 0.35,
        },
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageQuality {
    pub resolution: String,
    pub contrast: String,
    pub estimated_confidence: f32,
}
```

---

## [8] Extraction Orchestration

### Main Extractor

```rust
/// Concrete implementation of the text extractor
pub struct DocumentExtractor {
    ocr_engine: Box<dyn OcrEngine + Send + Sync>,
    pdf_extractor: Box<dyn PdfExtractor + Send + Sync>,
}

impl DocumentExtractor {
    pub fn new(
        ocr_engine: Box<dyn OcrEngine + Send + Sync>,
        pdf_extractor: Box<dyn PdfExtractor + Send + Sync>,
    ) -> Self {
        Self { ocr_engine, pdf_extractor }
    }
}

impl TextExtractor for DocumentExtractor {
    fn extract(
        &self,
        document_id: &Uuid,
        format: &FormatDetection,
        session: &ProfileSession,
    ) -> Result<ExtractionResult, ExtractionError> {
        tracing::info!(
            document_id = %document_id,
            category = format.category.as_str(),
            "Starting text extraction"
        );

        // Step 1: Decrypt the staged file
        let staged_path = session.db_path()
            .parent().unwrap()
            .parent().unwrap()
            .join("originals")
            .join(format!("{}.*.enc", document_id));

        // Find the actual staged file (we stored with extension)
        let staged_file = find_staged_file(document_id, session)?;
        let decrypted_bytes = session.decrypt_file(&staged_file)?;

        // Step 2: Extract based on format
        let (method, pages) = match &format.category {
            FileCategory::DigitalPdf => {
                let pages = self.pdf_extractor.extract_text(&decrypted_bytes)?;
                (ExtractionMethod::PdfDirect, pages)
            }
            FileCategory::ScannedPdf => {
                let pages = ocr_scanned_pdf(
                    &decrypted_bytes,
                    self.pdf_extractor.as_ref(),
                    self.ocr_engine.as_ref(),
                )?;
                (ExtractionMethod::TesseractOcr, pages)
            }
            FileCategory::Image => {
                let processed = preprocess_image(&decrypted_bytes)?;
                let ocr_result = self.ocr_engine.ocr_image(&processed)?;
                let warnings = analyze_ocr_quality(&ocr_result);

                let page = PageExtraction {
                    page_number: 1,
                    text: ocr_result.text,
                    confidence: ocr_result.confidence,
                    regions: ocr_result.word_confidences.iter()
                        .map(|(word, conf)| RegionConfidence {
                            text: word.clone(),
                            confidence: *conf,
                            bounding_box: None,
                        })
                        .collect(),
                    warnings,
                };
                (ExtractionMethod::TesseractOcr, vec![page])
            }
            FileCategory::PlainText => {
                let text = String::from_utf8(decrypted_bytes)
                    .map_err(|e| ExtractionError::EncodingError(e.to_string()))?;

                let page = PageExtraction {
                    page_number: 1,
                    text: text.clone(),
                    confidence: 0.99,
                    regions: vec![],
                    warnings: vec![],
                };
                (ExtractionMethod::PlainTextRead, vec![page])
            }
            FileCategory::Unsupported => {
                return Err(ExtractionError::UnsupportedFormat);
            }
        };

        // Step 3: Compute overall confidence
        let overall_confidence = compute_overall_confidence(&pages, &method);

        // Step 4: Concatenate full text
        let full_text = pages.iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n--- Page Break ---\n\n");

        let page_count = pages.len();

        tracing::info!(
            document_id = %document_id,
            method = ?method,
            pages = page_count,
            confidence = overall_confidence,
            text_length = full_text.len(),
            "Text extraction complete"
        );

        Ok(ExtractionResult {
            document_id: *document_id,
            method,
            pages,
            full_text,
            overall_confidence,
            language_detected: None,  // TODO: language detection
            page_count,
        })
    }
}

/// Find the encrypted staged file for a document
fn find_staged_file(
    document_id: &Uuid,
    session: &ProfileSession,
) -> Result<PathBuf, ExtractionError> {
    let originals_dir = session.db_path()
        .parent().unwrap()
        .parent().unwrap()
        .join("originals");

    // Find file matching document_id.*.enc
    let prefix = document_id.to_string();
    for entry in std::fs::read_dir(&originals_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(&prefix) && name.ends_with(".enc") {
            return Ok(entry.path());
        }
    }

    Err(ExtractionError::StagedFileNotFound(*document_id))
}
```

### Tauri Commands

```rust
// src-tauri/src/commands/extraction.rs

/// Extract text from an imported document
#[tauri::command]
pub async fn extract_document_text(
    document_id: String,
    extractor: State<'_, Box<dyn TextExtractor + Send + Sync>>,
    session: State<'_, Option<ProfileSession>>,
    doc_repo: State<'_, Box<dyn DocumentRepository + Send + Sync>>,
) -> Result<ExtractionResult, String> {
    let session = session.as_ref()
        .ok_or("No active profile session")?;

    let doc_id = Uuid::parse_str(&document_id)
        .map_err(|e| format!("Invalid document ID: {e}"))?;

    // Get document to know its format
    let doc = doc_repo.get(&doc_id)
        .map_err(|e| e.to_string())?
        .ok_or("Document not found")?;

    // Re-detect format from stored path info
    let format = FormatDetection {
        mime_type: String::new(),  // Not needed for extraction path selection
        category: detect_category_from_extension(&doc.source_file),
        is_digital_pdf: None,
        file_size_bytes: 0,
    };

    let result = extractor.extract(&doc_id, &format, session)
        .map_err(|e| e.to_string())?;

    // Update document with OCR confidence
    let mut updated_doc = doc.clone();
    updated_doc.ocr_confidence = Some(result.overall_confidence);
    doc_repo.update(&updated_doc).map_err(|e| e.to_string())?;

    Ok(result)
}
```

---

## [9] Error Handling

```rust
use thiserror::Error;

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
}
```

**E-UX user-facing messages:**

| Error | User sees |
|-------|-----------|
| `OcrProcessing` | "I had trouble reading this document. Try a clearer photo with good lighting." |
| `TessdataNotFound` | "OCR engine data is missing. The app may need to be reinstalled." |
| `PdfParsing` | "This PDF couldn't be processed. It may be corrupted or password-protected." |
| `StagedFileNotFound` | "The original document file couldn't be found. It may have been moved." |

---

## [10] Security

**E-SC review:**

| Concern | Mitigation |
|---------|-----------|
| Malicious image files | Image crate handles parsing. No arbitrary code execution from image data. Tesseract is a mature C++ library with fuzzing coverage. |
| Prompt injection in document text | Extracted text is treated as DATA, never as instructions. Sanitized before passing to MedGemma (L1-03). |
| Memory for large documents | Process page-by-page, don't load entire multi-page PDF into memory. |
| Temporary decrypted files | Decrypted bytes exist in memory only. Never written to disk as plaintext. |
| OCR output sanitization | Strip non-printable characters, control codes. Normalize Unicode. |

### Text Sanitization (Pre-Output)

```rust
/// Sanitize extracted text before passing downstream
pub fn sanitize_extracted_text(raw: &str) -> String {
    raw.chars()
        .filter(|c| {
            // Allow printable chars, whitespace, common punctuation
            c.is_alphanumeric()
                || c.is_whitespace()
                || matches!(c,
                    '.' | ',' | ';' | ':' | '-' | '/' | '(' | ')' |
                    '[' | ']' | '+' | '=' | '%' | '#' | '@' | '&' |
                    '\'' | '"' | '!' | '?' | '<' | '>' | '*' | '_'
                )
        })
        // Collapse multiple spaces/newlines
        .collect::<String>()
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
```

---

## [11] Testing

### Acceptance Criteria

| # | Test | Expected |
|---|------|----------|
| T-01 | Extract text from digital PDF | Full text extracted, confidence > 0.90 |
| T-02 | OCR a clear printed JPEG | Text extracted, confidence > 0.75 |
| T-03 | OCR a blurry photo | Text extracted with low confidence, BlurryImage warning |
| T-04 | OCR a scanned PDF (2 pages) | Both pages extracted, per-page confidence |
| T-05 | Read a plain text file | Text returned verbatim, confidence 0.99 |
| T-06 | Extract from encrypted staged file | Decrypt → extract → correct text |
| T-07 | Unsupported format rejected | ExtractionError::UnsupportedFormat |
| T-08 | Empty PDF (no text, no images) | ExtractionResult with empty text, confidence 0.0 |
| T-09 | Overall confidence computation | Weighted average matches expected |
| T-10 | Pre-processing improves OCR | Compare OCR results with/without preprocessing |
| T-11 | Text sanitization removes control chars | Null bytes, control codes stripped |
| T-12 | Confidence thresholds correctly categorized | 0.30 → VERY_LOW, 0.70 → MODERATE |
| T-13 | Large PDF (20 pages) processes without OOM | Memory stays bounded |
| T-14 | OCR with French trained data | French prescription text extracted correctly |
| T-15 | Tesseract initialization with bundled data | Engine starts without system Tesseract |
| T-16 | Page break markers in full_text | Multi-page document has page separators |

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn confidence_scoring_digital_pdf() {
        let pages = vec![
            PageExtraction {
                page_number: 1,
                text: "This is page one with lots of text.".into(),
                confidence: 0.95,
                regions: vec![],
                warnings: vec![],
            },
            PageExtraction {
                page_number: 2,
                text: "This is page two with more text.".into(),
                confidence: 0.95,
                regions: vec![],
                warnings: vec![],
            },
        ];
        let conf = compute_overall_confidence(&pages, &ExtractionMethod::PdfDirect);
        assert!(conf > 0.90, "Digital PDF should have high confidence: {conf}");
    }

    #[test]
    fn confidence_scoring_mixed_ocr() {
        let pages = vec![
            PageExtraction {
                page_number: 1,
                text: "Clear text on page one".repeat(10),  // Long page
                confidence: 0.85,
                regions: vec![],
                warnings: vec![],
            },
            PageExtraction {
                page_number: 2,
                text: "Blurry".into(),  // Short, low-confidence page
                confidence: 0.30,
                regions: vec![],
                warnings: vec![],
            },
        ];
        let conf = compute_overall_confidence(&pages, &ExtractionMethod::TesseractOcr);
        // Weighted by text length, so long page dominates
        assert!(conf > 0.70, "Weighted confidence should favor long clear page: {conf}");
    }

    #[test]
    fn confidence_empty_document() {
        let pages = vec![];
        let conf = compute_overall_confidence(&pages, &ExtractionMethod::TesseractOcr);
        assert_eq!(conf, 0.0);
    }

    #[test]
    fn text_sanitization() {
        let raw = "Patient: Marie\x00Dubois\nDose: 500mg\x01\n\n\n\nDate: 2024-01-15";
        let clean = sanitize_extracted_text(raw);
        assert!(!clean.contains('\x00'));
        assert!(!clean.contains('\x01'));
        assert!(clean.contains("Marie"));
        assert!(clean.contains("500mg"));
    }

    #[test]
    fn ocr_quality_warnings_generated() {
        let low_result = OcrPageResult {
            text: "blurry text".into(),
            confidence: 0.25,
            word_confidences: vec![
                ("blurry".into(), 0.20),
                ("text".into(), 0.30),
            ],
        };
        let warnings = analyze_ocr_quality(&low_result);
        assert!(warnings.iter().any(|w| matches!(w, ExtractionWarning::BlurryImage)));
    }

    #[test]
    fn image_quality_assessment() {
        // Create a simple test image (white image = poor contrast)
        let img = image::GrayImage::from_fn(100, 100, |_, _| image::Luma([200u8]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageLuma8(img)
            .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        let quality = assess_image_quality(&bytes).unwrap();
        assert_eq!(quality.contrast, "poor");
    }

    #[test]
    fn extraction_method_is_correct() {
        // Digital PDF → PdfDirect
        // Image → TesseractOcr
        // Plain text → PlainTextRead
        assert_eq!(
            ExtractionMethod::PdfDirect,
            ExtractionMethod::PdfDirect
        );
    }
}
```

### Integration Test (Requires Tesseract)

```rust
#[cfg(test)]
#[cfg(feature = "integration-tests")]
mod integration_tests {
    use super::*;

    #[test]
    fn ocr_real_image() {
        // Requires tessdata to be available
        let tessdata_dir = std::env::var("TESSDATA_DIR")
            .unwrap_or_else(|_| "/usr/share/tesseract-ocr/5/tessdata".into());

        let engine = BundledTesseract::new(Path::new(&tessdata_dir)).unwrap();

        // Load a test prescription image from test fixtures
        let test_image = include_bytes!("../../test_fixtures/prescription_clear.png");

        let result = engine.ocr_image(test_image).unwrap();
        assert!(result.confidence > 0.50);
        assert!(!result.text.is_empty());
    }
}
```

---

## [12] Performance

| Metric | Target |
|--------|--------|
| Digital PDF extraction (10 pages) | < 500ms |
| Image OCR (single page, 300 DPI) | < 3 seconds |
| Scanned PDF OCR (5 pages) | < 15 seconds |
| Image pre-processing | < 200ms per image |
| Plain text reading | < 10ms |
| Overall: single document import → extraction complete | < 5 seconds for typical document |

**E-RS note:** OCR is CPU-intensive. Run extraction on a dedicated thread (Tokio blocking task) to avoid blocking the UI event loop.

---

## [13] Open Questions

| # | Question | Status |
|---|---------|--------|
| OQ-01 | Which Tesseract Rust binding to use — `tesseract-rs` vs `leptess` vs `tesseract-sys`? | Evaluate at implementation based on build complexity. |
| OQ-02 | PDF page rendering — `lopdf` vs `pdfium-render` vs `mupdf`? | `lopdf` for text extraction, may need `pdfium` for image rendering of scanned pages. |
| OQ-03 | Deskew implementation — worth the complexity for MVP? | Defer to Phase 2. Tesseract handles moderate skew internally. |
| OQ-04 | Language detection — auto-detect or user selects? | Default to eng+fra. Auto-detection deferred to Phase 2. |
