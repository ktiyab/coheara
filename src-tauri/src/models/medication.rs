use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::enums::{DoseType, FrequencyType, MedicationStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Medication {
    pub id: Uuid,
    pub generic_name: String,
    pub brand_name: Option<String>,
    pub dose: String,
    pub frequency: String,
    pub frequency_type: FrequencyType,
    pub route: String,
    pub prescriber_id: Option<Uuid>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub reason_start: Option<String>,
    pub reason_stop: Option<String>,
    pub is_otc: bool,
    pub status: MedicationStatus,
    pub administration_instructions: Option<String>,
    pub max_daily_dose: Option<String>,
    pub condition: Option<String>,
    pub dose_type: DoseType,
    pub is_compound: bool,
    pub document_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompoundIngredient {
    pub id: Uuid,
    pub medication_id: Uuid,
    pub ingredient_name: String,
    pub ingredient_dose: Option<String>,
    pub maps_to_generic: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaperingStep {
    pub id: Uuid,
    pub medication_id: Uuid,
    pub step_number: i32,
    pub dose: String,
    pub duration_days: i32,
    pub start_date: Option<NaiveDate>,
    pub document_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationInstruction {
    pub id: Uuid,
    pub medication_id: Uuid,
    pub instruction: String,
    pub timing: Option<String>,
    pub source_document_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoseChange {
    pub id: Uuid,
    pub medication_id: Uuid,
    pub old_dose: Option<String>,
    pub new_dose: String,
    pub old_frequency: Option<String>,
    pub new_frequency: Option<String>,
    pub change_date: NaiveDate,
    pub changed_by_id: Option<Uuid>,
    pub reason: Option<String>,
    pub document_id: Option<Uuid>,
}
