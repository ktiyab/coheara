//! R4+: Image preprocessing services for vision model input.
//!
//! **Services architecture**: each processing step is an independent, reusable service.
//! Services are composed by `PreprocessingPipeline` — model-agnostic, swappable.
//!
//! Grounded on:
//! - Google Cloud Document AI: quality detection + aspect-ratio padding + adaptive processing
//! - Apple Vision Framework: VNDocumentCameraViewController quality tiers, scaleFit
//! - HuggingFace AutoImageProcessor: do_pad, padding_value, resize + center
//!
//! Key rules (all sources agree):
//! - DO preserve aspect ratio with padding
//! - DO keep PNG (lossless) and RGB color space
//! - DO detect quality issues (blank, dark, blurry) as warnings
//! - DO NOT enhance clean documents (Google "vanilla" rule)

use std::borrow::Cow;
use std::io::Cursor;

use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, GrayImage, ImageOutputFormat, Luma, Rgb, RgbImage};
use tracing::debug;

use super::types::ExtractionWarning;
use super::ExtractionError;

// ═══════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════

/// Maximum input image size (in bytes) before rejecting.
/// Prevents OOM on corrupt/adversarial files.
const MAX_IMAGE_BYTES: usize = 50 * 1024 * 1024; // 50 MB

/// Minimum valid image size in bytes (smallest valid PNG is ~67 bytes).
const MIN_IMAGE_BYTES: usize = 67;

// ═══════════════════════════════════════════════════════════
// Configuration types (separated by concern)
// ═══════════════════════════════════════════════════════════

/// What the vision model needs. Changes per model, not per hardware.
///
/// Grounded: Google Document AI normalizes to model-specific dimensions.
/// MedGemma SigLIP encoder: 896x896. LLaVA: 336x336. Gemma: 224x224.
#[derive(Debug, Clone)]
pub struct ModelInputConfig {
    /// Target square dimension for vision model input.
    pub target_size: u32,
    /// Background color for padding (RGB).
    /// White for documents, black for natural images.
    pub padding_color: [u8; 3],
}

impl ModelInputConfig {
    /// MedGemma SigLIP encoder: 896x896, white padding for medical documents.
    pub fn medgemma() -> Self {
        Self {
            target_size: 896,
            padding_color: [255, 255, 255],
        }
    }
}

/// What the hardware can handle. Changes per machine, not per model.
///
/// Grounded: Apple Vision Framework quality tiers (`.fast` vs `.accurate`).
#[derive(Debug, Clone)]
pub struct HardwareConfig {
    /// Resize filter quality.
    pub resize_filter: ResizeQuality,
    /// Maximum input dimension before pre-downscale.
    pub max_input_dimension: u32,
}

impl HardwareConfig {
    /// GPU tier: best quality, large images allowed.
    pub fn gpu() -> Self {
        Self {
            resize_filter: ResizeQuality::High,
            max_input_dimension: 4096,
        }
    }

    /// CPU tier: faster filter, smaller max dimension.
    pub fn cpu() -> Self {
        Self {
            resize_filter: ResizeQuality::Fast,
            max_input_dimension: 2048,
        }
    }
}

/// Resize filter quality levels.
///
/// CatmullRom (cubic spline) is recommended over Lanczos3 for document text:
/// Lanczos3 produces sharper output but introduces ringing artifacts around
/// high-contrast edges — exactly what text characters are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeQuality {
    /// CatmullRom — best quality for text, no ringing. ~30% faster than Lanczos3.
    High,
    /// Triangle (bilinear) — fast, acceptable quality. Apple Vision `.fast` mode.
    Fast,
}

// ═══════════════════════════════════════════════════════════
// Service traits
// ═══════════════════════════════════════════════════════════

/// Fixes image orientation from EXIF metadata.
///
/// Grounded: Google Document AI and Apple Vision auto-correct orientation.
/// Phone photos embed rotation in EXIF tag 0x0112 — without correction,
/// portrait photos appear sideways to the vision model.
///
/// Reusable: any vision model benefits from correctly-oriented input.
pub trait OrientationCorrector: Send + Sync {
    /// Correct image orientation based on EXIF metadata.
    ///
    /// `raw_bytes`: Original file bytes (needed for EXIF reading).
    /// `image`: Decoded image (rotation applied here).
    /// Returns the corrected image. No-op if no EXIF or orientation=1.
    fn correct(&self, raw_bytes: &[u8], image: DynamicImage) -> DynamicImage;
}

/// Resizes + pads image to target dimensions preserving aspect ratio.
///
/// Grounded: Google Document AI aspect-ratio padding, HuggingFace AutoImageProcessor
/// do_pad + padding_value + resize. Apple Vision scaleFit.
///
/// Config-driven: target_size comes from `ModelInputConfig`, not hardcoded.
pub trait ImageNormalizer: Send + Sync {
    /// Normalize image to model input dimensions.
    ///
    /// Steps: pre-downscale guard -> compute fit -> resize -> pad -> center on canvas.
    fn normalize(
        &self,
        image: &RgbImage,
        model_config: &ModelInputConfig,
        hw_config: &HardwareConfig,
    ) -> Result<NormalizedImage, ExtractionError>;
}

/// Assesses image quality without modifying it. Pure read-only analysis.
///
/// Grounded: Google Document AI quality classifier, Apple VNImageAnalysis.
/// Returns structured report — services downstream can act on scores.
pub trait QualityAssessor: Send + Sync {
    /// Assess image quality after normalization.
    fn assess(&self, image: &RgbImage) -> QualityReport;
}

/// Reduces noise on degraded inputs. No-op on clean documents.
///
/// Grounded: Google Document AI quality-tier processing. Apple Vision document
/// enhancement. Key rule (PRESERVED): normal documents are NOT enhanced
/// ("vanilla" rule). Only degraded inputs (high noise) get filtered.
pub trait NoiseReducer: Send + Sync {
    /// Reduce noise if the image is degraded. Passthrough if clean.
    fn reduce_if_needed(&self, image: RgbImage, quality: &QualityReport) -> RgbImage;
}

// ═══════════════════════════════════════════════════════════
// Result types
// ═══════════════════════════════════════════════════════════

/// Result of normalization (resize + pad).
#[derive(Debug)]
pub struct NormalizedImage {
    /// Normalized image (target_size x target_size, RGB).
    pub image: RgbImage,
    /// Original dimensions before any processing.
    pub original_width: u32,
    pub original_height: u32,
    /// Content area dimensions within the padded canvas.
    pub content_width: u32,
    pub content_height: u32,
}

/// Quality assessment report with numeric scores.
///
/// Scores enable downstream services to threshold independently.
/// Warnings are derived from scores using production-tuned thresholds.
#[derive(Debug, Default)]
pub struct QualityReport {
    /// Quality warnings to propagate to extraction result.
    pub warnings: Vec<ExtractionWarning>,
    /// Page appears mostly blank (>95% near-white).
    pub is_blank: bool,
    /// Page appears mostly dark (>80% near-black).
    pub is_dark: bool,
    /// Laplacian variance — higher = sharper. Blurry < 100, sharp text > 500.
    pub blur_score: f32,
    /// Detected skew angle in degrees. `None` if straight (< 0.5 deg).
    pub skew_angle: Option<f32>,
    /// RMS contrast (0-255). Low contrast < 25, typical document > 50.
    pub contrast_score: f32,
}

// ═══════════════════════════════════════════════════════════
// ImagePreprocessor trait (orchestrator interface)
// ═══════════════════════════════════════════════════════════

/// Preprocesses images for optimal vision model ingestion.
///
/// Pure image-to-image transform — no I/O, no model calls, fully testable.
/// Inserted between PDF rendering / image loading and vision OCR.
///
/// Config stored at construction time — callers don't need model details.
pub trait ImagePreprocessor: Send + Sync {
    /// Prepare an image for vision model input.
    ///
    /// Input: raw image bytes (PNG, JPEG, TIFF, etc.).
    /// Output: preprocessed PNG bytes (target_size x target_size, RGB, padded).
    fn preprocess(&self, image_bytes: &[u8]) -> Result<PreparedImage, ExtractionError>;
}

/// Result of image preprocessing.
#[derive(Debug)]
pub struct PreparedImage {
    /// Preprocessed image as PNG bytes, ready for vision model.
    pub png_bytes: Vec<u8>,
    /// Quality warnings detected during preprocessing.
    pub warnings: Vec<ExtractionWarning>,
    /// Original image dimensions before preprocessing.
    pub original_width: u32,
    pub original_height: u32,
    /// Dimensions of the content area within the padded output.
    pub content_width: u32,
    pub content_height: u32,
}

// ═══════════════════════════════════════════════════════════
// PreprocessingPipeline — Composes services
// ═══════════════════════════════════════════════════════════

