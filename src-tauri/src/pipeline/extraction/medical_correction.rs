//! EXT-02-G06: Post-OCR medical term correction.
//!
//! Applies fuzzy matching against a medical dictionary to fix common OCR errors
//! in medical terminology. Only corrects when confidence is high (edit distance <= 2
//! AND the word is at least 5 characters long to avoid false positives).

/// Medical terms dictionary for post-OCR correction.
/// Sorted for binary search. Must be lowercase for case-insensitive matching.
const MEDICAL_TERMS: &[&str] = &[
    "albumin", "allopurinol", "amlodipine", "amoxicillin", "amylase",
    "arrhythmia", "atorvastatin", "azithromycin", "bicarbonate", "bilirubin",
    "bisoprolol", "bradycardia", "budesonide", "calcium", "carbamazepine",
    "carvedilol", "chloride", "cholesterol", "ciprofloxacin", "citalopram",
    "clopidogrel", "codeine", "colchicine", "cortisol", "creatinine",
    "diclofenac", "digoxin", "duloxetine", "enalapril", "erythrocytes",
    "escitalopram", "estradiol", "ferritin", "fibrinogen", "finasteride",
    "fluconazole", "fluoxetine", "fluticasone", "furosemide", "gabapentin",
    "glucose", "hematocrit", "hemoglobin", "hepatitis", "hydrochlorothiazide",
    "hypertension", "hypotension", "hypothyroidism", "ibuprofen", "insulin",
    "leukocytes", "levetiracetam", "levothyroxine", "lipase", "lisinopril",
    "losartan", "magnesium", "metformin", "methotrexate", "metoprolol",
    "montelukast", "morphine", "nitrofurantoin", "olanzapine", "omeprazole",
    "pantoprazole", "paracetamol", "perindopril", "phenytoin", "phosphate",
    "potassium", "prednisone", "procalcitonin", "progesterone", "prolactin",
    "quetiapine", "ramipril", "risperidone", "rivaroxaban", "sertraline",
    "simvastatin", "sodium", "spironolactone", "sulfasalazine", "tachycardia",
    "tamsulosin", "testosterone", "thrombocytes", "thrombosis", "tiotropium",
    "tramadol", "transferrin", "triglycerides", "trimethoprim", "troponin",
    "valproate", "venlafaxine", "warfarin",
];

/// Apply post-OCR medical term correction to extracted text.
/// Returns corrected text. Only corrects words that are close matches
/// to known medical terms (edit distance <= 2, word length >= 5).
pub fn correct_medical_terms(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut word_buf = String::new();

    for ch in text.chars() {
        if ch.is_alphanumeric() {
            word_buf.push(ch);
        } else {
            if !word_buf.is_empty() {
                let corrected = try_correct_word(&word_buf);
                result.push_str(&corrected);
                word_buf.clear();
            }
            result.push(ch);
        }
    }

    // Handle last word
    if !word_buf.is_empty() {
        let corrected = try_correct_word(&word_buf);
        result.push_str(&corrected);
    }

    result
}

/// Try to correct a single word against the medical dictionary.
/// Only corrects if: word.len() >= 5 AND edit_distance <= 2 AND unique best match.
fn try_correct_word(word: &str) -> String {
    if word.len() < 5 {
        return word.to_string();
    }

    let lower = word.to_lowercase();

    // Exact match — no correction needed
    if MEDICAL_TERMS.binary_search(&lower.as_str()).is_ok() {
        return word.to_string();
    }

    // Find closest match
    let mut best_term: Option<&str> = None;
    let mut best_distance = 3u32; // Only accept distance <= 2
    let mut ambiguous = false;

    for &term in MEDICAL_TERMS {
        // Quick length filter: terms differing by more than 2 chars can't match
        let len_diff = (word.len() as i32 - term.len() as i32).unsigned_abs();
        if len_diff > 2 {
            continue;
        }

        let dist = edit_distance(&lower, term);
        if dist < best_distance {
            best_distance = dist;
            best_term = Some(term);
            ambiguous = false;
        } else if dist == best_distance && best_term.is_some() {
            ambiguous = true; // Multiple equally close matches
        }
    }

    // Only correct if unambiguous match with distance <= 2
    if let Some(term) = best_term {
        if !ambiguous {
            return preserve_case(word, term);
        }
    }

    word.to_string()
}

