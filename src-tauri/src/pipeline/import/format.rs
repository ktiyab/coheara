use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::ImportError;

/// Broad file categories we handle
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FileCategory {
    DigitalPdf,
    ScannedPdf,
    Image,
    PlainText,
    Unsupported,
}

impl FileCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DigitalPdf => "digital_pdf",
            Self::ScannedPdf => "scanned_pdf",
            Self::Image => "image",
            Self::PlainText => "plain_text",
            Self::Unsupported => "unsupported",
        }
    }

    pub fn needs_ocr(&self) -> bool {
        matches!(self, Self::ScannedPdf | Self::Image)
    }

    pub fn is_supported(&self) -> bool {
        !matches!(self, Self::Unsupported)
    }
}

/// Result of format detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatDetection {
    pub mime_type: String,
    pub category: FileCategory,
    pub is_digital_pdf: Option<bool>,
    pub file_size_bytes: u64,
}

const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024; // 100MB

/// Detect file format from magic bytes (NOT file extensions).
/// Magic bytes don't lie — extensions can be wrong.
pub fn detect_format(path: &Path) -> Result<FormatDetection, ImportError> {
    let metadata = std::fs::metadata(path)?;
    let file_size = metadata.len();

    if file_size > MAX_FILE_SIZE {
        return Ok(FormatDetection {
            mime_type: "unknown".into(),
            category: FileCategory::Unsupported,
            is_digital_pdf: None,
            file_size_bytes: file_size,
        });
    }

    // Read first 16 bytes for magic number detection
    let mut file = std::fs::File::open(path)?;
    let mut header = [0u8; 16];
    let bytes_read = file.read(&mut header)?;

    let (mime_type, category, is_digital_pdf) = match &header[..bytes_read.min(8)] {
        // PDF: starts with %PDF
        [0x25, 0x50, 0x44, 0x46, ..] => {
            let is_digital = check_pdf_has_text(path).unwrap_or(false);
            let category = if is_digital {
                FileCategory::DigitalPdf
            } else {
                FileCategory::ScannedPdf
            };
            (
                "application/pdf".to_string(),
                category,
                Some(is_digital),
            )
        }
        // JPEG: starts with FF D8 FF
        [0xFF, 0xD8, 0xFF, ..] => ("image/jpeg".to_string(), FileCategory::Image, None),
        // PNG: starts with 89 50 4E 47
        [0x89, 0x50, 0x4E, 0x47, ..] => ("image/png".to_string(), FileCategory::Image, None),
        // TIFF: little-endian (49 49 2A 00) or big-endian (4D 4D 00 2A)
        [0x49, 0x49, 0x2A, 0x00, ..] | [0x4D, 0x4D, 0x00, 0x2A, ..] => {
            ("image/tiff".to_string(), FileCategory::Image, None)
        }
        // HEIC/HEIF: "ftyp" at offset 4
        _ if bytes_read >= 12 && &header[4..8] == b"ftyp" => {
            ("image/heic".to_string(), FileCategory::Image, None)
        }
        _ => {
            // Try as plain text (UTF-8 validation on first chunk)
            if is_likely_text(path)? {
                ("text/plain".to_string(), FileCategory::PlainText, None)
            } else {
                (
                    "application/octet-stream".to_string(),
                    FileCategory::Unsupported,
                    None,
                )
            }
        }
    };

    Ok(FormatDetection {
        mime_type,
        category,
        is_digital_pdf,
        file_size_bytes: file_size,
    })
}

/// Check if a PDF has extractable text (digital vs scanned).
/// Uses a heuristic: search for text stream markers in raw PDF bytes.
fn check_pdf_has_text(path: &Path) -> Result<bool, ImportError> {
    let file = std::fs::File::open(path)?;
    let mut buffer = Vec::new();
    // Read up to 256KB to check for text markers
    let mut limited = file.take(256 * 1024);
    limited.read_to_end(&mut buffer)?;

    let content = String::from_utf8_lossy(&buffer);

    // Count text-related PDF operators:
    // BT/ET = begin/end text, Tj/TJ = show text, Tf = set font
    let text_markers = ["BT", "ET", " Tj", " TJ", " Tf"];
    let marker_count: usize = text_markers
        .iter()
        .map(|m| content.matches(m).count())
        .sum();

    // Heuristic: >= 3 text markers suggests a digital PDF with text layer
    Ok(marker_count >= 3)
}

