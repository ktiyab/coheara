use chrono::NaiveDate;

use crate::models::enums::DocumentType;

/// Classify document type from MedGemma's response string.
pub fn classify_document_type(type_str: &str) -> DocumentType {
    match type_str.to_lowercase().trim() {
        "prescription" => DocumentType::Prescription,
        "lab_result" | "lab result" | "laboratory" => DocumentType::LabResult,
        "clinical_note" | "clinical note" | "consultation" => DocumentType::ClinicalNote,
        "discharge_summary" | "discharge summary" | "discharge" => DocumentType::DischargeSummary,
        "radiology_report" | "radiology report" | "radiology" | "imaging" => {
            DocumentType::RadiologyReport
        }
        "pharmacy_record" | "pharmacy record" | "pharmacy" => DocumentType::PharmacyRecord,
        _ => DocumentType::Other,
    }
}

/// Parse a date string from MedGemma output (handles various formats).
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
    None
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
}
