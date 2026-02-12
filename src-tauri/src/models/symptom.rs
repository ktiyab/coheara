use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::enums::SymptomSource;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symptom {
    pub id: Uuid,
    pub category: String,
    pub specific: String,
    pub severity: i32,
    pub body_region: Option<String>,
    pub duration: Option<String>,
    pub character: Option<String>,
    pub aggravating: Option<String>,
    pub relieving: Option<String>,
    pub timing_pattern: Option<String>,
    pub onset_date: NaiveDate,
    pub onset_time: Option<String>,
    pub recorded_date: NaiveDate,
    pub still_active: bool,
    pub resolved_date: Option<NaiveDate>,
    pub related_medication_id: Option<Uuid>,
    pub related_diagnosis_id: Option<Uuid>,
    pub source: SymptomSource,
    pub notes: Option<String>,
}