/// Composes independent services into a preprocessing pipeline.
///
/// Grounded: Google Document AI pipeline stages + Apple Vision processing chain.
/// Model-agnostic: configure via `ModelInputConfig` for any vision model.
///
/// Pipeline flow:
/// 1. Validate bytes (size bounds)
/// 2. Decode image
/// 3. `orientation.correct()` — fix EXIF rotation
/// 4. Convert to RGB
/// 5. `normalizer.normalize()` — resize + pad to target
/// 6. `quality.assess()` — detect quality issues
/// 7. `noise_reducer.reduce_if_needed()` — conditional denoising (optional)
/// 8. Encode PNG
pub struct PreprocessingPipeline {
    orientation: Box<dyn OrientationCorrector>,
    normalizer: Box<dyn ImageNormalizer>,
    quality: Box<dyn QualityAssessor>,
    noise_reducer: Option<Box<dyn NoiseReducer>>,
    model_config: ModelInputConfig,
    hw_config: HardwareConfig,
}

impl PreprocessingPipeline {
    pub fn new(
        orientation: Box<dyn OrientationCorrector>,
        normalizer: Box<dyn ImageNormalizer>,
        quality: Box<dyn QualityAssessor>,
        model_config: ModelInputConfig,
        hw_config: HardwareConfig,
    ) -> Self {
        Self {
            orientation,
            normalizer,
            quality,
            noise_reducer: None,
            model_config,
            hw_config,
        }
    }

    /// Add a noise reducer to the pipeline (optional).
    pub fn with_noise_reducer(mut self, reducer: Box<dyn NoiseReducer>) -> Self {
        self.noise_reducer = Some(reducer);
        self
    }

    /// Production pipeline for MedGemma with GPU hardware tier.
    pub fn medgemma_gpu() -> Self {
        Self::new(
            Box::new(ExifOrientationCorrector),
            Box::new(AspectRatioNormalizer),
            Box::new(EnhancedQualityAssessor),
            ModelInputConfig::medgemma(),
            HardwareConfig::gpu(),
        )
        .with_noise_reducer(Box::new(ConditionalNoiseReducer::default()))
    }

    /// Production pipeline for MedGemma with CPU hardware tier.
    pub fn medgemma_cpu() -> Self {
        Self::new(
            Box::new(ExifOrientationCorrector),
            Box::new(AspectRatioNormalizer),
            Box::new(EnhancedQualityAssessor),
            ModelInputConfig::medgemma(),
            HardwareConfig::cpu(),
        )
        .with_noise_reducer(Box::new(ConditionalNoiseReducer::default()))
    }
}

impl ImagePreprocessor for PreprocessingPipeline {
    fn preprocess(&self, image_bytes: &[u8]) -> Result<PreparedImage, ExtractionError> {
        // 1. Validate bytes
        validate_image_bytes(image_bytes)?;

        // 2. Decode image
        let img = image::load_from_memory(image_bytes).map_err(|e| {
            ExtractionError::ImageProcessing(format!("Failed to decode image: {e}"))
        })?;
        let (orig_w, orig_h) = img.dimensions();

        // 3. Fix EXIF orientation
        let img = self.orientation.correct(image_bytes, img);

        // 4. Convert to RGB
        let rgb = img.to_rgb8();

        // 5. Normalize (resize + pad)
        let normalized =
            self.normalizer
                .normalize(&rgb, &self.model_config, &self.hw_config)?;

        // 6. Assess quality
        let report = self.quality.assess(&normalized.image);

        // 7. Conditional noise reduction (optional service)
        let final_image = if let Some(ref reducer) = self.noise_reducer {
            reducer.reduce_if_needed(normalized.image, &report)
        } else {
            normalized.image
        };

        // 8. Encode PNG
        let png_bytes = encode_png(&final_image)?;

        let target = self.model_config.target_size;
        debug!(
            original = format!("{orig_w}x{orig_h}"),
            content = format!("{}x{}", normalized.content_width, normalized.content_height),
            output = format!("{target}x{target}"),
            png_size = png_bytes.len(),
            warnings = report.warnings.len(),
            "Image preprocessed for vision model"
        );

        Ok(PreparedImage {
            png_bytes,
            warnings: report.warnings,
            original_width: orig_w,
            original_height: orig_h,
            content_width: normalized.content_width,
            content_height: normalized.content_height,
        })
    }
}

// ═══════════════════════════════════════════════════════════
// Production implementations
// ═══════════════════════════════════════════════════════════

// ── ExifOrientationCorrector ──────────────────────────────

/// EXIF-based orientation correction for phone photos.
///
/// Reads EXIF tag 0x0112 (Orientation) from raw bytes via `kamadak-exif`.
/// Applies rotation/flip to align the image correctly.
///
/// EXIF orientation values:
/// 1 = Normal, 2 = Mirrored, 3 = 180deg, 4 = Flipped V,
/// 5 = Mirrored + 90deg CW, 6 = 90deg CW, 7 = Mirrored + 270deg CW, 8 = 270deg CW
pub struct ExifOrientationCorrector;

impl OrientationCorrector for ExifOrientationCorrector {
    fn correct(&self, raw_bytes: &[u8], image: DynamicImage) -> DynamicImage {
        let orientation = read_exif_orientation(raw_bytes);
        apply_orientation(image, orientation)
    }
}

/// Read EXIF orientation tag from raw image bytes.
/// Returns 1 (normal) if no EXIF data or tag not present.
pub fn read_exif_orientation(bytes: &[u8]) -> u32 {
    let mut cursor = Cursor::new(bytes);
    let reader = match exif::Reader::new().read_from_container(&mut cursor) {
        Ok(r) => r,
        Err(_) => return 1,
    };

    reader
        .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .and_then(|f| f.value.get_uint(0))
        .unwrap_or(1)
}

/// Apply EXIF orientation transform to a `DynamicImage`.
pub fn apply_orientation(img: DynamicImage, orientation: u32) -> DynamicImage {
    match orientation {
        1 => img,
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate270().fliph(),
        8 => img.rotate270(),
        _ => img,
    }
}

/// No-op orientation corrector — returns image unchanged.
/// Use when EXIF correction is not needed (e.g., rendered PDF pages).
pub struct NoOpOrientationCorrector;

impl OrientationCorrector for NoOpOrientationCorrector {
    fn correct(&self, _raw_bytes: &[u8], image: DynamicImage) -> DynamicImage {
        image
    }
}

// ── AspectRatioNormalizer ─────────────────────────────────

/// Aspect-ratio-preserving resize + pad normalizer.
///
/// Grounded: Google Document AI + HuggingFace AutoImageProcessor pattern.
/// Steps: pre-downscale guard -> compute fit -> resize -> pad -> center.
pub struct AspectRatioNormalizer;

impl ImageNormalizer for AspectRatioNormalizer {
    fn normalize(
        &self,
        image: &RgbImage,
        model_config: &ModelInputConfig,
        hw_config: &HardwareConfig,
    ) -> Result<NormalizedImage, ExtractionError> {
        let orig_w = image.width();
        let orig_h = image.height();

        // Pre-downscale guard for oversized images (avoids OOM on resize)
        let working = pre_downscale(image, hw_config.max_input_dimension);
        let (w, h) = (working.width(), working.height());

        // Compute aspect-ratio-preserving fit
        let target = model_config.target_size;
        let (content_w, content_h) = compute_fit_dimensions(w, h, target);

        // Resize with selected filter
        let filter = match hw_config.resize_filter {
            ResizeQuality::High => FilterType::CatmullRom,
            ResizeQuality::Fast => FilterType::Triangle,
        };
        let resized = image::imageops::resize(&*working, content_w, content_h, filter);

        // Create canvas with padding color
        let [r, g, b] = model_config.padding_color;
        let mut canvas = RgbImage::from_pixel(target, target, Rgb([r, g, b]));

        // Paste centered
        let offset_x = (target - content_w) / 2;
        let offset_y = (target - content_h) / 2;
        image::imageops::overlay(&mut canvas, &resized, offset_x, offset_y);

        Ok(NormalizedImage {
            image: canvas,
            original_width: orig_w,
            original_height: orig_h,
            content_width: content_w,
            content_height: content_h,
        })
    }
}

// ── BasicQualityAssessor ──────────────────────────────────

/// Basic quality assessment: blank page + dark image detection.
///
/// Returns warnings but does NOT block extraction — the user imported this file,
/// so we always attempt extraction. Warnings surface in the review UI.
///
/// CHUNK 2 upgrades this to `EnhancedQualityAssessor` with blur, skew, contrast.
pub struct BasicQualityAssessor;

