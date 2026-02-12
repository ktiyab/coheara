use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::RagError;
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
    pub confidence: f32,
    pub query_type: QueryType,
    pub context_used: ContextSummary,
    pub boundary_check: BoundaryCheck,
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
    pub recent_conversations: Vec<Message>,
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
    pub include_conversations: bool,
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
