//! L3-04 Review Screen — types and repository functions.
//!
//! Provides the data layer for the side-by-side document review screen:
//! original document display, extracted field review with confidence flagging,
//! inline corrections, confirm/reject flows, and trust metrics updates.
//! All functions operate on the profile's SQLite database via rusqlite.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DatabaseError;
use crate::pipeline::structuring::types::{ExtractedEntities, StructuringResult};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Complete data needed to render the review screen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewData {
    pub document_id: Uuid,
    pub original_file_path: String,
    pub original_file_type: OriginalFileType,
    pub document_type: String,
    pub document_date: Option<String>,
    pub professional_name: Option<String>,
    pub professional_specialty: Option<String>,
    pub structured_markdown: String,
    pub extracted_fields: Vec<ExtractedField>,
    pub plausibility_warnings: Vec<PlausibilityWarning>,
    pub overall_confidence: f32,
}

/// The type of the original file for rendering.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OriginalFileType {
    Image,
    Pdf,
}

/// A single extracted field for review with confidence and category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedField {
    pub id: Uuid,
    pub entity_type: EntityCategory,
    pub entity_index: usize,
    pub field_name: String,
    pub display_label: String,
    pub value: String,
    pub confidence: f32,
    pub is_flagged: bool,
    pub source_hint: Option<String>,
}

/// Category of entity for color-coding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EntityCategory {
    Medication,
    LabResult,
    Diagnosis,
    Allergy,
    Procedure,
    Referral,
    Professional,
    Date,
}

/// A plausibility warning from the coherence engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlausibilityWarning {
    pub field_id: Uuid,
    pub warning_type: PlausibilityType,
    pub message: String,
    pub severity: WarningSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlausibilityType {
    DoseUnusuallyHigh,
    DoseUnusuallyLow,
    FrequencyUnusual,
    LabValueCritical,
    AllergyConflict,
    DuplicateMedication,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WarningSeverity {
    Info,
    Warning,
    Critical,
}

/// A field correction submitted by the patient.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldCorrection {
    pub field_id: Uuid,
    pub original_value: String,
    pub corrected_value: String,
}

/// Result of confirming a review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewConfirmResult {
    pub document_id: Uuid,
    pub status: ReviewOutcome,
    pub entities_stored: EntitiesStoredSummary,
    pub corrections_applied: usize,
    pub chunks_stored: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReviewOutcome {
    Confirmed,
    Corrected,
}

/// Summary of entities stored, for the frontend confirm result.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EntitiesStoredSummary {
    pub medications: usize,
    pub lab_results: usize,
    pub diagnoses: usize,
    pub allergies: usize,
    pub procedures: usize,
    pub referrals: usize,
    pub instructions: usize,
}

/// Result of rejecting a review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRejectResult {
    pub document_id: Uuid,
    pub reason: Option<String>,
    /// True if the document can be reprocessed (retry action was chosen).
    pub can_retry: bool,
}

/// Confidence threshold below which fields are flagged.
pub const CONFIDENCE_THRESHOLD: f32 = 0.70;

// ---------------------------------------------------------------------------
// Repository / Helper Functions
// ---------------------------------------------------------------------------

/// Flatten extracted entities into a flat list of fields for the review UI.
///
/// Each entity field becomes a separate `ExtractedField` with a unique ID,
/// a category for color-coding, and a confidence flag for low-quality extractions.
pub fn flatten_entities_to_fields(structuring: &StructuringResult) -> Vec<ExtractedField> {
    let mut fields = Vec::new();

    // Medications: generic_name + dose + frequency
    for (i, med) in structuring.extracted_entities.medications.iter().enumerate() {
        if let Some(ref name) = med.generic_name {
            fields.push(ExtractedField {
                id: Uuid::new_v4(),
                entity_type: EntityCategory::Medication,
                entity_index: i,
                field_name: "generic_name".into(),
                display_label: "Medication name".into(),
                value: name.clone(),
                confidence: med.confidence,
                is_flagged: med.confidence < CONFIDENCE_THRESHOLD,
                source_hint: None,
            });
        }
        fields.push(ExtractedField {
            id: Uuid::new_v4(),
            entity_type: EntityCategory::Medication,
            entity_index: i,
            field_name: "dose".into(),
            display_label: "Dose".into(),
            value: med.dose.clone(),
            confidence: med.confidence,
            is_flagged: med.confidence < CONFIDENCE_THRESHOLD,
            source_hint: None,
        });
        fields.push(ExtractedField {
            id: Uuid::new_v4(),
            entity_type: EntityCategory::Medication,
            entity_index: i,
            field_name: "frequency".into(),
            display_label: "Frequency".into(),
            value: med.frequency.clone(),
            confidence: med.confidence,
            is_flagged: med.confidence < CONFIDENCE_THRESHOLD,
            source_hint: None,
        });
    }

    // Lab results: test_name + value + unit
    for (i, lab) in structuring.extracted_entities.lab_results.iter().enumerate() {
        fields.push(ExtractedField {
            id: Uuid::new_v4(),
            entity_type: EntityCategory::LabResult,
            entity_index: i,
            field_name: "test_name".into(),
            display_label: "Test name".into(),
            value: lab.test_name.clone(),
            confidence: lab.confidence,
            is_flagged: lab.confidence < CONFIDENCE_THRESHOLD,
            source_hint: None,
        });
        if let Some(val) = lab.value {
            fields.push(ExtractedField {
                id: Uuid::new_v4(),
                entity_type: EntityCategory::LabResult,
                entity_index: i,
                field_name: "value".into(),
                display_label: "Value".into(),
                value: val.to_string(),
                confidence: lab.confidence,
                is_flagged: lab.confidence < CONFIDENCE_THRESHOLD,
                source_hint: None,
            });
        }
        if let Some(ref unit) = lab.unit {
            fields.push(ExtractedField {
                id: Uuid::new_v4(),
                entity_type: EntityCategory::LabResult,
                entity_index: i,
                field_name: "unit".into(),
                display_label: "Unit".into(),
                value: unit.clone(),
                confidence: lab.confidence,
                is_flagged: lab.confidence < CONFIDENCE_THRESHOLD,
                source_hint: None,
            });
        }
    }

    // Diagnoses: name
    for (i, diag) in structuring.extracted_entities.diagnoses.iter().enumerate() {
        fields.push(ExtractedField {
            id: Uuid::new_v4(),
            entity_type: EntityCategory::Diagnosis,
            entity_index: i,
            field_name: "name".into(),
            display_label: "Diagnosis".into(),
            value: diag.name.clone(),
            confidence: diag.confidence,
            is_flagged: diag.confidence < CONFIDENCE_THRESHOLD,
            source_hint: None,
        });
    }

    // Allergies: allergen
    for (i, allergy) in structuring.extracted_entities.allergies.iter().enumerate() {
        fields.push(ExtractedField {
            id: Uuid::new_v4(),
            entity_type: EntityCategory::Allergy,
            entity_index: i,
            field_name: "allergen".into(),
            display_label: "Allergen".into(),
            value: allergy.allergen.clone(),
            confidence: allergy.confidence,
            is_flagged: allergy.confidence < CONFIDENCE_THRESHOLD,
            source_hint: None,
        });
    }

    // Procedures: name
    for (i, proc) in structuring.extracted_entities.procedures.iter().enumerate() {
        fields.push(ExtractedField {
            id: Uuid::new_v4(),
            entity_type: EntityCategory::Procedure,
            entity_index: i,
            field_name: "name".into(),
            display_label: "Procedure".into(),
            value: proc.name.clone(),
            confidence: proc.confidence,
            is_flagged: proc.confidence < CONFIDENCE_THRESHOLD,
            source_hint: None,
        });
    }

    // Referrals: referred_to
    for (i, referral) in structuring.extracted_entities.referrals.iter().enumerate() {
        fields.push(ExtractedField {
            id: Uuid::new_v4(),
            entity_type: EntityCategory::Referral,
            entity_index: i,
            field_name: "referred_to".into(),
            display_label: "Referred to".into(),
            value: referral.referred_to.clone(),
            confidence: referral.confidence,
            is_flagged: referral.confidence < CONFIDENCE_THRESHOLD,
            source_hint: None,
        });
    }

    // Professional: name
    if let Some(ref prof) = structuring.professional {
        fields.push(ExtractedField {
            id: Uuid::new_v4(),
            entity_type: EntityCategory::Professional,
            entity_index: 0,
            field_name: "name".into(),
            display_label: "Professional".into(),
            value: prof.name.clone(),
            confidence: structuring.structuring_confidence,
            is_flagged: structuring.structuring_confidence < CONFIDENCE_THRESHOLD,
            source_hint: None,
        });
    }

    // Document date
    if let Some(date) = structuring.document_date {
        fields.push(ExtractedField {
            id: Uuid::new_v4(),
            entity_type: EntityCategory::Date,
            entity_index: 0,
            field_name: "document_date".into(),
            display_label: "Document date".into(),
            value: date.to_string(),
            confidence: structuring.structuring_confidence,
            is_flagged: structuring.structuring_confidence < CONFIDENCE_THRESHOLD,
            source_hint: None,
        });
    }

    fields
}

