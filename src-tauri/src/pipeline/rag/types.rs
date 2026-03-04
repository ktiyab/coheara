use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::RagError;
use super::scored_context::GroundingLevel;
use crate::crypto::ProfileSession;
use crate::models::*;

/// A patient's query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientQuery {
    pub text: String,
    pub conversation_id: Uuid,
    pub query_type: Option<QueryType>,
}

/// Classified query type determines retrieval strategy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum QueryType {
    Factual,
    Exploratory,
    Symptom,
    Timeline,
    General,
}

/// Complete RAG response (before safety filtering)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagResponse {
    pub text: String,
    pub citations: Vec<Citation>,
    /// ME-03: Guideline citations from clinical insights (deterministic, not LLM-generated).
    pub guideline_citations: Vec<GuidelineCitation>,
    pub confidence: f32,
    pub query_type: QueryType,
    pub context_used: ContextSummary,
    pub boundary_check: BoundaryCheck,
    /// ME-01: Data-driven grounding level (computed from scored items, not LLM self-report).
    pub grounding: GroundingLevel,
}

/// A source citation linking a response claim to a document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    pub document_id: Uuid,
    pub document_title: String,
    pub document_date: Option<String>,
    pub professional_name: Option<String>,
    pub chunk_text: String,
    pub relevance_score: f32,
}

/// ME-03: A clinical guideline citation from the invariant enrichment engine.
///
/// Unlike document citations (extracted from LLM output or chunk scores),
/// guideline citations are deterministic — sourced from ClinicalInsight references.
/// Examples: "ISH 2020", "KDIGO 2024", "WHO EML", "BTS 2017".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuidelineCitation {
    /// Guideline source identifier (e.g., "ISH 2020").
    pub source: String,
    /// Number of clinical insights that reference this guideline.
    pub insight_count: usize,
}

/// Summary of context used for generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSummary {
    pub semantic_chunks_used: usize,
    pub structured_records_used: usize,
    pub total_context_tokens: usize,
}

/// Boundary check from structured output
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BoundaryCheck {
    Understanding,
    Awareness,
    Preparation,
    /// No patient data available - not a safety issue, just empty database.
    /// The helpful "import documents first" message should pass through safety.
    NoContext,
    OutOfBounds,
}

/// A chunk of streaming output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    pub text: String,
    pub is_final: bool,
    pub partial_citations: Vec<Citation>,
}

/// A chunk with its relevance score (from vector search)
#[derive(Debug, Clone)]
pub struct ScoredChunk {
    pub chunk_id: String,
    pub document_id: Uuid,
    pub content: String,
    pub score: f32,
    pub doc_type: String,
    pub doc_date: Option<String>,
    pub professional_name: Option<String>,
}

/// Structured data retrieved from SQLite
#[derive(Debug, Clone, Default)]
pub struct StructuredContext {
    pub medications: Vec<Medication>,
    pub lab_results: Vec<LabResult>,
    pub diagnoses: Vec<Diagnosis>,
    pub allergies: Vec<Allergy>,
    pub symptoms: Vec<Symptom>,
    pub vital_signs: Vec<VitalSign>,
    pub recent_conversations: Vec<Message>,
    /// ME-06/G5: Screening and vaccination records for RAG context.
    pub screening_records: Vec<crate::db::repository::ScreeningRecord>,
    /// B2-G6: Entity connections (semantic graph edges between entities).
    pub entity_connections: Vec<crate::models::entity_connection::EntityConnection>,
}

/// Retrieved context from both data layers
#[derive(Debug, Clone)]
pub struct RetrievedContext {
    pub semantic_chunks: Vec<ScoredChunk>,
    pub structured_data: StructuredContext,
    pub dismissed_alerts: Vec<Uuid>,
}

/// Retrieval parameters per query type
#[derive(Debug, Clone)]
pub struct RetrievalParams {
    pub semantic_top_k: usize,
    pub include_medications: bool,
    pub include_labs: bool,
    pub include_diagnoses: bool,
    pub include_allergies: bool,
    pub include_symptoms: bool,
    pub include_vital_signs: bool,
    pub include_conversations: bool,
    /// ME-06/G5: Include screening and vaccination records in RAG context.
    pub include_screening_records: bool,
    /// B2-G6: Include entity connections (semantic graph edges) in RAG context.
    pub include_entity_connections: bool,
    pub temporal_weight: f32,
}

/// Assembled context ready for prompt
#[derive(Debug, Clone)]
pub struct AssembledContext {
    pub text: String,
    pub estimated_tokens: usize,
    pub chunks_included: Vec<ScoredChunk>,
}

/// Vector store search trait (extends storage VectorStore for RAG queries)
pub trait VectorSearch {
    fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> Result<Vec<ScoredChunk>, RagError>;
}

/// Main RAG pipeline trait
pub trait RagPipeline {
    fn query(
        &self,
        query: &PatientQuery,
        session: &ProfileSession,
    ) -> Result<RagResponse, RagError>;
}
