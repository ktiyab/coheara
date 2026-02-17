use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::params;
use uuid::Uuid;

use super::StorageError;
use super::types::{TextChunk, VectorStore};
use crate::crypto::{EncryptedData, ProfileSession};
use crate::pipeline::rag::RagError;
use crate::pipeline::rag::types::{ScoredChunk, VectorSearch};

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

// ═══════════════════════════════════════════════════════════
// SQLite-backed Vector Store (IMP-004)
// ═══════════════════════════════════════════════════════════

/// Helper struct for SQLite row deserialization.
struct ChunkRow {
    id: String,
    document_id: String,
    content_blob: Vec<u8>,
    is_encrypted: bool,
    embedding_blob: Vec<u8>,
    doc_type: String,
    doc_date: Option<String>,
    professional_name: Option<String>,
}

/// Persistent vector store using SQLite for chunk + embedding storage.
/// Implements both `VectorStore` (for the storage pipeline) and
/// `VectorSearch` (for the RAG retrieval pipeline).
///
/// Embeddings are stored as little-endian f32 byte blobs.
/// Search uses brute-force cosine similarity (sufficient for
/// medical document scale: ~1000s of chunks per profile).
pub struct SqliteVectorStore {
    db_path: PathBuf,
    db_key: Option<[u8; 32]>,
}

impl SqliteVectorStore {
    pub fn new(db_path: PathBuf, db_key: Option<[u8; 32]>) -> Self {
        Self { db_path, db_key }
    }

    fn open_conn(&self) -> Result<rusqlite::Connection, StorageError> {
        crate::db::open_database(&self.db_path, self.db_key.as_ref()).map_err(|e| StorageError::VectorDb(e.to_string()))
    }
}

impl VectorStore for SqliteVectorStore {
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

        let conn = self.open_conn()?;
        let mut stmt = conn
            .prepare(
                "INSERT INTO vector_chunks (id, document_id, chunk_index, content, is_encrypted,
                 embedding, doc_type, doc_date, professional_name)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )
            .map_err(|e| StorageError::VectorDb(e.to_string()))?;

        let doc_id_str = document_id.to_string();
        let mut count = 0;

        for (chunk, embedding) in chunks.iter().zip(embeddings.iter()) {
            let chunk_id = Uuid::new_v4().to_string();

            let (content_blob, is_encrypted): (Vec<u8>, bool) = match session {
                Some(s) => {
                    let enc = s.encrypt(chunk.content.as_bytes())?;
                    (serde_json::to_vec(&enc).map_err(|e| {
                        StorageError::VectorDb(format!("Serialize encrypted: {e}"))
                    })?, true)
                }
                None => (chunk.content.as_bytes().to_vec(), false),
            };

            let embedding_blob = embedding_to_bytes(embedding);

            stmt.execute(params![
                chunk_id,
                doc_id_str,
                chunk.chunk_index,
                content_blob,
                is_encrypted as i32,
                embedding_blob,
                doc_type,
                doc_date,
                professional_name,
            ])
            .map_err(|e| StorageError::VectorDb(e.to_string()))?;

            count += 1;
        }

        tracing::debug!(count, document_id = %document_id, "Stored vector chunks");
        Ok(count)
    }

    fn delete_by_document(&self, document_id: &Uuid) -> Result<(), StorageError> {
        let conn = self.open_conn()?;
        conn.execute(
            "DELETE FROM vector_chunks WHERE document_id = ?1",
            params![document_id.to_string()],
        )
        .map_err(|e| StorageError::VectorDb(e.to_string()))?;
        Ok(())
    }
}

impl VectorSearch for SqliteVectorStore {
    fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> Result<Vec<ScoredChunk>, RagError> {
        let conn = self
            .open_conn()
            .map_err(|e| RagError::VectorSearch(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, document_id, content, is_encrypted, embedding,
                        doc_type, doc_date, professional_name
                 FROM vector_chunks",
            )
            .map_err(|e| RagError::VectorSearch(e.to_string()))?;

        let mut scored: Vec<(f32, ScoredChunk)> = Vec::new();

        let rows = stmt
            .query_map([], |row| {
                Ok(ChunkRow {
                    id: row.get(0)?,
                    document_id: row.get(1)?,
                    content_blob: row.get(2)?,
                    is_encrypted: row.get(3)?,
                    embedding_blob: row.get(4)?,
                    doc_type: row.get(5)?,
                    doc_date: row.get(6)?,
                    professional_name: row.get(7)?,
                })
            })
            .map_err(|e| RagError::VectorSearch(e.to_string()))?;

        for row in rows {
            let r: ChunkRow =
                row.map_err(|e: rusqlite::Error| RagError::VectorSearch(e.to_string()))?;

            let embedding = bytes_to_embedding(&r.embedding_blob);
            let score = cosine_similarity(query_embedding, &embedding);

            // For encrypted content, return placeholder — caller decrypts later
            let content = if r.is_encrypted {
                "[encrypted]".to_string()
            } else {
                String::from_utf8(r.content_blob).unwrap_or_default()
            };

            let document_id = Uuid::parse_str(&r.document_id).unwrap_or_default();

            scored.push((
                score,
                ScoredChunk {
                    chunk_id: r.id,
                    document_id,
                    content,
                    score,
                    doc_type: r.doc_type,
                    doc_date: r.doc_date,
                    professional_name: r.professional_name,
                },
            ));
        }

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        Ok(scored.into_iter().take(top_k).map(|(_, chunk)| chunk).collect())
    }
}

/// Convert f32 slice to little-endian byte blob for SQLite storage.
fn embedding_to_bytes(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Convert little-endian byte blob back to f32 vector.
fn bytes_to_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// Cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
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

    // ── SqliteVectorStore tests (IMP-004) ──────────────────────

    fn make_sqlite_store() -> (tempfile::TempDir, SqliteVectorStore) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        crate::db::sqlite::run_migrations(&conn).unwrap();
        drop(conn);
        (dir, SqliteVectorStore::new(db_path, None))
    }

