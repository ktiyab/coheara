use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use uuid::Uuid;

use crate::models::{Diagnosis, Medication};

use super::reference::CoherenceReferenceData;
use super::types::CoherenceAlert;

/// Resolve the canonical generic name for a medication, using alias table if needed.
pub fn resolve_generic_name(med: &Medication, reference: &CoherenceReferenceData) -> String {
    let generic = med.generic_name.to_lowercase();
    if !generic.is_empty() {
        return generic;
    }
    if let Some(brand) = &med.brand_name {
        if let Some(resolved) = reference.resolve_generic(brand) {
            return resolved.to_lowercase();
        }
    }
    generic
}

/// Normalize dose string for comparison (extract numeric mg value).
pub fn normalize_dose(dose: &str) -> String {
    dose.to_lowercase()
        .replace(' ', "")
        .replace("milligrams", "mg")
        .replace("grams", "g")
        .replace("micrograms", "mcg")
}

/// Normalize frequency string for comparison.
pub fn normalize_frequency(freq: &str) -> String {
    let lower = freq.to_lowercase();
    lower
        .replace("twice daily", "2x/day")
        .replace("two times a day", "2x/day")
        .replace("bid", "2x/day")
        .replace("once daily", "1x/day")
        .replace("once a day", "1x/day")
        .replace("qd", "1x/day")
        .replace("three times daily", "3x/day")
        .replace("tid", "3x/day")
        .replace("four times daily", "4x/day")
        .replace("qid", "4x/day")
        .trim()
        .to_string()
}

/// Regex patterns for dose parsing (compiled once via LazyLock).
static RE_MG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d+\.?\d*)\s*(?:mg|milligrams?)").unwrap());
static RE_G: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d+\.?\d*)\s*(?:g|grams?)").unwrap());
static RE_MCG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d+\.?\d*)\s*(?:mcg|micrograms?|ug|µg)").unwrap());
static RE_BARE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d+\.?\d*)$").unwrap());

/// Parse a dose string into milligrams.
/// Handles: "500mg", "1g", "250 mg", "0.5g", "100mcg", "500 milligrams"
pub fn parse_dose_to_mg(dose: &str) -> Option<f64> {
    let lower = dose.to_lowercase().replace(' ', "");

    if let Some(caps) = RE_MG.captures(&lower) {
        return caps.get(1)?.as_str().parse::<f64>().ok();
    }
    if let Some(caps) = RE_G.captures(&lower) {
        return caps.get(1)?.as_str().parse::<f64>().ok().map(|v| v * 1000.0);
    }
    if let Some(caps) = RE_MCG.captures(&lower) {
        return caps
            .get(1)?
            .as_str()
            .parse::<f64>()
            .ok()
            .map(|v| v / 1000.0);
    }
    if let Some(caps) = RE_BARE.captures(&lower) {
        return caps.get(1)?.as_str().parse::<f64>().ok();
    }

    None
}

/// Format a milligram value for display.
pub fn format_dose_mg(mg: f64) -> String {
    if mg >= 1000.0 {
        format!("{}g", mg / 1000.0)
    } else if mg < 1.0 {
        format!("{}mcg", mg * 1000.0)
    } else {
        format!("{}mg", mg)
    }
}

/// Get display name for a medication (prefer brand_name, fall back to generic_name).
pub fn display_name(med: &Medication) -> String {
    med.brand_name
        .clone()
        .unwrap_or_else(|| med.generic_name.clone())
}

/// Check if a medication appears to relate to a diagnosis.
pub fn medication_relates_to_diagnosis(med: &Medication, diag: &Diagnosis) -> bool {
    let diag_lower = diag.name.to_lowercase();

    if let Some(ref condition) = med.condition {
        let cond_lower = condition.to_lowercase();
        if cond_lower.contains(&diag_lower) || diag_lower.contains(&cond_lower) {
            return true;
        }
    }

    if let Some(ref reason) = med.reason_start {
        let reason_lower = reason.to_lowercase();
        if reason_lower.contains(&diag_lower) || diag_lower.contains(&reason_lower) {
            return true;
        }
    }

    false
}

