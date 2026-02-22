//! R3: PDF page rendering via Google PDFium.
//!
//! Renders individual PDF pages to PNG images for vision model OCR.
//! Replaces LopdfImageExtractor with high-fidelity rendering that handles
//! CIDFonts, embedded fonts, form fields, and complex layouts.
//!
//! `PdfiumRenderer` is stateless (`Send + Sync`). Each operation creates
//! a fresh `Pdfium` instance because the upstream type is `!Send`.
//! The OS caches `dlopen`/`LoadLibrary` calls, so repeat loads are near-free.

use std::io::Cursor;

use image::ImageOutputFormat;
use pdfium_render::prelude::*;
use tracing::{debug, warn};

use super::types::PdfPageRenderer;
use super::ExtractionError;

/// Maximum dimension (width or height) for rendered page images.
/// Prevents OOM on extremely large pages or absurd DPI settings.
const MAX_DIMENSION_PX: u32 = 4096;

/// Default rendering DPI for vision model OCR.
/// 200 DPI balances quality and inference speed (vs 300 DPI for old Tesseract).
pub const DEFAULT_RENDER_DPI: u32 = 200;

/// PDF points per inch (standard PDF unit).
const POINTS_PER_INCH: f32 = 72.0;

/// Renders PDF pages to PNG images using Google PDFium.
///
/// PDFium handles all PDF complexities: CIDFont encodings, embedded fonts,
/// form fields, transparency, layers — unlike pdf-extract + lopdf.
///
/// Stateless: the `Pdfium` library handle is loaded per-operation because
/// the upstream `Pdfium` type is `!Send + !Sync`. OS-level library caching
/// (dlopen/LoadLibrary) makes repeat loads effectively free.
pub struct PdfiumRenderer;

impl PdfiumRenderer {
    /// Create a new renderer, verifying the PDFium library is loadable.
    ///
    /// Discovery order:
    /// 1. `PDFIUM_DYNAMIC_LIB_PATH` env var (explicit path to library file)
    /// 2. Alongside the running executable
    /// 3. System library search paths
    pub fn new() -> Result<Self, ExtractionError> {
        // Verify library is loadable at construction time (fail-fast).
        let _ = load_pdfium()?;
        Ok(Self)
    }
}

/// Load the PDFium dynamic library.
///
/// Discovery order:
/// 1. `PDFIUM_DYNAMIC_LIB_PATH` env var (explicit path)
/// 2. Alongside the running executable
/// 3. Tauri bundled resources — platform-specific:
///    - Windows/Linux: `<exe_dir>/resources/pdfium/{bin,lib}/`
///    - macOS .app:    `<exe_dir>/../Resources/pdfium/{bin,lib}/`
///    - Linux alt:     `<exe_dir>/../resources/pdfium/{bin,lib}/`
/// 4. System library search paths
fn load_pdfium() -> Result<Pdfium, ExtractionError> {
    // 1. Explicit path via env var
    if let Ok(path) = std::env::var("PDFIUM_DYNAMIC_LIB_PATH") {
        debug!(path = %path, "Loading PDFium from env var");
        let bindings = Pdfium::bind_to_library(&path).map_err(|e| {
            ExtractionError::PdfRendering {
                page: 0,
                reason: format!("Failed to load PDFium from {path}: {e}"),
            }
        })?;
        return Ok(Pdfium::new(bindings));
    }

    // 2 + 3. Search candidate directories relative to executable.
    // pdfium_platform_library_name_at_path() handles platform-specific names:
    //   Windows → pdfium.dll | Linux → libpdfium.so | macOS → libpdfium.dylib
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let candidates = [
                // Alongside executable (dev / portable)
                exe_dir.to_path_buf(),
                // Tauri resources — Windows & Linux
                exe_dir.join("resources").join("pdfium").join("bin"),
                exe_dir.join("resources").join("pdfium").join("lib"),
                // Tauri resources — macOS .app bundle (exe in Contents/MacOS/)
                exe_dir.join("..").join("Resources").join("pdfium").join("bin"),
                exe_dir.join("..").join("Resources").join("pdfium").join("lib"),
                // Linux alternative layout
                exe_dir.join("..").join("resources").join("pdfium").join("bin"),
                exe_dir.join("..").join("resources").join("pdfium").join("lib"),
            ];

            for dir in &candidates {
                let lib_path = Pdfium::pdfium_platform_library_name_at_path(
                    dir.to_string_lossy().as_ref(),
                );
                if let Ok(bindings) = Pdfium::bind_to_library(&lib_path) {
                    debug!(dir = %dir.display(), "Loaded PDFium from candidate directory");
                    return Ok(Pdfium::new(bindings));
                }
            }
        }
    }

    // 4. System library
    let bindings =
        Pdfium::bind_to_system_library().map_err(|e| ExtractionError::PdfRendering {
            page: 0,
            reason: format!(
                "PDFium library not found. Set PDFIUM_DYNAMIC_LIB_PATH or install PDFium: {e}"
            ),
        })?;
    Ok(Pdfium::new(bindings))
}