    /// Insert a document row so FK constraints pass.
    fn insert_test_doc(store: &SqliteVectorStore, doc_id: &Uuid) {
        let conn = store.open_conn().unwrap();
        conn.execute(
            "INSERT INTO documents (id, type, title, ingestion_date, source_file, verified)
             VALUES (?1, 'prescription', 'Test', datetime('now'), '/tmp/test.pdf', 0)",
            params![doc_id.to_string()],
        )
        .unwrap();
    }

    #[test]
    fn sqlite_store_and_count() {
        let (_dir, store) = make_sqlite_store();
        let doc_id = Uuid::new_v4();
        insert_test_doc(&store, &doc_id);
        let chunks = make_chunks(3);
        let embeddings = make_embeddings(3, 384);

        let stored = store
            .store_chunks(&chunks, &embeddings, &doc_id, "prescription", None, None, None)
            .unwrap();

        assert_eq!(stored, 3);

        // Verify via direct SQL
        let conn = store.open_conn().unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM vector_chunks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn sqlite_delete_by_document() {
        let (_dir, store) = make_sqlite_store();
        let doc1 = Uuid::new_v4();
        let doc2 = Uuid::new_v4();
        insert_test_doc(&store, &doc1);
        insert_test_doc(&store, &doc2);

        store
            .store_chunks(&make_chunks(3), &make_embeddings(3, 384), &doc1, "a", None, None, None)
            .unwrap();
        store
            .store_chunks(&make_chunks(2), &make_embeddings(2, 384), &doc2, "b", None, None, None)
            .unwrap();

        store.delete_by_document(&doc1).unwrap();

        let conn = store.open_conn().unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM vector_chunks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn sqlite_search_returns_by_similarity() {
        let (_dir, store) = make_sqlite_store();
        let doc_id = Uuid::new_v4();
        insert_test_doc(&store, &doc_id);

        let chunks = vec![
            TextChunk { content: "Metformin 500mg".to_string(), chunk_index: 0, section_title: None, char_offset: 0 },
            TextChunk { content: "Blood pressure 120/80".to_string(), chunk_index: 1, section_title: None, char_offset: 100 },
        ];

        // Embedding 0: points toward [1, 0, 0, ...]
        let mut emb0 = vec![0.0f32; 384];
        emb0[0] = 1.0;
        // Embedding 1: points toward [0, 1, 0, ...]
        let mut emb1 = vec![0.0f32; 384];
        emb1[1] = 1.0;

        store
            .store_chunks(&chunks, &[emb0.clone(), emb1], &doc_id, "prescription", None, None, None)
            .unwrap();

        // Query toward emb0
        let results = store.search(&emb0, 2).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].content, "Metformin 500mg");
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn sqlite_search_top_k_limits_results() {
        let (_dir, store) = make_sqlite_store();
        let doc_id = Uuid::new_v4();
        insert_test_doc(&store, &doc_id);

        store
            .store_chunks(&make_chunks(10), &make_embeddings(10, 384), &doc_id, "lab_result", None, None, None)
            .unwrap();

        let query = vec![1.0f32; 384];
        let results = store.search(&query, 3).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn sqlite_search_empty_store_returns_empty() {
        let (_dir, store) = make_sqlite_store();
        let query = vec![1.0f32; 384];
        let results = store.search(&query, 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn embedding_roundtrip_preserves_values() {
        let original = vec![1.5f32, -0.25, 0.0, 3.14159, f32::MIN, f32::MAX];
        let bytes = embedding_to_bytes(&original);
        let restored = bytes_to_embedding(&bytes);
        assert_eq!(original, restored);
    }

    #[test]
    fn cosine_similarity_identical_vectors() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 0.001);
    }
}
