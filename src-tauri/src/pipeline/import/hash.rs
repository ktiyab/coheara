use std::path::Path;

use base64::Engine;
use sha2::{Digest, Sha256};

use super::format::FileCategory;
use super::ImportError;

/// Compute the appropriate hash based on file category
pub fn compute_hash(path: &Path, category: &FileCategory) -> Result<String, ImportError> {
    match category {
        FileCategory::Image => compute_image_hash(path),
        FileCategory::DigitalPdf | FileCategory::ScannedPdf => compute_content_hash(path),
        FileCategory::PlainText => compute_content_hash(path),
        FileCategory::Unsupported => Err(ImportError::UnsupportedFormat),
    }
}

/// Compute perceptual hash for an image file.
/// Uses DoubleGradient algorithm (256-bit hash) for near-duplicate detection.
/// Uses img_hash's re-exported image crate for compatibility.
pub fn compute_image_hash(path: &Path) -> Result<String, ImportError> {
    let img =
        img_hash::image::open(path).map_err(|e| ImportError::ImageProcessing(e.to_string()))?;

    let hasher = img_hash::HasherConfig::new()
        .hash_alg(img_hash::HashAlg::DoubleGradient)
        .hash_size(16, 16)
        .to_hasher();

    let hash = hasher.hash_image(&img);
    Ok(hash.to_base64())
}

/// Compute SHA-256 content hash for PDFs and text files
pub fn compute_content_hash(path: &Path) -> Result<String, ImportError> {
    let content = std::fs::read(path)?;
    let hash = Sha256::digest(&content);
    Ok(base64::engine::general_purpose::STANDARD.encode(hash))
}

/// Compare two perceptual hashes and return similarity score (0.0-1.0)
pub fn hash_similarity(hash_a: &str, hash_b: &str) -> Option<f64> {
    let a = img_hash::ImageHash::<Vec<u8>>::from_base64(hash_a).ok()?;
    let b = img_hash::ImageHash::<Vec<u8>>::from_base64(hash_b).ok()?;

    let distance = a.dist(&b);
    let max_bits = (a.as_bytes().len() * 8).max(1) as f64;
    Some(1.0 - (distance as f64 / max_bits))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_deterministic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        std::fs::write(&path, "Hello medical world").unwrap();

        let h1 = compute_content_hash(&path).unwrap();
        let h2 = compute_content_hash(&path).unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn different_content_different_hash() {
        let dir = tempfile::tempdir().unwrap();
        let p1 = dir.path().join("a.txt");
        let p2 = dir.path().join("b.txt");
        std::fs::write(&p1, "Content A").unwrap();
        std::fs::write(&p2, "Content B").unwrap();

        let h1 = compute_content_hash(&p1).unwrap();
        let h2 = compute_content_hash(&p2).unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn compute_hash_dispatches_by_category() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        std::fs::write(&path, "Some text content").unwrap();

        let hash = compute_hash(&path, &FileCategory::PlainText).unwrap();
        assert!(!hash.is_empty());
    }

    #[test]
    fn compute_hash_rejects_unsupported() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");
        std::fs::write(&path, &[0x00, 0x01, 0x02]).unwrap();

        let result = compute_hash(&path, &FileCategory::Unsupported);
        assert!(result.is_err());
    }

    /// Helper: create a test JPEG image (using image v0.23 via img_hash re-export)
    fn create_test_jpeg(path: &std::path::Path, width: u32, height: u32, color: [u8; 3]) {
        let img = img_hash::image::RgbImage::from_pixel(width, height, img_hash::image::Rgb(color));
        img.save(path).unwrap();
    }

    #[test]
    fn image_hash_works_for_valid_image() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.jpg");
        create_test_jpeg(&path, 8, 8, [255, 0, 0]);

        let hash = compute_image_hash(&path).unwrap();
        assert!(!hash.is_empty());
    }

    #[test]
    fn image_hash_deterministic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.jpg");
        create_test_jpeg(&path, 10, 10, [0, 128, 255]);

        let h1 = compute_image_hash(&path).unwrap();
        let h2 = compute_image_hash(&path).unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn identical_images_have_perfect_similarity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.jpg");
        create_test_jpeg(&path, 32, 32, [100, 150, 200]);

        let hash = compute_image_hash(&path).unwrap();
        let similarity = hash_similarity(&hash, &hash).unwrap();
        assert!((similarity - 1.0).abs() < f64::EPSILON);
    }
}