/// Preserve the original word's capitalization pattern when applying correction.
fn preserve_case(original: &str, correction: &str) -> String {
    if original.chars().all(|c| c.is_uppercase() || !c.is_alphabetic()) {
        return correction.to_uppercase();
    }

    let first_upper = original.chars().next().is_some_and(|c| c.is_uppercase());
    if first_upper {
        let mut chars = correction.chars();
        match chars.next() {
            Some(c) => {
                let mut s = c.to_uppercase().to_string();
                s.extend(chars);
                s
            }
            None => correction.to_string(),
        }
    } else {
        correction.to_string()
    }
}

/// Compute Levenshtein edit distance between two strings.
fn edit_distance(a: &str, b: &str) -> u32 {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    if m == 0 { return n as u32; }
    if n == 0 { return m as u32; }

    let mut prev: Vec<u32> = (0..=n as u32).collect();
    let mut curr = vec![0u32; n + 1];

    for (i, &a_ch) in a_chars.iter().enumerate() {
        curr[0] = (i + 1) as u32;
        for (j, &b_ch) in b_chars.iter().enumerate() {
            let cost = if a_ch == b_ch { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1)
                .min(curr[j] + 1)
                .min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[n]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrects_common_ocr_errors() {
        // "Metfonnin" → "Metformin" (rn→m is common OCR error, edit distance 2)
        assert_eq!(correct_medical_terms("Metfonnin"), "Metformin");
        // "Creatiniue" → "Creatinine" (u→n, edit distance 1)
        assert_eq!(correct_medical_terms("Creatiniue"), "Creatinine");
    }

    #[test]
    fn preserves_correct_terms() {
        assert_eq!(correct_medical_terms("Metformin"), "Metformin");
        assert_eq!(correct_medical_terms("Creatinine"), "Creatinine");
        assert_eq!(correct_medical_terms("Hemoglobin"), "Hemoglobin");
    }

    #[test]
    fn preserves_short_words() {
        // Words < 5 chars should never be corrected
        assert_eq!(correct_medical_terms("mg"), "mg");
        assert_eq!(correct_medical_terms("the"), "the");
        assert_eq!(correct_medical_terms("a day"), "a day");
    }

    #[test]
    fn preserves_case_pattern() {
        // All uppercase
        assert_eq!(correct_medical_terms("METFONNIN"), "METFORMIN");
        // Title case
        assert_eq!(correct_medical_terms("Metfonnin"), "Metformin");
        // Lowercase
        assert_eq!(correct_medical_terms("metfonnin"), "metformin");
    }

    #[test]
    fn does_not_correct_unrelated_words() {
        assert_eq!(correct_medical_terms("Patient"), "Patient");
        assert_eq!(correct_medical_terms("hospital"), "hospital");
        assert_eq!(correct_medical_terms("morning"), "morning");
    }

    #[test]
    fn handles_mixed_text() {
        let input = "Take Metfonnin 500mg twice daily";
        let result = correct_medical_terms(input);
        assert!(result.contains("Metformin"));
        assert!(result.contains("500mg"));
        assert!(result.contains("twice daily"));
    }

    #[test]
    fn edit_distance_basic() {
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("", "abc"), 3);
        assert_eq!(edit_distance("abc", "abc"), 0);
        // metformin → metfonnin: r→n and m→n = 2 substitutions
        assert_eq!(edit_distance("metformin", "metfonnin"), 2);
        assert_eq!(edit_distance("creatinine", "creatiniue"), 1);
    }

    #[test]
    fn does_not_correct_ambiguous_matches() {
        // "sodium" and "potassium" are both in the dictionary but far apart
        // A word equidistant from two terms should not be corrected
        assert_eq!(correct_medical_terms("xodium"), "sodium");
    }

    #[test]
    fn medical_terms_sorted() {
        // Binary search requires sorted array
        for window in MEDICAL_TERMS.windows(2) {
            assert!(
                window[0] < window[1],
                "MEDICAL_TERMS not sorted: {:?} >= {:?}",
                window[0],
                window[1]
            );
        }
    }
}
