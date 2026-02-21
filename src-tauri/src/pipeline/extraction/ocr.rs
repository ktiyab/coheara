use super::types::{BoundingBox, OcrEngine, OcrPageResult, OcrWordResult};
use super::ExtractionError;

/// Bundled Tesseract OCR engine.
/// Only available when compiled with the `ocr` feature flag.
#[cfg(feature = "ocr")]
pub struct BundledTesseract {
    tessdata_dir: std::path::PathBuf,
    default_lang: String,
    /// Optional path to a medical wordlist file for improved recognition.
    medical_wordlist: Option<std::path::PathBuf>,
}

#[cfg(feature = "ocr")]
impl BundledTesseract {
    /// Initialize with a tessdata directory.
    /// Defaults to "eng+fra+deu" when all three are available, progressively
    /// falls back to "eng+fra", "eng+deu", or "eng" based on what's installed.
    pub fn new(tessdata_dir: &std::path::Path) -> Result<Self, ExtractionError> {
        if !tessdata_dir.join("eng.traineddata").exists() {
            return Err(ExtractionError::TessdataNotFound(tessdata_dir.to_path_buf()));
        }

        // EXT-04-G01: Default to multilingual OCR based on available traineddata
        let has_fra = tessdata_dir.join("fra.traineddata").exists();
        let has_deu = tessdata_dir.join("deu.traineddata").exists();

        let default_lang = match (has_fra, has_deu) {
            (true, true) => {
                tracing::info!("French + German traineddata found, defaulting to eng+fra+deu");
                "eng+fra+deu".to_string()
            }
            (true, false) => {
                tracing::info!("French traineddata found, defaulting to eng+fra");
                "eng+fra".to_string()
            }
            (false, true) => {
                tracing::info!("German traineddata found, defaulting to eng+deu");
                "eng+deu".to_string()
            }
            (false, false) => {
                tracing::warn!(
                    "No additional traineddata found at {}, using English only",
                    tessdata_dir.display()
                );
                "eng".to_string()
            }
        };

        Ok(Self {
            tessdata_dir: tessdata_dir.to_path_buf(),
            default_lang,
            medical_wordlist: None,
        })
    }

    /// Set language(s) for OCR (e.g., "eng", "eng+fra")
    pub fn with_languages(mut self, langs: &str) -> Self {
        self.default_lang = langs.to_string();
        self
    }

    /// EXT-02-G04: Set a medical wordlist file for improved OCR accuracy.
    /// The file should contain one term per line (comments starting with # are ignored).
    pub fn with_medical_wordlist(mut self, path: &std::path::Path) -> Self {
        if path.exists() {
            self.medical_wordlist = Some(path.to_path_buf());
        } else {
            tracing::warn!(
                path = %path.display(),
                "Medical wordlist file not found, skipping"
            );
        }
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

        let tess = tesseract::Tesseract::new(Some(tessdata_str), Some(lang))
            .map_err(|e| ExtractionError::OcrInit(format!("{e:?}")))?;

        // EXT-02-G04: Apply medical wordlist for improved recognition
        let tess = if let Some(ref wordlist_path) = self.medical_wordlist {
            if let Some(path_str) = wordlist_path.to_str() {
                tess.set_variable("user_words_file", path_str)
                    .map_err(|e| ExtractionError::OcrConfig(format!("Failed to set wordlist: {e:?}")))?
            } else {
                tess
            }
        } else {
            tess
        };

        let mut tess = tess
            .set_image_from_mem(image_bytes)
            .map_err(|e| ExtractionError::OcrProcessing(format!("{e:?}")))?;

        let text = tess
            .get_text()
            .map_err(|e| ExtractionError::OcrProcessing(format!("{e:?}")))?;

        let confidence = tess.mean_text_conf().max(0) as f32 / 100.0;

        // EXT-02-G01: Real per-word confidence via TSV output.
        // TSV columns: level page_num block_num par_num line_num word_num left top width height conf text
        // Level 5 = word-level entries with individual confidence scores.
        let word_confidences = match tess.get_tsv_text(0) {
            Ok(tsv) => parse_tsv_word_confidences(&tsv),
            Err(_) => {
                // Fallback: split text with page-mean confidence (no bounding boxes)
                text.split_whitespace()
                    .map(|w| OcrWordResult {
                        text: w.to_string(),
                        confidence,
                        bounding_box: None,
                    })
                    .collect()
            }
        };

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
        let word_confidences: Vec<OcrWordResult> = self
            .text
            .split_whitespace()
            .map(|w| OcrWordResult {
                text: w.to_string(),
                confidence: self.confidence,
                bounding_box: None,
            })
            .collect();

        Ok(OcrPageResult {
            text: self.text.clone(),
            confidence: self.confidence,
            word_confidences,
        })
    }
}