/// Map PDF load errors — detect encrypted PDFs for user-friendly messaging.
fn map_load_error(e: PdfiumError) -> ExtractionError {
    let msg = format!("{e}");
    let lower = msg.to_lowercase();
    if lower.contains("password") || lower.contains("encrypt") {
        ExtractionError::PdfEncrypted
    } else {
        ExtractionError::PdfRendering {
            page: 0,
            reason: format!("Failed to load PDF: {e}"),
        }
    }
}

/// Compute pixel dimensions for rendering, applying the dimension guard.
///
/// Returns (width_px, height_px), both clamped to [1, MAX_DIMENSION_PX].
/// Preserves aspect ratio when capping.
fn compute_render_dimensions(width_points: f32, height_points: f32, dpi: u32) -> (u32, u32) {
    let scale = dpi as f32 / POINTS_PER_INCH;
    let raw_w = (width_points * scale).max(1.0);
    let raw_h = (height_points * scale).max(1.0);

    let max_dim = raw_w.max(raw_h);
    if max_dim > MAX_DIMENSION_PX as f32 {
        let ratio = MAX_DIMENSION_PX as f32 / max_dim;
        let w = ((raw_w * ratio) as u32).max(1).min(MAX_DIMENSION_PX);
        let h = ((raw_h * ratio) as u32).max(1).min(MAX_DIMENSION_PX);
        (w, h)
    } else {
        (raw_w as u32, raw_h as u32)
    }
}

impl PdfPageRenderer for PdfiumRenderer {
    fn page_count(&self, pdf_bytes: &[u8]) -> Result<usize, ExtractionError> {
        let pdfium = load_pdfium()?;
        let document = pdfium
            .load_pdf_from_byte_slice(pdf_bytes, None)
            .map_err(map_load_error)?;
        Ok(document.pages().len() as usize)
    }

