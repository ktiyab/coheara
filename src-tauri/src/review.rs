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
use crate::pipeline::structuring::classify::parse_document_date;
use crate::pipeline::structuring::types::{ExtractedProfessional, StructuringResult};

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

/// An entity excluded by the patient during review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcludedEntity {
    pub entity_type: EntityCategory,
    pub entity_index: usize,
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

/// 12-ERC B5: Humanize abnormal flag values for display.
fn humanize_abnormal_flag(flag: &str) -> String {
    match flag.to_lowercase().as_str() {
        "high" | "h" => "High".into(),
        "low" | "l" => "Low".into(),
        "critical_high" | "ch" | "critical high" => "Critical high".into(),
        "critical_low" | "cl" | "critical low" => "Critical low".into(),
        "normal" | "n" | "" => "Normal".into(),
        other => other.to_string(),
    }
}

/// 12-ERC B5: Parse a reference range display string into (low, high).
/// Handles formats: "4.28 - 6.00", "(4.28-6.00)", "4,28 – 6,00".
fn parse_reference_range_display(text: &str) -> Option<(f64, f64)> {
    let cleaned = text.trim().trim_matches(|c| c == '(' || c == ')');
    let parts: Vec<&str> = cleaned
        .split(|c: char| c == '-' || c == '–')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() == 2 {
        let low = parts[0].replace(',', ".").parse::<f64>().ok()?;
        let high = parts[1].replace(',', ".").parse::<f64>().ok()?;
        Some((low, high))
    } else {
        None
    }
}

/// 12-ERC B1: Namespace UUID for deterministic field IDs.
/// Uses the DNS namespace UUID (RFC 4122) as a stable seed.
const FIELD_ID_NAMESPACE: Uuid = uuid::uuid!("6ba7b810-9dad-11d1-80b4-00c04fd430c8");

