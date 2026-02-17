use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::enums::{DocumentType, PipelineStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: Uuid,
    pub doc_type: DocumentType,
    pub title: String,
    pub document_date: Option<NaiveDate>,
    pub ingestion_date: NaiveDateTime,
    pub professional_id: Option<Uuid>,
    pub source_file: String,
    pub markdown_file: Option<String>,
    pub ocr_confidence: Option<f32>,
    pub verified: bool,
    pub source_deleted: bool,
    pub perceptual_hash: Option<String>,
    pub notes: Option<String>,
    pub pipeline_status: PipelineStatus,
}
