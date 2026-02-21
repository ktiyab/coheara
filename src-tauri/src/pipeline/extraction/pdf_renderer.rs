//! PDF page-to-image extraction using lopdf.
//!
//! Extracts embedded images (JPEG/TIFF) from scanned PDF pages for OCR.
//! Uses only existing Cargo dependencies — zero new crates.

use image::ImageOutputFormat;
use lopdf::{Document, Object, ObjectId};

use super::types::PdfPageRenderer;
use super::ExtractionError;

/// Extracts embedded images from PDF pages using lopdf.
///
/// Works for scanned PDFs where each page contains an image XObject
/// (JPEG, TIFF, or raw pixel data). This covers 95%+ of medical
/// scanned documents (lab results, prescriptions, records).
pub struct LopdfImageExtractor;

impl PdfPageRenderer for LopdfImageExtractor {
    fn render_page(
        &self,
        pdf_bytes: &[u8],
        page_number: usize,
        _dpi: u32,
    ) -> Result<Vec<u8>, ExtractionError> {
        let doc = Document::load_mem(pdf_bytes)
            .map_err(|e| ExtractionError::PdfParsing(format!("Failed to parse PDF: {e}")))?;

        let page_ids: Vec<ObjectId> = doc.page_iter().collect();
        let &page_id = page_ids.get(page_number).ok_or_else(|| {
            ExtractionError::PdfParsing(format!(
                "Page {} not found (PDF has {} pages)",
                page_number,
                page_ids.len()
            ))
        })?;

        let image_bytes = extract_largest_page_image(&doc, page_id)?;

        // Validate and re-encode to PNG for the OCR pipeline
        let img = image::load_from_memory(&image_bytes).map_err(|e| {
            ExtractionError::ImageProcessing(format!("Failed to decode extracted image: {e}"))
        })?;

        let mut png_buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut png_buf, ImageOutputFormat::Png)
            .map_err(|e| {
                ExtractionError::ImageProcessing(format!("Failed to encode PNG: {e}"))
            })?;

        tracing::debug!(
            page = page_number,
            raw_size = image_bytes.len(),
            png_size = png_buf.get_ref().len(),
            "Extracted image from PDF page"
        );

        Ok(png_buf.into_inner())
    }
}

/// Extract the largest image XObject from a PDF page.
///
/// Walks: page dict → /Resources → /XObject → find /Subtype /Image entries.
/// Returns the raw image bytes of the largest image found.
fn extract_largest_page_image(
    doc: &Document,
    page_id: ObjectId,
) -> Result<Vec<u8>, ExtractionError> {
    let page_obj = doc
        .get_object(page_id)
        .map_err(|e| ExtractionError::PdfParsing(format!("Page object error: {e}")))?;

    let page_dict = page_obj
        .as_dict()
        .map_err(|_| ExtractionError::PdfParsing("Page is not a dictionary".into()))?;

    // Navigate: /Resources → /XObject
    let resources = resolve_dict_entry(doc, page_dict, b"Resources")?;
    let xobjects = resolve_dict_entry(doc, resources, b"XObject")?;

    let mut largest: Option<Vec<u8>> = None;

    for (_name, obj_ref) in xobjects.iter() {
        // Dereference the XObject reference to get the actual object
        let xobj = match obj_ref {
            Object::Reference(id) => match doc.get_object(*id) {
                Ok(obj) => obj,
                Err(_) => continue,
            },
            other => other,
        };

        let stream = match xobj {
            Object::Stream(ref s) => s,
            _ => continue,
        };

        // Check /Subtype == /Image
        if !is_image_subtype(&stream.dict) {
            continue;
        }

        let image_bytes = extract_image_bytes(doc, stream)?;

        // Keep the largest image (the main page scan)
        if largest.as_ref().map_or(true, |prev| image_bytes.len() > prev.len()) {
            largest = Some(image_bytes);
        }
    }

    largest.ok_or_else(|| {
        ExtractionError::PdfParsing("No image XObjects found on this page".into())
    })
}

