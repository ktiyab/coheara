use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::enums::AppointmentType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Appointment {
    pub id: Uuid,
    pub professional_id: Uuid,
    pub date: NaiveDate,
    pub appointment_type: AppointmentType,
    pub pre_summary_generated: bool,
    pub post_notes: Option<String>,
}
