use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Aggregate summary for a dependent's profile, visible to caregiver
/// without unlocking the dependent's encrypted database (Spec 46).
///
/// Stored as JSON at profiles level (outside encryption).
/// Updated when the dependent's profile is opened.
/// Contains ONLY non-sensitive aggregate counts — no health details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaregiverSummary {
    pub managed_profile_id: Uuid,
    pub managed_profile_name: String,
    pub caregiver_profile_id: Uuid,
    pub alert_count: u32,
    pub critical_alert_count: u32,
    pub active_medication_count: u32,
    pub next_appointment_date: Option<String>,
    pub last_document_date: Option<String>,
    pub color_index: Option<u8>,
    pub updated_at: NaiveDateTime,
}

/// Collection of caregiver summaries stored at profiles directory level.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CaregiverSummaries {
    pub summaries: Vec<CaregiverSummary>,
}
