use rusqlite::Connection;

use super::citation::{
    calculate_confidence, clean_citations_for_display, extract_citations, parse_boundary_check,
    validate_citations,
};
use super::classify::{classify_query, retrieval_strategy};
use super::context::assemble_context;
use super::conversation::ConversationManager;
use super::prompt::{build_conversation_prompt, conversation_system_prompt_i18n, no_context_response_i18n};
use super::retrieval::retrieve;
use super::types::{
    AssembledContext, ContextSummary, PatientQuery, RagResponse, VectorSearch,
};
use super::RagError;
use crate::pipeline::safety::sanitize::sanitize_patient_input;
use crate::pipeline::storage::types::EmbeddingModel;

/// Trait for LLM text generation within the RAG pipeline.
pub trait LlmGenerate {
    fn generate(&self, system: &str, prompt: &str) -> Result<String, RagError>;
}

/// Full RAG pipeline orchestrator.
///
/// Coordinates: classify → retrieve → assemble → generate → cite → persist.
pub struct DocumentRagPipeline<'a, G: LlmGenerate, E: EmbeddingModel, V: VectorSearch> {
    generator: &'a G,
    embedder: &'a E,
    vector_store: &'a V,
    conn: &'a Connection,
    /// I18N-19: Language for system prompt and no-context response.
    lang: String,
}

impl<'a, G: LlmGenerate, E: EmbeddingModel, V: VectorSearch> DocumentRagPipeline<'a, G, E, V> {
    pub fn new(
        generator: &'a G,
        embedder: &'a E,
        vector_store: &'a V,
        conn: &'a Connection,
    ) -> Self {
        Self {
            generator,
            embedder,
            vector_store,
            conn,
            lang: "en".to_string(),
        }
    }

    /// I18N-19: Create a pipeline with a specific response language.
    pub fn with_language(
        generator: &'a G,
        embedder: &'a E,
        vector_store: &'a V,
        conn: &'a Connection,
        lang: &str,
    ) -> Self {
        Self {
            generator,
            embedder,
            vector_store,
            conn,
            lang: lang.to_string(),
        }
    }

    /// Execute the full RAG pipeline for a patient query.
    ///
    /// Generates a response AND persists both patient message and response
    /// to the database. Use `generate()` when the caller manages persistence
    /// separately (e.g., after safety filtering).
    pub fn query(&self, query: &PatientQuery) -> Result<RagResponse, RagError> {
        let response = self.generate(query)?;

        // Persist patient message + response
        let conversation_mgr = ConversationManager::new(self.conn);
        conversation_mgr.add_patient_message(query.conversation_id, &query.text)?;

        let chunk_ids: Vec<&str> = response
            .citations
            .iter()
            .map(|c| c.document_title.as_str())
            .collect();
        let source_chunks_json = serde_json::to_string(&chunk_ids).ok();

        conversation_mgr.add_response(
            query.conversation_id,
            &response.text,
            source_chunks_json.as_deref(),
            response.confidence,
        )?;

        Ok(response)
    }

    /// Generate a RAG response without persisting to the database.
    ///
    /// Use this when the caller manages persistence separately,
    /// e.g., after applying safety filtering to the response text.
    pub fn generate(&self, query: &PatientQuery) -> Result<RagResponse, RagError> {
        // Step 1: Classify query
        let query_type = query
            .query_type
            .clone()
            .unwrap_or_else(|| classify_query(&query.text));

        // Step 2: Determine retrieval strategy
        let params = retrieval_strategy(&query_type);

        // Step 3: Retrieve context (semantic + structured)
        let retrieved = retrieve(
            &query.text,
            self.embedder,
            self.vector_store,
            &params,
            self.conn,
        )?;

        // Step 4: Check if we have any context
        let has_semantic = !retrieved.semantic_chunks.is_empty();
        let has_structured = !retrieved.structured_data.medications.is_empty()
            || !retrieved.structured_data.diagnoses.is_empty()
            || !retrieved.structured_data.allergies.is_empty()
            || !retrieved.structured_data.lab_results.is_empty()
            || !retrieved.structured_data.symptoms.is_empty();

        if !has_semantic && !has_structured {
            return Ok(self.no_context_result(query_type));
        }

        // Step 5: Assemble context within token budget
        let assembled = assemble_context(&retrieved, &query_type);

        if assembled.text.is_empty() {
            return Ok(self.no_context_result(query_type));
        }

        // Step 6: Get conversation history
        let conversation_mgr = ConversationManager::new(self.conn);
        let history = conversation_mgr
            .get_history(query.conversation_id)
            .unwrap_or_default();

        // Step 6b: Sanitize query before prompt construction (SEC-01-G03)
        let sanitized_query = sanitize_patient_input(&query.text, 2000)
            .map(|s| s.text)
            .unwrap_or_else(|_| query.text.clone());

        // Step 7: Build prompt
        let prompt = build_conversation_prompt(&sanitized_query, &assembled, &history);

        // Step 8: Generate response via LLM (I18N-19: language-keyed system prompt)
        let system_prompt = conversation_system_prompt_i18n(&self.lang);
        let raw_response = self
            .generator
            .generate(&system_prompt, &prompt)?;

        // Step 9: Parse boundary check + clean response
        let (boundary_check, cleaned_response) = parse_boundary_check(&raw_response);

        // Step 10: Extract citations
        let raw_citations = extract_citations(&cleaned_response, &assembled.chunks_included);

        // Step 10b: Validate citation document_ids against DB (RS-L2-01-002)
        let citations = validate_citations(self.conn, raw_citations);

        // Step 11: Clean citation markers from display text
        let display_text = clean_citations_for_display(&cleaned_response);

        // Step 12: Calculate confidence
        let confidence = calculate_confidence(
            &boundary_check,
            citations.len(),
            assembled.chunks_included.len(),
        );

        // Step 12b: M.11 — Low confidence gate
        // If confidence is very low, prepend a disclaimer to help the patient
        let display_text = if confidence < 0.3 {
            tracing::info!(confidence, "Low RAG confidence — adding disclaimer");
            format!(
                "**Note:** This response is based on limited information from your documents. \
                Please verify with your healthcare provider.\n\n{display_text}"
            )
        } else {
            display_text
        };

        // Step 13: Build response
        let context_used = build_context_summary(&assembled, &retrieved.structured_data);

        Ok(RagResponse {
            text: display_text,
            citations,
            confidence,
            query_type,
            context_used,
            boundary_check,
        })
    }

