use std::collections::HashSet;

use rusqlite::Connection;

use super::RagError;
use super::classify::extract_medical_keywords;
use super::types::{RetrievalParams, RetrievedContext, ScoredChunk, StructuredContext, VectorSearch};
use crate::db::repository;
use crate::pipeline::storage::types::EmbeddingModel;

/// Minimum cosine similarity threshold for semantic search results (M.8).
/// Chunks scoring below this are noise and should be filtered out.
const MIN_RELEVANCE_SCORE: f32 = 0.3;

/// Run semantic search using a vector store.
/// Filters out results below MIN_RELEVANCE_SCORE (M.8).
pub fn semantic_search(
    query_text: &str,
    embedder: &dyn EmbeddingModel,
    vector_store: &dyn VectorSearch,
    top_k: usize,
) -> Result<Vec<ScoredChunk>, RagError> {
    let query_embedding = embedder
        .embed(query_text)
        .map_err(|e| RagError::EmbeddingFailed(e.to_string()))?;

    let results = vector_store.search(&query_embedding, top_k)?;
    let total = results.len();

    // M.8: Filter out low-relevance chunks
    let filtered: Vec<ScoredChunk> = results
        .into_iter()
        .filter(|chunk| chunk.score >= MIN_RELEVANCE_SCORE)
        .collect();

    if filtered.len() < total {
        tracing::debug!(
            before = total,
            after = filtered.len(),
            threshold = MIN_RELEVANCE_SCORE,
            "Filtered low-relevance semantic chunks"
        );
    }

    Ok(filtered)
}

/// Retrieve structured data from SQLite based on query and retrieval params.
pub fn structured_search(
    query_text: &str,
    params: &RetrievalParams,
    conn: &Connection,
) -> Result<StructuredContext, RagError> {
    let mut ctx = StructuredContext::default();
    let keywords = extract_medical_keywords(query_text);

    if params.include_medications {
        ctx.medications = repository::get_active_medications(conn)?;

        for keyword in &keywords {
            let matches = repository::get_medications_by_name(conn, keyword)?;
            for med in matches {
                if !ctx.medications.iter().any(|m| m.id == med.id) {
                    ctx.medications.push(med);
                }
            }
        }
    }

    if params.include_labs {
        let six_months_ago =
            chrono::Local::now().date_naive() - chrono::Duration::days(180);
        ctx.lab_results = repository::get_lab_results_since(conn, &six_months_ago)?;

        for keyword in &keywords {
            let trending = repository::get_lab_results_by_test_name(conn, keyword)?;
            for lab in trending {
                if !ctx.lab_results.iter().any(|l| l.id == lab.id) {
                    ctx.lab_results.push(lab);
                }
            }
        }
    }

    if params.include_diagnoses {
        ctx.diagnoses = repository::get_active_diagnoses(conn)?;
    }

    if params.include_allergies {
        ctx.allergies = repository::get_all_allergies(conn)?;
    }

    if params.include_symptoms {
        let thirty_days_ago =
            chrono::Local::now().date_naive() - chrono::Duration::days(30);
        let today = chrono::Local::now().date_naive();
        ctx.symptoms = repository::get_symptoms_in_date_range(conn, &thirty_days_ago, &today)?;
    }

    Ok(ctx)
}

/// Apply temporal reranking to semantic chunks (M.6).
/// Boosts scores of recent documents based on temporal_weight.
/// Formula: final_score = (1 - temporal_weight) * score + temporal_weight * recency_factor
/// where recency_factor decays from 1.0 (today) to 0.0 (365+ days old).
fn apply_temporal_reranking(chunks: &mut [ScoredChunk], temporal_weight: f32) {
    if temporal_weight <= 0.0 {
        return;
    }
    let today = chrono::Local::now().date_naive();

    for chunk in chunks.iter_mut() {
        let recency = chunk
            .doc_date
            .as_deref()
            .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
            .map(|date| {
                let days_old = (today - date).num_days().max(0) as f32;
                // Linear decay over 365 days: 1.0 (today) → 0.0 (365+ days)
                (1.0 - days_old / 365.0).max(0.0)
            })
            .unwrap_or(0.5); // No date → neutral recency

        chunk.score = (1.0 - temporal_weight) * chunk.score + temporal_weight * recency;
    }

    // Re-sort by updated score (descending)
    chunks.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
}

