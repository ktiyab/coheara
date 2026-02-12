use chrono::NaiveDate;
use uuid::Uuid;

use super::enums::{DocumentType, MedicationStatus};

#[derive(Debug, Default)]
pub struct DocumentFilter {
    pub doc_type: Option<DocumentType>,
    pub professional_id: Option<Uuid>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub verified_only: bool,
}

#[derive(Debug, Default)]
pub struct MedicationFilter {
    pub status: Option<MedicationStatus>,
    pub generic_name: Option<String>,
    pub prescriber_id: Option<Uuid>,
    pub include_otc: bool,
}

#[derive(Debug, Default)]
pub struct LabResultFilter {
    pub test_name: Option<String>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub abnormal_only: bool,
    pub critical_only: bool,
}

#[derive(Debug, Default)]
pub struct SymptomFilter {
    pub active_only: bool,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub category: Option<String>,
    pub related_medication_id: Option<Uuid>,
}

#[derive(Debug, Default)]
pub struct AllergyFilter {
    pub verified_only: bool,
}

#[derive(Debug, Default)]
pub struct ProfessionalFilter {
    pub name: Option<String>,
    pub specialty: Option<String>,
}
