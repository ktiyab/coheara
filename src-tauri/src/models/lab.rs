use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::enums::AbnormalFlag;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabResult {
    pub id: Uuid,
    pub test_name: String,
    pub test_code: Option<String>,
    pub value: Option<f64>,
    pub value_text: Option<String>,
    pub unit: Option<String>,
    pub reference_range_low: Option<f64>,
    pub reference_range_high: Option<f64>,
    pub abnormal_flag: AbnormalFlag,
    pub collection_date: NaiveDate,
    pub lab_facility: Option<String>,
    pub ordering_physician_id: Option<Uuid>,
    pub document_id: Uuid,
}
