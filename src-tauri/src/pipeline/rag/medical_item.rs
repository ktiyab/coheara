//! ME-01 Brick 1: Unified medical item type for scoring pipeline.
//!
//! MedicalItem wraps all 6 scorable entity types into a single enum
//! so the scoring engine can process them uniformly.

use chrono::NaiveDate;
use uuid::Uuid;

use crate::models::enums::{
    AbnormalFlag, AllergySeverity, DiagnosisStatus, MedicationStatus,
};
use crate::models::VitalType;

/// The 6 scorable entity types in the medical knowledge graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemType {
    Medication,
    LabResult,
    Diagnosis,
    Allergy,
    Symptom,
    VitalSign,
}

impl ItemType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Medication => "medication",
            Self::LabResult => "lab_result",
            Self::Diagnosis => "diagnosis",
            Self::Allergy => "allergy",
            Self::Symptom => "symptom",
            Self::VitalSign => "vital_sign",
        }
    }
}

/// A medical item flattened for scoring. Avoids lifetime complexity by
/// extracting only the fields the scoring engine needs.
#[derive(Debug, Clone)]
pub struct MedicalItem {
    pub id: Uuid,
    pub item_type: ItemType,
    /// Human-readable display name (generic_name, test_name, allergen, etc.)
    pub display_name: String,
    /// Searchable text fields concatenated for BM25.
    pub searchable_text: String,
    /// Source document UUID (for citation and verification lookup).
    pub document_id: Option<Uuid>,
    /// Most relevant date for this item (start_date, collection_date, etc.)
    pub relevant_date: Option<NaiveDate>,
    /// Clinical severity signal for the S factor.
    pub severity: SeveritySignal,
    /// Status signal for temporal decay and filtering.
    pub status: StatusSignal,
}

/// Severity signal extracted from entity-specific severity fields.
#[derive(Debug, Clone, PartialEq)]
pub enum SeveritySignal {
    /// No severity information (default).
    None,
    /// Numeric severity (symptoms: 0-10).
    Numeric(i32),
    /// Lab abnormal flag.
    LabFlag(AbnormalFlag),
    /// Allergy severity.
    AllergySeverity(AllergySeverity),
}

/// Status signal for active/inactive determination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusSignal {
    /// Active entity (medication active, diagnosis active, symptom active).
    Active,
    /// Inactive entity (medication stopped, diagnosis resolved, symptom resolved).
    Inactive,
    /// Unknown or not applicable (labs, vitals - always "current" at their date).
    Current,
}

// ── Conversion from domain models ─────────────────────────────────

impl MedicalItem {
    pub fn from_medication(med: &crate::models::Medication) -> Self {
        let mut searchable = med.generic_name.clone();
        if let Some(ref brand) = med.brand_name {
            searchable.push(' ');
            searchable.push_str(brand);
        }
        if let Some(ref condition) = med.condition {
            searchable.push(' ');
            searchable.push_str(condition);
        }
        searchable.push(' ');
        searchable.push_str(&med.dose);

        Self {
            id: med.id,
            item_type: ItemType::Medication,
            display_name: med.generic_name.clone(),
            searchable_text: searchable,
            document_id: Some(med.document_id),
            relevant_date: med.start_date,
            severity: SeveritySignal::None,
            status: match med.status {
                MedicationStatus::Active => StatusSignal::Active,
                MedicationStatus::Stopped | MedicationStatus::Paused => StatusSignal::Inactive,
            },
        }
    }

    pub fn from_lab(lab: &crate::models::LabResult) -> Self {
        let mut searchable = lab.test_name.clone();
        if let Some(ref code) = lab.test_code {
            searchable.push(' ');
            searchable.push_str(code);
        }
        if let Some(ref text) = lab.value_text {
            searchable.push(' ');
            searchable.push_str(text);
        }

        Self {
            id: lab.id,
            item_type: ItemType::LabResult,
            display_name: lab.test_name.clone(),
            searchable_text: searchable,
            document_id: Some(lab.document_id),
            relevant_date: Some(lab.collection_date),
            severity: SeveritySignal::LabFlag(lab.abnormal_flag.clone()),
            status: StatusSignal::Current,
        }
    }

    pub fn from_diagnosis(dx: &crate::models::Diagnosis) -> Self {
        let mut searchable = dx.name.clone();
        if let Some(ref icd) = dx.icd_code {
            searchable.push(' ');
            searchable.push_str(icd);
        }

        Self {
            id: dx.id,
            item_type: ItemType::Diagnosis,
            display_name: dx.name.clone(),
            searchable_text: searchable,
            document_id: Some(dx.document_id),
            relevant_date: dx.date_diagnosed,
            severity: SeveritySignal::None,
            status: match dx.status {
                DiagnosisStatus::Active | DiagnosisStatus::Monitoring => StatusSignal::Active,
                DiagnosisStatus::Resolved => StatusSignal::Inactive,
            },
        }
    }

