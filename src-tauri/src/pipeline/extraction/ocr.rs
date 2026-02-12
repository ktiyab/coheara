use super::types::{OcrEngine, OcrPageResult};
use super::ExtractionError;

/// Bundled Tesseract OCR engine.
/// Only available when compiled with the `ocr` feature flag.
#[cfg(feature = "ocr")]
pub struct BundledTesseract {
    tessdata_dir: std::path::PathBuf,
    default_lang: String,
}

#[cfg(feature = "ocr")]
impl BundledTesseract {
    /// Initialize with a tessdata directory.
    pub fn new(tessdata_dir: &std::path::Path) -> Result<Self, ExtractionError> {
        if !tessdata_dir.join("eng.traineddata").exists() {
            return Err(ExtractionError::TessdataNotFound(tessdata_dir.to_path_buf()));
        }

        Ok(Self {
            tessdata_dir: tessdata_dir.to_path_buf(),
            default_lang: "eng".to_string(),
        })
    }

    /// Set language(s) for OCR (e.g., "eng", "eng+fra")
    pub fn with_languages(mut self, langs: &str) -> Self {
        self.default_lang = langs.to_string();
        self
    }
}

#[cfg(feature = "ocr")]
impl OcrEngine for BundledTesseract {
    fn ocr_image(&self, image_bytes: &[u8]) -> Result<OcrPageResult, ExtractionError> {
        self.ocr_image_with_lang(image_bytes, &self.default_lang)
    }

    fn ocr_image_with_lang(
        &self,
        image_bytes: &[u8],
        lang: &str,
    ) -> Result<OcrPageResult, ExtractionError> {
        let tessdata_str = self
            .tessdata_dir
            .to_str()
            .ok_or_else(|| ExtractionError::OcrInit("Invalid tessdata path".into()))?;

        let mut tess = tesseract::Tesseract::new(Some(tessdata_str), Some(lang))
            .map_err(|e| ExtractionError::OcrInit(format!("{e:?}")))?
            .set_image_from_mem(image_bytes)
            .map_err(|e| ExtractionError::OcrProcessing(format!("{e:?}")))?;

        let text = tess
            .get_text()
            .map_err(|e| ExtractionError::OcrProcessing(format!("{e:?}")))?;

        let confidence = tess.mean_text_conf().max(0) as f32 / 100.0;

        // Build word-level confidences from text (mean confidence per word)
        let word_confidences: Vec<(String, f32)> = text
            .split_whitespace()
            .map(|w| (w.to_string(), confidence))
            .collect();

        Ok(OcrPageResult {
            text,
            confidence,
            word_confidences,
        })
    }
}

/// Mock OCR engine for unit testing without Tesseract.
pub struct MockOcrEngine {
    pub text: String,
    pub confidence: f32,
}

impl MockOcrEngine {
    pub fn new(text: &str, confidence: f32) -> Self {
        Self {
            text: text.to_string(),
            confidence,
        }
    }
}

impl OcrEngine for MockOcrEngine {
    fn ocr_image(&self, _image_bytes: &[u8]) -> Result<OcrPageResult, ExtractionError> {
        self.ocr_image_with_lang(_image_bytes, "eng")
    }

    fn ocr_image_with_lang(
        &self,
        _image_bytes: &[u8],
        _lang: &str,
    ) -> Result<OcrPageResult, ExtractionError> {
        let word_confidences: Vec<(String, f32)> = self
            .text
            .split_whitespace()
            .map(|w| (w.to_string(), self.confidence))
            .collect();

        Ok(OcrPageResult {
            text: self.text.clone(),
            confidence: self.confidence,
            word_confidences,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_ocr_returns_configured_text() {
        let engine = MockOcrEngine::new("Metformin 500mg", 0.92);
        let result = engine.ocr_image(b"fake_image_bytes").unwrap();
        assert_eq!(result.text, "Metformin 500mg");
        assert!((result.confidence - 0.92).abs() < f32::EPSILON);
    }

    #[test]
    fn mock_ocr_word_confidences() {
        let engine = MockOcrEngine::new("Blood pressure normal", 0.85);
        let result = engine.ocr_image(b"fake").unwrap();
        assert_eq!(result.word_confidences.len(), 3);
        assert_eq!(result.word_confidences[0].0, "Blood");
        assert!((result.word_confidences[0].1 - 0.85).abs() < f32::EPSILON);
    }

    #[test]
    fn mock_ocr_with_lang_ignores_lang() {
        let engine = MockOcrEngine::new("Bonjour", 0.88);
        let result = engine.ocr_image_with_lang(b"fake", "fra").unwrap();
        assert_eq!(result.text, "Bonjour");
    }

    #[cfg(feature = "ocr")]
    #[test]
    fn bundled_tesseract_rejects_missing_tessdata() {
        let dir = tempfile::tempdir().unwrap();
        let result = BundledTesseract::new(dir.path());
        assert!(matches!(
            result,
            Err(ExtractionError::TessdataNotFound(_))
        ));
    }

    #[cfg(feature = "ocr")]
    #[test]
    fn bundled_tesseract_initializes_with_system_tessdata() {
        let tessdata_dir = std::path::Path::new("/usr/share/tesseract-ocr/5/tessdata");
        if !tessdata_dir.exists() {
            return; // Skip on systems without Tesseract
        }
        let engine = BundledTesseract::new(tessdata_dir).unwrap();
        assert_eq!(engine.default_lang, "eng");
    }
}