/// Check if a stream dictionary has /Subtype /Image.
fn is_image_subtype(dict: &lopdf::Dictionary) -> bool {
    dict.get(b"Subtype")
        .map(|obj| matches!(obj, Object::Name(ref n) if n == b"Image"))
        .unwrap_or(false)
}

/// Extract image bytes from a PDF stream, handling different filters.
fn extract_image_bytes(
    doc: &Document,
    stream: &lopdf::Stream,
) -> Result<Vec<u8>, ExtractionError> {
    let filter = stream.dict.get(b"Filter").ok();

    let is_dct = filter
        .map(|f| match f {
            Object::Name(n) => n == b"DCTDecode",
            Object::Array(arr) => arr
                .iter()
                .any(|o| matches!(o, Object::Name(ref n) if n == b"DCTDecode")),
            _ => false,
        })
        .unwrap_or(false);

    if is_dct {
        // DCTDecode = JPEG. The raw stream content IS the JPEG file.
        // Use decompressed content to handle any additional filters in the chain.
        let content = stream
            .decompressed_content()
            .unwrap_or_else(|_| stream.content.clone());
        return Ok(content);
    }

    // For FlateDecode or other filters, decompress and reconstruct the image
    let content = stream
        .decompressed_content()
        .unwrap_or_else(|_| stream.content.clone());

    // Try to decode directly — some streams contain full image files (TIFF, PNG)
    if image::load_from_memory(&content).is_ok() {
        return Ok(content);
    }

    // Raw pixel data: reconstruct using /Width, /Height, /BitsPerComponent, /ColorSpace
    reconstruct_raw_image(doc, &stream.dict, &content)
}

/// Reconstruct an image from raw pixel data using PDF metadata.
fn reconstruct_raw_image(
    doc: &Document,
    dict: &lopdf::Dictionary,
    raw_pixels: &[u8],
) -> Result<Vec<u8>, ExtractionError> {
    let width = get_int(dict, b"Width")? as u32;
    let height = get_int(dict, b"Height")? as u32;
    let bpc = get_int(dict, b"BitsPerComponent").unwrap_or(8) as u32;

    let channels = determine_channels(doc, dict);
    let expected_size = (width * height * channels * bpc / 8) as usize;

    if raw_pixels.len() < expected_size {
        return Err(ExtractionError::ImageProcessing(format!(
            "Raw pixel buffer too small: {} bytes, expected {} ({}x{}x{}x{}/8)",
            raw_pixels.len(),
            expected_size,
            width,
            height,
            channels,
            bpc
        )));
    }

    let img = match channels {
        1 => {
            // Grayscale
            let gray = image::GrayImage::from_raw(width, height, raw_pixels.to_vec())
                .ok_or_else(|| {
                    ExtractionError::ImageProcessing("Failed to create grayscale image".into())
                })?;
            image::DynamicImage::ImageLuma8(gray)
        }
        3 => {
            // RGB
            let rgb =
                image::RgbImage::from_raw(width, height, raw_pixels.to_vec()).ok_or_else(|| {
                    ExtractionError::ImageProcessing("Failed to create RGB image".into())
                })?;
            image::DynamicImage::ImageRgb8(rgb)
        }
        4 => {
            // RGBA / CMYK (treat CMYK as RGBA for now — OCR doesn't care about color accuracy)
            let rgba = image::RgbaImage::from_raw(width, height, raw_pixels.to_vec())
                .ok_or_else(|| {
                    ExtractionError::ImageProcessing("Failed to create RGBA image".into())
                })?;
            image::DynamicImage::ImageRgba8(rgba)
        }
        _ => {
            return Err(ExtractionError::ImageProcessing(format!(
                "Unsupported channel count: {channels}"
            )));
        }
    };

    let mut png_buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut png_buf, ImageOutputFormat::Png)
        .map_err(|e| ExtractionError::ImageProcessing(format!("PNG encode failed: {e}")))?;

    Ok(png_buf.into_inner())
}

