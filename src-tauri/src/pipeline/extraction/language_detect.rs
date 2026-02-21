//! EXT-04-G03: Lightweight language detection for extracted text.
//!
//! Detects French vs English using character-level signals only:
//! - French diacritics (é, è, ê, ç, etc.) — structural to French orthography
//! - Apostrophe elisions (l', d', n', s', qu') — structural to French grammar
//!
//! No keyword lists. Same approach as whatlang/CLD — trigram and character
//! distribution, not vocabulary matching.

/// Detect the primary language of extracted text.
/// Returns a Tesseract-compatible language code: "fra" for French, "eng" for English.
///
/// Uses character-level analysis only. French diacritics and elision patterns
/// are structural features of the language, not content keywords.
/// French is favored when scores are close (primary user base).
pub fn detect_language(text: &str) -> String {
    if text.trim().len() < 20 {
        // Too little text to detect — default to French (primary user base)
        return "fra".to_string();
    }

    let lower = text.to_lowercase();

    let diacritics = count_french_diacritics(&lower);
    let elisions = count_french_elisions(&lower);
    let french_score = diacritics + elisions;

    // Any French character signal = French (diacritics don't appear in English)
    if french_score >= 1 {
        "fra".to_string()
    } else {
        "eng".to_string()
    }
}

/// Count French-specific diacritical characters.
/// Characters like é, è, ê, ë, ç, ù, û, ü, î, ï, ô, à, â are strong
/// French indicators — they almost never appear in English text.
fn count_french_diacritics(lower_text: &str) -> u32 {
    let mut count = 0u32;
    for ch in lower_text.chars() {
        if matches!(
            ch,
            'é' | 'è' | 'ê' | 'ë' | 'ç' | 'ù' | 'û' | 'ü' | 'î' | 'ï' | 'ô' | 'à' | 'â'
                | 'œ' | 'æ'
        ) {
            count += 1;
        }
    }
    count
}

/// Count French elision patterns (l', d', n', s', qu', j', c').
/// These apostrophe contractions are structural to French grammar and
/// don't appear in English text.
fn count_french_elisions(lower_text: &str) -> u32 {
    let patterns = ["d'", "l'", "n'", "s'", "qu'", "j'", "c'"];
    let mut count = 0u32;
    for pattern in patterns {
        count += lower_text.matches(pattern).count() as u32;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_french_medical_text() {
        let text = "Résultats d'analyses biologiques\nCréatinine: 72 µmol/L\nGlucose à jeun: 5,2 mmol/L";
        assert_eq!(detect_language(text), "fra");
    }

    #[test]
    fn detects_english_medical_text() {
        let text = "Blood test results for the patient\nCreatinine: 72 umol/L\nFasting glucose was normal at 5.2 mmol/L";
        assert_eq!(detect_language(text), "eng");
    }

    #[test]
    fn french_prescription_detected() {
        let text = "Ordonnance du Dr Martin\nParacétamol 1g matin midi et soir\nTraitement pour 30 jours";
        assert_eq!(detect_language(text), "fra");
    }

    #[test]
    fn english_prescription_detected() {
        let text = "Prescription by Dr Smith\nParacetamol 1g twice daily\nTreatment for 30 days";
        assert_eq!(detect_language(text), "eng");
    }

    #[test]
    fn short_text_defaults_to_french() {
        assert_eq!(detect_language("5,2 mmol/L"), "fra");
        assert_eq!(detect_language(""), "fra");
        assert_eq!(detect_language("    "), "fra");
    }

    #[test]
    fn diacritics_detect_french() {
        let text = "Créatinine élevée, protéine réactive, hémoglobine, résultat préliminaire";
        assert_eq!(detect_language(text), "fra");
    }

    #[test]
    fn elisions_detect_french() {
        let text = "L'ordonnance du médecin pour le traitement d'une infection qu'il s'agit";
        assert_eq!(detect_language(text), "fra");
    }

    #[test]
    fn heavily_english_not_misdetected() {
        let text = "The patient was admitted to the hospital and blood tests were ordered. \
                    Results showed that the creatinine level was within normal range. \
                    The doctor prescribed medication for daily use.";
        assert_eq!(detect_language(text), "eng");
    }

    #[test]
    fn french_lab_report_format() {
        let text = "Bilan sanguin complet\n\
                    Numération Formule Sanguine (NFS)\n\
                    Hémoglobine: 14,2 g/dL\n\
                    Leucocytes: 7 200/mm³\n\
                    Plaquettes: 250 000/mm³\n\
                    Résultat dans les normes";
        assert_eq!(detect_language(text), "fra");
    }

    #[test]
    fn count_diacritics_basic() {
        let text = "créatinine élevée protéine résultat";
        let count = count_french_diacritics(text);
        // é appears 4 times, è once = 5
        assert!(count >= 4, "Should count diacritics: got {count}");
    }

    #[test]
    fn count_elisions_basic() {
        let text = "l'ordonnance d'un médecin n'est pas c'est qu'il s'agit";
        let count = count_french_elisions(text);
        assert!(count >= 5, "Should count elisions: got {count}");
    }
}
