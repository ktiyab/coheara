use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::StructuringError;
use crate::crypto::ProfileSession;
use crate::models::enums::DocumentType;

/// Complete result of medical structuring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuringResult {
    pub document_id: Uuid,
    pub document_type: DocumentType,
    pub document_date: Option<NaiveDate>,
    pub professional: Option<ExtractedProfessional>,
    pub structured_markdown: String,
    pub extracted_entities: ExtractedEntities,
    pub structuring_confidence: f32,
    pub markdown_file_path: Option<String>,
}

/// All entities extracted from a single document
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtractedEntities {
    pub medications: Vec<ExtractedMedication>,
    pub lab_results: Vec<ExtractedLabResult>,
    pub diagnoses: Vec<ExtractedDiagnosis>,
    pub allergies: Vec<ExtractedAllergy>,
    pub procedures: Vec<ExtractedProcedure>,
    pub referrals: Vec<ExtractedReferral>,
    pub instructions: Vec<ExtractedInstruction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedMedication {
    pub generic_name: Option<String>,
    pub brand_name: Option<String>,
    pub dose: String,
    pub frequency: String,
    pub frequency_type: String,
    pub route: String,
    pub reason: Option<String>,
    pub instructions: Vec<String>,
    pub is_compound: bool,
    pub compound_ingredients: Vec<ExtractedCompoundIngredient>,
    pub tapering_steps: Vec<ExtractedTaperingStep>,
    pub max_daily_dose: Option<String>,
    pub condition: Option<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedCompoundIngredient {
    pub name: String,
    pub dose: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedTaperingStep {
    pub step_number: u32,
    pub dose: String,
    pub duration_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedLabResult {
    pub test_name: String,
    pub test_code: Option<String>,
    pub value: Option<f64>,
    pub value_text: Option<String>,
    pub unit: Option<String>,
    pub reference_range_low: Option<f64>,
    pub reference_range_high: Option<f64>,
    pub abnormal_flag: Option<String>,
    pub collection_date: Option<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedDiagnosis {
    pub name: String,
    pub icd_code: Option<String>,
    pub date: Option<String>,
    pub status: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedAllergy {
    pub allergen: String,
    pub reaction: Option<String>,
    pub severity: Option<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedProcedure {
    pub name: String,
    pub date: Option<String>,
    pub outcome: Option<String>,
    pub follow_up_required: bool,
    pub follow_up_date: Option<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedReferral {
    pub referred_to: String,
    pub specialty: Option<String>,
    pub reason: Option<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedInstruction {
    pub text: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedProfessional {
    pub name: String,
    pub specialty: Option<String>,
    pub institution: Option<String>,
}

/// Orchestrates the structuring process
pub trait MedicalStructurer {
    fn structure_document(
        &self,
        document_id: &Uuid,
        raw_text: &str,
        ocr_confidence: f32,
        session: &ProfileSession,
    ) -> Result<StructuringResult, StructuringError>;
}

/// Ollama LLM client abstraction (allows mocking)
pub trait LlmClient {
    fn generate(
        &self,
        model: &str,
        prompt: &str,
        system: &str,
    ) -> Result<String, StructuringError>;

    fn is_model_available(&self, model: &str) -> Result<bool, StructuringError>;

    fn list_models(&self) -> Result<Vec<String>, StructuringError>;
}
