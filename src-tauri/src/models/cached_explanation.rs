use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Type of entity that has a cached explanation (Spec 44: AI Pipeline).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExplanationEntityType {
    LabResult,
    Medication,
    Diagnosis,
    Document,
}

impl ExplanationEntityType {
    pub fn as_str(self) -> &'static str {
        match self {
            ExplanationEntityType::LabResult => "lab_result",
            ExplanationEntityType::Medication => "medication",
            ExplanationEntityType::Diagnosis => "diagnosis",
            ExplanationEntityType::Document => "document",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "lab_result" => Some(ExplanationEntityType::LabResult),
            "medication" => Some(ExplanationEntityType::Medication),
            "diagnosis" => Some(ExplanationEntityType::Diagnosis),
            "document" => Some(ExplanationEntityType::Document),
            _ => None,
        }
    }
}

/// Pre-computed AI explanation cached at import time.
///
/// Layer 1 of the 3-layer response strategy (Spec 44):
/// - Cached explanations serve in <100ms
/// - Invalidated when source entity changes
/// - Per-language (EN/FR/DE)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedExplanation {
    pub id: Uuid,
    pub entity_type: ExplanationEntityType,
    pub entity_id: Uuid,
    pub explanation_text: String,
    pub language: String,
    pub model_version: Option<String>,
    pub created_at: NaiveDateTime,
    pub invalidated_at: Option<NaiveDateTime>,
}