/// M.4: Remove duplicate chunks by chunk_id, keeping the first (highest-scored) occurrence.
/// Chunks are expected to already be sorted by score descending.
fn deduplicate_chunks(chunks: Vec<ScoredChunk>) -> Vec<ScoredChunk> {
    let mut seen = HashSet::new();
    let before = chunks.len();
    let deduped: Vec<ScoredChunk> = chunks
        .into_iter()
        .filter(|chunk| seen.insert(chunk.chunk_id.clone()))
        .collect();

    if deduped.len() < before {
        tracing::debug!(
            before,
            after = deduped.len(),
            "Removed duplicate semantic chunks"
        );
    }

    deduped
}

/// Run both semantic and structured retrieval.
pub fn retrieve(
    query_text: &str,
    embedder: &dyn EmbeddingModel,
    vector_store: &dyn VectorSearch,
    params: &RetrievalParams,
    conn: &Connection,
) -> Result<RetrievedContext, RagError> {
    let mut semantic_chunks = semantic_search(query_text, embedder, vector_store, params.semantic_top_k)?;

    // M.6: Apply temporal reranking
    apply_temporal_reranking(&mut semantic_chunks, params.temporal_weight);

    // M.4: Deduplicate chunks (keep highest-scored occurrence)
    let semantic_chunks = deduplicate_chunks(semantic_chunks);

    let structured_data = structured_search(query_text, params, conn)?;

    let dismissed_alerts = repository::get_dismissed_alerts(conn)?
        .into_iter()
        .map(|a| a.id)
        .collect();

    Ok(RetrievedContext {
        semantic_chunks,
        structured_data,
        dismissed_alerts,
    })
}

/// In-memory vector search for testing — uses cosine similarity.
pub struct InMemoryVectorSearch {
    entries: Vec<StoredEntry>,
}

struct StoredEntry {
    chunk_id: String,
    document_id: uuid::Uuid,
    content: String,
    embedding: Vec<f32>,
    doc_type: String,
    doc_date: Option<String>,
    professional_name: Option<String>,
}

impl InMemoryVectorSearch {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add(
        &mut self,
        chunk_id: &str,
        document_id: uuid::Uuid,
        content: &str,
        embedding: Vec<f32>,
        doc_type: &str,
        doc_date: Option<&str>,
        professional_name: Option<&str>,
    ) {
        self.entries.push(StoredEntry {
            chunk_id: chunk_id.to_string(),
            document_id,
            content: content.to_string(),
            embedding,
            doc_type: doc_type.to_string(),
            doc_date: doc_date.map(|s| s.to_string()),
            professional_name: professional_name.map(|s| s.to_string()),
        });
    }
}

impl Default for InMemoryVectorSearch {
    fn default() -> Self {
        Self::new()
    }
}

