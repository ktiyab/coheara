use chrono::NaiveDate;

use super::types::ExtractedEntities;
use crate::models::enums::DocumentType;

/// Classify document type from MedGemma's response string.
/// Handles English and French document type names.
pub fn classify_document_type(type_str: &str) -> DocumentType {
    match type_str.to_lowercase().trim() {
        // English
        "prescription" => DocumentType::Prescription,
        "lab_result" | "lab result" | "laboratory" => DocumentType::LabResult,
        "clinical_note" | "clinical note" | "consultation" => DocumentType::ClinicalNote,
        "discharge_summary" | "discharge summary" | "discharge" => DocumentType::DischargeSummary,
        "radiology_report" | "radiology report" | "radiology" | "imaging" => {
            DocumentType::RadiologyReport
        }
        "pharmacy_record" | "pharmacy record" | "pharmacy" => DocumentType::PharmacyRecord,
        // French (K.1)
        "ordonnance" => DocumentType::Prescription,
        "résultat" | "résultats" | "résultat de laboratoire" | "résultats de laboratoire"
        | "bilan sanguin" | "bilan biologique" | "analyse" | "analyses" => DocumentType::LabResult,
        "note clinique" | "compte rendu" | "compte-rendu" | "consultation médicale" => {
            DocumentType::ClinicalNote
        }
        "lettre de sortie" | "résumé de sortie" | "compte rendu de sortie" => {
            DocumentType::DischargeSummary
        }
        "radiologie" | "imagerie" | "compte rendu radiologique" | "irm" | "scanner" => {
            DocumentType::RadiologyReport
        }
        "pharmacie" | "fiche pharmacie" | "dispensation" => DocumentType::PharmacyRecord,
        _ => DocumentType::Other,
    }
}

/// Infer document type from extracted entities when classifier returns Other (K.2).
pub fn classify_from_entities(entities: &ExtractedEntities) -> DocumentType {
    let has_meds = !entities.medications.is_empty();
    let has_labs = !entities.lab_results.is_empty();
    let has_procedures = !entities.procedures.is_empty();
    let has_referrals = !entities.referrals.is_empty();

    // Strong signals
    if has_labs && !has_meds {
        return DocumentType::LabResult;
    }
    if has_meds && !has_labs && !has_procedures {
        return DocumentType::Prescription;
    }

    // Mixed content
    if has_procedures && has_meds {
        return DocumentType::ClinicalNote;
    }
    if has_referrals {
        return DocumentType::ClinicalNote;
    }

    DocumentType::Other
}

/// Parse a date string from MedGemma output (handles various formats).
/// Supports: ISO 8601, European DD/MM/YYYY, US MM/DD/YYYY, French textual dates.
pub fn parse_document_date(date_str: &str) -> Option<NaiveDate> {
    let trimmed = date_str.trim();
    if trimmed.is_empty() || trimmed == "null" || trimmed == "NOT_FOUND" {
        return None;
    }

    // ISO 8601: YYYY-MM-DD
    if let Ok(d) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        return Some(d);
    }
    // European: DD/MM/YYYY
    if let Ok(d) = NaiveDate::parse_from_str(trimmed, "%d/%m/%Y") {
        return Some(d);
    }
    // European dash: DD-MM-YYYY
    if let Ok(d) = NaiveDate::parse_from_str(trimmed, "%d-%m-%Y") {
        return Some(d);
    }
    // US: MM/DD/YYYY
    if let Ok(d) = NaiveDate::parse_from_str(trimmed, "%m/%d/%Y") {
        return Some(d);
    }
    // French textual: "15 janvier 2024", "1er mars 2024" (K.3)
    if let Some(d) = parse_french_textual_date(trimmed) {
        return Some(d);
    }
    None
}

