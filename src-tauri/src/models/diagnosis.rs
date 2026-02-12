use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::enums::DiagnosisStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnosis {
    pub id: Uuid,
    pub name: String,
    pub icd_code: Option<String>,
    pub date_diagnosed: Option<NaiveDate>,
    pub diagnosing_professional_id: Option<Uuid>,
    pub status: DiagnosisStatus,
    pub document_id: Uuid,
}