/// Determine the number of color channels from the /ColorSpace entry.
fn determine_channels(doc: &Document, dict: &lopdf::Dictionary) -> u32 {
    let cs = match dict.get(b"ColorSpace") {
        Ok(obj) => resolve_object(doc, obj),
        Err(_) => return 3, // default to RGB
    };

    match cs {
        Object::Name(ref n) => match n.as_slice() {
            b"DeviceGray" => 1,
            b"DeviceRGB" => 3,
            b"DeviceCMYK" => 4,
            _ => 3,
        },
        Object::Array(ref arr) if !arr.is_empty() => {
            // ICCBased, Indexed, etc. — check the base name
            match &arr[0] {
                Object::Name(ref n) if n == b"ICCBased" => {
                    // ICCBased: the /N entry in the ICC stream gives channel count
                    if arr.len() > 1 {
                        if let Object::Reference(id) = &arr[1] {
                            if let Ok(Object::Stream(ref s)) = doc.get_object(*id) {
                                return get_int(&s.dict, b"N").unwrap_or(3) as u32;
                            }
                        }
                    }
                    3
                }
                Object::Name(ref n) if n == b"Indexed" => {
                    // Indexed color: output is single channel (palette index → RGB handled by decoder)
                    1
                }
                _ => 3,
            }
        }
        _ => 3,
    }
}

/// Resolve a PDF object reference to its target, or return the object as-is.
fn resolve_object<'a>(doc: &'a Document, obj: &'a Object) -> &'a Object {
    match obj {
        Object::Reference(id) => doc.get_object(*id).unwrap_or(obj),
        _ => obj,
    }
}

/// Get a dictionary entry, following references, and return as a Dictionary.
fn resolve_dict_entry<'a>(
    doc: &'a Document,
    dict: &'a lopdf::Dictionary,
    key: &[u8],
) -> Result<&'a lopdf::Dictionary, ExtractionError> {
    let obj = dict.get(key).map_err(|_| {
        ExtractionError::PdfParsing(format!(
            "Missing /{} in dictionary",
            String::from_utf8_lossy(key)
        ))
    })?;

    let resolved = resolve_object(doc, obj);
    resolved.as_dict().map_err(|_| {
        ExtractionError::PdfParsing(format!(
            "/{} is not a dictionary",
            String::from_utf8_lossy(key)
        ))
    })
}

