use std::sync::Mutex;

use uuid::Uuid;

use super::StorageError;
use super::types::{TextChunk, VectorStore};
use crate::crypto::{EncryptedData, ProfileSession};

/// In-memory vector store for testing.
/// Stores chunks with their embeddings for later retrieval.
/// Supports optional at-rest encryption of chunk content.
pub struct InMemoryVectorStore {
    entries: Mutex<Vec<StoredChunk>>,
}

/// Content stored as either plaintext or encrypted, depending on whether
/// a `ProfileSession` was provided at storage time.
#[derive(Debug, Clone)]
enum ChunkContent {
    Plain(String),
    Encrypted(EncryptedData),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct StoredChunk {
    id: Uuid,
    document_id: Uuid,
    content: ChunkContent,
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

impl InMemoryVectorStore {
    /// Decrypt and retrieve chunk content for a given document.
    /// Used to validate the encryption round-trip pattern in tests.
    pub fn get_decrypted_content(
        &self,
        document_id: &Uuid,
        session: &ProfileSession,
    ) -> Result<Vec<String>, StorageError> {
        let entries = self.entries.lock().unwrap();
        let mut result = Vec::new();

        for entry in entries.iter().filter(|e| e.document_id == *document_id) {
            let text = match &entry.content {
                ChunkContent::Plain(s) => s.clone(),
                ChunkContent::Encrypted(enc) => {
                    let bytes = session.decrypt(enc)?;
                    String::from_utf8(bytes).map_err(|e| {
                        StorageError::VectorDb(format!("UTF-8 decode failed: {e}"))
                    })?
                }
            };
            result.push(text);
        }

        Ok(result)
    }

    /// Check whether stored chunks are encrypted (for test assertions).
    pub fn is_encrypted(&self, document_id: &Uuid) -> bool {
        let entries = self.entries.lock().unwrap();
        entries
            .iter()
            .filter(|e| e.document_id == *document_id)
            .all(|e| matches!(e.content, ChunkContent::Encrypted(_)))
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
        session: Option<&ProfileSession>,
    ) -> Result<usize, StorageError> {
        if chunks.len() != embeddings.len() {
            return Err(StorageError::VectorDb(
                "Chunk count does not match embedding count".into(),
            ));
        }

        let mut entries = self.entries.lock().unwrap();
        let count = chunks.len();

        for (chunk, embedding) in chunks.iter().zip(embeddings.iter()) {
            let content = match session {
                Some(s) => ChunkContent::Encrypted(s.encrypt(chunk.content.as_bytes())?),
                None => ChunkContent::Plain(chunk.content.clone()),
            };

            entries.push(StoredChunk {
                id: Uuid::new_v4(),
                document_id: *document_id,
                content,
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
            .store_chunks(&chunks, &embeddings, &doc_id, "prescription", None, None, None)
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
            .store_chunks(&make_chunks(3), &make_embeddings(3, 384), &doc1, "lab_result", None, None, None)
            .unwrap();
        store
            .store_chunks(&make_chunks(2), &make_embeddings(2, 384), &doc2, "prescription", None, None, None)
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
            .store_chunks(&make_chunks(3), &make_embeddings(3, 384), &doc1, "a", None, None, None)
            .unwrap();
        store
            .store_chunks(&make_chunks(2), &make_embeddings(2, 384), &doc2, "b", None, None, None)
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
                None,
            )
            .unwrap();

        assert_eq!(stored, 1);
    }

    // ── Encryption tests (RS-L1-04-003) ──────────────────────

    fn make_test_session() -> (tempfile::TempDir, ProfileSession) {
        use crate::crypto::profile;

        let dir = tempfile::tempdir().unwrap();
        let (info, _phrase) =
            profile::create_profile(dir.path(), "VecStoreTest", "test_pass_123", None).unwrap();
        let session = profile::open_profile(dir.path(), &info.id, "test_pass_123").unwrap();
        (dir, session)
    }

    #[test]
    fn encrypted_store_round_trip() {
        let store = InMemoryVectorStore::new();
        let doc_id = Uuid::new_v4();
        let (_dir, session) = make_test_session();
        let chunks = make_chunks(3);

        store
            .store_chunks(
                &chunks,
                &make_embeddings(3, 384),
                &doc_id,
                "prescription",
                None,
                None,
                Some(&session),
            )
            .unwrap();

        // Content should be encrypted
        assert!(store.is_encrypted(&doc_id));

        // Decrypt and verify round-trip
        let decrypted = store.get_decrypted_content(&doc_id, &session).unwrap();
        assert_eq!(decrypted.len(), 3);
        assert_eq!(decrypted[0], "Chunk 0 content");
        assert_eq!(decrypted[1], "Chunk 1 content");
        assert_eq!(decrypted[2], "Chunk 2 content");
    }

    #[test]
    fn encrypted_content_differs_from_plaintext() {
        let store = InMemoryVectorStore::new();
        let doc_id = Uuid::new_v4();
        let (_dir, session) = make_test_session();

        store
            .store_chunks(
                &make_chunks(1),
                &make_embeddings(1, 384),
                &doc_id,
                "lab_result",
                None,
                None,
                Some(&session),
            )
            .unwrap();

        // Stored content is encrypted (not plaintext)
        assert!(store.is_encrypted(&doc_id));
    }

    #[test]
    fn decrypt_with_wrong_key_fails() {
        let store = InMemoryVectorStore::new();
        let doc_id = Uuid::new_v4();
        let (_dir1, session1) = make_test_session();
        let (_dir2, session2) = make_test_session();

        store
            .store_chunks(
                &make_chunks(1),
                &make_embeddings(1, 384),
                &doc_id,
                "prescription",
                None,
                None,
                Some(&session1),
            )
            .unwrap();

        // Decrypting with a different session key should fail
        let result = store.get_decrypted_content(&doc_id, &session2);
        assert!(result.is_err());
    }

    #[test]
    fn unencrypted_store_is_not_encrypted() {
        let store = InMemoryVectorStore::new();
        let doc_id = Uuid::new_v4();

        store
            .store_chunks(
                &make_chunks(2),
                &make_embeddings(2, 384),
                &doc_id,
                "prescription",
                None,
                None,
                None,
            )
            .unwrap();

        assert!(!store.is_encrypted(&doc_id));
    }
}
