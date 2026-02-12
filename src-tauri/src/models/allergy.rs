use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::enums::{AllergySeverity, AllergySource};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Allergy {
    pub id: Uuid,
    pub allergen: String,
    pub reaction: Option<String>,
    pub severity: AllergySeverity,
    pub date_identified: Option<NaiveDate>,
    pub source: AllergySource,
    pub document_id: Option<Uuid>,
    pub verified: bool,
}