/// Generate plausibility warnings during document review (RS-L3-04-001).
///
/// Runs four checks against extracted entities:
/// 1. Medication dose plausibility (against dose_references table)
/// 2. Critical lab value detection (abnormal flags + reference ranges)
/// 3. Allergy-medication conflicts (extracted + DB allergies vs medications)
/// 4. Duplicate medication detection (within document + against DB)
pub fn generate_plausibility_warnings(
    conn: &Connection,
    structuring: &StructuringResult,
    fields: &[ExtractedField],
) -> Vec<PlausibilityWarning> {
    let mut warnings = Vec::new();

    warnings.extend(check_dose_plausibility(conn, structuring, fields));
    warnings.extend(check_critical_lab_values(structuring, fields));
    warnings.extend(check_allergy_conflicts(conn, structuring, fields));
    warnings.extend(check_duplicate_medications(conn, structuring, fields));

    warnings
}

/// Find the field ID for an entity by type, index, and preferred field name.
/// Falls back to `fallback_field` if preferred is not found.
fn find_field_id(
    fields: &[ExtractedField],
    entity_type: &EntityCategory,
    entity_index: usize,
    preferred_field: &str,
    fallback_field: &str,
) -> Option<Uuid> {
    fields
        .iter()
        .find(|f| {
            f.entity_type == *entity_type
                && f.entity_index == entity_index
                && f.field_name == preferred_field
        })
        .or_else(|| {
            fields.iter().find(|f| {
                f.entity_type == *entity_type
                    && f.entity_index == entity_index
                    && f.field_name == fallback_field
            })
        })
        .map(|f| f.id)
}

/// Check medication doses against reference ranges.
fn check_dose_plausibility(
    conn: &Connection,
    structuring: &StructuringResult,
    fields: &[ExtractedField],
) -> Vec<PlausibilityWarning> {
    let mut warnings = Vec::new();

    for field in fields {
        if field.entity_type != EntityCategory::Medication || field.field_name != "dose" {
            continue;
        }

        let med = match structuring.extracted_entities.medications.get(field.entity_index) {
            Some(m) => m,
            None => continue,
        };

        let med_name = match &med.generic_name {
            Some(n) => n.as_str(),
            None => continue,
        };

        let dose_value = match parse_dose_value(&med.dose) {
            Some(v) => v,
            None => continue,
        };

        let result = match crate::trust::check_dose_plausibility(conn, med_name, dose_value, "mg")
        {
            Ok(r) => r,
            Err(_) => continue,
        };

        let (warning_type, severity, message) = match &result.plausibility {
            crate::trust::PlausibilityResult::Plausible => continue,
            crate::trust::PlausibilityResult::VeryHighDose { message } => (
                PlausibilityType::DoseUnusuallyHigh,
                WarningSeverity::Critical,
                message.clone(),
            ),
            crate::trust::PlausibilityResult::HighDose { message } => (
                PlausibilityType::DoseUnusuallyHigh,
                WarningSeverity::Warning,
                message.clone(),
            ),
            crate::trust::PlausibilityResult::LowDose { message } => (
                PlausibilityType::DoseUnusuallyLow,
                WarningSeverity::Info,
                message.clone(),
            ),
            crate::trust::PlausibilityResult::UnknownMedication => continue,
        };

        warnings.push(PlausibilityWarning {
            field_id: field.id,
            warning_type,
            message,
            severity,
        });
    }

    warnings
}

