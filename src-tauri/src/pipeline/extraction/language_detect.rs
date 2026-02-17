//! EXT-04-G03: Lightweight language detection for extracted text.
//!
//! Detects French vs English using character patterns and keyword frequency.
//! No external dependencies — uses simple heuristic scoring appropriate for
//! medical document context where the primary languages are French and English.

/// Detected language code (ISO 639-3 compatible with Tesseract).
const FRENCH_INDICATORS: &[&str] = &[
    // Common French words unlikely in English medical text
    "le ", "la ", "les ", "un ", "une ", "des ", "du ", "de ", "et ", "est ",
    "en ", "au ", "aux ", "pour ", "par ", "sur ", "dans ", "avec ", "qui ",
    "que ", "pas ", "son ", "ses ", "mais ", "ou ", "ce ", "cette ",
    // Medical French
    "résultat", "analyse", "ordonnance", "médecin", "traitement",
    "comprimé", "posologie", "quotidien", "matin", "midi", "soir",
    "patient", "examen", "bilan", "sanguin", "urinaire",
    "à jeun", "voie orale", "par jour", "fois par",
    // French-specific patterns
    "d'", "l'", "n'", "s'", "qu'", "j'", "c'",
];

/// English indicators (common English words rarely found in French text).
const ENGLISH_INDICATORS: &[&str] = &[
    "the ", "and ", "was ", "for ", "are ", "but ", "not ", "you ",
    "all ", "can ", "her ", "has ", "his ", "how ", "its ", "may ",
    "our ", "out ", "who ", "did ", "get ", "been ", "from ",
    "have ", "this ", "that ", "with ", "they ", "will ",
    // Medical English
    "patient", "treatment", "medication", "dosage", "daily",
    "twice", "tablet", "diagnosis", "prescription", "results",
    "blood", "urine", "fasting", "normal", "range",
];

/// Detect the primary language of extracted text.
/// Returns a Tesseract-compatible language code: "fra" for French, "eng" for English.
///
/// Uses case-insensitive keyword frequency analysis. French is favored when
/// scores are close, since the primary user base produces French documents.
pub fn detect_language(text: &str) -> String {
    if text.trim().len() < 20 {
        // Too little text to detect — default to French (primary user base)
        return "fra".to_string();
    }

    let lower = text.to_lowercase();

    let french_score = count_indicators(&lower, FRENCH_INDICATORS);
    let english_score = count_indicators(&lower, ENGLISH_INDICATORS);

    // Bonus for French-specific diacritics that are rare in English
    let diacritic_bonus = count_french_diacritics(&lower);

    let total_french = french_score + diacritic_bonus;

    // French wins ties (primary user base)
    if total_french >= english_score {
        "fra".to_string()
    } else {
        "eng".to_string()
    }
}

/// Count how many indicator patterns appear in the text.
fn count_indicators(lower_text: &str, indicators: &[&str]) -> u32 {
    let mut score = 0u32;
    for &indicator in indicators {
        // Count occurrences (each occurrence adds 1)
        score += lower_text.matches(indicator).count() as u32;
    }
    score
}

/// Count French-specific diacritical characters as a language signal.
/// Characters like é, è, ê, ë, ç, ù, û, ü, î, ï, ô, à, â are strong
/// French indicators when they appear frequently.
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
    // Each 2 diacritics = 1 point (weighted to reflect strong French signal)
    count / 2
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
    fn mixed_text_favors_french() {
        // Equal-ish indicators — French should win ties
        let text = "Patient Marie Dubois, résultat de l'analyse pour le traitement";
        assert_eq!(detect_language(text), "fra");
    }

    #[test]
    fn diacritics_boost_french_score() {
        // Text with French diacritics boosts French detection
        let text = "Créatinine élevée, protéine réactive, hémoglobine, résultat préliminaire";
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
    fn count_indicators_basic() {
        let text = "le patient est dans la salle";
        let score = count_indicators(text, FRENCH_INDICATORS);
        assert!(score >= 3, "Should match 'le ', 'est ', 'dans ', 'la ': got {}", score);
    }

    #[test]
    fn count_french_diacritics_basic() {
        let text = "créatinine élevée protéine résultat";
        let count = count_french_diacritics(text);
        // é×4, è×1 = 5 diacritics → 5/3 = 1
        assert!(count >= 1, "Should count diacritics: got {}", count);
    }
}
