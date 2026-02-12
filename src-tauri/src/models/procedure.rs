use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Procedure {
    pub id: Uuid,
    pub name: String,
    pub date: Option<NaiveDate>,
    pub performing_professional_id: Option<Uuid>,
    pub facility: Option<String>,
    pub outcome: Option<String>,
    pub follow_up_required: bool,
    pub follow_up_date: Option<NaiveDate>,
    pub document_id: Uuid,
}