/// Check extracted lab values for critical results.
///
/// Flags labs where `abnormal_flag` contains "critical" or "panic",
/// or where the value is far outside the reference range (>2× high or <0.5× low).
fn check_critical_lab_values(
    structuring: &StructuringResult,
    fields: &[ExtractedField],
) -> Vec<PlausibilityWarning> {
    let mut warnings = Vec::new();

    for (i, lab) in structuring.extracted_entities.lab_results.iter().enumerate() {
        let is_critical = lab.abnormal_flag.as_ref().is_some_and(|flag| {
            let lower = flag.to_lowercase();
            lower.contains("critical") || lower.contains("panic")
        });

        let is_far_outside = match (lab.value, lab.reference_range_low, lab.reference_range_high) {
            (Some(val), _, Some(high)) if high > 0.0 && val > high * 2.0 => true,
            (Some(val), Some(low), _) if low > 0.0 && val < low * 0.5 => true,
            _ => false,
        };

        if !is_critical && !is_far_outside {
            continue;
        }

        let field_id = match find_field_id(fields, &EntityCategory::LabResult, i, "value", "test_name") {
            Some(id) => id,
            None => continue,
        };

        let severity = if is_critical {
            WarningSeverity::Critical
        } else {
            WarningSeverity::Warning
        };

        let value_str = lab.value.map_or_else(|| "unknown".to_string(), |v| v.to_string());
        let unit_str = lab.unit.as_deref().unwrap_or("");

        let message = if is_critical {
            format!(
                "{} result ({} {}) is flagged as critical. Please verify with your healthcare provider.",
                lab.test_name, value_str, unit_str
            )
        } else {
            format!(
                "{} result ({} {}) is significantly outside the expected range. Please verify this value.",
                lab.test_name, value_str, unit_str
            )
        };

        warnings.push(PlausibilityWarning {
            field_id,
            warning_type: PlausibilityType::LabValueCritical,
            message,
            severity,
        });
    }

    warnings
}

/// Check extracted medications against known allergies (both extracted and in DB).
///
/// Uses case-insensitive substring matching between medication generic names
/// and allergen names. The coherence engine performs more sophisticated drug
/// family matching after storage.
fn check_allergy_conflicts(
    conn: &Connection,
    structuring: &StructuringResult,
    fields: &[ExtractedField],
) -> Vec<PlausibilityWarning> {
    let mut warnings = Vec::new();

    // Collect unique allergen names from extracted entities
    let mut allergens: Vec<String> = structuring
        .extracted_entities
        .allergies
        .iter()
        .map(|a| a.allergen.to_lowercase())
        .collect();

    // Also include existing DB allergies
    if let Ok(db_allergies) = crate::db::repository::get_all_allergies(conn) {
        for allergy in &db_allergies {
            let lower = allergy.allergen.to_lowercase();
            if !allergens.contains(&lower) {
                allergens.push(lower);
            }
        }
    }

    if allergens.is_empty() {
        return warnings;
    }

    for (i, med) in structuring.extracted_entities.medications.iter().enumerate() {
        let med_name = match &med.generic_name {
            Some(n) => n.to_lowercase(),
            None => continue,
        };

        for allergen in &allergens {
            if med_name.contains(allergen.as_str()) || allergen.contains(med_name.as_str()) {
                let field_id = match find_field_id(
                    fields,
                    &EntityCategory::Medication,
                    i,
                    "generic_name",
                    "dose",
                ) {
                    Some(id) => id,
                    None => continue,
                };

                warnings.push(PlausibilityWarning {
                    field_id,
                    warning_type: PlausibilityType::AllergyConflict,
                    message: format!(
                        "You have a recorded allergy to {}. {} may conflict with this allergy.",
                        allergen,
                        med.generic_name.as_deref().unwrap_or("This medication")
                    ),
                    severity: WarningSeverity::Critical,
                });

                break; // One warning per medication
            }
        }
    }

    warnings
}

/// Check for duplicate medications within the document and against existing records.
fn check_duplicate_medications(
    conn: &Connection,
    structuring: &StructuringResult,
    fields: &[ExtractedField],
) -> Vec<PlausibilityWarning> {
    let mut warnings = Vec::new();

    let existing_meds = crate::db::repository::get_active_medications(conn).unwrap_or_default();

    for (i, med) in structuring.extracted_entities.medications.iter().enumerate() {
        let med_name = match &med.generic_name {
            Some(n) => n.to_lowercase(),
            None => continue,
        };

        // Check against existing DB medications
        if let Some(existing) = existing_meds
            .iter()
            .find(|m| m.generic_name.to_lowercase() == med_name)
        {
            let field_id = match find_field_id(
                fields,
                &EntityCategory::Medication,
                i,
                "generic_name",
                "dose",
            ) {
                Some(id) => id,
                None => continue,
            };

            let display_name = existing
                .brand_name
                .as_deref()
                .unwrap_or(&existing.generic_name);

            warnings.push(PlausibilityWarning {
                field_id,
                warning_type: PlausibilityType::DuplicateMedication,
                message: format!(
                    "You already have {} ({}) in your records. This may be a duplicate prescription.",
                    display_name, existing.dose
                ),
                severity: WarningSeverity::Warning,
            });
        }

        // Check within same document (same generic in different entries)
        for (j, other) in structuring.extracted_entities.medications.iter().enumerate() {
            if j <= i {
                continue;
            }
            let other_name = match &other.generic_name {
                Some(n) => n.to_lowercase(),
                None => continue,
            };
            if med_name == other_name {
                let field_id = match find_field_id(
                    fields,
                    &EntityCategory::Medication,
                    j,
                    "generic_name",
                    "dose",
                ) {
                    Some(id) => id,
                    None => continue,
                };

                warnings.push(PlausibilityWarning {
                    field_id,
                    warning_type: PlausibilityType::DuplicateMedication,
                    message: format!(
                        "{} appears more than once in this document. Please verify this is intentional.",
                        med.generic_name.as_deref().unwrap_or("This medication")
                    ),
                    severity: WarningSeverity::Warning,
                });
            }
        }
    }

    warnings
}

/// Parse a numeric dose value from a dose string.
///
/// Handles formats like "500mg", "500 mg", "0.5mg", "10", "500".
fn parse_dose_value(dose: &str) -> Option<f64> {
    let numeric: String = dose
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
        .collect();
    if numeric.is_empty() {
        return None;
    }
    numeric.replace(',', ".").parse::<f64>().ok()
}