/// Parse French textual date like "15 janvier 2024" or "1er mars 2024".
fn parse_french_textual_date(text: &str) -> Option<NaiveDate> {
    let lower = text.to_lowercase();
    let parts: Vec<&str> = lower.split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }

    // Parse day (handle "1er")
    let day_str = parts[0].trim_end_matches("er").trim_end_matches("ème");
    let day: u32 = day_str.parse().ok()?;

    // Parse month
    let month = match parts[1] {
        "janvier" => 1,
        "février" | "fevrier" => 2,
        "mars" => 3,
        "avril" => 4,
        "mai" => 5,
        "juin" => 6,
        "juillet" => 7,
        "août" | "aout" => 8,
        "septembre" => 9,
        "octobre" => 10,
        "novembre" => 11,
        "décembre" | "decembre" => 12,
        _ => return None,
    };

    // Parse year
    let year: i32 = parts[2].parse().ok()?;

    NaiveDate::from_ymd_opt(year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_prescription() {
        assert!(matches!(
            classify_document_type("prescription"),
            DocumentType::Prescription
        ));
    }

    #[test]
    fn classify_lab_result_variants() {
        assert!(matches!(
            classify_document_type("lab_result"),
            DocumentType::LabResult
        ));
        assert!(matches!(
            classify_document_type("Lab Result"),
            DocumentType::LabResult
        ));
        assert!(matches!(
            classify_document_type("laboratory"),
            DocumentType::LabResult
        ));
    }

    #[test]
    fn classify_clinical_note() {
        assert!(matches!(
            classify_document_type("clinical_note"),
            DocumentType::ClinicalNote
        ));
        assert!(matches!(
            classify_document_type("consultation"),
            DocumentType::ClinicalNote
        ));
    }

    #[test]
    fn classify_discharge_summary() {
        assert!(matches!(
            classify_document_type("discharge_summary"),
            DocumentType::DischargeSummary
        ));
        assert!(matches!(
            classify_document_type("discharge"),
            DocumentType::DischargeSummary
        ));
    }

    #[test]
    fn classify_radiology() {
        assert!(matches!(
            classify_document_type("radiology_report"),
            DocumentType::RadiologyReport
        ));
        assert!(matches!(
            classify_document_type("imaging"),
            DocumentType::RadiologyReport
        ));
    }

    #[test]
    fn classify_pharmacy() {
        assert!(matches!(
            classify_document_type("pharmacy_record"),
            DocumentType::PharmacyRecord
        ));
    }

    #[test]
    fn classify_unknown_as_other() {
        assert!(matches!(
            classify_document_type("unknown_type"),
            DocumentType::Other
        ));
        assert!(matches!(
            classify_document_type("something else"),
            DocumentType::Other
        ));
    }

    #[test]
    fn parse_iso_date() {
        assert_eq!(
            parse_document_date("2024-01-15"),
            Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap())
        );
    }

    #[test]
    fn parse_european_date() {
        assert_eq!(
            parse_document_date("15/01/2024"),
            Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap())
        );
    }

    #[test]
    fn parse_european_dash_date() {
        assert_eq!(
            parse_document_date("15-01-2024"),
            Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap())
        );
    }

    #[test]
    fn parse_invalid_date_returns_none() {
        assert_eq!(parse_document_date("invalid"), None);
        assert_eq!(parse_document_date(""), None);
        assert_eq!(parse_document_date("null"), None);
        assert_eq!(parse_document_date("NOT_FOUND"), None);
    }

    // ── K.1: French document type classification ─────────────────────

    #[test]
    fn classify_french_ordonnance() {
        assert!(matches!(
            classify_document_type("ordonnance"),
            DocumentType::Prescription
        ));
    }

    #[test]
    fn classify_french_lab_results() {
        assert!(matches!(
            classify_document_type("résultats de laboratoire"),
            DocumentType::LabResult
        ));
        assert!(matches!(
            classify_document_type("bilan sanguin"),
            DocumentType::LabResult
        ));
        assert!(matches!(
            classify_document_type("analyses"),
            DocumentType::LabResult
        ));
    }

    #[test]
    fn classify_french_clinical_note() {
        assert!(matches!(
            classify_document_type("compte rendu"),
            DocumentType::ClinicalNote
        ));
        assert!(matches!(
            classify_document_type("consultation médicale"),
            DocumentType::ClinicalNote
        ));
    }

    #[test]
    fn classify_french_radiology() {
        assert!(matches!(
            classify_document_type("radiologie"),
            DocumentType::RadiologyReport
        ));
        assert!(matches!(
            classify_document_type("imagerie"),
            DocumentType::RadiologyReport
        ));
    }

    #[test]
    fn classify_french_discharge() {
        assert!(matches!(
            classify_document_type("lettre de sortie"),
            DocumentType::DischargeSummary
        ));
    }

    // ── K.2: Entity-based fallback classification ────────────────────

    #[test]
    fn fallback_meds_only_is_prescription() {
        use super::super::types::*;
        let entities = ExtractedEntities {
            medications: vec![ExtractedMedication {
                generic_name: Some("Metformin".into()),
                brand_name: None,
                dose: "500mg".into(),
                frequency: "daily".into(),
                frequency_type: "scheduled".into(),
                route: "oral".into(),
                reason: None,
                instructions: vec![],
                is_compound: false,
                compound_ingredients: vec![],
                tapering_steps: vec![],
                max_daily_dose: None,
                condition: None,
                confidence: 0.0,
            }],
            ..Default::default()
        };
        assert!(matches!(
            classify_from_entities(&entities),
            DocumentType::Prescription
        ));
    }

    #[test]
    fn fallback_labs_only_is_lab_result() {
        use super::super::types::*;
        let entities = ExtractedEntities {
            lab_results: vec![ExtractedLabResult {
                test_name: "Potassium".into(),
                test_code: None,
                value: Some(4.2),
                value_text: None,
                unit: Some("mmol/L".into()),
                reference_range_low: Some(3.5),
                reference_range_high: Some(5.0),
                reference_range_text: None,
                abnormal_flag: None,
                collection_date: None,
                confidence: 0.0,
            }],
            ..Default::default()
        };
        assert!(matches!(
            classify_from_entities(&entities),
            DocumentType::LabResult
        ));
    }

    #[test]
    fn fallback_empty_is_other() {
        use super::super::types::ExtractedEntities;
        assert!(matches!(
            classify_from_entities(&ExtractedEntities::default()),
            DocumentType::Other
        ));
    }

    // ── K.3: French textual date parsing ─────────────────────────────

    #[test]
    fn parse_french_textual_date_janvier() {
        assert_eq!(
            parse_document_date("15 janvier 2024"),
            Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap())
        );
    }

    #[test]
    fn parse_french_date_premier_mars() {
        assert_eq!(
            parse_document_date("1er mars 2024"),
            Some(NaiveDate::from_ymd_opt(2024, 3, 1).unwrap())
        );
    }

    #[test]
    fn parse_french_date_decembre() {
        assert_eq!(
            parse_document_date("25 décembre 2023"),
            Some(NaiveDate::from_ymd_opt(2023, 12, 25).unwrap())
        );
    }

    #[test]
    fn parse_french_date_accented_variants() {
        assert_eq!(
            parse_document_date("14 février 2024"),
            Some(NaiveDate::from_ymd_opt(2024, 2, 14).unwrap())
        );
        assert_eq!(
            parse_document_date("15 août 2024"),
            Some(NaiveDate::from_ymd_opt(2024, 8, 15).unwrap())
        );
    }
}