    fn no_context_result(&self, query_type: super::types::QueryType) -> RagResponse {
        RagResponse {
            text: no_context_response_i18n(&self.lang),
            citations: vec![],
            confidence: 0.0,
            query_type,
            context_used: ContextSummary {
                semantic_chunks_used: 0,
                structured_records_used: 0,
                total_context_tokens: 0,
            },
            boundary_check: super::types::BoundaryCheck::OutOfBounds,
        }
    }

}

fn build_context_summary(
    assembled: &AssembledContext,
    structured: &super::types::StructuredContext,
) -> ContextSummary {
    let structured_count = structured.medications.len()
        + structured.diagnoses.len()
        + structured.allergies.len()
        + structured.lab_results.len()
        + structured.symptoms.len();

    ContextSummary {
        semantic_chunks_used: assembled.chunks_included.len(),
        structured_records_used: structured_count,
        total_context_tokens: assembled.estimated_tokens,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;
    use crate::pipeline::rag::retrieval::InMemoryVectorSearch;
    use crate::pipeline::rag::types::{BoundaryCheck, PatientQuery};
    use crate::pipeline::storage::StorageError;
    use uuid::Uuid;

    /// Mock LLM that returns a canned response with boundary check.
    struct MockLlm {
        response: String,
    }

    impl MockLlm {
        fn understanding(text: &str) -> Self {
            Self {
                response: format!("BOUNDARY_CHECK: understanding\n{text}"),
            }
        }

        fn out_of_bounds() -> Self {
            Self {
                response: "I cannot answer that based on your documents.".to_string(),
            }
        }
    }

    impl LlmGenerate for MockLlm {
        fn generate(&self, _system: &str, _prompt: &str) -> Result<String, RagError> {
            Ok(self.response.clone())
        }
    }

    /// Mock embedder that returns a fixed vector.
    struct MockEmbedder;

    impl EmbeddingModel for MockEmbedder {
        fn embed(&self, _text: &str) -> Result<Vec<f32>, StorageError> {
            Ok(vec![1.0, 0.0, 0.0])
        }
        fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, StorageError> {
            Ok(texts.iter().map(|_| vec![1.0, 0.0, 0.0]).collect())
        }
        fn dimension(&self) -> usize {
            3
        }
    }

    fn make_query(conv_id: Uuid, text: &str) -> PatientQuery {
        PatientQuery {
            text: text.to_string(),
            conversation_id: conv_id,
            query_type: None,
        }
    }

    #[test]
    fn no_context_returns_helpful_message() {
        let conn = open_memory_database().unwrap();
        let llm = MockLlm::understanding("Some response");
        let embedder = MockEmbedder;
        let vector_store = InMemoryVectorSearch::new();

        let conv_mgr = ConversationManager::new(&conn);
        let conv_id = conv_mgr.start(Some("Test")).unwrap();

        let pipeline = DocumentRagPipeline::new(&llm, &embedder, &vector_store, &conn);
        let query = make_query(conv_id, "What dose of metformin?");

        let result = pipeline.query(&query).unwrap();
        assert!(result.text.contains("documents"));
        assert_eq!(result.confidence, 0.0);
        assert!(result.citations.is_empty());
    }

    #[test]
    fn pipeline_with_semantic_context_generates_response() {
        let conn = open_memory_database().unwrap();
        let llm = MockLlm::understanding(
            "Your documents show metformin 500mg twice daily.",
        );
        let embedder = MockEmbedder;
        let mut vector_store = InMemoryVectorSearch::new();
        let doc_id = Uuid::new_v4();

        // Add a chunk so retrieval finds something
        vector_store.add(
            "c1",
            doc_id,
            "Metformin 500mg twice daily for type 2 diabetes management",
            vec![1.0, 0.0, 0.0],
            "prescription",
            Some("2024-01-15"),
            Some("Dr. Chen"),
        );

        let conv_mgr = ConversationManager::new(&conn);
        let conv_id = conv_mgr.start(Some("Test")).unwrap();

        let pipeline = DocumentRagPipeline::new(&llm, &embedder, &vector_store, &conn);
        let query = make_query(conv_id, "What dose of metformin?");

        let result = pipeline.query(&query).unwrap();
        assert!(result.text.contains("metformin"));
        assert_eq!(result.boundary_check, BoundaryCheck::Understanding);
        assert!(result.confidence > 0.0);
        assert!(result.context_used.semantic_chunks_used > 0);
    }

    #[test]
    fn out_of_bounds_response_has_zero_confidence() {
        let conn = open_memory_database().unwrap();
        let llm = MockLlm::out_of_bounds();
        let embedder = MockEmbedder;
        let mut vector_store = InMemoryVectorSearch::new();

        vector_store.add(
            "c1",
            Uuid::new_v4(),
            "Some medical content for context",
            vec![1.0, 0.0, 0.0],
            "note",
            None,
            None,
        );

        let conv_mgr = ConversationManager::new(&conn);
        let conv_id = conv_mgr.start(Some("Test")).unwrap();

        let pipeline = DocumentRagPipeline::new(&llm, &embedder, &vector_store, &conn);
        let query = make_query(conv_id, "Should I stop taking my medication?");

        let result = pipeline.query(&query).unwrap();
        assert_eq!(result.boundary_check, BoundaryCheck::OutOfBounds);
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn pipeline_persists_messages() {
        let conn = open_memory_database().unwrap();
        let llm = MockLlm::understanding("Your documents show metformin.");
        let embedder = MockEmbedder;
        let mut vector_store = InMemoryVectorSearch::new();

        vector_store.add(
            "c1",
            Uuid::new_v4(),
            "Metformin 500mg prescribed",
            vec![1.0, 0.0, 0.0],
            "prescription",
            None,
            None,
        );

        let conv_mgr = ConversationManager::new(&conn);
        let conv_id = conv_mgr.start(Some("Test")).unwrap();

        let pipeline = DocumentRagPipeline::new(&llm, &embedder, &vector_store, &conn);
        let query = make_query(conv_id, "What medications am I on?");

        pipeline.query(&query).unwrap();

        // Verify messages were persisted
        let history = conv_mgr.get_history(conv_id).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].role, crate::models::enums::MessageRole::Patient);
        assert_eq!(history[1].role, crate::models::enums::MessageRole::Coheara);
        assert!(history[1].confidence.unwrap() > 0.0);
    }

    #[test]
    fn query_type_override_respected() {
        let conn = open_memory_database().unwrap();
        let llm = MockLlm::understanding("Timeline of your medications.");
        let embedder = MockEmbedder;
        let mut vector_store = InMemoryVectorSearch::new();

        vector_store.add(
            "c1",
            Uuid::new_v4(),
            "Started metformin January 2024",
            vec![1.0, 0.0, 0.0],
            "prescription",
            Some("2024-01-01"),
            None,
        );

        let conv_mgr = ConversationManager::new(&conn);
        let conv_id = conv_mgr.start(Some("Test")).unwrap();

        let pipeline = DocumentRagPipeline::new(&llm, &embedder, &vector_store, &conn);
        let query = PatientQuery {
            text: "Tell me about my medications".to_string(),
            conversation_id: conv_id,
            query_type: Some(super::super::types::QueryType::Timeline),
        };

        let result = pipeline.query(&query).unwrap();
        assert_eq!(
            result.query_type,
            super::super::types::QueryType::Timeline
        );
    }

    #[test]
    fn context_summary_counts_structured_records() {
        use crate::db::repository;
        use crate::models::*;
        use crate::models::enums::*;

        let conn = open_memory_database().unwrap();
        let doc_id = Uuid::new_v4();

        // Insert a document first (foreign key)
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

        // Insert medication
        repository::insert_medication(
            &conn,
            &Medication {
                id: Uuid::new_v4(),
                generic_name: "Metformin".into(),
                brand_name: None,
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

        let llm = MockLlm::understanding("Your Metformin is 500mg twice daily.");
        let embedder = MockEmbedder;
        let mut vector_store = InMemoryVectorSearch::new();

        vector_store.add(
            "c1",
            doc_id,
            "Metformin 500mg twice daily",
            vec![1.0, 0.0, 0.0],
            "prescription",
            None,
            None,
        );

        let conv_mgr = ConversationManager::new(&conn);
        let conv_id = conv_mgr.start(Some("Test")).unwrap();

        let pipeline = DocumentRagPipeline::new(&llm, &embedder, &vector_store, &conn);
        let query = make_query(conv_id, "What is my metformin dose?");

        let result = pipeline.query(&query).unwrap();
        assert!(result.context_used.structured_records_used >= 1);
    }
}