/// Parse Tesseract TSV output to extract per-word confidence and bounding boxes.
/// TSV columns: level page_num block_num par_num line_num word_num left top width height conf text
/// Level 5 = individual word entries. Confidence is 0-100, scaled to 0.0-1.0.
/// EXT-05-G01: Bounding box populated from left/top/width/height columns.
fn parse_tsv_word_confidences(tsv: &str) -> Vec<OcrWordResult> {
    let mut results = Vec::new();

    for line in tsv.lines().skip(1) {
        // Skip header row
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 12 {
            continue;
        }

        // Level 5 = word
        let level: i32 = match fields[0].parse() {
            Ok(l) => l,
            Err(_) => continue,
        };
        if level != 5 {
            continue;
        }

        let conf: i32 = match fields[10].parse() {
            Ok(c) => c,
            Err(_) => continue,
        };

        let word = fields[11].trim();
        if word.is_empty() {
            continue;
        }

        // Tesseract returns -1 for words it can't assign confidence to
        let confidence = if conf < 0 { 0.0 } else { conf as f32 / 100.0 };

        // EXT-05-G01: Extract bounding box from TSV columns 6-9 (left, top, width, height)
        let bounding_box = parse_bounding_box(fields[6], fields[7], fields[8], fields[9]);

        results.push(OcrWordResult {
            text: word.to_string(),
            confidence,
            bounding_box,
        });
    }

    results
}