/// Check if a file is likely plain text (valid UTF-8, mostly printable)
fn is_likely_text(path: &Path) -> Result<bool, ImportError> {
    let mut file = std::fs::File::open(path)?;
    let mut buffer = vec![0u8; 4096];
    let n = file.read(&mut buffer)?;
    buffer.truncate(n);

    if n == 0 {
        return Ok(false);
    }

    let text = match std::str::from_utf8(&buffer) {
        Ok(t) => t,
        Err(_) => return Ok(false),
    };

    // At least 80% printable characters (or whitespace)
    let printable = text
        .chars()
        .filter(|c| !c.is_control() || c.is_whitespace())
        .count();
    let ratio = printable as f64 / text.len().max(1) as f64;
    Ok(ratio > 0.80)
}

/// Sanitize a filename — strip path components, limit length
pub fn sanitize_filename(original: &str) -> String {
    let name = Path::new(original)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document");

    let clean: String = name
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | '\0'))
        .take(255)
        .collect();

    if clean.is_empty() {
        "document".to_string()
    } else {
        clean
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_jpeg_from_magic_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.jpg");
        // Minimal JPEG magic bytes
        std::fs::write(&path, &[0xFF, 0xD8, 0xFF, 0xE0, 0x00]).unwrap();
        let format = detect_format(&path).unwrap();
        assert_eq!(format.category, FileCategory::Image);
        assert_eq!(format.mime_type, "image/jpeg");
    }

    #[test]
    fn detect_png_from_magic_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.png");
        std::fs::write(&path, &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]).unwrap();
        let format = detect_format(&path).unwrap();
        assert_eq!(format.category, FileCategory::Image);
        assert_eq!(format.mime_type, "image/png");
    }

    #[test]
    fn detect_tiff_little_endian() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.tiff");
        std::fs::write(&path, &[0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00]).unwrap();
        let format = detect_format(&path).unwrap();
        assert_eq!(format.category, FileCategory::Image);
        assert_eq!(format.mime_type, "image/tiff");
    }

    #[test]
    fn detect_text_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        std::fs::write(&path, "This is a medical report. Patient: Marie Dubois. Date: 2024-01-15.").unwrap();
        let format = detect_format(&path).unwrap();
        assert_eq!(format.category, FileCategory::PlainText);
        assert_eq!(format.mime_type, "text/plain");
    }

    #[test]
    fn detect_binary_as_unsupported() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("binary.exe");
        std::fs::write(&path, &[0x4D, 0x5A, 0x90, 0x00, 0x03, 0x00]).unwrap();
        let format = detect_format(&path).unwrap();
        assert_eq!(format.category, FileCategory::Unsupported);
    }

    #[test]
    fn wrong_extension_detected_by_magic_bytes() {
        let dir = tempfile::tempdir().unwrap();
        // JPEG content with .pdf extension
        let path = dir.path().join("misleading.pdf");
        std::fs::write(&path, &[0xFF, 0xD8, 0xFF, 0xE0, 0x00]).unwrap();
        let format = detect_format(&path).unwrap();
        assert_eq!(format.category, FileCategory::Image);
    }

    #[test]
    fn oversized_file_unsupported() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("huge.bin");
        // Create a file just over 100MB using sparse writing
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(101 * 1024 * 1024).unwrap();
        let format = detect_format(&path).unwrap();
        assert_eq!(format.category, FileCategory::Unsupported);
    }

    #[test]
    fn pdf_magic_bytes_detected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.pdf");
        // Minimal PDF with text markers
        std::fs::write(&path, b"%PDF-1.4 some content BT /F1 12 Tf (Hello) Tj ET").unwrap();
        let format = detect_format(&path).unwrap();
        assert!(
            matches!(format.category, FileCategory::DigitalPdf | FileCategory::ScannedPdf),
            "Expected PDF category, got {:?}",
            format.category
        );
        assert_eq!(format.mime_type, "application/pdf");
    }

    #[test]
    fn file_category_traits() {
        assert!(FileCategory::Image.is_supported());
        assert!(FileCategory::DigitalPdf.is_supported());
        assert!(!FileCategory::Unsupported.is_supported());
        assert!(FileCategory::ScannedPdf.needs_ocr());
        assert!(FileCategory::Image.needs_ocr());
        assert!(!FileCategory::DigitalPdf.needs_ocr());
        assert!(!FileCategory::PlainText.needs_ocr());
    }

    #[test]
    fn sanitize_path_traversal() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename("normal_file.pdf"), "normal_file.pdf");
        assert_eq!(sanitize_filename(""), "document");
        assert_eq!(sanitize_filename("file\0name.pdf"), "filename.pdf");
    }

    #[test]
    fn sanitize_preserves_normal_names() {
        assert_eq!(sanitize_filename("prescription_2024.pdf"), "prescription_2024.pdf");
        assert_eq!(sanitize_filename("lab results (1).jpg"), "lab results (1).jpg");
    }
}