/// Apply field corrections to extracted entities before storage.
///
/// Matches each correction by `entity_type + entity_index + field_name` encoded
/// in the field list. Returns the number of corrections successfully applied.
pub fn apply_corrections(
    entities: &mut ExtractedEntities,
    corrections: &[FieldCorrection],
    field_map: &[ExtractedField],
) -> usize {
    let mut applied = 0;

    for correction in corrections {
        // Find the corresponding field to know entity_type, entity_index, field_name
        let field = match field_map.iter().find(|f| f.id == correction.field_id) {
            Some(f) => f,
            None => continue,
        };

        let success = match (&field.entity_type, field.field_name.as_str()) {
            (EntityCategory::Medication, "generic_name") => {
                if let Some(med) = entities.medications.get_mut(field.entity_index) {
                    med.generic_name = Some(correction.corrected_value.clone());
                    true
                } else {
                    false
                }
            }
            (EntityCategory::Medication, "dose") => {
                if let Some(med) = entities.medications.get_mut(field.entity_index) {
                    med.dose = correction.corrected_value.clone();
                    true
                } else {
                    false
                }
            }
            (EntityCategory::Medication, "frequency") => {
                if let Some(med) = entities.medications.get_mut(field.entity_index) {
                    med.frequency = correction.corrected_value.clone();
                    true
                } else {
                    false
                }
            }
            (EntityCategory::LabResult, "test_name") => {
                if let Some(lab) = entities.lab_results.get_mut(field.entity_index) {
                    lab.test_name = correction.corrected_value.clone();
                    true
                } else {
                    false
                }
            }
            (EntityCategory::LabResult, "value") => {
                if let Some(lab) = entities.lab_results.get_mut(field.entity_index) {
                    lab.value = correction.corrected_value.parse::<f64>().ok();
                    lab.value_text = Some(correction.corrected_value.clone());
                    true
                } else {
                    false
                }
            }
            (EntityCategory::LabResult, "unit") => {
                if let Some(lab) = entities.lab_results.get_mut(field.entity_index) {
                    lab.unit = Some(correction.corrected_value.clone());
                    true
                } else {
                    false
                }
            }
            (EntityCategory::Diagnosis, "name") => {
                if let Some(diag) = entities.diagnoses.get_mut(field.entity_index) {
                    diag.name = correction.corrected_value.clone();
                    true
                } else {
                    false
                }
            }
            (EntityCategory::Allergy, "allergen") => {
                if let Some(allergy) = entities.allergies.get_mut(field.entity_index) {
                    allergy.allergen = correction.corrected_value.clone();
                    true
                } else {
                    false
                }
            }
            (EntityCategory::Procedure, "name") => {
                if let Some(proc) = entities.procedures.get_mut(field.entity_index) {
                    proc.name = correction.corrected_value.clone();
                    true
                } else {
                    false
                }
            }
            (EntityCategory::Referral, "referred_to") => {
                if let Some(referral) = entities.referrals.get_mut(field.entity_index) {
                    referral.referred_to = correction.corrected_value.clone();
                    true
                } else {
                    false
                }
            }
            _ => false,
        };

        if success {
            applied += 1;
        }
    }

    applied
}

/// Count total extractable fields for accuracy calculation.
pub fn count_extracted_fields(entities: &ExtractedEntities) -> usize {
    let mut count = 0;
    // Each medication: name + dose + frequency (3 fields)
    count += entities.medications.len() * 3;
    // Each lab result: test_name + value + unit (3 fields)
    count += entities.lab_results.len() * 3;
    // Each diagnosis: name (1 field)
    count += entities.diagnoses.len();
    // Each allergy: allergen (1 field)
    count += entities.allergies.len();
    // Each procedure: name (1 field)
    count += entities.procedures.len();
    // Each referral: referred_to (1 field)
    count += entities.referrals.len();
    count
}

/// Update document `verified` column to 1 (confirmed).
pub fn update_document_verified(conn: &Connection, document_id: &Uuid) -> Result<(), DatabaseError> {
    conn.execute(
        "UPDATE documents SET verified = 1 WHERE id = ?1",
        params![document_id.to_string()],
    )?;
    Ok(())
}

/// Update document to rejected status (set verified back to 0, store rejection reason in notes).
pub fn update_document_rejected(
    conn: &Connection,
    document_id: &Uuid,
    reason: Option<&str>,
) -> Result<(), DatabaseError> {
    conn.execute(
        "UPDATE documents SET verified = 0, notes = COALESCE(?2, notes) WHERE id = ?1",
        params![document_id.to_string(), reason],
    )?;
    Ok(())
}