impl VectorSearch for InMemoryVectorSearch {
    fn search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> Result<Vec<ScoredChunk>, RagError> {
        let mut scored: Vec<(f32, &StoredEntry)> = self
            .entries
            .iter()
            .map(|entry| {
                let score = cosine_similarity(query_embedding, &entry.embedding);
                (score, entry)
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        Ok(scored
            .into_iter()
            .take(top_k)
            .map(|(score, entry)| ScoredChunk {
                chunk_id: entry.chunk_id.clone(),
                document_id: entry.document_id,
                content: entry.content.clone(),
                score,
                doc_type: entry.doc_type.clone(),
                doc_date: entry.doc_date.clone(),
                professional_name: entry.professional_name.clone(),
            })
            .collect())
    }
}

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
    use crate::db::sqlite::open_memory_database;
    use crate::db::repository;
    use crate::models::*;
    use crate::models::enums::*;

    fn test_db_with_data() -> Connection {
        let conn = open_memory_database().unwrap();
        let doc_id = uuid::Uuid::new_v4();

        repository::insert_document(
            &conn,
            &Document {
                id: doc_id,
                doc_type: DocumentType::Prescription,
                title: "Test".into(),
                document_date: None,
                ingestion_date: chrono::Local::now().naive_local(),
                professional_id: None,
                source_file: "/test.jpg".into(),
                markdown_file: None,
                ocr_confidence: Some(0.9),
                verified: false,
                source_deleted: false,
                perceptual_hash: None,
                notes: None,
                pipeline_status: PipelineStatus::Imported,
            },
        )
        .unwrap();

        repository::insert_medication(
            &conn,
            &Medication {
                id: uuid::Uuid::new_v4(),
                generic_name: "Metformin".into(),
                brand_name: Some("Glucophage".into()),
                dose: "500mg".into(),
                frequency: "twice daily".into(),
                frequency_type: FrequencyType::Scheduled,
                route: "oral".into(),
                prescriber_id: None,
                start_date: None,
                end_date: None,
                reason_start: None,
                reason_stop: None,
                is_otc: false,
                status: MedicationStatus::Active,
                administration_instructions: None,
                max_daily_dose: None,
                condition: None,
                dose_type: DoseType::Fixed,
                is_compound: false,
                document_id: doc_id,
            },
        )
        .unwrap();

        repository::insert_allergy(
            &conn,
            &Allergy {
                id: uuid::Uuid::new_v4(),
                allergen: "Penicillin".into(),
                reaction: Some("Rash".into()),
                severity: AllergySeverity::Severe,
                date_identified: None,
                source: AllergySource::DocumentExtracted,
                document_id: Some(doc_id),
                verified: true,
            },
        )
        .unwrap();

        conn
    }

    #[test]
    fn cosine_similarity_identical_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 0.01);
    }

    #[test]
    fn cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 0.01);
    }

    #[test]
    fn in_memory_search_returns_top_k() {
        let mut store = InMemoryVectorSearch::new();
        let doc_id = uuid::Uuid::new_v4();

        store.add("c1", doc_id, "Metformin 500mg", vec![1.0, 0.0, 0.0], "prescription", None, None);
        store.add("c2", doc_id, "HbA1c 7.2%", vec![0.8, 0.6, 0.0], "lab_result", None, None);
        store.add("c3", doc_id, "Blood pressure", vec![0.0, 1.0, 0.0], "clinical_note", None, None);

        let results = store.search(&[1.0, 0.0, 0.0], 2).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].chunk_id, "c1"); // Most similar
    }

    #[test]
    fn structured_search_includes_medications_and_allergies() {
        let conn = test_db_with_data();
        let params = super::super::classify::retrieval_strategy(&super::super::types::QueryType::Factual);

        let ctx = structured_search("What is my medication?", &params, &conn).unwrap();
        assert_eq!(ctx.medications.len(), 1);
        assert_eq!(ctx.medications[0].generic_name, "Metformin");
        assert_eq!(ctx.allergies.len(), 1);
    }

    #[test]
    fn structured_search_respects_params() {
        let conn = test_db_with_data();
        let params = super::super::classify::retrieval_strategy(&super::super::types::QueryType::Symptom);

        let ctx = structured_search("I have pain", &params, &conn).unwrap();
        // Symptom query doesn't include labs or allergies
        assert!(ctx.lab_results.is_empty());
        assert!(ctx.allergies.is_empty());
        // But includes medications
        assert_eq!(ctx.medications.len(), 1);
    }

    // ── M.4: Chunk deduplication tests ────────────────────────────

    fn make_chunk(id: &str, score: f32) -> ScoredChunk {
        ScoredChunk {
            chunk_id: id.to_string(),
            document_id: uuid::Uuid::new_v4(),
            content: format!("Content for {id}"),
            score,
            doc_type: "note".to_string(),
            doc_date: None,
            professional_name: None,
        }
    }

    #[test]
    fn deduplicate_removes_duplicate_chunk_ids() {
        let chunks = vec![
            make_chunk("c1", 0.9),
            make_chunk("c2", 0.8),
            make_chunk("c1", 0.7), // duplicate
            make_chunk("c3", 0.6),
        ];

        let deduped = deduplicate_chunks(chunks);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0].chunk_id, "c1");
        assert!((deduped[0].score - 0.9).abs() < 0.01); // kept first (highest)
    }

    #[test]
    fn deduplicate_no_duplicates_unchanged() {
        let chunks = vec![
            make_chunk("c1", 0.9),
            make_chunk("c2", 0.8),
            make_chunk("c3", 0.7),
        ];

        let deduped = deduplicate_chunks(chunks);
        assert_eq!(deduped.len(), 3);
    }

    #[test]
    fn deduplicate_empty_input() {
        let deduped = deduplicate_chunks(vec![]);
        assert!(deduped.is_empty());
    }

    // ── M.6: Temporal reranking tests ─────────────────────────────

    #[test]
    fn temporal_reranking_boosts_recent_documents() {
        let today = chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();
        let old_date = (chrono::Local::now().date_naive() - chrono::Duration::days(300))
            .format("%Y-%m-%d")
            .to_string();

        let mut chunks = vec![
            ScoredChunk {
                chunk_id: "old".to_string(),
                document_id: uuid::Uuid::new_v4(),
                content: "Old content".to_string(),
                score: 0.8,
                doc_type: "note".to_string(),
                doc_date: Some(old_date),
                professional_name: None,
            },
            ScoredChunk {
                chunk_id: "recent".to_string(),
                document_id: uuid::Uuid::new_v4(),
                content: "Recent content".to_string(),
                score: 0.7,
                doc_type: "note".to_string(),
                doc_date: Some(today),
                professional_name: None,
            },
        ];

        apply_temporal_reranking(&mut chunks, 0.5);
        // Recent doc should be boosted above old doc despite lower initial score
        assert_eq!(chunks[0].chunk_id, "recent");
    }

    #[test]
    fn temporal_reranking_zero_weight_is_noop() {
        let mut chunks = vec![
            make_chunk("c1", 0.9),
            make_chunk("c2", 0.5),
        ];
        let scores_before: Vec<f32> = chunks.iter().map(|c| c.score).collect();

        apply_temporal_reranking(&mut chunks, 0.0);
        let scores_after: Vec<f32> = chunks.iter().map(|c| c.score).collect();
        assert_eq!(scores_before, scores_after);
    }

    #[test]
    fn temporal_reranking_no_date_gets_neutral_recency() {
        let mut chunks = vec![make_chunk("c1", 0.8)];
        apply_temporal_reranking(&mut chunks, 0.4);
        // score = (1-0.4)*0.8 + 0.4*0.5 = 0.48 + 0.20 = 0.68
        assert!((chunks[0].score - 0.68).abs() < 0.01);
    }

    // ── M.8: Relevance score filtering tests ──────────────────────

    #[test]
    fn semantic_search_filters_low_relevance() {
        let mut store = InMemoryVectorSearch::new();
        let doc_id = uuid::Uuid::new_v4();

        // High similarity (will pass threshold)
        store.add("c1", doc_id, "Metformin 500mg", vec![1.0, 0.0, 0.0], "prescription", None, None);
        // Low similarity (will be filtered: cos([1,0,0], [0,0,1]) = 0.0)
        store.add("c2", doc_id, "Unrelated", vec![0.0, 0.0, 1.0], "note", None, None);

        let results = semantic_search("test", &MockEmbedder, &store, 10).unwrap();
        // Only the high-relevance chunk should remain
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].chunk_id, "c1");
    }

    /// Mock embedder for semantic_search tests
    struct MockEmbedder;

    impl crate::pipeline::storage::types::EmbeddingModel for MockEmbedder {
        fn embed(&self, _text: &str) -> Result<Vec<f32>, crate::pipeline::storage::StorageError> {
            Ok(vec![1.0, 0.0, 0.0])
        }
        fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, crate::pipeline::storage::StorageError> {
            Ok(texts.iter().map(|_| vec![1.0, 0.0, 0.0]).collect())
        }
        fn dimension(&self) -> usize {
            3
        }
    }

    // ── M.9: End-to-end retrieval test suite ──────────────────────

    /// Build a test database with a comprehensive medical profile:
    /// medications, allergies, diagnoses, labs.
    fn test_db_with_full_profile() -> (Connection, uuid::Uuid) {
        let conn = open_memory_database().unwrap();
        let doc_id = uuid::Uuid::new_v4();

        repository::insert_document(
            &conn,
            &Document {
                id: doc_id,
                doc_type: DocumentType::Prescription,
                title: "Full Profile Test".into(),
                document_date: None,
                ingestion_date: chrono::Local::now().naive_local(),
                professional_id: None,
                source_file: "/test.jpg".into(),
                markdown_file: None,
                ocr_confidence: Some(0.9),
                verified: false,
                source_deleted: false,
                perceptual_hash: None,
                notes: None,
                pipeline_status: PipelineStatus::Imported,
            },
        )
        .unwrap();

        // Medications
        repository::insert_medication(
            &conn,
            &Medication {
                id: uuid::Uuid::new_v4(),
                generic_name: "Metformin".into(),
                brand_name: Some("Glucophage".into()),
                dose: "500mg".into(),
                frequency: "twice daily".into(),
                frequency_type: FrequencyType::Scheduled,
                route: "oral".into(),
                prescriber_id: None,
                start_date: None,
                end_date: None,
                reason_start: None,
                reason_stop: None,
                is_otc: false,
                status: MedicationStatus::Active,
                administration_instructions: None,
                max_daily_dose: None,
                condition: None,
                dose_type: DoseType::Fixed,
                is_compound: false,
                document_id: doc_id,
            },
        )
        .unwrap();

        // Allergy
        repository::insert_allergy(
            &conn,
            &Allergy {
                id: uuid::Uuid::new_v4(),
                allergen: "Penicillin".into(),
                reaction: Some("Anaphylaxis".into()),
                severity: AllergySeverity::LifeThreatening,
                date_identified: None,
                source: AllergySource::DocumentExtracted,
                document_id: Some(doc_id),
                verified: true,
            },
        )
        .unwrap();

        // Diagnosis
        repository::insert_diagnosis(
            &conn,
            &Diagnosis {
                id: uuid::Uuid::new_v4(),
                name: "Type 2 Diabetes".into(),
                icd_code: Some("E11".into()),
                date_diagnosed: None,
                diagnosing_professional_id: None,
                status: DiagnosisStatus::Active,
                document_id: doc_id,
            },
        )
        .unwrap();

        (conn, doc_id)
    }

    /// Build a vector store with medical content chunks for testing.
    fn test_vector_store(doc_id: uuid::Uuid) -> InMemoryVectorSearch {
        let mut store = InMemoryVectorSearch::new();
        let today = chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();

        // Prescription chunk (high relevance to medication queries)
        store.add(
            "rx-1", doc_id,
            "Metformin 500mg twice daily for type 2 diabetes management",
            vec![1.0, 0.0, 0.0],
            "prescription", Some(&today), Some("Dr. Chen"),
        );

        // Lab result chunk
        store.add(
            "lab-1", doc_id,
            "HbA1c: 7.2% (target < 7.0%), fasting glucose 130 mg/dL",
            vec![0.8, 0.6, 0.0],
            "lab_result", Some(&today), Some("Dr. Chen"),
        );

        // Clinical note chunk
        store.add(
            "note-1", doc_id,
            "Patient reports improved blood sugar control since starting Metformin",
            vec![0.7, 0.5, 0.3],
            "clinical_note", Some(&today), None,
        );

        // Low relevance chunk (should be filtered by M.8)
        store.add(
            "noise-1", doc_id,
            "Administrative note: appointment rescheduled",
            vec![0.0, 0.0, 1.0],
            "other", None, None,
        );

        store
    }

    #[test]
    fn retrieve_returns_semantic_and_structured_data() {
        let (conn, doc_id) = test_db_with_full_profile();
        let store = test_vector_store(doc_id);
        let params = super::super::classify::retrieval_strategy(
            &super::super::types::QueryType::Factual,
        );

        let result = retrieve("What is my metformin dose?", &MockEmbedder, &store, &params, &conn).unwrap();

        // Should have semantic chunks (noise filtered by M.8)
        assert!(!result.semantic_chunks.is_empty());
        // Should have structured medications
        assert_eq!(result.structured_data.medications.len(), 1);
        assert_eq!(result.structured_data.medications[0].generic_name, "Metformin");
        // Should have structured allergies
        assert_eq!(result.structured_data.allergies.len(), 1);
        // Should have structured diagnoses
        assert_eq!(result.structured_data.diagnoses.len(), 1);
    }

    #[test]
    fn retrieve_filters_noise_chunks() {
        let (conn, doc_id) = test_db_with_full_profile();
        let store = test_vector_store(doc_id);
        let params = super::super::classify::retrieval_strategy(
            &super::super::types::QueryType::Factual,
        );

        let result = retrieve("metformin", &MockEmbedder, &store, &params, &conn).unwrap();

        // The noise chunk (cosine sim ~0.0 with query) should be filtered
        let chunk_ids: Vec<&str> = result.semantic_chunks.iter().map(|c| c.chunk_id.as_str()).collect();
        assert!(!chunk_ids.contains(&"noise-1"), "Noise chunk should be filtered");
    }

    #[test]
    fn retrieve_applies_temporal_reranking() {
        let (conn, doc_id) = test_db_with_full_profile();
        let mut store = InMemoryVectorSearch::new();
        let today = chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();
        let old = (chrono::Local::now().date_naive() - chrono::Duration::days(300))
            .format("%Y-%m-%d")
            .to_string();

        // Old chunk with slightly higher base score
        store.add(
            "old-chunk", doc_id,
            "Old prescription data",
            vec![0.95, 0.0, 0.0],
            "prescription", Some(&old), None,
        );
        // Recent chunk with slightly lower base score
        store.add(
            "new-chunk", doc_id,
            "Recent prescription data",
            vec![0.9, 0.0, 0.0],
            "prescription", Some(&today), None,
        );

        // Timeline query has temporal_weight = 1.0
        let params = super::super::classify::retrieval_strategy(
            &super::super::types::QueryType::Timeline,
        );

        let result = retrieve("timeline", &MockEmbedder, &store, &params, &conn).unwrap();

        // With high temporal weight, recent chunk should rank first
        assert!(result.semantic_chunks.len() >= 2);
        assert_eq!(result.semantic_chunks[0].chunk_id, "new-chunk");
    }

    #[test]
    fn retrieve_symptom_query_includes_medications_not_labs() {
        let (conn, doc_id) = test_db_with_full_profile();
        let store = test_vector_store(doc_id);
        let params = super::super::classify::retrieval_strategy(
            &super::super::types::QueryType::Symptom,
        );

        let result = retrieve("I feel dizzy", &MockEmbedder, &store, &params, &conn).unwrap();

        // Symptom strategy: include_medications=true, include_labs=false
        assert_eq!(result.structured_data.medications.len(), 1);
        assert!(result.structured_data.lab_results.is_empty());
        // Symptom strategy: include_allergies=false
        assert!(result.structured_data.allergies.is_empty());
    }

    #[test]
    fn retrieve_deduplicates_chunks() {
        let (conn, _doc_id) = test_db_with_full_profile();
        let mut store = InMemoryVectorSearch::new();
        let id = uuid::Uuid::new_v4();

        // Same chunk_id appearing twice (simulating overlap)
        store.add("dup-1", id, "Content A", vec![1.0, 0.0, 0.0], "note", None, None);
        store.add("dup-1", id, "Content A copy", vec![0.99, 0.0, 0.0], "note", None, None);
        store.add("unique-1", id, "Content B", vec![0.8, 0.5, 0.0], "note", None, None);

        let params = super::super::classify::retrieval_strategy(
            &super::super::types::QueryType::General,
        );
        let result = retrieve("test", &MockEmbedder, &store, &params, &conn).unwrap();

        // Should have deduplicated: 2 unique chunk_ids, not 3
        let ids: Vec<&str> = result.semantic_chunks.iter().map(|c| c.chunk_id.as_str()).collect();
        assert_eq!(ids.iter().filter(|&&id| id == "dup-1").count(), 1);
    }
}