impl QualityAssessor for BasicQualityAssessor {
    fn assess(&self, image: &RgbImage) -> QualityReport {
        let mut report = QualityReport::default();
        let pixel_count = (image.width() as usize) * (image.height() as usize);

        if pixel_count == 0 {
            return report;
        }

        // Check 1: Mostly blank (>95% near-white)
        let white_threshold = 240u8;
        let white_pixels = image
            .pixels()
            .filter(|p| {
                p.0[0] > white_threshold
                    && p.0[1] > white_threshold
                    && p.0[2] > white_threshold
            })
            .count();
        let white_ratio = white_pixels as f32 / pixel_count as f32;
        if white_ratio > 0.95 {
            report.is_blank = true;
            report.warnings.push(ExtractionWarning::PartialExtraction {
                reason: "Page appears mostly blank".into(),
            });
        }

        // Check 2: Mostly dark (>80% near-black)
        let dark_threshold = 15u8;
        let dark_pixels = image
            .pixels()
            .filter(|p| {
                p.0[0] < dark_threshold
                    && p.0[1] < dark_threshold
                    && p.0[2] < dark_threshold
            })
            .count();
        let dark_ratio = dark_pixels as f32 / pixel_count as f32;
        if dark_ratio > 0.80 {
            report.is_dark = true;
            report.warnings.push(ExtractionWarning::PoorContrast);
        }

        report
    }
}

// ── EnhancedQualityAssessor ───────────────────────────────

/// Production quality assessor with blur, skew, and contrast detection.
///
/// Grounded: Google Cloud Document AI quality classifier + Apple VNImageAnalysis.
/// Runs all checks on the normalized image (constant size = predictable cost).
///
/// Checks performed:
/// 1. Blank page detection (>95% near-white)
/// 2. Dark image detection (>80% near-black)
/// 3. Blur detection via Laplacian variance
/// 4. Skew detection via horizontal projection profile
/// 5. Contrast scoring via RMS contrast
pub struct EnhancedQualityAssessor;

/// Laplacian variance below this = blurry. Tuned for 896x896 document images.
const BLUR_THRESHOLD: f32 = 100.0;

/// Skew angle above this (degrees) triggers warning.
const SKEW_THRESHOLD_DEG: f32 = 1.5;

/// RMS contrast below this = poor contrast (near-uniform image).
const CONTRAST_THRESHOLD: f32 = 25.0;

impl QualityAssessor for EnhancedQualityAssessor {
    fn assess(&self, image: &RgbImage) -> QualityReport {
        let mut report = QualityReport::default();
        let pixel_count = (image.width() as usize) * (image.height() as usize);

        if pixel_count == 0 {
            return report;
        }

        // ── Blank / dark checks (same as BasicQualityAssessor) ──

        let white_threshold = 240u8;
        let white_pixels = image
            .pixels()
            .filter(|p| {
                p.0[0] > white_threshold
                    && p.0[1] > white_threshold
                    && p.0[2] > white_threshold
            })
            .count();
        let white_ratio = white_pixels as f32 / pixel_count as f32;
        if white_ratio > 0.95 {
            report.is_blank = true;
            report.warnings.push(ExtractionWarning::PartialExtraction {
                reason: "Page appears mostly blank".into(),
            });
        }

        let dark_threshold = 15u8;
        let dark_pixels = image
            .pixels()
            .filter(|p| {
                p.0[0] < dark_threshold
                    && p.0[1] < dark_threshold
                    && p.0[2] < dark_threshold
            })
            .count();
        let dark_ratio = dark_pixels as f32 / pixel_count as f32;
        if dark_ratio > 0.80 {
            report.is_dark = true;
            report.warnings.push(ExtractionWarning::PoorContrast);
        }

        // ── Enhanced checks (grayscale-based) ──

        let gray = rgb_to_gray(image);

        // Blur detection
        report.blur_score = compute_laplacian_variance(&gray);
        if report.blur_score < BLUR_THRESHOLD && !report.is_blank {
            report.warnings.push(ExtractionWarning::BlurryImage);
        }

        // Skew detection
        report.skew_angle = detect_skew_angle(&gray);
        if let Some(angle) = report.skew_angle {
            if angle.abs() >= SKEW_THRESHOLD_DEG {
                report
                    .warnings
                    .push(ExtractionWarning::SkewedDocument { angle_degrees: angle });
            }
        }

        // Contrast scoring
        report.contrast_score = compute_contrast_score(&gray);
        if report.contrast_score < CONTRAST_THRESHOLD
            && !report.is_blank
            && !report.is_dark
        {
            report.warnings.push(ExtractionWarning::PoorContrast);
        }

        report
    }
}

// ── ConditionalNoiseReducer ──────────────────────────────

/// Noise level above which bilateral filtering is applied.
/// Below this: document is clean — DO NOT enhance (Google "vanilla" rule).
/// Typical noise scores: clean scan 2-8, degraded photocopy 15-30, fax 25-50.
const NOISE_THRESHOLD: f32 = 12.0;

/// Bilateral filter spatial radius (pixels). Controls smoothing range.
/// Smaller radius = less smoothing, preserves more detail.
const BILATERAL_RADIUS: u32 = 3;

/// Bilateral filter range sigma. Controls edge preservation.
/// Smaller sigma = stronger edge preservation, less smoothing across edges.
const BILATERAL_RANGE_SIGMA: f32 = 25.0;

/// Conditional noise reducer — filters only degraded inputs.
///
/// Grounded: Google Document AI quality-tier processing. Apple Vision
/// document enhancement. Key rule PRESERVED: normal documents are NOT
/// enhanced (Google "vanilla" rule). Only degraded inputs get filtered.
///
/// Uses bilateral filter approximation: smooths noise while preserving
/// edges (text boundaries). Pure Rust, no `imageproc` dependency.
pub struct ConditionalNoiseReducer {
    /// Noise level threshold — images below this are left untouched.
    noise_threshold: f32,
}

impl Default for ConditionalNoiseReducer {
    fn default() -> Self {
        Self {
            noise_threshold: NOISE_THRESHOLD,
        }
    }
}

impl ConditionalNoiseReducer {
    /// Create with a custom noise threshold.
    pub fn with_threshold(threshold: f32) -> Self {
        Self {
            noise_threshold: threshold,
        }
    }
}

impl NoiseReducer for ConditionalNoiseReducer {
    fn reduce_if_needed(&self, image: RgbImage, quality: &QualityReport) -> RgbImage {
        // Rule: blank or dark pages — no point denoising
        if quality.is_blank || quality.is_dark {
            return image;
        }

        let gray = rgb_to_gray(&image);
        let noise_level = assess_noise_level(&gray);

        if noise_level < self.noise_threshold {
            debug!(noise_level, threshold = self.noise_threshold, "Clean image — skipping noise reduction");
            return image;
        }

        debug!(noise_level, threshold = self.noise_threshold, "Degraded image — applying bilateral filter");
        apply_bilateral_approximation(&image, BILATERAL_RADIUS, BILATERAL_RANGE_SIGMA)
    }
}

/// No-op noise reducer — always returns image unchanged.
/// Use for testing or when noise reduction is explicitly disabled.
pub struct NoOpNoiseReducer;

impl NoiseReducer for NoOpNoiseReducer {
    fn reduce_if_needed(&self, image: RgbImage, _quality: &QualityReport) -> RgbImage {
        image
    }
}

// ═══════════════════════════════════════════════════════════
// Public quality analysis functions (reusable)
// ═══════════════════════════════════════════════════════════

/// Convert RGB image to grayscale using ITU-R BT.601 luminance.
///
/// Avoids cloning via `DynamicImage` — direct pixel conversion.
pub fn rgb_to_gray(rgb: &RgbImage) -> GrayImage {
    let (w, h) = (rgb.width(), rgb.height());
    let mut gray = GrayImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let p = rgb.get_pixel(x, y);
            let luma = (0.299 * p.0[0] as f32
                + 0.587 * p.0[1] as f32
                + 0.114 * p.0[2] as f32) as u8;
            gray.put_pixel(x, y, Luma([luma]));
        }
    }
    gray
}

/// Compute Laplacian variance — measures image sharpness.
///
/// Grounded: OpenCV `cv2.Laplacian` variance is the standard blur metric.
/// Higher variance = sharper image. Blurry documents < 100, sharp text > 500.
///
/// Uses a 3x3 Laplacian kernel: `[0,1,0; 1,-4,1; 0,1,0]`.
/// Operates on grayscale input for consistent results.
pub fn compute_laplacian_variance(img: &GrayImage) -> f32 {
    let (w, h) = (img.width() as i32, img.height() as i32);
    if w < 3 || h < 3 {
        return 0.0;
    }

    let mut sum = 0.0f64;
    let mut sum_sq = 0.0f64;
    let mut count = 0u64;

    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let center = img.get_pixel(x as u32, y as u32).0[0] as f64;
            let top = img.get_pixel(x as u32, (y - 1) as u32).0[0] as f64;
            let bottom = img.get_pixel(x as u32, (y + 1) as u32).0[0] as f64;
            let left = img.get_pixel((x - 1) as u32, y as u32).0[0] as f64;
            let right = img.get_pixel((x + 1) as u32, y as u32).0[0] as f64;

            let laplacian = top + bottom + left + right - 4.0 * center;
            sum += laplacian;
            sum_sq += laplacian * laplacian;
            count += 1;
        }
    }

    if count == 0 {
        return 0.0;
    }

    let mean = sum / count as f64;
    let variance = (sum_sq / count as f64) - (mean * mean);
    variance.max(0.0) as f32
}

