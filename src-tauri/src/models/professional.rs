use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Professional {
    pub id: Uuid,
    pub name: String,
    pub specialty: Option<String>,
    pub institution: Option<String>,
    pub first_seen_date: Option<NaiveDate>,
    pub last_seen_date: Option<NaiveDate>,
}