    fn render_page(
        &self,
        pdf_bytes: &[u8],
        page_number: usize,
        dpi: u32,
    ) -> Result<Vec<u8>, ExtractionError> {
        let pdfium = load_pdfium()?;
        let document = pdfium
            .load_pdf_from_byte_slice(pdf_bytes, None)
            .map_err(map_load_error)?;

        let pages = document.pages();

        let page_index = u16::try_from(page_number).map_err(|_| ExtractionError::PdfRendering {
            page: page_number,
            reason: format!("Page index {page_number} exceeds u16 maximum"),
        })?;

        let page = pages
            .get(page_index)
            .map_err(|_| ExtractionError::PdfRendering {
                page: page_number,
                reason: format!(
                    "Page {page_number} out of range (document has {} pages)",
                    pages.len()
                ),
            })?;

        let width_points = page.width().value;
        let height_points = page.height().value;
        let (target_w, target_h) = compute_render_dimensions(width_points, height_points, dpi);

        let uncapped_w = (width_points * dpi as f32 / POINTS_PER_INCH) as u32;
        let uncapped_h = (height_points * dpi as f32 / POINTS_PER_INCH) as u32;
        if target_w != uncapped_w || target_h != uncapped_h {
            warn!(
                page = page_number,
                raw_width = uncapped_w,
                raw_height = uncapped_h,
                capped_width = target_w,
                capped_height = target_h,
                "Page dimensions capped to {MAX_DIMENSION_PX}px",
            );
        }

        let config = PdfRenderConfig::new()
            .set_target_width(target_w as i32)
            .set_maximum_height(target_h as i32);

        let bitmap = page
            .render_with_config(&config)
            .map_err(|e| ExtractionError::PdfRendering {
                page: page_number,
                reason: format!("Rendering failed: {e}"),
            })?;

        let dynamic_image = bitmap.as_image();
        let mut cursor = Cursor::new(Vec::new());
        dynamic_image
            .write_to(
                &mut cursor,
                ImageOutputFormat::Png,
            )
            .map_err(|e| ExtractionError::ImageProcessing(format!("PNG encoding failed: {e}")))?;

        let png_bytes = cursor.into_inner();

        debug!(
            page = page_number,
            width = target_w,
            height = target_h,
            png_size = png_bytes.len(),
            "Rendered PDF page to PNG"
        );

        Ok(png_bytes)
    }
}

// ── Mock for testing ──────────────────────────────────────

/// Mock PDF page renderer returning a minimal PNG for each valid page.
///
/// Used by orchestrator and processor tests that need a PdfPageRenderer
/// without requiring the actual PDFium binary.
pub struct MockPdfPageRenderer {
    page_count: usize,
}

impl MockPdfPageRenderer {
    pub fn new(page_count: usize) -> Self {
        Self { page_count }
    }
}

impl PdfPageRenderer for MockPdfPageRenderer {
    fn page_count(&self, _pdf_bytes: &[u8]) -> Result<usize, ExtractionError> {
        Ok(self.page_count)
    }

    fn render_page(
        &self,
        _pdf_bytes: &[u8],
        page_number: usize,
        _dpi: u32,
    ) -> Result<Vec<u8>, ExtractionError> {
        if page_number >= self.page_count {
            return Err(ExtractionError::PdfRendering {
                page: page_number,
                reason: format!(
                    "Page {page_number} out of range (mock has {} pages)",
                    self.page_count
                ),
            });
        }
        Ok(minimal_png())
    }
}