/// Determine the original file type from the source filename.
pub fn detect_file_type(source_file: &str) -> OriginalFileType {
    let lower = source_file.to_lowercase();
    if lower.ends_with(".pdf") {
        OriginalFileType::Pdf
    } else {
        OriginalFileType::Image
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;
    use crate::db::repository::{
        insert_document, get_document, update_profile_trust_verified,
        update_profile_trust_corrected, get_profile_trust,
    };
    use crate::models::document::Document;
    use crate::models::enums::{DocumentType, PipelineStatus};
    use crate::pipeline::structuring::types::*;
    use chrono::{NaiveDate, NaiveDateTime};

    fn make_structuring_result() -> StructuringResult {
        StructuringResult {
            document_id: Uuid::new_v4(),
            document_type: DocumentType::Prescription,
            document_date: Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()),
            professional: Some(ExtractedProfessional {
                name: "Dr. Chen".into(),
                specialty: Some("Cardiology".into()),
                institution: None,
            }),
            structured_markdown: "# Prescription\n...".into(),
            extracted_entities: ExtractedEntities {
                medications: vec![
                    ExtractedMedication {
                        generic_name: Some("Metformin".into()),
                        brand_name: None,
                        dose: "500mg".into(),
                        frequency: "twice daily".into(),
                        frequency_type: "regular".into(),
                        route: "oral".into(),
                        reason: Some("Type 2 diabetes".into()),
                        instructions: vec![],
                        is_compound: false,
                        compound_ingredients: vec![],
                        tapering_steps: vec![],
                        max_daily_dose: None,
                        condition: None,
                        confidence: 0.92,
                    },
                    ExtractedMedication {
                        generic_name: Some("Atorvastatin".into()),
                        brand_name: None,
                        dose: "20mg".into(),
                        frequency: "at bedtime".into(),
                        frequency_type: "regular".into(),
                        route: "oral".into(),
                        reason: None,
                        instructions: vec![],
                        is_compound: false,
                        compound_ingredients: vec![],
                        tapering_steps: vec![],
                        max_daily_dose: None,
                        condition: None,
                        confidence: 0.55,
                    },
                ],
                lab_results: vec![ExtractedLabResult {
                    test_name: "HbA1c".into(),
                    test_code: None,
                    value: Some(7.2),
                    value_text: None,
                    unit: Some("%".into()),
                    reference_range_low: Some(4.0),
                    reference_range_high: Some(6.0),
                    reference_range_text: None,
                    abnormal_flag: Some("high".into()),
                    collection_date: None,
                    confidence: 0.88,
                }],
                diagnoses: vec![ExtractedDiagnosis {
                    name: "Type 2 Diabetes".into(),
                    icd_code: Some("E11".into()),
                    date: None,
                    status: "active".into(),
                    confidence: 0.95,
                }],
                allergies: vec![ExtractedAllergy {
                    allergen: "Penicillin".into(),
                    reaction: Some("Rash".into()),
                    severity: Some("Moderate".into()),
                    confidence: 0.65,
                }],
                procedures: vec![],
                referrals: vec![ExtractedReferral {
                    referred_to: "Dr. Patel".into(),
                    specialty: Some("Endocrinology".into()),
                    reason: Some("Diabetes management".into()),
                    confidence: 0.80,
                }],
                instructions: vec![ExtractedInstruction {
                    text: "Return in 3 months".into(),
                    category: "follow_up".into(),
                }],
            },
            structuring_confidence: 0.85,
            markdown_file_path: None,
            validation_warnings: vec![],
            raw_llm_response: None,
        }
    }

    fn make_test_document(id: &Uuid) -> Document {
        Document {
            id: *id,
            doc_type: DocumentType::Prescription,
            title: "Prescription - Dr. Chen".into(),
            document_date: Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()),
            ingestion_date: NaiveDateTime::parse_from_str(
                "2024-01-15 10:00:00",
                "%Y-%m-%d %H:%M:%S",
            )
            .unwrap(),
            professional_id: None,
            source_file: "/profiles/test/staging/doc.jpg.enc".into(),
            markdown_file: None,
            ocr_confidence: Some(0.85),
            verified: false,
            source_deleted: false,
            perceptual_hash: Some("abc123".into()),
            notes: None,
            pipeline_status: PipelineStatus::Imported,
        }
    }

    // --- flatten_entities_to_fields ---

    #[test]
    fn flatten_produces_fields_for_all_entity_types() {
        let result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);

        // 2 medications × 3 fields (name+dose+freq) = 6
        // 1 lab result × 3 (name+value+unit) = 3
        // 1 diagnosis = 1
        // 1 allergy = 1
        // 0 procedures = 0
        // 1 referral = 1
        // 1 professional = 1
        // 1 date = 1
        // Total = 14
        assert_eq!(fields.len(), 14);
    }

    #[test]
    fn flatten_flags_low_confidence_fields() {
        let result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);

        let flagged: Vec<_> = fields.iter().filter(|f| f.is_flagged).collect();
        // Atorvastatin (0.55) has 3 fields flagged, Penicillin allergy (0.65) has 1
        assert_eq!(flagged.len(), 4);

        let confident: Vec<_> = fields.iter().filter(|f| !f.is_flagged).collect();
        assert_eq!(confident.len(), 10);
    }

    #[test]
    fn flatten_assigns_correct_categories() {
        let result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);

        let med_fields: Vec<_> = fields
            .iter()
            .filter(|f| f.entity_type == EntityCategory::Medication)
            .collect();
        assert_eq!(med_fields.len(), 6);

        let lab_fields: Vec<_> = fields
            .iter()
            .filter(|f| f.entity_type == EntityCategory::LabResult)
            .collect();
        assert_eq!(lab_fields.len(), 3);

        let prof_fields: Vec<_> = fields
            .iter()
            .filter(|f| f.entity_type == EntityCategory::Professional)
            .collect();
        assert_eq!(prof_fields.len(), 1);
        assert_eq!(prof_fields[0].value, "Dr. Chen");

        let date_fields: Vec<_> = fields
            .iter()
            .filter(|f| f.entity_type == EntityCategory::Date)
            .collect();
        assert_eq!(date_fields.len(), 1);
    }

    #[test]
    fn flatten_empty_entities_produces_no_fields() {
        let mut result = make_structuring_result();
        result.extracted_entities = ExtractedEntities::default();
        result.professional = None;
        result.document_date = None;

        let fields = flatten_entities_to_fields(&result);
        assert!(fields.is_empty());
    }

    #[test]
    fn flatten_medication_without_generic_name_skips_name_field() {
        let mut result = make_structuring_result();
        result.extracted_entities.medications = vec![ExtractedMedication {
            generic_name: None,
            brand_name: Some("Glucophage".into()),
            dose: "500mg".into(),
            frequency: "daily".into(),
            frequency_type: "regular".into(),
            route: "oral".into(),
            reason: None,
            instructions: vec![],
            is_compound: false,
            compound_ingredients: vec![],
            tapering_steps: vec![],
            max_daily_dose: None,
            condition: None,
            confidence: 0.90,
        }];
        result.extracted_entities.lab_results.clear();
        result.extracted_entities.diagnoses.clear();
        result.extracted_entities.allergies.clear();
        result.extracted_entities.referrals.clear();
        result.professional = None;
        result.document_date = None;

        let fields = flatten_entities_to_fields(&result);
        // Only dose + frequency (no generic_name)
        assert_eq!(fields.len(), 2);
        assert!(fields.iter().all(|f| f.field_name != "generic_name"));
    }

    #[test]
    fn flatten_lab_without_value_skips_value_field() {
        let mut result = make_structuring_result();
        result.extracted_entities.medications.clear();
        result.extracted_entities.lab_results = vec![ExtractedLabResult {
            test_name: "WBC".into(),
            test_code: None,
            value: None,
            value_text: None,
            unit: None,
            reference_range_low: None,
            reference_range_high: None,
            reference_range_text: None,
            abnormal_flag: None,
            collection_date: None,
            confidence: 0.90,
        }];
        result.extracted_entities.diagnoses.clear();
        result.extracted_entities.allergies.clear();
        result.extracted_entities.referrals.clear();
        result.professional = None;
        result.document_date = None;

        let fields = flatten_entities_to_fields(&result);
        // Only test_name (no value, no unit)
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].field_name, "test_name");
    }

    // --- apply_corrections ---

    #[test]
    fn apply_corrections_updates_medication_dose() {
        let result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);
        let mut entities = result.extracted_entities.clone();

        let dose_field = fields
            .iter()
            .find(|f| {
                f.entity_type == EntityCategory::Medication
                    && f.field_name == "dose"
                    && f.entity_index == 0
            })
            .unwrap();

        let corrections = vec![FieldCorrection {
            field_id: dose_field.id,
            original_value: "500mg".into(),
            corrected_value: "1000mg".into(),
        }];

        let applied = apply_corrections(&mut entities, &corrections, &fields);
        assert_eq!(applied, 1);
        assert_eq!(entities.medications[0].dose, "1000mg");
    }

    #[test]
    fn apply_corrections_updates_diagnosis_name() {
        let result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);
        let mut entities = result.extracted_entities.clone();

        let diag_field = fields
            .iter()
            .find(|f| f.entity_type == EntityCategory::Diagnosis && f.field_name == "name")
            .unwrap();

        let corrections = vec![FieldCorrection {
            field_id: diag_field.id,
            original_value: "Type 2 Diabetes".into(),
            corrected_value: "Type 2 Diabetes Mellitus".into(),
        }];

        let applied = apply_corrections(&mut entities, &corrections, &fields);
        assert_eq!(applied, 1);
        assert_eq!(entities.diagnoses[0].name, "Type 2 Diabetes Mellitus");
    }

    #[test]
    fn apply_corrections_updates_lab_value() {
        let result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);
        let mut entities = result.extracted_entities.clone();

        let lab_value_field = fields
            .iter()
            .find(|f| f.entity_type == EntityCategory::LabResult && f.field_name == "value")
            .unwrap();

        let corrections = vec![FieldCorrection {
            field_id: lab_value_field.id,
            original_value: "7.2".into(),
            corrected_value: "7.5".into(),
        }];

        let applied = apply_corrections(&mut entities, &corrections, &fields);
        assert_eq!(applied, 1);
        assert_eq!(entities.lab_results[0].value, Some(7.5));
        assert_eq!(
            entities.lab_results[0].value_text,
            Some("7.5".to_string())
        );
    }

    #[test]
    fn apply_corrections_skips_invalid_field_id() {
        let result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);
        let mut entities = result.extracted_entities.clone();

        let corrections = vec![FieldCorrection {
            field_id: Uuid::new_v4(), // non-existent
            original_value: "test".into(),
            corrected_value: "corrected".into(),
        }];

        let applied = apply_corrections(&mut entities, &corrections, &fields);
        assert_eq!(applied, 0);
    }

    #[test]
    fn apply_corrections_multiple() {
        let result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);
        let mut entities = result.extracted_entities.clone();

        let allergen_field = fields
            .iter()
            .find(|f| f.entity_type == EntityCategory::Allergy && f.field_name == "allergen")
            .unwrap();
        let referral_field = fields
            .iter()
            .find(|f| f.entity_type == EntityCategory::Referral && f.field_name == "referred_to")
            .unwrap();

        let corrections = vec![
            FieldCorrection {
                field_id: allergen_field.id,
                original_value: "Penicillin".into(),
                corrected_value: "Amoxicillin".into(),
            },
            FieldCorrection {
                field_id: referral_field.id,
                original_value: "Dr. Patel".into(),
                corrected_value: "Dr. Sharma".into(),
            },
        ];

        let applied = apply_corrections(&mut entities, &corrections, &fields);
        assert_eq!(applied, 2);
        assert_eq!(entities.allergies[0].allergen, "Amoxicillin");
        assert_eq!(entities.referrals[0].referred_to, "Dr. Sharma");
    }

    // --- count_extracted_fields ---

    #[test]
    fn count_fields_matches_expected() {
        let result = make_structuring_result();
        let count = count_extracted_fields(&result.extracted_entities);
        // 2 meds × 3 + 1 lab × 3 + 1 diag + 1 allergy + 0 proc + 1 referral = 12
        assert_eq!(count, 12);
    }

    #[test]
    fn count_fields_empty_entities() {
        let entities = ExtractedEntities::default();
        assert_eq!(count_extracted_fields(&entities), 0);
    }

    // --- detect_file_type ---

    #[test]
    fn detect_pdf_type() {
        assert_eq!(detect_file_type("document.pdf"), OriginalFileType::Pdf);
        assert_eq!(detect_file_type("DOC.PDF"), OriginalFileType::Pdf);
    }

    #[test]
    fn detect_image_type() {
        assert_eq!(detect_file_type("photo.jpg"), OriginalFileType::Image);
        assert_eq!(detect_file_type("scan.png"), OriginalFileType::Image);
        assert_eq!(detect_file_type("xray.tiff"), OriginalFileType::Image);
    }

    // --- Database integration tests ---

    #[test]
    fn update_document_verified_sets_flag() {
        let conn = open_memory_database().unwrap();
        let doc_id = Uuid::new_v4();
        let doc = make_test_document(&doc_id);
        insert_document(&conn, &doc).unwrap();

        // Initially not verified
        let fetched = get_document(&conn, &doc_id).unwrap().unwrap();
        assert!(!fetched.verified);

        // Mark verified
        update_document_verified(&conn, &doc_id).unwrap();
        let fetched = get_document(&conn, &doc_id).unwrap().unwrap();
        assert!(fetched.verified);
    }

    #[test]
    fn update_document_rejected_stores_reason() {
        let conn = open_memory_database().unwrap();
        let doc_id = Uuid::new_v4();
        let doc = make_test_document(&doc_id);
        insert_document(&conn, &doc).unwrap();

        update_document_rejected(&conn, &doc_id, Some("Extraction was wrong")).unwrap();
        let fetched = get_document(&conn, &doc_id).unwrap().unwrap();
        assert!(!fetched.verified);
        assert_eq!(fetched.notes, Some("Extraction was wrong".to_string()));
    }

    #[test]
    fn trust_update_after_verified_review() {
        let conn = open_memory_database().unwrap();

        update_profile_trust_verified(&conn).unwrap();
        let trust = get_profile_trust(&conn).unwrap();
        assert_eq!(trust.total_documents, 1);
        assert_eq!(trust.documents_verified, 1);
        assert_eq!(trust.documents_corrected, 0);
    }

    #[test]
    fn trust_update_after_corrected_review() {
        let conn = open_memory_database().unwrap();

        update_profile_trust_corrected(&conn).unwrap();
        let trust = get_profile_trust(&conn).unwrap();
        assert_eq!(trust.total_documents, 1);
        assert_eq!(trust.documents_verified, 1);
        assert_eq!(trust.documents_corrected, 1);
    }

    // --- ReviewOutcome determination ---

    #[test]
    fn review_outcome_with_no_corrections_is_confirmed() {
        let corrections: Vec<FieldCorrection> = vec![];
        let outcome = if corrections.is_empty() {
            ReviewOutcome::Confirmed
        } else {
            ReviewOutcome::Corrected
        };
        assert_eq!(outcome, ReviewOutcome::Confirmed);
    }

    #[test]
    fn review_outcome_with_corrections_is_corrected() {
        let corrections = vec![FieldCorrection {
            field_id: Uuid::new_v4(),
            original_value: "test".into(),
            corrected_value: "corrected".into(),
        }];
        let outcome = if corrections.is_empty() {
            ReviewOutcome::Confirmed
        } else {
            ReviewOutcome::Corrected
        };
        assert_eq!(outcome, ReviewOutcome::Corrected);
    }

    // --- parse_dose_value ---

    #[test]
    fn parse_dose_value_with_unit_suffix() {
        assert_eq!(parse_dose_value("500mg"), Some(500.0));
        assert_eq!(parse_dose_value("20 mg"), Some(20.0));
        assert_eq!(parse_dose_value("0.5mg"), Some(0.5));
    }

    #[test]
    fn parse_dose_value_plain_number() {
        assert_eq!(parse_dose_value("100"), Some(100.0));
        assert_eq!(parse_dose_value("7.5"), Some(7.5));
    }

    #[test]
    fn parse_dose_value_with_comma() {
        assert_eq!(parse_dose_value("1,5mg"), Some(1.5));
    }

    #[test]
    fn parse_dose_value_empty_or_text() {
        assert_eq!(parse_dose_value(""), None);
        assert_eq!(parse_dose_value("unknown"), None);
        assert_eq!(parse_dose_value("as needed"), None);
    }

    // --- generate_plausibility_warnings ---

    fn insert_dose_reference(conn: &Connection, name: &str, min: f64, max: f64, abs_max: f64) {
        conn.execute(
            "INSERT INTO dose_references (generic_name, typical_min_mg, typical_max_mg, absolute_max_mg, unit, source)
             VALUES (?1, ?2, ?3, ?4, 'mg', 'test')",
            params![name, min, max, abs_max],
        )
        .unwrap();
    }

    #[test]
    fn plausibility_no_warnings_for_normal_doses() {
        let conn = open_memory_database().unwrap();
        insert_dose_reference(&conn, "metformin", 500.0, 2550.0, 1000.0);

        let result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);
        let warnings = generate_plausibility_warnings(&conn, &result, &fields);

        // Metformin 500mg is within range; atorvastatin has no dose_reference → no warnings
        assert!(warnings.is_empty());
    }

    #[test]
    fn plausibility_warns_on_very_high_dose() {
        let conn = open_memory_database().unwrap();
        // absolute_max is 1000mg, so >5000mg triggers VeryHighDose
        insert_dose_reference(&conn, "metformin", 500.0, 2550.0, 1000.0);

        let mut result = make_structuring_result();
        result.extracted_entities.medications[0].dose = "50000mg".into();
        let fields = flatten_entities_to_fields(&result);

        let warnings = generate_plausibility_warnings(&conn, &result, &fields);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].severity, WarningSeverity::Critical);
        assert_eq!(warnings[0].warning_type, PlausibilityType::DoseUnusuallyHigh);
    }

    #[test]
    fn plausibility_warns_on_low_dose() {
        let conn = open_memory_database().unwrap();
        insert_dose_reference(&conn, "metformin", 500.0, 2550.0, 1000.0);

        let mut result = make_structuring_result();
        result.extracted_entities.medications[0].dose = "1mg".into();
        let fields = flatten_entities_to_fields(&result);

        let warnings = generate_plausibility_warnings(&conn, &result, &fields);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].severity, WarningSeverity::Info);
        assert_eq!(warnings[0].warning_type, PlausibilityType::DoseUnusuallyLow);
    }

    #[test]
    fn plausibility_skips_medications_without_generic_name() {
        let conn = open_memory_database().unwrap();
        insert_dose_reference(&conn, "metformin", 500.0, 2550.0, 1000.0);

        let mut result = make_structuring_result();
        // Remove generic_name from first medication
        result.extracted_entities.medications[0].generic_name = None;
        let fields = flatten_entities_to_fields(&result);

        let warnings = generate_plausibility_warnings(&conn, &result, &fields);
        assert!(warnings.is_empty());
    }

    #[test]
    fn plausibility_skips_unparseable_doses() {
        let conn = open_memory_database().unwrap();
        insert_dose_reference(&conn, "metformin", 500.0, 2550.0, 1000.0);

        let mut result = make_structuring_result();
        result.extracted_entities.medications[0].dose = "as needed".into();
        let fields = flatten_entities_to_fields(&result);

        let warnings = generate_plausibility_warnings(&conn, &result, &fields);
        assert!(warnings.is_empty());
    }

    // --- check_critical_lab_values ---

    #[test]
    fn critical_lab_with_critical_flag() {
        let mut result = make_structuring_result();
        result.extracted_entities.lab_results[0].abnormal_flag = Some("critical_high".into());
        let fields = flatten_entities_to_fields(&result);

        let warnings = check_critical_lab_values(&result, &fields);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].warning_type, PlausibilityType::LabValueCritical);
        assert_eq!(warnings[0].severity, WarningSeverity::Critical);
        assert!(warnings[0].message.contains("HbA1c"));
        assert!(warnings[0].message.contains("critical"));
    }

    #[test]
    fn critical_lab_with_panic_flag() {
        let mut result = make_structuring_result();
        result.extracted_entities.lab_results[0].abnormal_flag = Some("panic".into());
        let fields = flatten_entities_to_fields(&result);

        let warnings = check_critical_lab_values(&result, &fields);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].severity, WarningSeverity::Critical);
    }

    #[test]
    fn critical_lab_far_above_range() {
        let mut result = make_structuring_result();
        result.extracted_entities.lab_results[0].value = Some(15.0); // > 6.0 * 2.0 = 12.0
        result.extracted_entities.lab_results[0].abnormal_flag = Some("high".into());
        let fields = flatten_entities_to_fields(&result);

        let warnings = check_critical_lab_values(&result, &fields);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].warning_type, PlausibilityType::LabValueCritical);
        assert_eq!(warnings[0].severity, WarningSeverity::Warning);
        assert!(warnings[0].message.contains("outside the expected range"));
    }

    #[test]
    fn critical_lab_far_below_range() {
        let mut result = make_structuring_result();
        result.extracted_entities.lab_results[0].value = Some(1.5); // < 4.0 * 0.5 = 2.0
        result.extracted_entities.lab_results[0].abnormal_flag = None;
        let fields = flatten_entities_to_fields(&result);

        let warnings = check_critical_lab_values(&result, &fields);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].severity, WarningSeverity::Warning);
    }

    #[test]
    fn normal_lab_no_critical_warning() {
        let result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);

        // HbA1c 7.2 with range 4.0-6.0, flag "high" — not "critical" or "panic",
        // 7.2 < 6.0*2=12.0, 7.2 > 4.0*0.5=2.0 — no critical warning
        let warnings = check_critical_lab_values(&result, &fields);
        assert!(warnings.is_empty());
    }

    // --- check_allergy_conflicts ---

    #[test]
    fn allergy_conflict_from_extracted_allergen() {
        let conn = open_memory_database().unwrap();
        let mut result = make_structuring_result();
        // Add a medication that matches the existing Penicillin allergy
        result.extracted_entities.medications.push(ExtractedMedication {
            generic_name: Some("Penicillin V".into()),
            brand_name: None,
            dose: "500mg".into(),
            frequency: "four times daily".into(),
            frequency_type: "regular".into(),
            route: "oral".into(),
            reason: None,
            instructions: vec![],
            is_compound: false,
            compound_ingredients: vec![],
            tapering_steps: vec![],
            max_daily_dose: None,
            condition: None,
            confidence: 0.9,
        });
        let fields = flatten_entities_to_fields(&result);

        let warnings = check_allergy_conflicts(&conn, &result, &fields);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].warning_type, PlausibilityType::AllergyConflict);
        assert_eq!(warnings[0].severity, WarningSeverity::Critical);
        assert!(warnings[0].message.contains("penicillin"));
    }

    #[test]
    fn allergy_conflict_from_db_allergen() {
        let conn = open_memory_database().unwrap();
        let doc_id = Uuid::new_v4();
        let doc = make_test_document(&doc_id);
        insert_document(&conn, &doc).unwrap();

        // Insert an allergy into the DB
        crate::db::repository::insert_allergy(
            &conn,
            &crate::models::Allergy {
                id: Uuid::new_v4(),
                allergen: "Metformin".into(),
                reaction: Some("Nausea".into()),
                severity: crate::models::enums::AllergySeverity::Moderate,
                date_identified: None,
                source: crate::models::enums::AllergySource::DocumentExtracted,
                document_id: Some(doc_id),
                verified: true,
            },
        )
        .unwrap();

        // Make a result with no extracted allergies but has Metformin medication
        let mut result = make_structuring_result();
        result.extracted_entities.allergies.clear();
        let fields = flatten_entities_to_fields(&result);

        let warnings = check_allergy_conflicts(&conn, &result, &fields);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("metformin"));
    }

    #[test]
    fn no_allergy_conflict_when_no_match() {
        let conn = open_memory_database().unwrap();
        let result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);

        // Penicillin allergy vs Metformin/Atorvastatin — no overlap
        let warnings = check_allergy_conflicts(&conn, &result, &fields);
        assert!(warnings.is_empty());
    }

    // --- check_duplicate_medications ---

    #[test]
    fn duplicate_medication_from_db() {
        let conn = open_memory_database().unwrap();
        let doc_id = Uuid::new_v4();
        let doc = make_test_document(&doc_id);
        insert_document(&conn, &doc).unwrap();

        // Insert an existing Metformin into the DB
        crate::db::repository::insert_medication(
            &conn,
            &crate::models::Medication {
                id: Uuid::new_v4(),
                generic_name: "Metformin".into(),
                brand_name: Some("Glucophage".into()),
                dose: "850mg".into(),
                frequency: "twice daily".into(),
                frequency_type: crate::models::enums::FrequencyType::Scheduled,
                route: "oral".into(),
                prescriber_id: None,
                start_date: None,
                end_date: None,
                reason_start: None,
                reason_stop: None,
                is_otc: false,
                status: crate::models::enums::MedicationStatus::Active,
                administration_instructions: None,
                max_daily_dose: None,
                condition: None,
                dose_type: crate::models::enums::DoseType::Fixed,
                is_compound: false,
                document_id: doc_id,
            },
        )
        .unwrap();

        let result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);

        let warnings = check_duplicate_medications(&conn, &result, &fields);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].warning_type, PlausibilityType::DuplicateMedication);
        assert_eq!(warnings[0].severity, WarningSeverity::Warning);
        assert!(warnings[0].message.contains("Glucophage"));
    }

    #[test]
    fn duplicate_medication_within_same_document() {
        let conn = open_memory_database().unwrap();
        let mut result = make_structuring_result();
        // Add a second Metformin entry (different dose)
        result.extracted_entities.medications.push(ExtractedMedication {
            generic_name: Some("Metformin".into()),
            brand_name: Some("Glucophage XR".into()),
            dose: "1000mg".into(),
            frequency: "once daily".into(),
            frequency_type: "regular".into(),
            route: "oral".into(),
            reason: None,
            instructions: vec![],
            is_compound: false,
            compound_ingredients: vec![],
            tapering_steps: vec![],
            max_daily_dose: None,
            condition: None,
            confidence: 0.88,
        });
        let fields = flatten_entities_to_fields(&result);

        let warnings = check_duplicate_medications(&conn, &result, &fields);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].warning_type, PlausibilityType::DuplicateMedication);
        assert!(warnings[0].message.contains("more than once"));
    }

    #[test]
    fn no_duplicate_for_unique_medications() {
        let conn = open_memory_database().unwrap();
        let result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);

        // Metformin + Atorvastatin — no duplicates
        let warnings = check_duplicate_medications(&conn, &result, &fields);
        assert!(warnings.is_empty());
    }
}