/// Get an integer value from a dictionary.
fn get_int(dict: &lopdf::Dictionary, key: &[u8]) -> Result<i64, ExtractionError> {
    dict.get(key)
        .map_err(|_| {
            ExtractionError::PdfParsing(format!(
                "Missing /{} in image dictionary",
                String::from_utf8_lossy(key)
            ))
        })?
        .as_i64()
        .map_err(|_| {
            ExtractionError::PdfParsing(format!(
                "/{} is not an integer",
                String::from_utf8_lossy(key)
            ))
        })
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;
    use lopdf::{dictionary, Stream};

    /// Compile-time check: LopdfImageExtractor is Send + Sync.
    #[test]
    fn extractor_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LopdfImageExtractor>();
    }

    /// Create a minimal JPEG image for testing.
    fn make_test_jpeg(width: u32, height: u32) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(width, height, image::Rgb([128u8, 128, 128]));
        let mut jpeg_bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut jpeg_bytes, ImageOutputFormat::Jpeg(85))
            .unwrap();
        jpeg_bytes.into_inner()
    }

    /// Create a valid PDF with an embedded JPEG XObject on one page.
    fn make_scanned_pdf(jpeg_bytes: &[u8]) -> Vec<u8> {
        let mut doc = Document::with_version("1.4");

        // Image stream with DCTDecode filter (= JPEG)
        let mut img_stream = Stream::new(
            dictionary! {
                "Type" => Object::Name(b"XObject".to_vec()),
                "Subtype" => Object::Name(b"Image".to_vec()),
                "Width" => Object::Integer(200),
                "Height" => Object::Integer(300),
                "ColorSpace" => Object::Name(b"DeviceRGB".to_vec()),
                "BitsPerComponent" => Object::Integer(8),
                "Filter" => Object::Name(b"DCTDecode".to_vec()),
                "Length" => Object::Integer(jpeg_bytes.len() as i64),
            },
            jpeg_bytes.to_vec(),
        );
        img_stream.allows_compression = false;
        let img_id = doc.add_object(Object::Stream(img_stream));

        // Minimal content stream (draw the image full-page)
        let content = b"q 612 0 0 792 0 0 cm /Img1 Do Q".to_vec();
        let content_stream = Stream::new(dictionary! {}, content);
        let content_id = doc.add_object(Object::Stream(content_stream));

        // Page with resources pointing to the image
        let page_id = doc.add_object(dictionary! {
            "Type" => Object::Name(b"Page".to_vec()),
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => Object::Reference(content_id),
            "Resources" => dictionary! {
                "XObject" => dictionary! {
                    "Img1" => Object::Reference(img_id),
                },
            },
        });

        let pages_id = doc.add_object(dictionary! {
            "Type" => Object::Name(b"Pages".to_vec()),
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => Object::Integer(1),
        });

        // Set parent on page
        if let Ok(Object::Dictionary(ref mut dict)) = doc.get_object_mut(page_id) {
            dict.set("Parent", Object::Reference(pages_id));
        }

        let catalog_id = doc.add_object(dictionary! {
            "Type" => Object::Name(b"Catalog".to_vec()),
            "Pages" => Object::Reference(pages_id),
        });

        doc.trailer.set("Root", Object::Reference(catalog_id));

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    #[test]
    fn extract_image_from_scanned_pdf() {
        let jpeg = make_test_jpeg(200, 300);
        let pdf_bytes = make_scanned_pdf(&jpeg);

        let extractor = LopdfImageExtractor;
        let png = extractor.render_page(&pdf_bytes, 0, 300).unwrap();

        // Verify PNG magic bytes
        assert!(png.len() > 8, "PNG should have content");
        assert_eq!(&png[0..4], b"\x89PNG", "Should be valid PNG header");

        // Verify it loads as a valid image
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!(img.width(), 200);
        assert_eq!(img.height(), 300);
    }

    #[test]
    fn invalid_page_number_returns_error() {
        let jpeg = make_test_jpeg(100, 100);
        let pdf_bytes = make_scanned_pdf(&jpeg);

        let extractor = LopdfImageExtractor;
        let result = extractor.render_page(&pdf_bytes, 5, 300);
        assert!(result.is_err(), "Page 5 should not exist in a 1-page PDF");

        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"), "Error should mention page not found");
    }

    #[test]
    fn pdf_without_images_returns_error() {
        // Create a text-only PDF (same pattern as pdf.rs tests)
        let mut doc = Document::with_version("1.4");

        let font_id = doc.add_object(dictionary! {
            "Type" => Object::Name(b"Font".to_vec()),
            "Subtype" => Object::Name(b"Type1".to_vec()),
            "BaseFont" => Object::Name(b"Helvetica".to_vec()),
        });

        let content = Stream::new(
            dictionary! {},
            b"BT /F1 12 Tf 100 700 Td (Hello) Tj ET".to_vec(),
        );
        let content_id = doc.add_object(Object::Stream(content));

        let page_id = doc.add_object(dictionary! {
            "Type" => Object::Name(b"Page".to_vec()),
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => Object::Reference(content_id),
            "Resources" => dictionary! {
                "Font" => dictionary! {
                    "F1" => Object::Reference(font_id),
                },
                "XObject" => dictionary! {},
            },
        });

        let pages_id = doc.add_object(dictionary! {
            "Type" => Object::Name(b"Pages".to_vec()),
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => Object::Integer(1),
        });

        if let Ok(Object::Dictionary(ref mut dict)) = doc.get_object_mut(page_id) {
            dict.set("Parent", Object::Reference(pages_id));
        }

        let catalog_id = doc.add_object(dictionary! {
            "Type" => Object::Name(b"Catalog".to_vec()),
            "Pages" => Object::Reference(pages_id),
        });
        doc.trailer.set("Root", Object::Reference(catalog_id));

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();

        let extractor = LopdfImageExtractor;
        let result = extractor.render_page(&buf, 0, 300);
        assert!(result.is_err(), "Text-only PDF should have no images");
    }

    #[test]
    fn extracts_largest_when_multiple_images() {
        let small_jpeg = make_test_jpeg(10, 10);
        let large_jpeg = make_test_jpeg(200, 300);

        let mut doc = Document::with_version("1.4");

        // Small image
        let mut small_stream = Stream::new(
            dictionary! {
                "Type" => Object::Name(b"XObject".to_vec()),
                "Subtype" => Object::Name(b"Image".to_vec()),
                "Width" => Object::Integer(10),
                "Height" => Object::Integer(10),
                "ColorSpace" => Object::Name(b"DeviceRGB".to_vec()),
                "BitsPerComponent" => Object::Integer(8),
                "Filter" => Object::Name(b"DCTDecode".to_vec()),
                "Length" => Object::Integer(small_jpeg.len() as i64),
            },
            small_jpeg,
        );
        small_stream.allows_compression = false;
        let small_id = doc.add_object(Object::Stream(small_stream));

        // Large image
        let mut large_stream = Stream::new(
            dictionary! {
                "Type" => Object::Name(b"XObject".to_vec()),
                "Subtype" => Object::Name(b"Image".to_vec()),
                "Width" => Object::Integer(200),
                "Height" => Object::Integer(300),
                "ColorSpace" => Object::Name(b"DeviceRGB".to_vec()),
                "BitsPerComponent" => Object::Integer(8),
                "Filter" => Object::Name(b"DCTDecode".to_vec()),
                "Length" => Object::Integer(large_jpeg.len() as i64),
            },
            large_jpeg,
        );
        large_stream.allows_compression = false;
        let large_id = doc.add_object(Object::Stream(large_stream));

        let content = Stream::new(dictionary! {}, b"q 10 0 0 10 0 0 cm /Small Do Q q 612 0 0 792 0 0 cm /Large Do Q".to_vec());
        let content_id = doc.add_object(Object::Stream(content));

        let page_id = doc.add_object(dictionary! {
            "Type" => Object::Name(b"Page".to_vec()),
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => Object::Reference(content_id),
            "Resources" => dictionary! {
                "XObject" => dictionary! {
                    "Small" => Object::Reference(small_id),
                    "Large" => Object::Reference(large_id),
                },
            },
        });

        let pages_id = doc.add_object(dictionary! {
            "Type" => Object::Name(b"Pages".to_vec()),
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => Object::Integer(1),
        });

        if let Ok(Object::Dictionary(ref mut dict)) = doc.get_object_mut(page_id) {
            dict.set("Parent", Object::Reference(pages_id));
        }

        let catalog_id = doc.add_object(dictionary! {
            "Type" => Object::Name(b"Catalog".to_vec()),
            "Pages" => Object::Reference(pages_id),
        });
        doc.trailer.set("Root", Object::Reference(catalog_id));

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();

        let extractor = LopdfImageExtractor;
        let png = extractor.render_page(&buf, 0, 300).unwrap();

        // The extracted image should be the large one (200x300)
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!(img.width(), 200, "Should extract the larger image");
        assert_eq!(img.height(), 300, "Should extract the larger image");
    }
}