    pub fn from_allergy(allergy: &crate::models::Allergy) -> Self {
        let mut searchable = allergy.allergen.clone();
        if let Some(ref reaction) = allergy.reaction {
            searchable.push(' ');
            searchable.push_str(reaction);
        }

        Self {
            id: allergy.id,
            item_type: ItemType::Allergy,
            display_name: allergy.allergen.clone(),
            searchable_text: searchable,
            document_id: allergy.document_id,
            relevant_date: allergy.date_identified,
            severity: SeveritySignal::AllergySeverity(allergy.severity.clone()),
            status: StatusSignal::Active, // Allergies are always active
        }
    }

    pub fn from_symptom(symptom: &crate::models::Symptom) -> Self {
        let mut searchable = format!("{} {}", symptom.category, symptom.specific);
        if let Some(ref region) = symptom.body_region {
            searchable.push(' ');
            searchable.push_str(region);
        }
        if let Some(ref character) = symptom.character {
            searchable.push(' ');
            searchable.push_str(character);
        }

        Self {
            id: symptom.id,
            item_type: ItemType::Symptom,
            display_name: symptom.specific.clone(),
            searchable_text: searchable,
            document_id: None, // Symptoms don't have document_id
            relevant_date: Some(symptom.onset_date),
            severity: SeveritySignal::Numeric(symptom.severity),
            status: if symptom.still_active {
                StatusSignal::Active
            } else {
                StatusSignal::Inactive
            },
        }
    }

    pub fn from_vital_sign(vital: &crate::models::VitalSign) -> Self {
        let display = vital_type_display(&vital.vital_type);
        let searchable = format!(
            "{} {} {}",
            display,
            vital.value_primary,
            vital.unit
        );

        Self {
            id: vital.id,
            item_type: ItemType::VitalSign,
            display_name: display.to_string(),
            searchable_text: searchable,
            document_id: None, // Vitals don't have document_id
            relevant_date: Some(vital.recorded_at.date()),
            severity: SeveritySignal::None,
            status: StatusSignal::Current,
        }
    }
}

fn vital_type_display(vt: &VitalType) -> &'static str {
    match vt {
        VitalType::Temperature => "Temperature",
        VitalType::BloodPressure => "Blood Pressure",
        VitalType::Weight => "Weight",
        VitalType::Height => "Height",
        VitalType::HeartRate => "Heart Rate",
        VitalType::BloodGlucose => "Blood Glucose",
        VitalType::OxygenSaturation => "Oxygen Saturation",
    }
}

