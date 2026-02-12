use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::enums::{AlertType, DismissedBy};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DismissedAlert {
    pub id: Uuid,
    pub alert_type: AlertType,
    pub entity_ids: String,
    pub dismissed_date: NaiveDateTime,
    pub reason: Option<String>,
    pub dismissed_by: DismissedBy,
}