/// 12-ERC B1: Produce a stable field ID from entity type, index, and field name.
/// Same entity always produces the same UUID across flatten calls,
/// so corrections match even when the field list is recomputed.
fn deterministic_field_id(entity_type: &EntityCategory, index: usize, field: &str) -> Uuid {
    Uuid::new_v5(
        &FIELD_ID_NAMESPACE,
        format!("{:?}:{}:{}", entity_type, index, field).as_bytes(),
    )
}

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
                id: deterministic_field_id(&EntityCategory::Medication, i, "generic_name"),
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
            id: deterministic_field_id(&EntityCategory::Medication, i, "dose"),
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
            id: deterministic_field_id(&EntityCategory::Medication, i, "frequency"),
            entity_type: EntityCategory::Medication,
            entity_index: i,
            field_name: "frequency".into(),
            display_label: "Frequency".into(),
            value: med.frequency.clone(),
            confidence: med.confidence,
            is_flagged: med.confidence < CONFIDENCE_THRESHOLD,
            source_hint: None,
        });
        // 12-ERC B5: route (when non-empty)
        if !med.route.is_empty() {
            fields.push(ExtractedField {
                id: deterministic_field_id(&EntityCategory::Medication, i, "route"),
                entity_type: EntityCategory::Medication,
                entity_index: i,
                field_name: "route".into(),
                display_label: "Route".into(),
                value: med.route.clone(),
                confidence: med.confidence,
                is_flagged: med.confidence < CONFIDENCE_THRESHOLD,
                source_hint: None,
            });
        }
    }

    // Lab results: test_name + value + unit
    for (i, lab) in structuring.extracted_entities.lab_results.iter().enumerate() {
        fields.push(ExtractedField {
            id: deterministic_field_id(&EntityCategory::LabResult, i, "test_name"),
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
                id: deterministic_field_id(&EntityCategory::LabResult, i, "value"),
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
                id: deterministic_field_id(&EntityCategory::LabResult, i, "unit"),
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
        // 12-ERC B5: reference_range (combined display)
        if lab.reference_range_low.is_some() || lab.reference_range_high.is_some() {
            let display = match (lab.reference_range_low, lab.reference_range_high) {
                (Some(low), Some(high)) => format!("{low} - {high}"),
                (Some(low), None) => format!(">= {low}"),
                (None, Some(high)) => format!("<= {high}"),
                (None, None) => unreachable!(),
            };
            fields.push(ExtractedField {
                id: deterministic_field_id(&EntityCategory::LabResult, i, "reference_range"),
                entity_type: EntityCategory::LabResult,
                entity_index: i,
                field_name: "reference_range".into(),
                display_label: "Reference range".into(),
                value: display,
                confidence: lab.confidence,
                is_flagged: lab.confidence < CONFIDENCE_THRESHOLD,
                source_hint: None,
            });
        } else if let Some(ref text) = lab.reference_range_text {
            fields.push(ExtractedField {
                id: deterministic_field_id(&EntityCategory::LabResult, i, "reference_range"),
                entity_type: EntityCategory::LabResult,
                entity_index: i,
                field_name: "reference_range".into(),
                display_label: "Reference range".into(),
                value: text.clone(),
                confidence: lab.confidence,
                is_flagged: lab.confidence < CONFIDENCE_THRESHOLD,
                source_hint: None,
            });
        }
        // 12-ERC B5: abnormal_flag (humanized)
        if let Some(ref flag) = lab.abnormal_flag {
            fields.push(ExtractedField {
                id: deterministic_field_id(&EntityCategory::LabResult, i, "abnormal_flag"),
                entity_type: EntityCategory::LabResult,
                entity_index: i,
                field_name: "abnormal_flag".into(),
                display_label: "Abnormal flag".into(),
                value: humanize_abnormal_flag(flag),
                confidence: lab.confidence,
                is_flagged: lab.confidence < CONFIDENCE_THRESHOLD,
                source_hint: None,
            });
        }
        // 12-ERC B5: collection_date
        if let Some(ref date) = lab.collection_date {
            fields.push(ExtractedField {
                id: deterministic_field_id(&EntityCategory::LabResult, i, "collection_date"),
                entity_type: EntityCategory::LabResult,
                entity_index: i,
                field_name: "collection_date".into(),
                display_label: "Collection date".into(),
                value: date.clone(),
                confidence: lab.confidence,
                is_flagged: lab.confidence < CONFIDENCE_THRESHOLD,
                source_hint: None,
            });
        }
    }

    // Diagnoses: name + date + status
    for (i, diag) in structuring.extracted_entities.diagnoses.iter().enumerate() {
        fields.push(ExtractedField {
            id: deterministic_field_id(&EntityCategory::Diagnosis, i, "name"),
            entity_type: EntityCategory::Diagnosis,
            entity_index: i,
            field_name: "name".into(),
            display_label: "Diagnosis".into(),
            value: diag.name.clone(),
            confidence: diag.confidence,
            is_flagged: diag.confidence < CONFIDENCE_THRESHOLD,
            source_hint: None,
        });
        // 12-ERC B5: date
        if let Some(ref date) = diag.date {
            fields.push(ExtractedField {
                id: deterministic_field_id(&EntityCategory::Diagnosis, i, "date"),
                entity_type: EntityCategory::Diagnosis,
                entity_index: i,
                field_name: "date".into(),
                display_label: "Date".into(),
                value: date.clone(),
                confidence: diag.confidence,
                is_flagged: diag.confidence < CONFIDENCE_THRESHOLD,
                source_hint: None,
            });
        }
        // 12-ERC B5: status
        if !diag.status.is_empty() {
            fields.push(ExtractedField {
                id: deterministic_field_id(&EntityCategory::Diagnosis, i, "status"),
                entity_type: EntityCategory::Diagnosis,
                entity_index: i,
                field_name: "status".into(),
                display_label: "Status".into(),
                value: diag.status.clone(),
                confidence: diag.confidence,
                is_flagged: diag.confidence < CONFIDENCE_THRESHOLD,
                source_hint: None,
            });
        }
    }

    // Allergies: allergen + reaction + severity
    for (i, allergy) in structuring.extracted_entities.allergies.iter().enumerate() {
        fields.push(ExtractedField {
            id: deterministic_field_id(&EntityCategory::Allergy, i, "allergen"),
            entity_type: EntityCategory::Allergy,
            entity_index: i,
            field_name: "allergen".into(),
            display_label: "Allergen".into(),
            value: allergy.allergen.clone(),
            confidence: allergy.confidence,
            is_flagged: allergy.confidence < CONFIDENCE_THRESHOLD,
            source_hint: None,
        });
        // 12-ERC B5: reaction
        if let Some(ref reaction) = allergy.reaction {
            fields.push(ExtractedField {
                id: deterministic_field_id(&EntityCategory::Allergy, i, "reaction"),
                entity_type: EntityCategory::Allergy,
                entity_index: i,
                field_name: "reaction".into(),
                display_label: "Reaction".into(),
                value: reaction.clone(),
                confidence: allergy.confidence,
                is_flagged: allergy.confidence < CONFIDENCE_THRESHOLD,
                source_hint: None,
            });
        }
        // 12-ERC B5: severity
        if let Some(ref severity) = allergy.severity {
            fields.push(ExtractedField {
                id: deterministic_field_id(&EntityCategory::Allergy, i, "severity"),
                entity_type: EntityCategory::Allergy,
                entity_index: i,
                field_name: "severity".into(),
                display_label: "Severity".into(),
                value: severity.clone(),
                confidence: allergy.confidence,
                is_flagged: allergy.confidence < CONFIDENCE_THRESHOLD,
                source_hint: None,
            });
        }
    }

    // Procedures: name + date + outcome
    for (i, proc) in structuring.extracted_entities.procedures.iter().enumerate() {
        fields.push(ExtractedField {
            id: deterministic_field_id(&EntityCategory::Procedure, i, "name"),
            entity_type: EntityCategory::Procedure,
            entity_index: i,
            field_name: "name".into(),
            display_label: "Procedure".into(),
            value: proc.name.clone(),
            confidence: proc.confidence,
            is_flagged: proc.confidence < CONFIDENCE_THRESHOLD,
            source_hint: None,
        });
        // 12-ERC B5: date
        if let Some(ref date) = proc.date {
            fields.push(ExtractedField {
                id: deterministic_field_id(&EntityCategory::Procedure, i, "date"),
                entity_type: EntityCategory::Procedure,
                entity_index: i,
                field_name: "date".into(),
                display_label: "Date".into(),
                value: date.clone(),
                confidence: proc.confidence,
                is_flagged: proc.confidence < CONFIDENCE_THRESHOLD,
                source_hint: None,
            });
        }
        // 12-ERC B5: outcome
        if let Some(ref outcome) = proc.outcome {
            fields.push(ExtractedField {
                id: deterministic_field_id(&EntityCategory::Procedure, i, "outcome"),
                entity_type: EntityCategory::Procedure,
                entity_index: i,
                field_name: "outcome".into(),
                display_label: "Outcome".into(),
                value: outcome.clone(),
                confidence: proc.confidence,
                is_flagged: proc.confidence < CONFIDENCE_THRESHOLD,
                source_hint: None,
            });
        }
    }

    // Referrals: referred_to + specialty + reason
    for (i, referral) in structuring.extracted_entities.referrals.iter().enumerate() {
        fields.push(ExtractedField {
            id: deterministic_field_id(&EntityCategory::Referral, i, "referred_to"),
            entity_type: EntityCategory::Referral,
            entity_index: i,
            field_name: "referred_to".into(),
            display_label: "Referred to".into(),
            value: referral.referred_to.clone(),
            confidence: referral.confidence,
            is_flagged: referral.confidence < CONFIDENCE_THRESHOLD,
            source_hint: None,
        });
        // 12-ERC B5: specialty
        if let Some(ref specialty) = referral.specialty {
            fields.push(ExtractedField {
                id: deterministic_field_id(&EntityCategory::Referral, i, "specialty"),
                entity_type: EntityCategory::Referral,
                entity_index: i,
                field_name: "specialty".into(),
                display_label: "Specialty".into(),
                value: specialty.clone(),
                confidence: referral.confidence,
                is_flagged: referral.confidence < CONFIDENCE_THRESHOLD,
                source_hint: None,
            });
        }
        // 12-ERC B5: reason
        if let Some(ref reason) = referral.reason {
            fields.push(ExtractedField {
                id: deterministic_field_id(&EntityCategory::Referral, i, "reason"),
                entity_type: EntityCategory::Referral,
                entity_index: i,
                field_name: "reason".into(),
                display_label: "Reason".into(),
                value: reason.clone(),
                confidence: referral.confidence,
                is_flagged: referral.confidence < CONFIDENCE_THRESHOLD,
                source_hint: None,
            });
        }
    }

    // Professional: name + specialty
    if let Some(ref prof) = structuring.professional {
        fields.push(ExtractedField {
            id: deterministic_field_id(&EntityCategory::Professional, 0, "name"),
            entity_type: EntityCategory::Professional,
            entity_index: 0,
            field_name: "name".into(),
            display_label: "Professional".into(),
            value: prof.name.clone(),
            confidence: structuring.structuring_confidence,
            is_flagged: structuring.structuring_confidence < CONFIDENCE_THRESHOLD,
            source_hint: None,
        });
        // 12-ERC B5: specialty
        if let Some(ref specialty) = prof.specialty {
            fields.push(ExtractedField {
                id: deterministic_field_id(&EntityCategory::Professional, 0, "specialty"),
                entity_type: EntityCategory::Professional,
                entity_index: 0,
                field_name: "specialty".into(),
                display_label: "Specialty".into(),
                value: specialty.clone(),
                confidence: structuring.structuring_confidence,
                is_flagged: structuring.structuring_confidence < CONFIDENCE_THRESHOLD,
                source_hint: None,
            });
        }
    }

    // Document date
    if let Some(date) = structuring.document_date {
        fields.push(ExtractedField {
            id: deterministic_field_id(&EntityCategory::Date, 0, "document_date"),
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

/// 12-ERC B5: Apply field corrections to the structuring result before storage.
///
/// Matches each correction by `entity_type + entity_index + field_name` encoded
/// in the field list. Supports ALL entity fields including Professional and Date
/// (which live on StructuringResult, not ExtractedEntities).
/// Returns the number of corrections successfully applied.
pub fn apply_corrections(
    structuring: &mut StructuringResult,
    corrections: &[FieldCorrection],
    field_map: &[ExtractedField],
) -> usize {
    let mut applied = 0;

    for correction in corrections {
        let field = match field_map.iter().find(|f| f.id == correction.field_id) {
            Some(f) => f,
            None => continue,
        };

        let success = match (&field.entity_type, field.field_name.as_str()) {
            // === Medications ===
            (EntityCategory::Medication, "generic_name") => {
                if let Some(med) = structuring.extracted_entities.medications.get_mut(field.entity_index) {
                    med.generic_name = Some(correction.corrected_value.clone()); true
                } else { false }
            }
            (EntityCategory::Medication, "dose") => {
                if let Some(med) = structuring.extracted_entities.medications.get_mut(field.entity_index) {
                    med.dose = correction.corrected_value.clone(); true
                } else { false }
            }
            (EntityCategory::Medication, "frequency") => {
                if let Some(med) = structuring.extracted_entities.medications.get_mut(field.entity_index) {
                    med.frequency = correction.corrected_value.clone(); true
                } else { false }
            }
            (EntityCategory::Medication, "route") => {
                if let Some(med) = structuring.extracted_entities.medications.get_mut(field.entity_index) {
                    med.route = correction.corrected_value.clone(); true
                } else { false }
            }
            // === Lab Results ===
            (EntityCategory::LabResult, "test_name") => {
                if let Some(lab) = structuring.extracted_entities.lab_results.get_mut(field.entity_index) {
                    lab.test_name = correction.corrected_value.clone(); true
                } else { false }
            }
            (EntityCategory::LabResult, "value") => {
                if let Some(lab) = structuring.extracted_entities.lab_results.get_mut(field.entity_index) {
                    lab.value = correction.corrected_value.parse::<f64>().ok();
                    lab.value_text = Some(correction.corrected_value.clone()); true
                } else { false }
            }
            (EntityCategory::LabResult, "unit") => {
                if let Some(lab) = structuring.extracted_entities.lab_results.get_mut(field.entity_index) {
                    lab.unit = Some(correction.corrected_value.clone()); true
                } else { false }
            }
            (EntityCategory::LabResult, "reference_range") => {
                if let Some(lab) = structuring.extracted_entities.lab_results.get_mut(field.entity_index) {
                    if let Some((low, high)) = parse_reference_range_display(&correction.corrected_value) {
                        lab.reference_range_low = Some(low);
                        lab.reference_range_high = Some(high);
                    } else {
                        lab.reference_range_text = Some(correction.corrected_value.clone());
                    }
                    true
                } else { false }
            }
            (EntityCategory::LabResult, "abnormal_flag") => {
                if let Some(lab) = structuring.extracted_entities.lab_results.get_mut(field.entity_index) {
                    lab.abnormal_flag = Some(correction.corrected_value.clone()); true
                } else { false }
            }
            (EntityCategory::LabResult, "collection_date") => {
                if let Some(lab) = structuring.extracted_entities.lab_results.get_mut(field.entity_index) {
                    lab.collection_date = Some(correction.corrected_value.clone()); true
                } else { false }
            }
            // === Diagnoses ===
            (EntityCategory::Diagnosis, "name") => {
                if let Some(d) = structuring.extracted_entities.diagnoses.get_mut(field.entity_index) {
                    d.name = correction.corrected_value.clone(); true
                } else { false }
            }
            (EntityCategory::Diagnosis, "date") => {
                if let Some(d) = structuring.extracted_entities.diagnoses.get_mut(field.entity_index) {
                    d.date = Some(correction.corrected_value.clone()); true
                } else { false }
            }
            (EntityCategory::Diagnosis, "status") => {
                if let Some(d) = structuring.extracted_entities.diagnoses.get_mut(field.entity_index) {
                    d.status = correction.corrected_value.clone(); true
                } else { false }
            }
            // === Allergies ===
            (EntityCategory::Allergy, "allergen") => {
                if let Some(a) = structuring.extracted_entities.allergies.get_mut(field.entity_index) {
                    a.allergen = correction.corrected_value.clone(); true
                } else { false }
            }
            (EntityCategory::Allergy, "reaction") => {
                if let Some(a) = structuring.extracted_entities.allergies.get_mut(field.entity_index) {
                    a.reaction = Some(correction.corrected_value.clone()); true
                } else { false }
            }
            (EntityCategory::Allergy, "severity") => {
                if let Some(a) = structuring.extracted_entities.allergies.get_mut(field.entity_index) {
                    a.severity = Some(correction.corrected_value.clone()); true
                } else { false }
            }
            // === Procedures ===
            (EntityCategory::Procedure, "name") => {
                if let Some(p) = structuring.extracted_entities.procedures.get_mut(field.entity_index) {
                    p.name = correction.corrected_value.clone(); true
                } else { false }
            }
            (EntityCategory::Procedure, "date") => {
                if let Some(p) = structuring.extracted_entities.procedures.get_mut(field.entity_index) {
                    p.date = Some(correction.corrected_value.clone()); true
                } else { false }
            }
            (EntityCategory::Procedure, "outcome") => {
                if let Some(p) = structuring.extracted_entities.procedures.get_mut(field.entity_index) {
                    p.outcome = Some(correction.corrected_value.clone()); true
                } else { false }
            }
            // === Referrals ===
            (EntityCategory::Referral, "referred_to") => {
                if let Some(r) = structuring.extracted_entities.referrals.get_mut(field.entity_index) {
                    r.referred_to = correction.corrected_value.clone(); true
                } else { false }
            }
            (EntityCategory::Referral, "specialty") => {
                if let Some(r) = structuring.extracted_entities.referrals.get_mut(field.entity_index) {
                    r.specialty = Some(correction.corrected_value.clone()); true
                } else { false }
            }
            (EntityCategory::Referral, "reason") => {
                if let Some(r) = structuring.extracted_entities.referrals.get_mut(field.entity_index) {
                    r.reason = Some(correction.corrected_value.clone()); true
                } else { false }
            }
            // === Professional (on StructuringResult) ===
            (EntityCategory::Professional, "name") => {
                if let Some(ref mut prof) = structuring.professional {
                    prof.name = correction.corrected_value.clone(); true
                } else {
                    structuring.professional = Some(ExtractedProfessional {
                        name: correction.corrected_value.clone(),
                        specialty: None,
                        institution: None,
                    }); true
                }
            }
            (EntityCategory::Professional, "specialty") => {
                if let Some(ref mut prof) = structuring.professional {
                    prof.specialty = Some(correction.corrected_value.clone()); true
                } else { false }
            }
            // === Document Date (on StructuringResult) ===
            (EntityCategory::Date, "document_date") => {
                structuring.document_date = parse_document_date(&correction.corrected_value);
                structuring.document_date.is_some()
            }
            _ => false,
        };

        if success {
            applied += 1;
        }
    }

    applied
}

/// Remove entities excluded by the patient during review.
/// Uses reverse-sorted indices to avoid shifting when removing by index.
pub fn remove_excluded_entities(
    structuring: &mut StructuringResult,
    excluded: &[ExcludedEntity],
) -> usize {
    if excluded.is_empty() {
        return 0;
    }

    let mut removed = 0;

    // Collect indices per category, sorted descending so removals don't shift later indices.
    let mut med_indices: Vec<usize> = excluded.iter()
        .filter(|e| e.entity_type == EntityCategory::Medication)
        .map(|e| e.entity_index).collect();
    let mut lab_indices: Vec<usize> = excluded.iter()
        .filter(|e| e.entity_type == EntityCategory::LabResult)
        .map(|e| e.entity_index).collect();
    let mut diag_indices: Vec<usize> = excluded.iter()
        .filter(|e| e.entity_type == EntityCategory::Diagnosis)
        .map(|e| e.entity_index).collect();
    let mut allergy_indices: Vec<usize> = excluded.iter()
        .filter(|e| e.entity_type == EntityCategory::Allergy)
        .map(|e| e.entity_index).collect();
    let mut proc_indices: Vec<usize> = excluded.iter()
        .filter(|e| e.entity_type == EntityCategory::Procedure)
        .map(|e| e.entity_index).collect();
    let mut ref_indices: Vec<usize> = excluded.iter()
        .filter(|e| e.entity_type == EntityCategory::Referral)
        .map(|e| e.entity_index).collect();

    for indices in [
        &mut med_indices, &mut lab_indices, &mut diag_indices,
        &mut allergy_indices, &mut proc_indices, &mut ref_indices,
    ] {
        indices.sort_unstable_by(|a, b| b.cmp(a));
        indices.dedup();
    }

    for &i in &med_indices {
        if i < structuring.extracted_entities.medications.len() {
            structuring.extracted_entities.medications.remove(i);
            removed += 1;
        }
    }
    for &i in &lab_indices {
        if i < structuring.extracted_entities.lab_results.len() {
            structuring.extracted_entities.lab_results.remove(i);
            removed += 1;
        }
    }
    for &i in &diag_indices {
        if i < structuring.extracted_entities.diagnoses.len() {
            structuring.extracted_entities.diagnoses.remove(i);
            removed += 1;
        }
    }
    for &i in &allergy_indices {
        if i < structuring.extracted_entities.allergies.len() {
            structuring.extracted_entities.allergies.remove(i);
            removed += 1;
        }
    }
    for &i in &proc_indices {
        if i < structuring.extracted_entities.procedures.len() {
            structuring.extracted_entities.procedures.remove(i);
            removed += 1;
        }
    }
    for &i in &ref_indices {
        if i < structuring.extracted_entities.referrals.len() {
            structuring.extracted_entities.referrals.remove(i);
            removed += 1;
        }
    }

    // Professional and Date are singletons — remove if excluded
    if excluded.iter().any(|e| e.entity_type == EntityCategory::Professional) {
        if structuring.professional.is_some() {
            structuring.professional = None;
            removed += 1;
        }
    }
    if excluded.iter().any(|e| e.entity_type == EntityCategory::Date) {
        if structuring.document_date.is_some() {
            structuring.document_date = None;
            removed += 1;
        }
    }

    removed
}

/// 12-ERC B5: Count total extractable fields for accuracy calculation.
/// Delegates to flatten for accurate variable-count fields.
pub fn count_extracted_fields(structuring: &StructuringResult) -> usize {
    flatten_entities_to_fields(structuring).len()
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
/// Handles encrypted paths like `{uuid}.pdf.enc` by stripping the `.enc` suffix.
pub fn detect_file_type(source_file: &str) -> OriginalFileType {
    let lower = source_file.to_lowercase();
    let effective = lower.strip_suffix(".enc").unwrap_or(&lower);
    if effective.ends_with(".pdf") {
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

        // 2 medications × 4 fields (name+dose+freq+route) = 8
        // 1 lab result × 5 (name+value+unit+range+flag) = 5
        // 1 diagnosis × 2 (name+status) = 2
        // 1 allergy × 3 (allergen+reaction+severity) = 3
        // 0 procedures = 0
        // 1 referral × 3 (referred_to+specialty+reason) = 3
        // 1 professional × 2 (name+specialty) = 2
        // 1 date = 1
        // Total = 24
        assert_eq!(fields.len(), 24);
    }

    #[test]
    fn flatten_flags_low_confidence_fields() {
        let result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);

        let flagged: Vec<_> = fields.iter().filter(|f| f.is_flagged).collect();
        // Atorvastatin (0.55) has 4 fields flagged (name+dose+freq+route), Penicillin allergy (0.65) has 3 (allergen+reaction+severity)
        assert_eq!(flagged.len(), 7);

        let confident: Vec<_> = fields.iter().filter(|f| !f.is_flagged).collect();
        assert_eq!(confident.len(), 17);
    }

    #[test]
    fn flatten_assigns_correct_categories() {
        let result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);

        let med_fields: Vec<_> = fields
            .iter()
            .filter(|f| f.entity_type == EntityCategory::Medication)
            .collect();
        // 2 meds × 4 fields (name+dose+freq+route) = 8
        assert_eq!(med_fields.len(), 8);

        let lab_fields: Vec<_> = fields
            .iter()
            .filter(|f| f.entity_type == EntityCategory::LabResult)
            .collect();
        // 1 lab × 5 fields (name+value+unit+range+flag) = 5
        assert_eq!(lab_fields.len(), 5);

        let prof_fields: Vec<_> = fields
            .iter()
            .filter(|f| f.entity_type == EntityCategory::Professional)
            .collect();
        // name + specialty = 2
        assert_eq!(prof_fields.len(), 2);
        assert!(prof_fields.iter().any(|f| f.value == "Dr. Chen"));

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
        // dose + frequency + route (no generic_name)
        assert_eq!(fields.len(), 3);
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
        let mut result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);

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

        let applied = apply_corrections(&mut result, &corrections, &fields);
        assert_eq!(applied, 1);
        assert_eq!(result.extracted_entities.medications[0].dose, "1000mg");
    }

    #[test]
    fn apply_corrections_updates_diagnosis_name() {
        let mut result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);

        let diag_field = fields
            .iter()
            .find(|f| f.entity_type == EntityCategory::Diagnosis && f.field_name == "name")
            .unwrap();

        let corrections = vec![FieldCorrection {
            field_id: diag_field.id,
            original_value: "Type 2 Diabetes".into(),
            corrected_value: "Type 2 Diabetes Mellitus".into(),
        }];

        let applied = apply_corrections(&mut result, &corrections, &fields);
        assert_eq!(applied, 1);
        assert_eq!(result.extracted_entities.diagnoses[0].name, "Type 2 Diabetes Mellitus");
    }

    #[test]
    fn apply_corrections_updates_lab_value() {
        let mut result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);

        let lab_value_field = fields
            .iter()
            .find(|f| f.entity_type == EntityCategory::LabResult && f.field_name == "value")
            .unwrap();

        let corrections = vec![FieldCorrection {
            field_id: lab_value_field.id,
            original_value: "7.2".into(),
            corrected_value: "7.5".into(),
        }];

        let applied = apply_corrections(&mut result, &corrections, &fields);
        assert_eq!(applied, 1);
        assert_eq!(result.extracted_entities.lab_results[0].value, Some(7.5));
        assert_eq!(
            result.extracted_entities.lab_results[0].value_text,
            Some("7.5".to_string())
        );
    }

    #[test]
    fn apply_corrections_skips_invalid_field_id() {
        let mut result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);

        let corrections = vec![FieldCorrection {
            field_id: Uuid::new_v4(), // non-existent
            original_value: "test".into(),
            corrected_value: "corrected".into(),
        }];

        let applied = apply_corrections(&mut result, &corrections, &fields);
        assert_eq!(applied, 0);
    }

    #[test]
    fn apply_corrections_multiple() {
        let mut result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);

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

        let applied = apply_corrections(&mut result, &corrections, &fields);
        assert_eq!(applied, 2);
        assert_eq!(result.extracted_entities.allergies[0].allergen, "Amoxicillin");
        assert_eq!(result.extracted_entities.referrals[0].referred_to, "Dr. Sharma");
    }

    // --- count_extracted_fields ---

    #[test]
    fn count_fields_matches_expected() {
        let result = make_structuring_result();
        let count = count_extracted_fields(&result);
        // Delegates to flatten — same as flatten test: 24 fields
        assert_eq!(count, 24);
    }

    #[test]
    fn count_fields_empty_entities() {
        let result = StructuringResult {
            document_id: Uuid::new_v4(),
            document_type: DocumentType::Other,
            document_date: None,
            professional: None,
            structured_markdown: String::new(),
            extracted_entities: ExtractedEntities::default(),
            structuring_confidence: 0.0,
            markdown_file_path: None,
            validation_warnings: vec![],
            raw_llm_response: None,
        };
        assert_eq!(count_extracted_fields(&result), 0);
    }

    // --- detect_file_type ---

    #[test]
    fn detect_pdf_type() {
        assert_eq!(detect_file_type("document.pdf"), OriginalFileType::Pdf);
        assert_eq!(detect_file_type("DOC.PDF"), OriginalFileType::Pdf);
    }

    #[test]
    fn detect_pdf_type_encrypted() {
        assert_eq!(detect_file_type("abc-123.pdf.enc"), OriginalFileType::Pdf);
        assert_eq!(detect_file_type("DOC.PDF.ENC"), OriginalFileType::Pdf);
    }

    #[test]
    fn detect_image_type() {
        assert_eq!(detect_file_type("photo.jpg"), OriginalFileType::Image);
        assert_eq!(detect_file_type("scan.png"), OriginalFileType::Image);
        assert_eq!(detect_file_type("xray.tiff"), OriginalFileType::Image);
    }

    #[test]
    fn detect_image_type_encrypted() {
        assert_eq!(detect_file_type("photo.jpg.enc"), OriginalFileType::Image);
        assert_eq!(detect_file_type("scan.png.enc"), OriginalFileType::Image);
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

    // --- 12-ERC Brick 1: Deterministic field IDs ---

    #[test]
    fn deterministic_field_id_stable() {
        let id1 = deterministic_field_id(&EntityCategory::Medication, 0, "dose");
        let id2 = deterministic_field_id(&EntityCategory::Medication, 0, "dose");
        assert_eq!(id1, id2);
    }

    #[test]
    fn deterministic_field_id_unique() {
        let a = deterministic_field_id(&EntityCategory::Medication, 0, "dose");
        let b = deterministic_field_id(&EntityCategory::Medication, 1, "dose");
        let c = deterministic_field_id(&EntityCategory::Medication, 0, "frequency");
        let d = deterministic_field_id(&EntityCategory::LabResult, 0, "dose");
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
    }

    #[test]
    fn flatten_stable_across_calls() {
        let result = make_structuring_result();
        let fields1 = flatten_entities_to_fields(&result);
        let fields2 = flatten_entities_to_fields(&result);
        assert_eq!(fields1.len(), fields2.len());
        for (f1, f2) in fields1.iter().zip(fields2.iter()) {
            assert_eq!(f1.id, f2.id, "Field ID mismatch for {}", f1.field_name);
        }
    }

    #[test]
    fn corrections_applied_cross_flatten() {
        let mut result = make_structuring_result();
        let fields1 = flatten_entities_to_fields(&result);
        let dose_field = fields1
            .iter()
            .find(|f| f.entity_type == EntityCategory::Medication && f.field_name == "dose" && f.entity_index == 0)
            .unwrap();
        let corrections = vec![FieldCorrection {
            field_id: dose_field.id,
            original_value: "500mg".into(),
            corrected_value: "750mg".into(),
        }];
        // Use a SECOND flatten call for field_map — deterministic IDs mean it still matches
        let fields2 = flatten_entities_to_fields(&result);
        let applied = apply_corrections(&mut result, &corrections, &fields2);
        assert_eq!(applied, 1);
        assert_eq!(result.extracted_entities.medications[0].dose, "750mg");
    }

    // --- 12-ERC Brick 5: Expanded flatten fields ---

    #[test]
    fn flatten_lab_all_fields() {
        let result = StructuringResult {
            document_id: Uuid::new_v4(),
            document_type: DocumentType::LabResult,
            document_date: None,
            professional: None,
            structured_markdown: String::new(),
            extracted_entities: ExtractedEntities {
                lab_results: vec![ExtractedLabResult {
                    test_name: "Glucose".into(),
                    test_code: None,
                    value: Some(5.6),
                    value_text: Some("5.6".into()),
                    unit: Some("mmol/L".into()),
                    reference_range_low: Some(3.9),
                    reference_range_high: Some(6.1),
                    reference_range_text: None,
                    abnormal_flag: Some("normal".into()),
                    collection_date: Some("2024-03-15".into()),
                    confidence: 0.9,
                }],
                ..Default::default()
            },
            structuring_confidence: 0.8,
            markdown_file_path: None,
            validation_warnings: vec![],
            raw_llm_response: None,
        };
        let fields = flatten_entities_to_fields(&result);
        // test_name + value + unit + reference_range + abnormal_flag + collection_date = 6
        assert_eq!(fields.len(), 6);
        assert!(fields.iter().any(|f| f.field_name == "collection_date"));
        assert!(fields.iter().any(|f| f.field_name == "reference_range" && f.value == "3.9 - 6.1"));
        assert!(fields.iter().any(|f| f.field_name == "abnormal_flag" && f.value == "Normal"));
    }

    #[test]
    fn flatten_medication_with_route() {
        let result = StructuringResult {
            document_id: Uuid::new_v4(),
            document_type: DocumentType::Prescription,
            document_date: None,
            professional: None,
            structured_markdown: String::new(),
            extracted_entities: ExtractedEntities {
                medications: vec![ExtractedMedication {
                    generic_name: Some("Amoxicillin".into()),
                    brand_name: None,
                    dose: "500mg".into(),
                    frequency: "3x daily".into(),
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
                }],
                ..Default::default()
            },
            structuring_confidence: 0.8,
            markdown_file_path: None,
            validation_warnings: vec![],
            raw_llm_response: None,
        };
        let fields = flatten_entities_to_fields(&result);
        // generic_name + dose + frequency + route = 4
        assert_eq!(fields.len(), 4);
        assert!(fields.iter().any(|f| f.field_name == "route" && f.value == "oral"));
    }

    #[test]
    fn flatten_diagnosis_with_date_status() {
        let result = StructuringResult {
            document_id: Uuid::new_v4(),
            document_type: DocumentType::Other,
            document_date: None,
            professional: None,
            structured_markdown: String::new(),
            extracted_entities: ExtractedEntities {
                diagnoses: vec![ExtractedDiagnosis {
                    name: "Hypertension".into(),
                    icd_code: None,
                    date: Some("2023-06-01".into()),
                    status: "active".into(),
                    confidence: 0.85,
                }],
                ..Default::default()
            },
            structuring_confidence: 0.8,
            markdown_file_path: None,
            validation_warnings: vec![],
            raw_llm_response: None,
        };
        let fields = flatten_entities_to_fields(&result);
        // name + date + status = 3
        assert_eq!(fields.len(), 3);
        assert!(fields.iter().any(|f| f.field_name == "date" && f.value == "2023-06-01"));
        assert!(fields.iter().any(|f| f.field_name == "status" && f.value == "active"));
    }

    #[test]
    fn flatten_allergy_with_reaction_severity() {
        let result = StructuringResult {
            document_id: Uuid::new_v4(),
            document_type: DocumentType::Other,
            document_date: None,
            professional: None,
            structured_markdown: String::new(),
            extracted_entities: ExtractedEntities {
                allergies: vec![ExtractedAllergy {
                    allergen: "Ibuprofen".into(),
                    reaction: Some("Hives".into()),
                    severity: Some("Severe".into()),
                    confidence: 0.78,
                }],
                ..Default::default()
            },
            structuring_confidence: 0.8,
            markdown_file_path: None,
            validation_warnings: vec![],
            raw_llm_response: None,
        };
        let fields = flatten_entities_to_fields(&result);
        // allergen + reaction + severity = 3
        assert_eq!(fields.len(), 3);
        assert!(fields.iter().any(|f| f.field_name == "reaction" && f.value == "Hives"));
        assert!(fields.iter().any(|f| f.field_name == "severity" && f.value == "Severe"));
    }

    #[test]
    fn flatten_procedure_with_date_outcome() {
        let result = StructuringResult {
            document_id: Uuid::new_v4(),
            document_type: DocumentType::Other,
            document_date: None,
            professional: None,
            structured_markdown: String::new(),
            extracted_entities: ExtractedEntities {
                procedures: vec![ExtractedProcedure {
                    name: "Colonoscopy".into(),
                    date: Some("2024-02-10".into()),
                    outcome: Some("Normal".into()),
                    follow_up_required: false,
                    follow_up_date: None,
                    confidence: 0.82,
                }],
                ..Default::default()
            },
            structuring_confidence: 0.8,
            markdown_file_path: None,
            validation_warnings: vec![],
            raw_llm_response: None,
        };
        let fields = flatten_entities_to_fields(&result);
        // name + date + outcome = 3
        assert_eq!(fields.len(), 3);
        assert!(fields.iter().any(|f| f.field_name == "date" && f.value == "2024-02-10"));
        assert!(fields.iter().any(|f| f.field_name == "outcome" && f.value == "Normal"));
    }

    #[test]
    fn flatten_referral_with_specialty_reason() {
        let result = StructuringResult {
            document_id: Uuid::new_v4(),
            document_type: DocumentType::Other,
            document_date: None,
            professional: None,
            structured_markdown: String::new(),
            extracted_entities: ExtractedEntities {
                referrals: vec![ExtractedReferral {
                    referred_to: "Dr. Lee".into(),
                    specialty: Some("Neurology".into()),
                    reason: Some("Chronic migraines".into()),
                    confidence: 0.75,
                }],
                ..Default::default()
            },
            structuring_confidence: 0.8,
            markdown_file_path: None,
            validation_warnings: vec![],
            raw_llm_response: None,
        };
        let fields = flatten_entities_to_fields(&result);
        // referred_to + specialty + reason = 3
        assert_eq!(fields.len(), 3);
        assert!(fields.iter().any(|f| f.field_name == "specialty" && f.value == "Neurology"));
        assert!(fields.iter().any(|f| f.field_name == "reason" && f.value == "Chronic migraines"));
    }

    #[test]
    fn flatten_professional_with_specialty() {
        let result = StructuringResult {
            document_id: Uuid::new_v4(),
            document_type: DocumentType::Other,
            document_date: None,
            professional: Some(ExtractedProfessional {
                name: "Dr. Kim".into(),
                specialty: Some("Oncology".into()),
                institution: None,
            }),
            structured_markdown: String::new(),
            extracted_entities: ExtractedEntities::default(),
            structuring_confidence: 0.8,
            markdown_file_path: None,
            validation_warnings: vec![],
            raw_llm_response: None,
        };
        let fields = flatten_entities_to_fields(&result);
        // name + specialty = 2
        assert_eq!(fields.len(), 2);
        assert!(fields.iter().any(|f| f.entity_type == EntityCategory::Professional && f.field_name == "name"));
        assert!(fields.iter().any(|f| f.entity_type == EntityCategory::Professional && f.field_name == "specialty" && f.value == "Oncology"));
    }

    // --- 12-ERC Brick 5: Expanded apply_corrections ---

    #[test]
    fn apply_correction_route() {
        let mut result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);
        let route_field = fields.iter().find(|f| f.entity_type == EntityCategory::Medication && f.field_name == "route" && f.entity_index == 0).unwrap();
        let corrections = vec![FieldCorrection {
            field_id: route_field.id,
            original_value: "oral".into(),
            corrected_value: "sublingual".into(),
        }];
        let applied = apply_corrections(&mut result, &corrections, &fields);
        assert_eq!(applied, 1);
        assert_eq!(result.extracted_entities.medications[0].route, "sublingual");
    }

    #[test]
    fn apply_correction_reference_range() {
        let mut result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);
        let range_field = fields.iter().find(|f| f.field_name == "reference_range").unwrap();
        let corrections = vec![FieldCorrection {
            field_id: range_field.id,
            original_value: "4 - 6".into(),
            corrected_value: "4.28 - 6.00".into(),
        }];
        let applied = apply_corrections(&mut result, &corrections, &fields);
        assert_eq!(applied, 1);
        assert!((result.extracted_entities.lab_results[0].reference_range_low.unwrap() - 4.28).abs() < 0.001);
        assert!((result.extracted_entities.lab_results[0].reference_range_high.unwrap() - 6.00).abs() < 0.001);
    }

    #[test]
    fn apply_correction_reference_range_french() {
        let mut result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);
        let range_field = fields.iter().find(|f| f.field_name == "reference_range").unwrap();
        let corrections = vec![FieldCorrection {
            field_id: range_field.id,
            original_value: "4 - 6".into(),
            corrected_value: "4,28 – 6,00".into(),
        }];
        let applied = apply_corrections(&mut result, &corrections, &fields);
        assert_eq!(applied, 1);
        assert!((result.extracted_entities.lab_results[0].reference_range_low.unwrap() - 4.28).abs() < 0.001);
        assert!((result.extracted_entities.lab_results[0].reference_range_high.unwrap() - 6.00).abs() < 0.001);
    }

    #[test]
    fn apply_correction_abnormal_flag() {
        let mut result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);
        let flag_field = fields.iter().find(|f| f.field_name == "abnormal_flag").unwrap();
        let corrections = vec![FieldCorrection {
            field_id: flag_field.id,
            original_value: "High".into(),
            corrected_value: "critical_high".into(),
        }];
        let applied = apply_corrections(&mut result, &corrections, &fields);
        assert_eq!(applied, 1);
        assert_eq!(result.extracted_entities.lab_results[0].abnormal_flag, Some("critical_high".into()));
    }

    #[test]
    fn apply_correction_professional_name() {
        let mut result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);
        let prof_field = fields.iter().find(|f| f.entity_type == EntityCategory::Professional && f.field_name == "name").unwrap();
        let corrections = vec![FieldCorrection {
            field_id: prof_field.id,
            original_value: "Dr. Chen".into(),
            corrected_value: "Dr. Zhang".into(),
        }];
        let applied = apply_corrections(&mut result, &corrections, &fields);
        assert_eq!(applied, 1);
        assert_eq!(result.professional.as_ref().unwrap().name, "Dr. Zhang");
    }

    #[test]
    fn apply_correction_professional_creates() {
        let mut result = make_structuring_result();
        result.professional = None;
        let fields = flatten_entities_to_fields(&make_structuring_result()); // Use original fields with professional
        let prof_field = fields.iter().find(|f| f.entity_type == EntityCategory::Professional && f.field_name == "name").unwrap();
        let corrections = vec![FieldCorrection {
            field_id: prof_field.id,
            original_value: "".into(),
            corrected_value: "Dr. New".into(),
        }];
        let applied = apply_corrections(&mut result, &corrections, &fields);
        assert_eq!(applied, 1);
        assert!(result.professional.is_some());
        assert_eq!(result.professional.as_ref().unwrap().name, "Dr. New");
    }

    #[test]
    fn apply_correction_document_date() {
        let mut result = make_structuring_result();
        let fields = flatten_entities_to_fields(&result);
        let date_field = fields.iter().find(|f| f.entity_type == EntityCategory::Date && f.field_name == "document_date").unwrap();
        let corrections = vec![FieldCorrection {
            field_id: date_field.id,
            original_value: "2024-01-15".into(),
            corrected_value: "2024-06-20".into(),
        }];
        let applied = apply_corrections(&mut result, &corrections, &fields);
        assert_eq!(applied, 1);
        assert_eq!(result.document_date, Some(NaiveDate::from_ymd_opt(2024, 6, 20).unwrap()));
    }

    #[test]
    fn humanize_abnormal_flag_variants() {
        assert_eq!(humanize_abnormal_flag("h"), "High");
        assert_eq!(humanize_abnormal_flag("HIGH"), "High");
        assert_eq!(humanize_abnormal_flag("l"), "Low");
        assert_eq!(humanize_abnormal_flag("critical_high"), "Critical high");
        assert_eq!(humanize_abnormal_flag("cl"), "Critical low");
        assert_eq!(humanize_abnormal_flag("normal"), "Normal");
        assert_eq!(humanize_abnormal_flag(""), "Normal");
        assert_eq!(humanize_abnormal_flag("custom"), "custom");
    }

    #[test]
    fn count_fields_delegates_to_flatten() {
        // Various entity combos all match flatten().len()
        let result = make_structuring_result();
        assert_eq!(count_extracted_fields(&result), flatten_entities_to_fields(&result).len());

        // Empty
        let empty = StructuringResult {
            document_id: Uuid::new_v4(),
            document_type: DocumentType::Other,
            document_date: None,
            professional: None,
            structured_markdown: String::new(),
            extracted_entities: ExtractedEntities::default(),
            structuring_confidence: 0.0,
            markdown_file_path: None,
            validation_warnings: vec![],
            raw_llm_response: None,
        };
        assert_eq!(count_extracted_fields(&empty), 0);

        // Just professional
        let prof_only = StructuringResult {
            professional: Some(ExtractedProfessional {
                name: "Dr. Test".into(),
                specialty: None,
                institution: None,
            }),
            ..empty
        };
        assert_eq!(count_extracted_fields(&prof_only), flatten_entities_to_fields(&prof_only).len());
    }

    // --- remove_excluded_entities ---

    #[test]
    fn remove_excluded_entities_removes_medication_by_index() {
        let mut result = make_structuring_result();
        assert_eq!(result.extracted_entities.medications.len(), 2);

        let excluded = vec![ExcludedEntity {
            entity_type: EntityCategory::Medication,
            entity_index: 0,
        }];
        let removed = remove_excluded_entities(&mut result, &excluded);

        assert_eq!(removed, 1);
        assert_eq!(result.extracted_entities.medications.len(), 1);
        assert_eq!(
            result.extracted_entities.medications[0].generic_name.as_deref(),
            Some("Atorvastatin"),
        );
    }

    #[test]
    fn remove_excluded_entities_removes_lab_result() {
        let mut result = make_structuring_result();
        assert_eq!(result.extracted_entities.lab_results.len(), 1);

        let excluded = vec![ExcludedEntity {
            entity_type: EntityCategory::LabResult,
            entity_index: 0,
        }];
        let removed = remove_excluded_entities(&mut result, &excluded);

        assert_eq!(removed, 1);
        assert!(result.extracted_entities.lab_results.is_empty());
    }

    #[test]
    fn remove_excluded_entities_removes_professional() {
        let mut result = make_structuring_result();
        assert!(result.professional.is_some());

        let excluded = vec![ExcludedEntity {
            entity_type: EntityCategory::Professional,
            entity_index: 0,
        }];
        let removed = remove_excluded_entities(&mut result, &excluded);

        assert_eq!(removed, 1);
        assert!(result.professional.is_none());
    }

    #[test]
    fn remove_excluded_entities_removes_date() {
        let mut result = make_structuring_result();
        assert!(result.document_date.is_some());

        let excluded = vec![ExcludedEntity {
            entity_type: EntityCategory::Date,
            entity_index: 0,
        }];
        let removed = remove_excluded_entities(&mut result, &excluded);

        assert_eq!(removed, 1);
        assert!(result.document_date.is_none());
    }

    #[test]
    fn remove_excluded_entities_empty_list_is_noop() {
        let mut result = make_structuring_result();
        let med_count = result.extracted_entities.medications.len();

        let removed = remove_excluded_entities(&mut result, &[]);
        assert_eq!(removed, 0);
        assert_eq!(result.extracted_entities.medications.len(), med_count);
    }

    #[test]
    fn remove_excluded_entities_out_of_bounds_skipped() {
        let mut result = make_structuring_result();
        let med_count = result.extracted_entities.medications.len();

        let excluded = vec![ExcludedEntity {
            entity_type: EntityCategory::Medication,
            entity_index: 999,
        }];
        let removed = remove_excluded_entities(&mut result, &excluded);

        assert_eq!(removed, 0);
        assert_eq!(result.extracted_entities.medications.len(), med_count);
    }

    #[test]
    fn remove_excluded_entities_multiple_categories() {
        let mut result = make_structuring_result();

        let excluded = vec![
            ExcludedEntity { entity_type: EntityCategory::Medication, entity_index: 1 },
            ExcludedEntity { entity_type: EntityCategory::LabResult, entity_index: 0 },
            ExcludedEntity { entity_type: EntityCategory::Professional, entity_index: 0 },
        ];
        let removed = remove_excluded_entities(&mut result, &excluded);

        assert_eq!(removed, 3);
        assert_eq!(result.extracted_entities.medications.len(), 1);
        assert_eq!(
            result.extracted_entities.medications[0].generic_name.as_deref(),
            Some("Metformin"),
        );
        assert!(result.extracted_entities.lab_results.is_empty());
        assert!(result.professional.is_none());
    }
}
