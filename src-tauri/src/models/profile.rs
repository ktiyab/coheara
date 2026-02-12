use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileTrust {
    pub total_documents: i32,
    pub documents_verified: i32,
    pub documents_corrected: i32,
    pub extraction_accuracy: f64,
    pub last_updated: NaiveDateTime,
}