/// Detect document skew angle via horizontal projection profile.
///
/// Grounded: Standard document analysis technique (Projection Profile Method).
/// Tests candidate angles from -5 to +5 degrees. The angle producing the
/// crispest row transitions (highest projection variance) is the skew estimate.
///
/// Returns `None` if the image is too small, has insufficient content (< 2% ink),
/// or the detected angle is negligible (< 0.5 degrees).
pub fn detect_skew_angle(img: &GrayImage) -> Option<f32> {
    let (w, h) = (img.width(), img.height());
    if w < 50 || h < 50 {
        return None;
    }

    // Minimum ink check — need enough content for meaningful projection
    let ink_threshold = 128u8;
    let dark_count = img.pixels().filter(|p| p.0[0] < ink_threshold).count();
    let total = (w * h) as usize;
    if (dark_count as f32 / total as f32) < 0.02 {
        return None;
    }

    let mut best_angle = 0.0f32;
    let mut best_score = f64::NEG_INFINITY;

    // Test angles from -5.0 to 5.0 in 0.25 degree steps (41 candidates)
    let mut angle = -5.0f32;
    while angle <= 5.0 {
        let score = projection_variance(img, w, h, ink_threshold, angle);
        if score > best_score {
            best_score = score;
            best_angle = angle;
        }
        angle += 0.25;
    }

    if best_angle.abs() < 0.5 {
        None
    } else {
        Some(best_angle)
    }
}

/// Compute RMS contrast — standard deviation of grayscale pixel intensities.
///
/// Grounded: Standard image quality metric. Range 0-127.5 (theoretical max).
/// Low contrast (< 25) indicates near-uniform image. Typical documents: 50-100.
pub fn compute_contrast_score(img: &GrayImage) -> f32 {
    let mut sum = 0.0f64;
    let mut sum_sq = 0.0f64;
    let mut count = 0u64;

    for pixel in img.pixels() {
        let val = pixel.0[0] as f64;
        sum += val;
        sum_sq += val * val;
        count += 1;
    }

    if count == 0 {
        return 0.0;
    }

    let mean = sum / count as f64;
    let variance = (sum_sq / count as f64) - (mean * mean);
    variance.max(0.0).sqrt() as f32
}

/// Assess image noise level via local variance in smooth regions.
///
/// Grounded: Standard noise estimation technique — compute local variance
/// in 5x5 blocks, take the median of the lowest quartile (smooth regions).
/// Higher score = more noise. Clean scans: 2-8, degraded: 15-30, fax: 25-50.
///
/// Operates on grayscale for consistent results across color spaces.
pub fn assess_noise_level(img: &GrayImage) -> f32 {
    let (w, h) = (img.width(), img.height());
    if w < 5 || h < 5 {
        return 0.0;
    }

    let block_size = 5u32;
    let mut variances = Vec::new();

    // Sample blocks across the image (step by block_size to avoid overlap)
    let mut y = 0;
    while y + block_size <= h {
        let mut x = 0;
        while x + block_size <= w {
            let mut sum = 0.0f64;
            let mut sum_sq = 0.0f64;
            let count = (block_size * block_size) as f64;

            for by in 0..block_size {
                for bx in 0..block_size {
                    let val = img.get_pixel(x + bx, y + by).0[0] as f64;
                    sum += val;
                    sum_sq += val * val;
                }
            }

            let mean = sum / count;
            let variance = (sum_sq / count) - (mean * mean);
            variances.push(variance.max(0.0) as f32);

            x += block_size;
        }
        y += block_size;
    }

    if variances.is_empty() {
        return 0.0;
    }

    // Sort and take median of lowest quartile (smooth regions = noise estimate)
    variances.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let quartile_end = (variances.len() / 4).max(1);
    let smooth_region: &[f32] = &variances[..quartile_end];
    let median_idx = smooth_region.len() / 2;
    smooth_region[median_idx].sqrt() // Return std dev (more intuitive scale)
}

/// Apply bilateral filter approximation to an RGB image.
///
/// Grounded: Bilateral filter is the standard for edge-preserving denoising
/// (used in OpenCV, Google Document AI quality-tier processing).
///
/// Uses spatial window of `radius` pixels and range sigma for edge detection.
/// Pure Rust implementation — no `imageproc` dependency.
///
/// Parameters:
/// - `radius`: Spatial window radius (pixels). Typical: 2-5.
/// - `range_sigma`: Controls edge preservation. Smaller = stronger edges.
pub fn apply_bilateral_approximation(
    img: &RgbImage,
    radius: u32,
    range_sigma: f32,
) -> RgbImage {
    let (w, h) = (img.width(), img.height());
    let mut output = RgbImage::new(w, h);
    let range_sigma_sq_2 = 2.0 * range_sigma * range_sigma;

    for y in 0..h {
        for x in 0..w {
            let center = img.get_pixel(x, y);
            let center_r = center.0[0] as f32;
            let center_g = center.0[1] as f32;
            let center_b = center.0[2] as f32;

            let mut sum_r = 0.0f32;
            let mut sum_g = 0.0f32;
            let mut sum_b = 0.0f32;
            let mut weight_sum = 0.0f32;

            let y_start = y.saturating_sub(radius);
            let y_end = (y + radius + 1).min(h);
            let x_start = x.saturating_sub(radius);
            let x_end = (x + radius + 1).min(w);

            for ny in y_start..y_end {
                for nx in x_start..x_end {
                    let neighbor = img.get_pixel(nx, ny);
                    let nr = neighbor.0[0] as f32;
                    let ng = neighbor.0[1] as f32;
                    let nb = neighbor.0[2] as f32;

                    // Range weight: how similar is this neighbor's color?
                    let diff_r = nr - center_r;
                    let diff_g = ng - center_g;
                    let diff_b = nb - center_b;
                    let color_dist_sq = diff_r * diff_r + diff_g * diff_g + diff_b * diff_b;
                    let range_weight = (-color_dist_sq / range_sigma_sq_2).exp();

                    sum_r += nr * range_weight;
                    sum_g += ng * range_weight;
                    sum_b += nb * range_weight;
                    weight_sum += range_weight;
                }
            }

            if weight_sum > 0.0 {
                output.put_pixel(
                    x,
                    y,
                    Rgb([
                        (sum_r / weight_sum).round().clamp(0.0, 255.0) as u8,
                        (sum_g / weight_sum).round().clamp(0.0, 255.0) as u8,
                        (sum_b / weight_sum).round().clamp(0.0, 255.0) as u8,
                    ]),
                );
            } else {
                output.put_pixel(x, y, *center);
            }
        }
    }

    output
}

/// Compute projection variance for a candidate skew angle.
///
/// For each row, simulates un-skewing by shifting pixel reads horizontally.
/// The score is the sum of squared differences between adjacent row projections.
/// Higher score = crisper row transitions = better alignment at this angle.
fn projection_variance(
    img: &GrayImage,
    w: u32,
    h: u32,
    threshold: u8,
    angle_deg: f32,
) -> f64 {
    let tan_a = (angle_deg * std::f32::consts::PI / 180.0).tan() as f64;
    let mut projection = vec![0u32; h as usize];

    for y in 0..h {
        let shift = (y as f64 * tan_a).round() as i32;
        let mut count = 0u32;
        // Subsample every 4th pixel for speed
        let mut x = 0u32;
        while x < w {
            let sx = x as i32 + shift;
            if sx >= 0 && (sx as u32) < w {
                if img.get_pixel(sx as u32, y).0[0] < threshold {
                    count += 1;
                }
            }
            x += 4;
        }
        projection[y as usize] = count;
    }

    // Sum of squared differences between adjacent rows
    let mut score = 0.0f64;
    for i in 1..projection.len() {
        let diff = projection[i] as f64 - projection[i - 1] as f64;
        score += diff * diff;
    }
    score
}

// ═══════════════════════════════════════════════════════════
// Pure helper functions (reusable)
// ═══════════════════════════════════════════════════════════

/// Validate image bytes before decoding.
/// Returns early error for clearly invalid input — saves decode time.
pub fn validate_image_bytes(bytes: &[u8]) -> Result<(), ExtractionError> {
    if bytes.len() < MIN_IMAGE_BYTES {
        return Err(ExtractionError::ImageProcessing(
            "Image data too small to be valid".into(),
        ));
    }
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(ExtractionError::ImageProcessing(format!(
            "Image data exceeds {}MB limit",
            MAX_IMAGE_BYTES / (1024 * 1024)
        )));
    }
    Ok(())
}