/// Parse bounding box coordinates from TSV string fields.
/// Returns None if any field fails to parse (graceful degradation).
fn parse_bounding_box(left: &str, top: &str, width: &str, height: &str) -> Option<BoundingBox> {
    Some(BoundingBox {
        x: left.parse().ok()?,
        y: top.parse().ok()?,
        width: width.parse().ok()?,
        height: height.parse().ok()?,
    })
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
        assert_eq!(result.word_confidences[0].text, "Blood");
        assert!((result.word_confidences[0].confidence - 0.85).abs() < f32::EPSILON);
        assert!(result.word_confidences[0].bounding_box.is_none(), "Mock should have no bounding box");
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
        // Default depends on which traineddata files are available
        let valid = ["eng", "eng+fra", "eng+deu", "eng+fra+deu"];
        assert!(
            valid.contains(&engine.default_lang.as_str()),
            "Expected one of {:?}, got {}",
            valid,
            engine.default_lang
        );
    }

    #[cfg(feature = "ocr")]
    #[test]
    fn bundled_tesseract_with_medical_wordlist() {
        let tessdata_dir = std::path::Path::new("/usr/share/tesseract-ocr/5/tessdata");
        if !tessdata_dir.exists() {
            return; // Skip on systems without Tesseract
        }
        let wordlist = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("medical_wordlist.txt");
        let engine = BundledTesseract::new(tessdata_dir)
            .unwrap()
            .with_medical_wordlist(&wordlist);
        assert!(engine.medical_wordlist.is_some());
    }

    #[cfg(feature = "ocr")]
    #[test]
    fn bundled_tesseract_missing_wordlist_stays_none() {
        let tessdata_dir = std::path::Path::new("/usr/share/tesseract-ocr/5/tessdata");
        if !tessdata_dir.exists() {
            return;
        }
        let engine = BundledTesseract::new(tessdata_dir)
            .unwrap()
            .with_medical_wordlist(std::path::Path::new("/nonexistent/wordlist.txt"));
        assert!(engine.medical_wordlist.is_none());
    }

    #[test]
    fn medical_wordlist_file_exists_and_valid() {
        let wordlist = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("medical_wordlist.txt");
        assert!(wordlist.exists(), "medical_wordlist.txt should exist in resources");

        let content = std::fs::read_to_string(&wordlist).unwrap();
        let terms: Vec<&str> = content
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .collect();
        // Should have a reasonable number of medical terms
        assert!(
            terms.len() >= 50,
            "Expected >= 50 terms, got {}",
            terms.len()
        );
        // Spot-check known terms
        assert!(terms.contains(&"Metformin"), "Should contain Metformin");
        assert!(terms.contains(&"Creatinine"), "Should contain Creatinine");
        assert!(terms.contains(&"Hypertension"), "Should contain Hypertension");
    }

    // --- parse_tsv_word_confidences tests (EXT-02-G01) ---

    #[test]
    fn tsv_parser_extracts_word_confidences() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
                   1\t1\t0\t0\t0\t0\t0\t0\t600\t800\t-1\t\n\
                   5\t1\t1\t1\t1\t1\t10\t20\t80\t30\t95\tMetformin\n\
                   5\t1\t1\t1\t1\t2\t100\t20\t60\t30\t88\t500mg\n\
                   5\t1\t1\t1\t2\t1\t10\t60\t120\t30\t72\ttwice\n\
                   5\t1\t1\t1\t2\t2\t140\t60\t80\t30\t70\tdaily";
        let result = parse_tsv_word_confidences(tsv);
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].text, "Metformin");
        assert!((result[0].confidence - 0.95).abs() < f32::EPSILON);
        assert_eq!(result[1].text, "500mg");
        assert!((result[1].confidence - 0.88).abs() < f32::EPSILON);
        assert_eq!(result[2].text, "twice");
        assert!((result[2].confidence - 0.72).abs() < f32::EPSILON);
        assert_eq!(result[3].text, "daily");
        assert!((result[3].confidence - 0.70).abs() < f32::EPSILON);
    }

    #[test]
    fn tsv_parser_extracts_bounding_boxes() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
                   5\t1\t1\t1\t1\t1\t10\t20\t80\t30\t95\tMetformin\n\
                   5\t1\t1\t1\t1\t2\t100\t25\t60\t28\t88\t500mg";
        let result = parse_tsv_word_confidences(tsv);
        assert_eq!(result.len(), 2);

        let bb0 = result[0].bounding_box.as_ref().expect("should have bounding box");
        assert_eq!(bb0.x, 10);
        assert_eq!(bb0.y, 20);
        assert_eq!(bb0.width, 80);
        assert_eq!(bb0.height, 30);

        let bb1 = result[1].bounding_box.as_ref().expect("should have bounding box");
        assert_eq!(bb1.x, 100);
        assert_eq!(bb1.y, 25);
        assert_eq!(bb1.width, 60);
        assert_eq!(bb1.height, 28);
    }

    #[test]
    fn tsv_parser_skips_non_word_levels() {
        // Level 1 = page, 2 = block, 3 = paragraph, 4 = line — all skipped
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
                   1\t1\t0\t0\t0\t0\t0\t0\t600\t800\t-1\t\n\
                   2\t1\t1\t0\t0\t0\t10\t10\t580\t780\t-1\t\n\
                   3\t1\t1\t1\t0\t0\t10\t10\t580\t780\t-1\t\n\
                   4\t1\t1\t1\t1\t0\t10\t20\t200\t30\t-1\t\n\
                   5\t1\t1\t1\t1\t1\t10\t20\t80\t30\t90\tBlood";
        let result = parse_tsv_word_confidences(tsv);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "Blood");
    }

    #[test]
    fn tsv_parser_handles_negative_confidence() {
        // Tesseract returns -1 for unrecognized words
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
                   5\t1\t1\t1\t1\t1\t10\t20\t80\t30\t-1\tgarbled";
        let result = parse_tsv_word_confidences(tsv);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "garbled");
        assert!((result[0].confidence - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn tsv_parser_skips_empty_words() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
                   5\t1\t1\t1\t1\t1\t10\t20\t80\t30\t90\t\n\
                   5\t1\t1\t1\t1\t2\t100\t20\t80\t30\t85\tvalid";
        let result = parse_tsv_word_confidences(tsv);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "valid");
    }

    #[test]
    fn tsv_parser_handles_empty_input() {
        let result = parse_tsv_word_confidences("");
        assert!(result.is_empty());
    }

    #[test]
    fn tsv_parser_handles_header_only() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext";
        let result = parse_tsv_word_confidences(tsv);
        assert!(result.is_empty());
    }

    #[test]
    fn tsv_parser_skips_malformed_lines() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
                   too\tfew\tfields\n\
                   5\t1\t1\t1\t1\t1\t10\t20\t80\t30\t92\tOK\n\
                   notanumber\t1\t1\t1\t1\t1\t10\t20\t80\t30\t50\tbad";
        let result = parse_tsv_word_confidences(tsv);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "OK");
    }
}
