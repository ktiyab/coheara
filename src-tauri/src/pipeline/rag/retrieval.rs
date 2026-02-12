use rusqlite::Connection;

use super::RagError;
use super::classify::extract_medical_keywords;
use super::types::{RetrievalParams, RetrievedContext, ScoredChunk, StructuredContext, VectorSearch};
use crate::db::repository;
use crate::pipeline::storage::types::EmbeddingModel;

/// Run semantic search using a vector store.
pub fn semantic_search(
    query_text: &str,
    embedder: &dyn EmbeddingModel,
    vector_store: &dyn VectorSearch,
    top_k: usize,
) -> Result<Vec<ScoredChunk>, RagError> {
    let query_embedding = embedder
        .embed(query_text)
        .map_err(|e| RagError::EmbeddingFailed(e.to_string()))?;

    vector_store.search(&query_embedding, top_k)
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

/// Run both semantic and structured retrieval.
pub fn retrieve(
    query_text: &str,
    embedder: &dyn EmbeddingModel,
    vector_store: &dyn VectorSearch,
    params: &RetrievalParams,
    conn: &Connection,
) -> Result<RetrievedContext, RagError> {
    let semantic_chunks = semantic_search(query_text, embedder, vector_store, params.semantic_top_k)?;
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
}