/// Minimal valid 1x1 white pixel PNG for mock testing.
fn minimal_png() -> Vec<u8> {
    vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1
        0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, // 8-bit RGB
        0xDE, // IHDR CRC
        0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, // IDAT chunk
        0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, // compressed
        0x00, 0x02, 0x00, 0x01, 0xE2, 0x21, 0xBC, 0x33, // IDAT CRC
        0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, // IEND chunk
        0xAE, 0x42, 0x60, 0x82, // IEND CRC
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Pure dimension logic tests (no PDFium needed) ──

    #[test]
    fn a4_at_200dpi() {
        let (w, h) = compute_render_dimensions(595.0, 842.0, 200);
        // 595 * 200/72 ~ 1653, 842 * 200/72 ~ 2339
        assert!(w > 1600 && w < 1700, "A4 width at 200dpi: got {w}");
        assert!(h > 2300 && h < 2400, "A4 height at 200dpi: got {h}");
    }

    #[test]
    fn a4_at_300dpi() {
        let (w, h) = compute_render_dimensions(595.0, 842.0, 300);
        assert!(w > 2400 && w < 2550, "A4 width at 300dpi: got {w}");
        assert!(h > 3450 && h < 3600, "A4 height at 300dpi: got {h}");
    }

    #[test]
    fn letter_at_200dpi() {
        // US Letter = 612 x 792 points
        let (w, h) = compute_render_dimensions(612.0, 792.0, 200);
        assert!(w > 1650 && w < 1750, "Letter width at 200dpi: got {w}");
        assert!(h > 2150 && h < 2250, "Letter height at 200dpi: got {h}");
    }

    #[test]
    fn dimension_guard_caps_oversized() {
        // 5000x7000 pts at 200 DPI -> 13889x19444 -> capped
        let (w, h) = compute_render_dimensions(5000.0, 7000.0, 200);
        assert!(w <= MAX_DIMENSION_PX, "Width {w} exceeds {MAX_DIMENSION_PX}");
        assert!(h <= MAX_DIMENSION_PX, "Height {h} exceeds {MAX_DIMENSION_PX}");
        assert!(w >= 1);
        assert!(h >= 1);
    }

    #[test]
    fn dimension_guard_preserves_aspect_ratio() {
        let (w, h) = compute_render_dimensions(5000.0, 10000.0, 200);
        let ratio = h as f32 / w as f32;
        assert!(
            (ratio - 2.0).abs() < 0.15,
            "Aspect ratio should be ~2:1, got {ratio}"
        );
    }

    #[test]
    fn zero_points_clamped_to_1() {
        let (w, h) = compute_render_dimensions(0.0, 0.0, 200);
        assert!(w >= 1, "Width must be >= 1, got {w}");
        assert!(h >= 1, "Height must be >= 1, got {h}");
    }

    #[test]
    fn small_page_not_capped() {
        let (w, h) = compute_render_dimensions(100.0, 100.0, 200);
        assert!(w > 270 && w < 290, "Small page width: got {w}");
        assert!(h > 270 && h < 290, "Small page height: got {h}");
        assert!(w < MAX_DIMENSION_PX);
        assert!(h < MAX_DIMENSION_PX);
    }

    #[test]
    fn single_dimension_oversized() {
        // Very wide but short: 20000x100 pts at 200 DPI
        let (w, h) = compute_render_dimensions(20000.0, 100.0, 200);
        assert!(w <= MAX_DIMENSION_PX, "Width {w} exceeds limit");
        assert!(h >= 1, "Height {h} too small");
    }

    #[test]
    fn high_dpi_triggers_guard() {
        // A4 at 1000 DPI -> 8264x11694 -> capped
        let (w, h) = compute_render_dimensions(595.0, 842.0, 1000);
        assert!(w <= MAX_DIMENSION_PX, "Width {w} exceeds limit");
        assert!(h <= MAX_DIMENSION_PX, "Height {h} exceeds limit");
    }

    // ── Mock renderer tests ──

    #[test]
    fn mock_returns_png_for_valid_page() {
        let mock = MockPdfPageRenderer::new(3);
        let result = mock.render_page(&[], 0, 200);
        assert!(result.is_ok());
        let png = result.unwrap();
        assert!(png.len() > 8);
        assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]); // PNG magic
    }

    #[test]
    fn mock_renders_all_pages() {
        let mock = MockPdfPageRenderer::new(5);
        for i in 0..5 {
            assert!(mock.render_page(&[], i, 200).is_ok());
        }
    }

    #[test]
    fn mock_errors_for_out_of_range() {
        let mock = MockPdfPageRenderer::new(2);
        let err = mock.render_page(&[], 2, 200).unwrap_err();
        assert!(matches!(err, ExtractionError::PdfRendering { page: 2, .. }));
    }

    #[test]
    fn mock_errors_for_zero_pages() {
        let mock = MockPdfPageRenderer::new(0);
        assert!(mock.render_page(&[], 0, 200).is_err());
    }

    #[test]
    fn minimal_png_has_valid_signature() {
        let png = minimal_png();
        assert_eq!(
            &png[..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
        );
        let iend = [0x49, 0x45, 0x4E, 0x44];
        assert!(png.windows(4).any(|w| w == iend));
    }
}
