use std::sync::Mutex;

use uuid::Uuid;

use super::StorageError;
use super::types::{TextChunk, VectorStore};

/// In-memory vector store for testing.
/// Stores chunks with their embeddings for later retrieval.
pub struct InMemoryVectorStore {
    entries: Mutex<Vec<StoredChunk>>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct StoredChunk {
    id: Uuid,
    document_id: Uuid,
    content: String,
    embedding: Vec<f32>,
    chunk_index: usize,
    doc_type: String,
    doc_date: Option<String>,
    professional_name: Option<String>,
}

impl InMemoryVectorStore {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }

    pub fn count(&self) -> usize {
        self.entries.lock().unwrap().len()
    }

    pub fn count_for_document(&self, document_id: &Uuid) -> usize {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.document_id == *document_id)
            .count()
    }
}

impl Default for InMemoryVectorStore {
    fn default() -> Self {
        Self::new()
    }
}

impl VectorStore for InMemoryVectorStore {
    fn store_chunks(
        &self,
        chunks: &[TextChunk],
        embeddings: &[Vec<f32>],
        document_id: &Uuid,
        doc_type: &str,
        doc_date: Option<&str>,
        professional_name: Option<&str>,
    ) -> Result<usize, StorageError> {
        if chunks.len() != embeddings.len() {
            return Err(StorageError::VectorDb(
                "Chunk count does not match embedding count".into(),
            ));
        }

        let mut entries = self.entries.lock().unwrap();
        let count = chunks.len();

        for (chunk, embedding) in chunks.iter().zip(embeddings.iter()) {
            entries.push(StoredChunk {
                id: Uuid::new_v4(),
                document_id: *document_id,
                content: chunk.content.clone(),
                embedding: embedding.clone(),
                chunk_index: chunk.chunk_index,
                doc_type: doc_type.to_string(),
                doc_date: doc_date.map(|s| s.to_string()),
                professional_name: professional_name.map(|s| s.to_string()),
            });
        }

        Ok(count)
    }

    fn delete_by_document(&self, document_id: &Uuid) -> Result<(), StorageError> {
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|e| e.document_id != *document_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chunks(n: usize) -> Vec<TextChunk> {
        (0..n)
            .map(|i| TextChunk {
                content: format!("Chunk {i} content"),
                chunk_index: i,
                section_title: Some(format!("Section {i}")),
                char_offset: i * 100,
            })
            .collect()
    }

    fn make_embeddings(n: usize, dim: usize) -> Vec<Vec<f32>> {
        (0..n).map(|i| vec![i as f32 / n as f32; dim]).collect()
    }

    #[test]
    fn store_and_count() {
        let store = InMemoryVectorStore::new();
        let doc_id = Uuid::new_v4();
        let chunks = make_chunks(5);
        let embeddings = make_embeddings(5, 384);

        let stored = store
            .store_chunks(&chunks, &embeddings, &doc_id, "prescription", None, None)
            .unwrap();

        assert_eq!(stored, 5);
        assert_eq!(store.count(), 5);
        assert_eq!(store.count_for_document(&doc_id), 5);
    }

    #[test]
    fn store_multiple_documents() {
        let store = InMemoryVectorStore::new();
        let doc1 = Uuid::new_v4();
        let doc2 = Uuid::new_v4();

        store
            .store_chunks(&make_chunks(3), &make_embeddings(3, 384), &doc1, "lab_result", None, None)
            .unwrap();
        store
            .store_chunks(&make_chunks(2), &make_embeddings(2, 384), &doc2, "prescription", None, None)
            .unwrap();

        assert_eq!(store.count(), 5);
        assert_eq!(store.count_for_document(&doc1), 3);
        assert_eq!(store.count_for_document(&doc2), 2);
    }

    #[test]
    fn delete_by_document_removes_only_matching() {
        let store = InMemoryVectorStore::new();
        let doc1 = Uuid::new_v4();
        let doc2 = Uuid::new_v4();

        store
            .store_chunks(&make_chunks(3), &make_embeddings(3, 384), &doc1, "a", None, None)
            .unwrap();
        store
            .store_chunks(&make_chunks(2), &make_embeddings(2, 384), &doc2, "b", None, None)
            .unwrap();

        store.delete_by_document(&doc1).unwrap();

        assert_eq!(store.count(), 2);
        assert_eq!(store.count_for_document(&doc1), 0);
        assert_eq!(store.count_for_document(&doc2), 2);
    }

    #[test]
    fn mismatched_chunks_and_embeddings_errors() {
        let store = InMemoryVectorStore::new();
        let doc_id = Uuid::new_v4();

        let result = store.store_chunks(
            &make_chunks(3),
            &make_embeddings(2, 384),
            &doc_id,
            "a",
            None,
            None,
        );

        assert!(result.is_err());
    }

    #[test]
    fn empty_store_returns_zero() {
        let store = InMemoryVectorStore::new();
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn stores_metadata() {
        let store = InMemoryVectorStore::new();
        let doc_id = Uuid::new_v4();

        let stored = store
            .store_chunks(
                &make_chunks(1),
                &make_embeddings(1, 384),
                &doc_id,
                "prescription",
                Some("2024-01-15"),
                Some("Dr. Chen"),
            )
            .unwrap();

        assert_eq!(stored, 1);
    }
}