/// Bulk convert all structured data into a flat Vec<MedicalItem>.
pub fn collect_items(ctx: &super::types::StructuredContext) -> Vec<MedicalItem> {
    let mut items = Vec::new();
    for med in &ctx.medications {
        items.push(MedicalItem::from_medication(med));
    }
    for lab in &ctx.lab_results {
        items.push(MedicalItem::from_lab(lab));
    }
    for dx in &ctx.diagnoses {
        items.push(MedicalItem::from_diagnosis(dx));
    }
    for allergy in &ctx.allergies {
        items.push(MedicalItem::from_allergy(allergy));
    }
    for symptom in &ctx.symptoms {
        items.push(MedicalItem::from_symptom(symptom));
    }
    for vital in &ctx.vital_signs {
        items.push(MedicalItem::from_vital_sign(vital));
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::enums::*;
    use crate::models::{VitalSource, VitalType};

    #[test]
    fn medication_item_extracts_fields() {
        let med = crate::models::Medication {
            id: Uuid::new_v4(),
            generic_name: "Metformin".into(),
            brand_name: Some("Glucophage".into()),
            dose: "500mg".into(),
            frequency: "twice daily".into(),
            frequency_type: FrequencyType::Scheduled,
            route: "oral".into(),
            prescriber_id: None,
            start_date: Some(NaiveDate::from_ymd_opt(2025, 6, 15).unwrap()),
            end_date: None,
            reason_start: None,
            reason_stop: None,
            is_otc: false,
            status: MedicationStatus::Active,
            administration_instructions: None,
            max_daily_dose: None,
            condition: Some("Type 2 Diabetes".into()),
            dose_type: DoseType::Fixed,
            is_compound: false,
            document_id: Uuid::new_v4(),
        };
        let item = MedicalItem::from_medication(&med);
        assert_eq!(item.item_type, ItemType::Medication);
        assert_eq!(item.display_name, "Metformin");
        assert!(item.searchable_text.contains("Glucophage"));
        assert!(item.searchable_text.contains("Type 2 Diabetes"));
        assert_eq!(item.status, StatusSignal::Active);
    }

    #[test]
    fn lab_item_extracts_abnormal_flag() {
        let lab = crate::models::LabResult {
            id: Uuid::new_v4(),
            test_name: "HbA1c".into(),
            test_code: Some("4548-4".into()),
            value: Some(7.2),
            value_text: None,
            unit: Some("%".into()),
            reference_range_low: Some(4.0),
            reference_range_high: Some(5.6),
            abnormal_flag: AbnormalFlag::High,
            collection_date: NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
            lab_facility: None,
            ordering_physician_id: None,
            document_id: Uuid::new_v4(),
        };
        let item = MedicalItem::from_lab(&lab);
        assert_eq!(item.item_type, ItemType::LabResult);
        assert_eq!(item.display_name, "HbA1c");
        assert_eq!(item.severity, SeveritySignal::LabFlag(AbnormalFlag::High));
    }

    #[test]
    fn allergy_item_extracts_severity() {
        let allergy = crate::models::Allergy {
            id: Uuid::new_v4(),
            allergen: "Penicillin".into(),
            reaction: Some("Anaphylaxis".into()),
            severity: AllergySeverity::LifeThreatening,
            allergen_category: None,
            date_identified: None,
            source: AllergySource::DocumentExtracted,
            document_id: Some(Uuid::new_v4()),
            verified: true,
        };
        let item = MedicalItem::from_allergy(&allergy);
        assert_eq!(item.display_name, "Penicillin");
        assert!(item.searchable_text.contains("Anaphylaxis"));
        assert_eq!(
            item.severity,
            SeveritySignal::AllergySeverity(AllergySeverity::LifeThreatening)
        );
        assert_eq!(item.status, StatusSignal::Active);
    }

    #[test]
    fn stopped_medication_is_inactive() {
        let med = crate::models::Medication {
            id: Uuid::new_v4(),
            generic_name: "Warfarin".into(),
            brand_name: None,
            dose: "5mg".into(),
            frequency: "daily".into(),
            frequency_type: FrequencyType::Scheduled,
            route: "oral".into(),
            prescriber_id: None,
            start_date: None,
            end_date: Some(NaiveDate::from_ymd_opt(2025, 12, 1).unwrap()),
            reason_start: None,
            reason_stop: Some("Switched to DOAC".into()),
            is_otc: false,
            status: MedicationStatus::Stopped,
            administration_instructions: None,
            max_daily_dose: None,
            condition: None,
            dose_type: DoseType::Fixed,
            is_compound: false,
            document_id: Uuid::new_v4(),
        };
        let item = MedicalItem::from_medication(&med);
        assert_eq!(item.status, StatusSignal::Inactive);
    }

    #[test]
    fn symptom_active_flag() {
        let symptom = crate::models::Symptom {
            id: Uuid::new_v4(),
            category: "Neurological".into(),
            specific: "Headache".into(),
            severity: 6,
            body_region: Some("Head".into()),
            duration: Some("3 days".into()),
            character: Some("Throbbing".into()),
            aggravating: None,
            relieving: None,
            timing_pattern: None,
            onset_date: NaiveDate::from_ymd_opt(2026, 2, 28).unwrap(),
            onset_time: None,
            recorded_date: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
            still_active: true,
            resolved_date: None,
            related_medication_id: None,
            related_diagnosis_id: None,
            source: SymptomSource::PatientReported,
            notes: None,
        };
        let item = MedicalItem::from_symptom(&symptom);
        assert_eq!(item.display_name, "Headache");
        assert_eq!(item.severity, SeveritySignal::Numeric(6));
        assert_eq!(item.status, StatusSignal::Active);
    }

    #[test]
    fn vital_sign_display_name() {
        let vital = crate::models::VitalSign {
            id: Uuid::new_v4(),
            vital_type: VitalType::BloodPressure,
            value_primary: 140.0,
            value_secondary: Some(90.0),
            unit: "mmHg".into(),
            recorded_at: chrono::NaiveDate::from_ymd_opt(2026, 3, 1)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap(),
            notes: None,
            source: VitalSource::Manual,
            created_at: chrono::Local::now().naive_local(),
        };
        let item = MedicalItem::from_vital_sign(&vital);
        assert_eq!(item.display_name, "Blood Pressure");
        assert!(item.searchable_text.contains("140"));
    }

    #[test]
    fn collect_items_aggregates_all_types() {
        let ctx = super::super::types::StructuredContext {
            medications: vec![crate::models::Medication {
                id: Uuid::new_v4(),
                generic_name: "Aspirin".into(),
                brand_name: None,
                dose: "100mg".into(),
                frequency: "daily".into(),
                frequency_type: FrequencyType::Scheduled,
                route: "oral".into(),
                prescriber_id: None,
                start_date: None,
                end_date: None,
                reason_start: None,
                reason_stop: None,
                is_otc: true,
                status: MedicationStatus::Active,
                administration_instructions: None,
                max_daily_dose: None,
                condition: None,
                dose_type: DoseType::Fixed,
                is_compound: false,
                document_id: Uuid::new_v4(),
            }],
            diagnoses: vec![crate::models::Diagnosis {
                id: Uuid::new_v4(),
                name: "Hypertension".into(),
                icd_code: Some("I10".into()),
                date_diagnosed: None,
                diagnosing_professional_id: None,
                status: DiagnosisStatus::Active,
                document_id: Uuid::new_v4(),
            }],
            lab_results: vec![],
            allergies: vec![],
            symptoms: vec![],
            vital_signs: vec![],
            recent_conversations: vec![],
            screening_records: vec![],
            entity_connections: vec![],
        };
        let items = collect_items(&ctx);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].item_type, ItemType::Medication);
        assert_eq!(items[1].item_type, ItemType::Diagnosis);
    }
}
