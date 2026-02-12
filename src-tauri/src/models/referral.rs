use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::enums::ReferralStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Referral {
    pub id: Uuid,
    pub referring_professional_id: Uuid,
    pub referred_to_professional_id: Uuid,
    pub reason: Option<String>,
    pub date: NaiveDate,
    pub status: ReferralStatus,
    pub document_id: Option<Uuid>,
}