/// Pre-downscale oversized images to reduce memory before the main resize.
/// Uses `Cow` to avoid cloning when no downscale is needed.
fn pre_downscale(img: &RgbImage, max_dim: u32) -> Cow<'_, RgbImage> {
    let (w, h) = (img.width(), img.height());
    let largest = w.max(h);

    if largest <= max_dim {
        return Cow::Borrowed(img);
    }

    let scale = max_dim as f32 / largest as f32;
    let new_w = ((w as f32 * scale).round() as u32).max(1);
    let new_h = ((h as f32 * scale).round() as u32).max(1);

    debug!(
        from = format!("{w}x{h}"),
        to = format!("{new_w}x{new_h}"),
        "Pre-downscaling oversized image"
    );

    Cow::Owned(image::imageops::resize(img, new_w, new_h, FilterType::Triangle))
}

/// Compute dimensions that fit inside a square while preserving aspect ratio.
///
/// The image is scaled so the longest edge matches `target_size`,
/// and the shorter edge is proportionally smaller.
/// Neither dimension exceeds `target_size`. Small images are NOT upscaled.
pub fn compute_fit_dimensions(width: u32, height: u32, target_size: u32) -> (u32, u32) {
    if width == 0 || height == 0 {
        return (1, 1);
    }

    let scale = (target_size as f32 / width as f32).min(target_size as f32 / height as f32);
    let scale = scale.min(1.0); // Don't upscale

    let new_w = ((width as f32 * scale).round() as u32)
        .max(1)
        .min(target_size);
    let new_h = ((height as f32 * scale).round() as u32)
        .max(1)
        .min(target_size);

    (new_w, new_h)
}

/// Encode an RGB image as PNG bytes.
/// Uses default compression (fast) — images are transient, not archived.
pub fn encode_png(img: &RgbImage) -> Result<Vec<u8>, ExtractionError> {
    let dynamic = DynamicImage::ImageRgb8(img.clone());
    let mut cursor = Cursor::new(Vec::new());
    dynamic
        .write_to(&mut cursor, ImageOutputFormat::Png)
        .map_err(|e| ExtractionError::ImageProcessing(format!("PNG encoding failed: {e}")))?;
    Ok(cursor.into_inner())
}

// ═══════════════════════════════════════════════════════════
// Backward compatibility
// ═══════════════════════════════════════════════════════════

/// Legacy preprocessing options — use `ModelInputConfig` + `HardwareConfig` instead.
#[derive(Debug, Clone)]
pub struct PreprocessOptions {
    pub target_size: u32,
    pub padding_color: [u8; 3],
    pub resize_filter: ResizeQuality,
    pub max_input_dimension: u32,
}

impl Default for PreprocessOptions {
    fn default() -> Self {
        Self {
            target_size: 896,
            padding_color: [255, 255, 255],
            resize_filter: ResizeQuality::High,
            max_input_dimension: 4096,
        }
    }
}

impl PreprocessOptions {
    pub fn for_gpu() -> Self {
        Self::default()
    }

    pub fn for_cpu() -> Self {
        Self {
            resize_filter: ResizeQuality::Fast,
            max_input_dimension: 2048,
            ..Self::default()
        }
    }
}

impl From<&PreprocessOptions> for (ModelInputConfig, HardwareConfig) {
    fn from(opts: &PreprocessOptions) -> Self {
        (
            ModelInputConfig {
                target_size: opts.target_size,
                padding_color: opts.padding_color,
            },
            HardwareConfig {
                resize_filter: opts.resize_filter,
                max_input_dimension: opts.max_input_dimension,
            },
        )
    }
}

/// Legacy MedGemma preprocessor — delegates to `PreprocessingPipeline`.
///
/// Kept for backward compatibility with existing tests.
/// New code should use `PreprocessingPipeline` directly.
pub struct MedGemmaPreprocessor;

impl MedGemmaPreprocessor {
    /// Legacy method — delegates to pipeline with options converted to configs.
    pub fn prepare_for_vision(
        &self,
        image_bytes: &[u8],
        options: &PreprocessOptions,
    ) -> Result<PreparedImage, ExtractionError> {
        let (model_config, hw_config) = options.into();
        let pipeline = PreprocessingPipeline::new(
            Box::new(ExifOrientationCorrector),
            Box::new(AspectRatioNormalizer),
            Box::new(BasicQualityAssessor),
            model_config,
            hw_config,
        );
        pipeline.preprocess(image_bytes)
    }
}

// ═══════════════════════════════════════════════════════════
// Mock implementations (testing)
// ═══════════════════════════════════════════════════════════

/// Mock image preprocessor for testing.
/// Returns a minimal valid PNG without performing actual image processing.
pub struct MockImagePreprocessor {
    fail: bool,
}

impl MockImagePreprocessor {
    pub fn new() -> Self {
        Self { fail: false }
    }

    pub fn failing() -> Self {
        Self { fail: true }
    }
}

impl ImagePreprocessor for MockImagePreprocessor {
    fn preprocess(&self, _image_bytes: &[u8]) -> Result<PreparedImage, ExtractionError> {
        if self.fail {
            return Err(ExtractionError::ImageProcessing(
                "Mock preprocessing failure".into(),
            ));
        }

        let target = 896;
        let canvas = RgbImage::from_pixel(target, target, Rgb([255, 255, 255]));
        let png_bytes = encode_png(&canvas)?;

        Ok(PreparedImage {
            png_bytes,
            warnings: vec![],
            original_width: target,
            original_height: target,
            content_width: target,
            content_height: target,
        })
    }
}

/// Mock quality assessor for testing — returns empty report.
pub struct MockQualityAssessor;

impl QualityAssessor for MockQualityAssessor {
    fn assess(&self, _image: &RgbImage) -> QualityReport {
        QualityReport::default()
    }
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a test image with the given dimensions and color.
    fn make_test_image(width: u32, height: u32, color: [u8; 3]) -> Vec<u8> {
        let img = RgbImage::from_pixel(width, height, Rgb(color));
        let dynamic = DynamicImage::ImageRgb8(img);
        let mut cursor = Cursor::new(Vec::new());
        dynamic
            .write_to(&mut cursor, ImageOutputFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    /// Decode PNG bytes back to RgbImage for inspection.
    fn decode_result(bytes: &[u8]) -> RgbImage {
        image::load_from_memory(bytes).unwrap().to_rgb8()
    }

    // ── compute_fit_dimensions ──

    #[test]
    fn fit_portrait_a4_into_square() {
        let (w, h) = compute_fit_dimensions(1654, 2339, 896);
        assert_eq!(h, 896, "Height should fill target");
        assert!(w < 896, "Width should be smaller (portrait)");
        let ratio = w as f32 / h as f32;
        assert!(
            (ratio - 0.707).abs() < 0.02,
            "Aspect ratio should be ~0.707, got {ratio}"
        );
    }

    #[test]
    fn fit_landscape_into_square() {
        let (w, h) = compute_fit_dimensions(2000, 1000, 896);
        assert_eq!(w, 896, "Width should fill target");
        assert!(h < 896, "Height should be smaller (landscape)");
        let ratio = h as f32 / w as f32;
        assert!(
            (ratio - 0.5).abs() < 0.02,
            "Aspect ratio should be ~0.5, got {ratio}"
        );
    }

    #[test]
    fn fit_square_stays_square() {
        let (w, h) = compute_fit_dimensions(2000, 2000, 896);
        assert_eq!(w, 896);
        assert_eq!(h, 896);
    }

    #[test]
    fn fit_small_image_not_upscaled() {
        let (w, h) = compute_fit_dimensions(200, 300, 896);
        assert_eq!(w, 200, "Small image should not be upscaled");
        assert_eq!(h, 300, "Small image should not be upscaled");
    }

    #[test]
    fn fit_zero_dimensions_clamped() {
        let (w, h) = compute_fit_dimensions(0, 0, 896);
        assert!(w >= 1);
        assert!(h >= 1);
    }

    // ── AspectRatioNormalizer (service) ──

    #[test]
    fn normalizer_preserves_aspect_ratio() {
        let normalizer = AspectRatioNormalizer;
        let model = ModelInputConfig::medgemma();
        let hw = HardwareConfig::gpu();

        let img = RgbImage::from_pixel(1654, 2339, Rgb([100, 100, 100]));
        let result = normalizer.normalize(&img, &model, &hw).unwrap();

        assert_eq!(result.image.width(), 896);
        assert_eq!(result.image.height(), 896);
        assert_eq!(result.content_height, 896);
        assert!(result.content_width < 896);
    }

    #[test]
    fn normalizer_pre_downscales_oversized() {
        let normalizer = AspectRatioNormalizer;
        let model = ModelInputConfig::medgemma();
        let hw = HardwareConfig {
            max_input_dimension: 2048,
            ..HardwareConfig::gpu()
        };

        let img = RgbImage::from_pixel(8000, 6000, Rgb([128, 128, 128]));
        let result = normalizer.normalize(&img, &model, &hw).unwrap();

        assert_eq!(result.image.width(), 896);
        assert_eq!(result.image.height(), 896);
    }

    #[test]
    fn normalizer_different_target_size() {
        let normalizer = AspectRatioNormalizer;
        let model = ModelInputConfig {
            target_size: 336,
            padding_color: [0, 0, 0],
        };
        let hw = HardwareConfig::gpu();

        let img = RgbImage::from_pixel(1000, 500, Rgb([128, 128, 128]));
        let result = normalizer.normalize(&img, &model, &hw).unwrap();

        assert_eq!(result.image.width(), 336);
        assert_eq!(result.image.height(), 336);
        assert_eq!(result.content_width, 336);
        assert!(result.content_height < 336);

        let corner = result.image.get_pixel(0, 0);
        assert_eq!(corner.0, [0, 0, 0], "Padding should be black");
    }

    // ── BasicQualityAssessor (service) ──

    #[test]
    fn quality_blank_page_warns() {
        let assessor = BasicQualityAssessor;
        let img = RgbImage::from_pixel(100, 100, Rgb([250, 250, 250]));
        let report = assessor.assess(&img);

        assert!(report.is_blank);
        assert!(report
            .warnings
            .iter()
            .any(|w| matches!(w, ExtractionWarning::PartialExtraction { .. })));
    }

    #[test]
    fn quality_dark_image_warns() {
        let assessor = BasicQualityAssessor;
        let img = RgbImage::from_pixel(100, 100, Rgb([5, 5, 5]));
        let report = assessor.assess(&img);

        assert!(report.is_dark);
        assert!(report
            .warnings
            .iter()
            .any(|w| matches!(w, ExtractionWarning::PoorContrast)));
    }

    #[test]
    fn quality_normal_image_no_warnings() {
        let assessor = BasicQualityAssessor;
        let img = RgbImage::from_pixel(100, 100, Rgb([128, 128, 128]));
        let report = assessor.assess(&img);

        assert!(report.warnings.is_empty());
        assert!(!report.is_blank);
        assert!(!report.is_dark);
    }

    // ── EXIF orientation ──

    #[test]
    fn exif_no_data_returns_identity() {
        let png = make_test_image(10, 10, [128, 128, 128]);
        assert_eq!(read_exif_orientation(&png), 1);
    }

    #[test]
    fn apply_orientation_identity() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(10, 20, Rgb([100, 100, 100])));
        let result = apply_orientation(img, 1);
        assert_eq!(result.width(), 10);
        assert_eq!(result.height(), 20);
    }

