use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::StorageError;
use crate::crypto::ProfileSession;
use crate::models::enums::DocumentType;
use crate::pipeline::structuring::types::StructuringResult;

/// Result of the full storage pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageResult {
    pub document_id: Uuid,
    pub chunks_stored: usize,
    pub entities_stored: EntitiesStoredCount,
    pub document_type: DocumentType,
    pub professional_id: Option<Uuid>,
    pub warnings: Vec<StorageWarning>,
}

/// Count of entities stored per type
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EntitiesStoredCount {
    pub medications: usize,
    pub lab_results: usize,
    pub diagnoses: usize,
    pub allergies: usize,
    pub procedures: usize,
    pub referrals: usize,
    pub instructions: usize,
}

/// Warnings from storage process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageWarning {
    DuplicateMedication { name: String, existing_id: Uuid },
    ProfessionalNameAmbiguous { name: String },
    DateParsingFailed { field: String, value: String },
    EmbeddingFailed { chunk_index: usize },
}

/// A semantic chunk of a Markdown document
#[derive(Debug, Clone)]
pub struct TextChunk {
    pub content: String,
    pub chunk_index: usize,
    pub section_title: Option<String>,
    pub char_offset: usize,
}

/// Chunking strategy trait
pub trait Chunker {
    fn chunk(&self, markdown: &str) -> Vec<TextChunk>;
}

/// Embedding model abstraction
pub trait EmbeddingModel {
    fn embed(&self, text: &str) -> Result<Vec<f32>, StorageError>;
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, StorageError>;
    fn dimension(&self) -> usize;
}

/// Allow `Box<dyn EmbeddingModel>` to be used as `&impl EmbeddingModel`.
impl EmbeddingModel for Box<dyn EmbeddingModel> {
    fn embed(&self, text: &str) -> Result<Vec<f32>, StorageError> {
        (**self).embed(text)
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, StorageError> {
        (**self).embed_batch(texts)
    }

    fn dimension(&self) -> usize {
        (**self).dimension()
    }
}

/// Vector store abstraction
#[allow(clippy::too_many_arguments)]
pub trait VectorStore {
    fn store_chunks(
        &self,
        chunks: &[TextChunk],
        embeddings: &[Vec<f32>],
        document_id: &Uuid,
        doc_type: &str,
        doc_date: Option<&str>,
        professional_name: Option<&str>,
        session: Option<&ProfileSession>,
    ) -> Result<usize, StorageError>;

    fn delete_by_document(&self, document_id: &Uuid) -> Result<(), StorageError>;
}

/// Main pipeline orchestrator trait
pub trait StoragePipeline {
    fn store(
        &self,
        structuring_result: &StructuringResult,
        session: &ProfileSession,
    ) -> Result<StorageResult, StorageError>;
}