/// Known drug family groupings for cross-allergy detection.
/// Phase 1: hardcoded common families. Phase 3: full pharmacological DB.
pub fn is_same_drug_family(allergen: &str, ingredient: &str) -> bool {
    let families: &[&[&str]] = &[
        // Penicillin family
        &[
            "penicillin",
            "amoxicillin",
            "ampicillin",
            "piperacillin",
            "oxacillin",
            "nafcillin",
            "dicloxacillin",
            "flucloxacillin",
        ],
        // Cephalosporin family
        &[
            "cephalexin",
            "cefazolin",
            "ceftriaxone",
            "cefuroxime",
            "cefixime",
            "cefpodoxime",
            "ceftazidime",
        ],
        // Sulfonamide family
        &[
            "sulfamethoxazole",
            "sulfasalazine",
            "sulfadiazine",
            "trimethoprim-sulfamethoxazole",
            "sulfisoxazole",
        ],
        // NSAID family
        &[
            "ibuprofen",
            "naproxen",
            "diclofenac",
            "indomethacin",
            "piroxicam",
            "meloxicam",
            "celecoxib",
            "aspirin",
        ],
        // Statin family
        &[
            "atorvastatin",
            "rosuvastatin",
            "simvastatin",
            "pravastatin",
            "lovastatin",
            "fluvastatin",
            "pitavastatin",
        ],
        // ACE inhibitor family
        &[
            "lisinopril",
            "enalapril",
            "ramipril",
            "captopril",
            "benazepril",
            "fosinopril",
            "quinapril",
            "perindopril",
        ],
        // Opioid family
        &[
            "morphine",
            "codeine",
            "hydrocodone",
            "oxycodone",
            "tramadol",
            "fentanyl",
            "methadone",
            "hydromorphone",
        ],
        // Fluoroquinolone family
        &[
            "ciprofloxacin",
            "levofloxacin",
            "moxifloxacin",
            "norfloxacin",
            "ofloxacin",
        ],
        // Macrolide family
        &["azithromycin", "clarithromycin", "erythromycin"],
        // Tetracycline family
        &["tetracycline", "doxycycline", "minocycline"],
    ];

    for family in families {
        let allergen_in = family
            .iter()
            .any(|&member| allergen.contains(member) || member.contains(allergen));
        let ingredient_in = family
            .iter()
            .any(|&member| ingredient.contains(member) || member.contains(ingredient));
        if allergen_in && ingredient_in {
            return true;
        }
    }

    false
}

/// Check if two entity ID sets refer to the same entities (order-independent).
pub fn entities_match(a: &[Uuid], b: &[Uuid]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut sorted_a: Vec<Uuid> = a.to_vec();
    let mut sorted_b: Vec<Uuid> = b.to_vec();
    sorted_a.sort();
    sorted_b.sort();
    sorted_a == sorted_b
}

/// Remove symmetric duplicates (A conflicts with B == B conflicts with A).
pub fn dedup_symmetric_alerts(alerts: &mut Vec<CoherenceAlert>) {
    let mut seen_pairs: HashSet<(Uuid, Uuid)> = HashSet::new();
    alerts.retain(|alert| {
        if alert.entity_ids.len() >= 2 {
            let a = alert.entity_ids[0];
            let b = alert.entity_ids[1];
            let pair = if a < b { (a, b) } else { (b, a) };
            seen_pairs.insert(pair)
        } else {
            true
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Dose parsing (T-30, T-31, T-32) ---

    #[test]
    fn parse_dose_mg() {
        assert_eq!(parse_dose_to_mg("500mg"), Some(500.0));
        assert_eq!(parse_dose_to_mg("500 mg"), Some(500.0));
        assert_eq!(parse_dose_to_mg("1.5g"), Some(1500.0));
        assert_eq!(parse_dose_to_mg("250mcg"), Some(0.25));
        assert_eq!(parse_dose_to_mg("100 micrograms"), Some(0.1));
        assert_eq!(parse_dose_to_mg("500"), Some(500.0));
        assert_eq!(parse_dose_to_mg("unknown"), None);
        assert_eq!(parse_dose_to_mg(""), None);
    }

    // --- Frequency normalization (T-33) ---

    #[test]
    fn normalize_frequency_synonyms() {
        assert_eq!(
            normalize_frequency("twice daily"),
            normalize_frequency("BID")
        );
        assert_eq!(
            normalize_frequency("once daily"),
            normalize_frequency("QD")
        );
        assert_eq!(
            normalize_frequency("three times daily"),
            normalize_frequency("TID")
        );
    }

    // --- Dose normalization ---

    #[test]
    fn normalize_dose_equivalents() {
        assert_eq!(normalize_dose("500 mg"), normalize_dose("500mg"));
        assert_eq!(normalize_dose("500 milligrams"), normalize_dose("500mg"));
    }

    // --- Drug family matching (T-15, T-16) ---

    #[test]
    fn drug_family_penicillin() {
        assert!(is_same_drug_family("penicillin", "amoxicillin"));
        assert!(is_same_drug_family("amoxicillin", "penicillin"));
        assert!(!is_same_drug_family("penicillin", "ibuprofen"));
    }

    #[test]
    fn drug_family_nsaid() {
        assert!(is_same_drug_family("aspirin", "ibuprofen"));
        assert!(is_same_drug_family("ibuprofen", "naproxen"));
        assert!(!is_same_drug_family("ibuprofen", "amoxicillin"));
    }

    #[test]
    fn drug_family_no_match() {
        assert!(!is_same_drug_family("metformin", "atorvastatin"));
    }

    // --- Entity matching ---

    #[test]
    fn entity_match_order_independent() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        assert!(entities_match(&[a, b], &[b, a]));
        assert!(entities_match(&[a, b], &[a, b]));
        assert!(!entities_match(&[a], &[a, b]));
    }

    // --- Format dose ---

    #[test]
    fn format_dose_mg_display() {
        assert_eq!(format_dose_mg(500.0), "500mg");
        assert_eq!(format_dose_mg(1000.0), "1g");
        assert_eq!(format_dose_mg(0.5), "500mcg");
    }
}
