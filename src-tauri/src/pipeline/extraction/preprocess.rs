use image::{DynamicImage, GrayImage, ImageOutputFormat, Luma};

use super::types::ImageQuality;
use super::ExtractionError;

/// Pre-process an image for better OCR results.
/// Pipeline: load → grayscale → auto-contrast → Otsu threshold → PNG bytes.
pub fn preprocess_image(image_bytes: &[u8]) -> Result<Vec<u8>, ExtractionError> {
    let img = image::load_from_memory(image_bytes)
        .map_err(|e| ExtractionError::ImageProcessing(e.to_string()))?;

    let gray = img.to_luma8();
    let contrasted = auto_contrast(&gray);
    let binary = otsu_threshold(&contrasted);

    let dynamic = DynamicImage::ImageLuma8(binary);
    let mut cursor = std::io::Cursor::new(Vec::new());
    dynamic
        .write_to(&mut cursor, ImageOutputFormat::Png)
        .map_err(|e| ExtractionError::ImageProcessing(e.to_string()))?;

    Ok(cursor.into_inner())
}

/// Stretch histogram to use the full 0-255 range.
fn auto_contrast(img: &GrayImage) -> GrayImage {
    let mut min = 255u8;
    let mut max = 0u8;
    for pixel in img.pixels() {
        let v = pixel.0[0];
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
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

/// Compute Otsu's threshold for a grayscale image.
/// Returns the optimal threshold value that minimizes intra-class variance.
fn compute_otsu_level(img: &GrayImage) -> u8 {
    let mut histogram = [0u32; 256];
    for pixel in img.pixels() {
        histogram[pixel.0[0] as usize] += 1;
    }

    let total = img.pixels().count() as f64;
    if total == 0.0 {
        return 128;
    }

    let mut sum_total = 0.0;
    for (i, &count) in histogram.iter().enumerate() {
        sum_total += i as f64 * count as f64;
    }

    let mut sum_bg = 0.0;
    let mut weight_bg = 0.0;
    let mut max_variance = 0.0;
    let mut best_threshold = 0u8;

    for (t, &count) in histogram.iter().enumerate() {
        weight_bg += count as f64;
        if weight_bg == 0.0 {
            continue;
        }

        let weight_fg = total - weight_bg;
        if weight_fg == 0.0 {
            break;
        }

        sum_bg += t as f64 * count as f64;

        let mean_bg = sum_bg / weight_bg;
        let mean_fg = (sum_total - sum_bg) / weight_fg;
        let diff = mean_bg - mean_fg;
        let variance = weight_bg * weight_fg * diff * diff;

        if variance > max_variance {
            max_variance = variance;
            best_threshold = t as u8;
        }
    }

    best_threshold
}

/// Apply Otsu thresholding to binarize a grayscale image.
fn otsu_threshold(img: &GrayImage) -> GrayImage {
    let threshold = compute_otsu_level(img);
    let (width, height) = img.dimensions();
    GrayImage::from_fn(width, height, |x, y| {
        if img.get_pixel(x, y).0[0] > threshold {
            Luma([255u8])
        } else {
            Luma([0u8])
        }
    })
}

/// Assess image quality before OCR to set expectations.
pub fn assess_image_quality(image_bytes: &[u8]) -> Result<ImageQuality, ExtractionError> {
    let img = image::load_from_memory(image_bytes)
        .map_err(|e| ExtractionError::ImageProcessing(e.to_string()))?;

    let gray = img.to_luma8();
    let (width, height) = gray.dimensions();
    let pixel_count = (width as u64) * (height as u64);

    let resolution = if pixel_count > 2_000_000 {
        "high"
    } else if pixel_count > 500_000 {
        "medium"
    } else {
        "low"
    };

    // Contrast check via standard deviation
    let mean: f64 = gray.pixels().map(|p| p.0[0] as f64).sum::<f64>() / pixel_count.max(1) as f64;
    let variance: f64 = gray
        .pixels()
        .map(|p| (p.0[0] as f64 - mean).powi(2))
        .sum::<f64>()
        / pixel_count.max(1) as f64;
    let std_dev = variance.sqrt();

    let contrast = if std_dev > 60.0 {
        "good"
    } else if std_dev > 30.0 {
        "fair"
    } else {
        "poor"
    };

    let estimated_confidence = match (resolution, contrast) {
        ("high", "good") => 0.85,
        ("high", "fair") | ("medium", "good") => 0.70,
        ("medium", "fair") => 0.55,
        _ => 0.35,
    };

    Ok(ImageQuality {
        resolution: resolution.into(),
        contrast: contrast.into(),
        estimated_confidence,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_image(width: u32, height: u32, pixel_val: u8) -> Vec<u8> {
        let img = GrayImage::from_fn(width, height, |_, _| Luma([pixel_val]));
        let dynamic = DynamicImage::ImageLuma8(img);
        let mut cursor = std::io::Cursor::new(Vec::new());
        dynamic
            .write_to(&mut cursor, ImageOutputFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    fn make_gradient_image(width: u32, height: u32) -> Vec<u8> {
        let img = GrayImage::from_fn(width, height, |x, _| {
            Luma([(x as f32 / width as f32 * 255.0) as u8])
        });
        let dynamic = DynamicImage::ImageLuma8(img);
        let mut cursor = std::io::Cursor::new(Vec::new());
        dynamic
            .write_to(&mut cursor, ImageOutputFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    #[test]
    fn preprocess_produces_binary_image() {
        let bytes = make_gradient_image(100, 100);
        let result = preprocess_image(&bytes).unwrap();
        assert!(!result.is_empty());

        // Load result and verify it's binary (only 0 and 255)
        let img = image::load_from_memory(&result).unwrap().to_luma8();
        for pixel in img.pixels() {
            assert!(
                pixel.0[0] == 0 || pixel.0[0] == 255,
                "Expected binary pixel, got {}",
                pixel.0[0]
            );
        }
    }

    #[test]
    fn preprocess_preserves_dimensions() {
        use image::GenericImageView;
        let bytes = make_gradient_image(200, 150);
        let result = preprocess_image(&bytes).unwrap();
        let img = image::load_from_memory(&result).unwrap();
        assert_eq!(img.width(), 200);
        assert_eq!(img.height(), 150);
    }

    #[test]
    fn auto_contrast_uniform_image_unchanged() {
        let img = GrayImage::from_fn(10, 10, |_, _| Luma([128u8]));
        let result = auto_contrast(&img);
        // Uniform image: all pixels same value, result should be unchanged
        for pixel in result.pixels() {
            assert_eq!(pixel.0[0], 128);
        }
    }

    #[test]
    fn auto_contrast_stretches_range() {
        // Image with values only in 100-200 range
        let img = GrayImage::from_fn(10, 10, |x, _| Luma([100 + (x as u8 * 10).min(100)]));
        let result = auto_contrast(&img);

        let mut min = 255u8;
        let mut max = 0u8;
        for pixel in result.pixels() {
            if pixel.0[0] < min {
                min = pixel.0[0];
            }
            if pixel.0[0] > max {
                max = pixel.0[0];
            }
        }
        // After contrast stretching, should span close to full range
        assert_eq!(min, 0);
        assert_eq!(max, 255);
    }

    #[test]
    fn otsu_level_bimodal_image() {
        // Create an image with two distinct groups: dark (30) and light (220)
        let img = GrayImage::from_fn(100, 100, |x, _| {
            if x < 50 {
                Luma([30u8])
            } else {
                Luma([220u8])
            }
        });
        let threshold = compute_otsu_level(&img);
        // For a perfectly bimodal image, Otsu picks the boundary between groups.
        // With values 30 and 220, any threshold in [30, 219] correctly separates them.
        assert!(
            threshold >= 30 && threshold <= 219,
            "Expected threshold to separate groups, got {threshold}"
        );

        // Verify the threshold actually works: apply it and check separation
        let binary = otsu_threshold(&img);
        let dark_count = binary.pixels().filter(|p| p.0[0] == 0).count();
        let light_count = binary.pixels().filter(|p| p.0[0] == 255).count();
        assert!(dark_count > 0, "Should have dark pixels");
        assert!(light_count > 0, "Should have light pixels");
    }

    #[test]
    fn assess_quality_poor_contrast_uniform_image() {
        let bytes = make_test_image(100, 100, 200);
        let quality = assess_image_quality(&bytes).unwrap();
        assert_eq!(quality.contrast, "poor");
        assert_eq!(quality.resolution, "low"); // 10K pixels
    }

    #[test]
    fn assess_quality_good_contrast_gradient() {
        let bytes = make_gradient_image(200, 200);
        let quality = assess_image_quality(&bytes).unwrap();
        assert_eq!(quality.contrast, "good");
    }

    #[test]
    fn assess_quality_high_resolution() {
        let bytes = make_gradient_image(2000, 1500);
        let quality = assess_image_quality(&bytes).unwrap();
        assert_eq!(quality.resolution, "high"); // 3M pixels
        assert!(quality.estimated_confidence >= 0.70);
    }

    #[test]
    fn preprocess_rejects_invalid_bytes() {
        let result = preprocess_image(&[0x00, 0x01, 0x02]);
        assert!(result.is_err());
    }
}