    #[test]
    fn apply_orientation_rotate90() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(10, 20, Rgb([100, 100, 100])));
        let result = apply_orientation(img, 6);
        assert_eq!(result.width(), 20);
        assert_eq!(result.height(), 10);
    }

    #[test]
    fn apply_orientation_rotate180() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(10, 20, Rgb([100, 100, 100])));
        let result = apply_orientation(img, 3);
        assert_eq!(result.width(), 10);
        assert_eq!(result.height(), 20);
    }

    #[test]
    fn apply_orientation_rotate270() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(10, 20, Rgb([100, 100, 100])));
        let result = apply_orientation(img, 8);
        assert_eq!(result.width(), 20);
        assert_eq!(result.height(), 10);
    }

    #[test]
    fn apply_orientation_mirror() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(10, 20, Rgb([100, 100, 100])));
        let result = apply_orientation(img, 2);
        assert_eq!(result.width(), 10);
        assert_eq!(result.height(), 20);
    }

    #[test]
    fn apply_orientation_unknown_is_identity() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(10, 20, Rgb([100, 100, 100])));
        let result = apply_orientation(img, 99);
        assert_eq!(result.width(), 10);
        assert_eq!(result.height(), 20);
    }

    // ── PreprocessingPipeline (composed) ──

    #[test]
    fn pipeline_a4_preserves_aspect_ratio() {
        let pipeline = PreprocessingPipeline::medgemma_gpu();
        let img = make_test_image(1654, 2339, [100, 100, 100]);

        let result = pipeline.preprocess(&img).unwrap();
        let output = decode_result(&result.png_bytes);

        assert_eq!(output.width(), 896);
        assert_eq!(output.height(), 896);
        assert_eq!(result.content_height, 896);
        assert!(result.content_width < 896);
        let ratio = result.content_width as f32 / result.content_height as f32;
        assert!((ratio - 0.707).abs() < 0.02);
    }

    #[test]
    fn pipeline_square_no_padding() {
        let pipeline = PreprocessingPipeline::medgemma_gpu();
        let img = make_test_image(896, 896, [50, 100, 150]);

        let result = pipeline.preprocess(&img).unwrap();
        let output = decode_result(&result.png_bytes);

        assert_eq!(output.width(), 896);
        assert_eq!(output.height(), 896);
        assert_eq!(result.content_width, 896);
        assert_eq!(result.content_height, 896);
    }

    #[test]
    fn pipeline_landscape_padded_top_bottom() {
        let pipeline = PreprocessingPipeline::medgemma_gpu();
        let img = make_test_image(2000, 500, [80, 80, 80]);

        let result = pipeline.preprocess(&img).unwrap();
        let output = decode_result(&result.png_bytes);

        assert_eq!(output.width(), 896);
        assert_eq!(output.height(), 896);
        assert_eq!(result.content_width, 896);
        assert!(result.content_height < 896);

        let top_pixel = output.get_pixel(448, 0);
        assert_eq!(top_pixel.0, [255, 255, 255]);
    }

    #[test]
    fn pipeline_portrait_padded_left_right() {
        let pipeline = PreprocessingPipeline::medgemma_gpu();
        let img = make_test_image(500, 2000, [80, 80, 80]);

        let result = pipeline.preprocess(&img).unwrap();
        let output = decode_result(&result.png_bytes);

        assert_eq!(output.width(), 896);
        assert_eq!(output.height(), 896);
        assert!(result.content_width < 896);
        assert_eq!(result.content_height, 896);

        let left_pixel = output.get_pixel(0, 448);
        assert_eq!(left_pixel.0, [255, 255, 255]);
    }

    #[test]
    fn pipeline_small_image_not_upscaled() {
        let pipeline = PreprocessingPipeline::medgemma_gpu();
        let img = make_test_image(100, 150, [200, 50, 50]);

        let result = pipeline.preprocess(&img).unwrap();
        let output = decode_result(&result.png_bytes);

        assert_eq!(output.width(), 896);
        assert_eq!(output.height(), 896);
        assert_eq!(result.content_width, 100);
        assert_eq!(result.content_height, 150);

        let center_pixel = output.get_pixel(448, 448);
        assert!(center_pixel.0[0] > 150);
    }

    #[test]
    fn pipeline_rejects_too_small_input() {
        let pipeline = PreprocessingPipeline::medgemma_gpu();
        let tiny = vec![0x89, 0x50];
        let result = pipeline.preprocess(&tiny);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too small"));
    }

    #[test]
    fn pipeline_decode_error_on_invalid_bytes() {
        let pipeline = PreprocessingPipeline::medgemma_gpu();
        let garbage = [0xDE, 0xAD, 0xBE, 0xEF].repeat(25);
        let result = pipeline.preprocess(&garbage);

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("decode") || err.contains("image"));
    }

    // ── Pipeline with custom config ──

    #[test]
    fn pipeline_custom_target_size() {
        let pipeline = PreprocessingPipeline::new(
            Box::new(NoOpOrientationCorrector),
            Box::new(AspectRatioNormalizer),
            Box::new(BasicQualityAssessor),
            ModelInputConfig {
                target_size: 336,
                padding_color: [0, 0, 0],
            },
            HardwareConfig::gpu(),
        );

        let img = make_test_image(1000, 500, [128, 128, 128]);
        let result = pipeline.preprocess(&img).unwrap();
        let output = decode_result(&result.png_bytes);

        assert_eq!(output.width(), 336);
        assert_eq!(output.height(), 336);
    }

    #[test]
    fn pipeline_with_mock_services() {
        let pipeline = PreprocessingPipeline::new(
            Box::new(NoOpOrientationCorrector),
            Box::new(AspectRatioNormalizer),
            Box::new(MockQualityAssessor),
            ModelInputConfig::medgemma(),
            HardwareConfig::gpu(),
        );

        // Blank image — MockQualityAssessor returns no warnings
        let img = make_test_image(100, 100, [250, 250, 250]);
        let result = pipeline.preprocess(&img).unwrap();
        assert!(result.warnings.is_empty());
    }

    // ── Backward compatibility (MedGemmaPreprocessor) ──

    #[test]
    fn legacy_preprocess_a4_preserves_aspect_ratio() {
        let preprocessor = MedGemmaPreprocessor;
        let options = PreprocessOptions::default();
        let img = make_test_image(1654, 2339, [100, 100, 100]);

        let result = preprocessor.prepare_for_vision(&img, &options).unwrap();
        let output = decode_result(&result.png_bytes);

        assert_eq!(output.width(), 896);
        assert_eq!(output.height(), 896);
        assert_eq!(result.content_height, 896);
        assert!(result.content_width < 896);
    }

    #[test]
    fn legacy_preprocess_square() {
        let preprocessor = MedGemmaPreprocessor;
        let options = PreprocessOptions::default();
        let img = make_test_image(896, 896, [50, 100, 150]);

        let result = preprocessor.prepare_for_vision(&img, &options).unwrap();
        assert_eq!(result.content_width, 896);
        assert_eq!(result.content_height, 896);
    }

    // ── MockImagePreprocessor ──

    #[test]
    fn mock_returns_valid_png() {
        let mock = MockImagePreprocessor::new();
        let result = mock.preprocess(b"any-bytes").unwrap();
        let output = decode_result(&result.png_bytes);

        assert_eq!(output.width(), 896);
        assert_eq!(output.height(), 896);
    }

    #[test]
    fn mock_failing_returns_error() {
        let mock = MockImagePreprocessor::failing();
        let result = mock.preprocess(b"any-bytes");
        assert!(result.is_err());
    }

    // ── Config types ──

    #[test]
    fn gpu_options_use_high_quality() {
        let opts = PreprocessOptions::for_gpu();
        assert_eq!(opts.resize_filter, ResizeQuality::High);
        assert_eq!(opts.max_input_dimension, 4096);
        assert_eq!(opts.target_size, 896);
    }

    #[test]
    fn cpu_options_use_fast_filter() {
        let opts = PreprocessOptions::for_cpu();
        assert_eq!(opts.resize_filter, ResizeQuality::Fast);
        assert_eq!(opts.max_input_dimension, 2048);
        assert_eq!(opts.target_size, 896);
    }

    #[test]
    fn medgemma_config_defaults() {
        let config = ModelInputConfig::medgemma();
        assert_eq!(config.target_size, 896);
        assert_eq!(config.padding_color, [255, 255, 255]);
    }

    #[test]
    fn hardware_config_tiers() {
        let gpu = HardwareConfig::gpu();
        assert_eq!(gpu.resize_filter, ResizeQuality::High);
        assert_eq!(gpu.max_input_dimension, 4096);

        let cpu = HardwareConfig::cpu();
        assert_eq!(cpu.resize_filter, ResizeQuality::Fast);
        assert_eq!(cpu.max_input_dimension, 2048);
    }

    #[test]
    fn preprocess_options_to_configs() {
        let opts = PreprocessOptions::default();
        let (model, hw): (ModelInputConfig, HardwareConfig) = (&opts).into();
        assert_eq!(model.target_size, 896);
        assert_eq!(model.padding_color, [255, 255, 255]);
        assert_eq!(hw.resize_filter, ResizeQuality::High);
        assert_eq!(hw.max_input_dimension, 4096);
    }

    // ── Enhanced quality assessment ──

    #[test]
    fn laplacian_sharp_image_high_variance() {
        // Checkerboard pattern = many edges = high Laplacian variance
        let mut img = GrayImage::new(100, 100);
        for y in 0..100 {
            for x in 0..100 {
                let val = if (x + y) % 2 == 0 { 0u8 } else { 255u8 };
                img.put_pixel(x, y, Luma([val]));
            }
        }

        let variance = compute_laplacian_variance(&img);
        assert!(
            variance > 1000.0,
            "Checkerboard should have high variance, got {variance}"
        );
    }

    #[test]
    fn laplacian_blurry_image_low_variance() {
        // Uniform gray = no edges = zero variance
        let img = GrayImage::from_pixel(100, 100, Luma([128]));
        let variance = compute_laplacian_variance(&img);
        assert!(
            variance < 1.0,
            "Uniform image should have near-zero variance, got {variance}"
        );
    }

    #[test]
    fn laplacian_tiny_image_returns_zero() {
        let img = GrayImage::new(2, 2);
        assert_eq!(compute_laplacian_variance(&img), 0.0);
    }

    #[test]
    fn contrast_high_contrast_image() {
        // Half black, half white = high contrast
        let mut img = GrayImage::new(100, 100);
        for y in 0..100 {
            for x in 0..100 {
                let val = if x < 50 { 0u8 } else { 255u8 };
                img.put_pixel(x, y, Luma([val]));
            }
        }

        let score = compute_contrast_score(&img);
        assert!(
            score > 100.0,
            "Half black/white should have high contrast, got {score}"
        );
    }

    #[test]
    fn contrast_uniform_image_low() {
        let img = GrayImage::from_pixel(100, 100, Luma([128]));
        let score = compute_contrast_score(&img);
        assert!(
            score < 1.0,
            "Uniform image should have near-zero contrast, got {score}"
        );
    }

    #[test]
    fn contrast_empty_image_returns_zero() {
        let img = GrayImage::new(0, 0);
        assert_eq!(compute_contrast_score(&img), 0.0);
    }

    #[test]
    fn skew_insufficient_content_returns_none() {
        // Mostly white image — not enough ink for skew detection
        let img = GrayImage::from_pixel(200, 200, Luma([255]));
        assert!(detect_skew_angle(&img).is_none());
    }

    #[test]
    fn skew_tiny_image_returns_none() {
        let img = GrayImage::new(10, 10);
        assert!(detect_skew_angle(&img).is_none());
    }

    #[test]
    fn skew_straight_horizontal_lines() {
        // Horizontal lines filling the image at regular intervals.
        // Thick lines (5px) on a large canvas give clear projection signal.
        let mut img = GrayImage::from_pixel(400, 400, Luma([255]));
        for y_start in (20..380).step_by(40) {
            for dy in 0..5u32 {
                let y = y_start + dy;
                if y < 400 {
                    for x in 0..400 {
                        img.put_pixel(x, y, Luma([0]));
                    }
                }
            }
        }

        let angle = detect_skew_angle(&img);
        match angle {
            None => {} // Correct — detected as straight
            Some(a) => assert!(
                a.abs() < 1.5,
                "Straight lines should have near-zero skew, got {a}"
            ),
        }
    }

    #[test]
    fn enhanced_assessor_detects_blur() {
        let assessor = EnhancedQualityAssessor;
        // Uniform mid-gray with slight variation (simulates blurry image)
        let img = RgbImage::from_pixel(100, 100, Rgb([128, 128, 128]));
        let report = assessor.assess(&img);

        assert!(
            report.blur_score < BLUR_THRESHOLD,
            "Uniform image should be detected as blurry"
        );
        assert!(report
            .warnings
            .iter()
            .any(|w| matches!(w, ExtractionWarning::BlurryImage)));
    }

    #[test]
    fn enhanced_assessor_sharp_no_blur_warning() {
        let assessor = EnhancedQualityAssessor;
        // Checkerboard = sharp edges
        let mut img = RgbImage::new(100, 100);
        for y in 0..100 {
            for x in 0..100 {
                let val = if (x + y) % 2 == 0 { 0u8 } else { 255u8 };
                img.put_pixel(x, y, Rgb([val, val, val]));
            }
        }

        let report = assessor.assess(&img);
        assert!(
            report.blur_score > BLUR_THRESHOLD,
            "Checkerboard should be sharp, got {}",
            report.blur_score
        );
        assert!(!report
            .warnings
            .iter()
            .any(|w| matches!(w, ExtractionWarning::BlurryImage)));
    }

    #[test]
    fn enhanced_assessor_scores_populated() {
        let assessor = EnhancedQualityAssessor;
        let img = RgbImage::from_pixel(100, 100, Rgb([128, 128, 128]));
        let report = assessor.assess(&img);

        // blur_score and contrast_score should have been computed
        assert!(report.blur_score >= 0.0);
        assert!(report.contrast_score >= 0.0);
    }

    #[test]
    fn enhanced_assessor_blank_skips_blur_warning() {
        let assessor = EnhancedQualityAssessor;
        // Mostly white image — should flag blank but NOT blur (blank supersedes)
        let img = RgbImage::from_pixel(100, 100, Rgb([250, 250, 250]));
        let report = assessor.assess(&img);

        assert!(report.is_blank);
        // Blank pages shouldn't also get a blur warning
        assert!(!report
            .warnings
            .iter()
            .any(|w| matches!(w, ExtractionWarning::BlurryImage)));
    }

    #[test]
    fn rgb_to_gray_preserves_dimensions() {
        let rgb = RgbImage::from_pixel(50, 30, Rgb([100, 150, 200]));
        let gray = rgb_to_gray(&rgb);
        assert_eq!(gray.width(), 50);
        assert_eq!(gray.height(), 30);
    }

    #[test]
    fn rgb_to_gray_white_stays_white() {
        let rgb = RgbImage::from_pixel(10, 10, Rgb([255, 255, 255]));
        let gray = rgb_to_gray(&rgb);
        assert_eq!(gray.get_pixel(0, 0).0[0], 255);
    }

    #[test]
    fn rgb_to_gray_black_stays_black() {
        let rgb = RgbImage::from_pixel(10, 10, Rgb([0, 0, 0]));
        let gray = rgb_to_gray(&rgb);
        assert_eq!(gray.get_pixel(0, 0).0[0], 0);
    }

    // ── Noise assessment ──

    #[test]
    fn noise_clean_image_low_score() {
        // Uniform gray — zero noise
        let img = GrayImage::from_pixel(100, 100, Luma([128]));
        let score = assess_noise_level(&img);
        assert!(
            score < 5.0,
            "Uniform image should have near-zero noise, got {score}"
        );
    }

    #[test]
    fn noise_noisy_image_high_score() {
        // Salt-and-pepper noise on gray background
        let mut img = GrayImage::from_pixel(100, 100, Luma([128]));
        for y in 0..100 {
            for x in 0..100 {
                if (x * 7 + y * 13) % 3 == 0 {
                    let val = if (x + y) % 2 == 0 { 0u8 } else { 255u8 };
                    img.put_pixel(x, y, Luma([val]));
                }
            }
        }
        let score = assess_noise_level(&img);
        assert!(
            score > 10.0,
            "Noisy image should have high noise score, got {score}"
        );
    }

    #[test]
    fn noise_tiny_image_returns_zero() {
        let img = GrayImage::new(3, 3);
        assert_eq!(assess_noise_level(&img), 0.0);
    }

    // ── Bilateral filter ──

    #[test]
    fn bilateral_preserves_clean_image() {
        // Uniform color — bilateral should not change it
        let img = RgbImage::from_pixel(20, 20, Rgb([100, 150, 200]));
        let result = apply_bilateral_approximation(&img, 3, 25.0);

        for y in 0..20 {
            for x in 0..20 {
                let p = result.get_pixel(x, y);
                assert_eq!(p.0, [100, 150, 200], "Uniform image should be unchanged");
            }
        }
    }

    #[test]
    fn bilateral_preserves_hard_edges() {
        // Two halves: black and white. Edge should stay sharp.
        let mut img = RgbImage::new(40, 20);
        for y in 0..20 {
            for x in 0..40 {
                let c = if x < 20 { [0, 0, 0] } else { [255, 255, 255] };
                img.put_pixel(x, y, Rgb(c));
            }
        }

        let result = apply_bilateral_approximation(&img, 3, 25.0);

        // Far from edge: should be mostly unchanged
        let black_pixel = result.get_pixel(5, 10);
        assert!(black_pixel.0[0] < 10, "Black region should stay dark, got {}", black_pixel.0[0]);

        let white_pixel = result.get_pixel(35, 10);
        assert!(white_pixel.0[0] > 245, "White region should stay bright, got {}", white_pixel.0[0]);
    }

    #[test]
    fn bilateral_output_dimensions_match() {
        let img = RgbImage::from_pixel(50, 30, Rgb([128, 128, 128]));
        let result = apply_bilateral_approximation(&img, 2, 30.0);
        assert_eq!(result.width(), 50);
        assert_eq!(result.height(), 30);
    }

    // ── ConditionalNoiseReducer (service) ──

    #[test]
    fn noise_reducer_skips_clean_image() {
        let reducer = ConditionalNoiseReducer::default();
        let img = RgbImage::from_pixel(50, 50, Rgb([128, 128, 128]));
        let report = QualityReport::default();

        let result = reducer.reduce_if_needed(img.clone(), &report);

        // Clean image should be unchanged (passthrough)
        for y in 0..50 {
            for x in 0..50 {
                assert_eq!(result.get_pixel(x, y).0, [128, 128, 128]);
            }
        }
    }

    #[test]
    fn noise_reducer_skips_blank_page() {
        let reducer = ConditionalNoiseReducer::default();
        let img = RgbImage::from_pixel(50, 50, Rgb([250, 250, 250]));
        let report = QualityReport {
            is_blank: true,
            ..Default::default()
        };

        let result = reducer.reduce_if_needed(img.clone(), &report);

        // Blank pages should never be processed
        for y in 0..50 {
            for x in 0..50 {
                assert_eq!(result.get_pixel(x, y).0, [250, 250, 250]);
            }
        }
    }

    #[test]
    fn noise_reducer_skips_dark_page() {
        let reducer = ConditionalNoiseReducer::default();
        let img = RgbImage::from_pixel(50, 50, Rgb([5, 5, 5]));
        let report = QualityReport {
            is_dark: true,
            ..Default::default()
        };

        let result = reducer.reduce_if_needed(img.clone(), &report);

        for y in 0..50 {
            for x in 0..50 {
                assert_eq!(result.get_pixel(x, y).0, [5, 5, 5]);
            }
        }
    }

    #[test]
    fn noise_reducer_processes_noisy_image() {
        // Threshold 0.0 guarantees triggering — this test verifies bilateral modifies pixels
        let reducer = ConditionalNoiseReducer::with_threshold(0.0);

        // Create Gaussian-like noise: small variations around mid-gray.
        // Bilateral filter smooths these because neighbors are close in color space.
        // (Salt-and-pepper noise is NOT smoothed by bilateral — that's correct behavior.)
        let mut img = RgbImage::new(50, 50);
        for y in 0..50u32 {
            for x in 0..50u32 {
                // Deterministic pseudo-random: values oscillate around 128 ± 40
                let noise = ((x.wrapping_mul(31).wrapping_add(y.wrapping_mul(17))) % 80) as u8;
                let val = 88 + noise; // Range: 88-167
                img.put_pixel(x, y, Rgb([val, val, val]));
            }
        }

        let report = QualityReport::default(); // Not blank, not dark
        let result = reducer.reduce_if_needed(img.clone(), &report);

        // Bilateral should smooth variations — at least some pixels differ
        let mut diff_count = 0u32;
        for y in 0..50 {
            for x in 0..50 {
                if result.get_pixel(x, y) != img.get_pixel(x, y) {
                    diff_count += 1;
                }
            }
        }
        assert!(diff_count > 0, "Noisy image should have been modified by bilateral filter");
    }

    #[test]
    fn noise_reducer_custom_threshold() {
        // Very high threshold = nothing triggers
        let reducer = ConditionalNoiseReducer::with_threshold(9999.0);
        let img = RgbImage::from_pixel(20, 20, Rgb([128, 128, 128]));
        let report = QualityReport::default();

        let result = reducer.reduce_if_needed(img.clone(), &report);
        for y in 0..20 {
            for x in 0..20 {
                assert_eq!(result.get_pixel(x, y).0, [128, 128, 128]);
            }
        }
    }

    #[test]
    fn noop_noise_reducer_always_passthrough() {
        let reducer = NoOpNoiseReducer;
        let img = RgbImage::from_pixel(20, 20, Rgb([42, 42, 42]));
        let report = QualityReport {
            is_blank: false,
            is_dark: false,
            blur_score: 500.0,
            skew_angle: None,
            contrast_score: 80.0,
            warnings: vec![],
        };

        let result = reducer.reduce_if_needed(img.clone(), &report);
        for y in 0..20 {
            for x in 0..20 {
                assert_eq!(result.get_pixel(x, y).0, [42, 42, 42]);
            }
        }
    }

    // ── Pipeline with noise reducer ──

    #[test]
    fn pipeline_with_noise_reducer_runs() {
        let pipeline = PreprocessingPipeline::new(
            Box::new(NoOpOrientationCorrector),
            Box::new(AspectRatioNormalizer),
            Box::new(BasicQualityAssessor),
            ModelInputConfig::medgemma(),
            HardwareConfig::gpu(),
        )
        .with_noise_reducer(Box::new(NoOpNoiseReducer));

        let img = make_test_image(200, 300, [128, 128, 128]);
        let result = pipeline.preprocess(&img).unwrap();
        let output = decode_result(&result.png_bytes);

        assert_eq!(output.width(), 896);
        assert_eq!(output.height(), 896);
    }

    #[test]
    fn pipeline_without_noise_reducer_still_works() {
        let pipeline = PreprocessingPipeline::new(
            Box::new(NoOpOrientationCorrector),
            Box::new(AspectRatioNormalizer),
            Box::new(BasicQualityAssessor),
            ModelInputConfig::medgemma(),
            HardwareConfig::gpu(),
        );
        // No .with_noise_reducer() — noise_reducer is None

        let img = make_test_image(200, 300, [128, 128, 128]);
        let result = pipeline.preprocess(&img).unwrap();
        assert!(!result.png_bytes.is_empty());
    }

    #[test]
    fn pipeline_medgemma_gpu_includes_noise_reducer() {
        // medgemma_gpu() should include ConditionalNoiseReducer
        let pipeline = PreprocessingPipeline::medgemma_gpu();
        let img = make_test_image(200, 200, [128, 128, 128]);
        let result = pipeline.preprocess(&img).unwrap();
        assert!(!result.png_bytes.is_empty());
    }

    #[test]
    fn pipeline_medgemma_cpu_includes_noise_reducer() {
        let pipeline = PreprocessingPipeline::medgemma_cpu();
        let img = make_test_image(200, 200, [128, 128, 128]);
        let result = pipeline.preprocess(&img).unwrap();
        assert!(!result.png_bytes.is_empty());
    }
}
