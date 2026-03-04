use rusqlite::Connection;

use super::citation::{
    calculate_confidence, clean_citations_for_display, extract_citations,
    extract_guideline_citations, parse_boundary_check, validate_citations,
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
use crate::crypto::profile::PatientDemographics;
use crate::invariants::InvariantRegistry;
use crate::pipeline::safety::output_sanitize::sanitize_llm_output;
use crate::pipeline::safety::sanitize::sanitize_patient_input;
use crate::pipeline::storage::types::EmbeddingModel;

/// Trait for LLM text generation within the RAG pipeline.
pub trait LlmGenerate {
    fn generate(&self, system: &str, prompt: &str) -> Result<String, RagError>;

    /// Stream tokens via channel during generation.
    ///
    /// Default implementation falls back to non-streaming: generates the full
    /// response, then sends it as a single token. Override for progressive output.
    fn generate_streaming(
        &self,
        system: &str,
        prompt: &str,
        token_tx: std::sync::mpsc::Sender<String>,
    ) -> Result<String, RagError> {
        let result = self.generate(system, prompt)?;
        let _ = token_tx.send(result.clone());
        Ok(result)
    }
}

/// Full RAG pipeline orchestrator.
///
/// Coordinates: classify → retrieve → enrich → assemble → generate → cite → persist.
/// ME-03: Enrichment stage pairs user data with invariant registry to produce
/// deterministic clinical insights before context assembly.
/// ME-04: Demographics enable sex/ethnicity-aware enrichment.
pub struct DocumentRagPipeline<'a, G: LlmGenerate, E: EmbeddingModel, V: VectorSearch> {
    generator: &'a G,
    embedder: &'a E,
    vector_store: &'a V,
    conn: &'a Connection,
    /// ME-03: Invariant reference data for clinical enrichment.
    registry: &'a InvariantRegistry,
    /// I18N-19: Language for system prompt and no-context response.
    lang: String,
    /// ME-04: Patient demographics for personalized enrichment.
    demographics: Option<PatientDemographics>,
}

impl<'a, G: LlmGenerate, E: EmbeddingModel, V: VectorSearch> DocumentRagPipeline<'a, G, E, V> {
    pub fn new(
        generator: &'a G,
        embedder: &'a E,
        vector_store: &'a V,
        conn: &'a Connection,
        registry: &'a InvariantRegistry,
    ) -> Self {
        Self {
            generator,
            embedder,
            vector_store,
            conn,
            registry,
            lang: "en".to_string(),
            demographics: None,
        }
    }

    /// I18N-19: Create a pipeline with a specific response language.
    pub fn with_language(
        generator: &'a G,
        embedder: &'a E,
        vector_store: &'a V,
        conn: &'a Connection,
        registry: &'a InvariantRegistry,
        lang: &str,
    ) -> Self {
        Self {
            generator,
            embedder,
            vector_store,
            conn,
            registry,
            lang: lang.to_string(),
            demographics: None,
        }
    }

    /// ME-04: Set patient demographics for personalized enrichment.
    pub fn with_demographics(mut self, demographics: Option<PatientDemographics>) -> Self {
        self.demographics = demographics;
        self
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
            || !retrieved.structured_data.symptoms.is_empty()
            || !retrieved.structured_data.vital_signs.is_empty()
            || !retrieved.structured_data.screening_records.is_empty()
            || !retrieved.structured_data.entity_connections.is_empty();

        if !has_semantic && !has_structured {
            return Ok(self.no_context_result(query_type));
        }

        // Step 4.5: Enrich with clinical insights (ME-03)
        // Pure, deterministic — no LLM, no async. Pairs user data with invariant registry.
        // ME-04: demographics threaded for sex-aware hemoglobin and ethnicity-aware BMI.
        let insights = crate::invariants::enrich::enrich(
            &retrieved.structured_data.medications,
            &retrieved.structured_data.lab_results,
            &retrieved.structured_data.allergies,
            &retrieved.structured_data.vital_signs,
            self.registry,
            chrono::Local::now().date_naive(),
            self.demographics.as_ref(),
        );

        // Step 4.8: ME-01 — Score all medical items with weighted equation
        // M(item, query) = D * R * V * T * S * (1-U)
        // Pre-computes meaning so SLM articulates, not discovers.
        let verification_ctx = build_verification_context(self.conn, &retrieved.structured_data);
        let alert_counts = build_alert_counts(self.conn);
        let scoring_result = super::scoring_pipeline::run_scoring(
            &query.text,
            &retrieved.structured_data,
            &retrieved.structured_data.entity_connections,
            &alert_counts,
            &verification_ctx,
        );
        let scored_section = super::scored_context::format_scored_context(
            &scoring_result,
            &retrieved.structured_data.entity_connections,
        );

        // Step 5: Assemble context within token budget
        let mut assembled = assemble_context(&retrieved, &query_type, &insights, self.demographics.as_ref());

        // Prepend scored context (highest-priority medical intelligence)
        if !scored_section.is_empty() {
            assembled.text = format!("{}\n\n{}", scored_section, assembled.text);
            assembled.estimated_tokens += scored_section.len() / 4;
        }

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

        // C2: Strip thinking tokens before parsing (prevents <unused94> leaking to user)
        let sanitized_response = sanitize_llm_output(&raw_response);

        // Step 9: Parse boundary check + clean response
        let (boundary_check, cleaned_response) = parse_boundary_check(&sanitized_response);

        // Step 10: Extract citations
        let raw_citations = extract_citations(&cleaned_response, &assembled.chunks_included);

        // Step 10b: Validate citation document_ids against DB (RS-L2-01-002)
        let citations = validate_citations(self.conn, raw_citations);

        // Step 11: Clean citation markers from display text
        let display_text = clean_citations_for_display(&cleaned_response);

        // Step 12: Calculate confidence (ME-01: data-driven via GroundingLevel)
        let grounding = super::scored_context::compute_grounding(&scoring_result);
        let confidence = calculate_confidence(
            &boundary_check,
            grounding,
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
        // ME-03: Extract guideline citations from clinical insights
        let guideline_citations = extract_guideline_citations(&insights);

        Ok(RagResponse {
            text: display_text,
            citations,
            guideline_citations,
            confidence,
            query_type,
            context_used,
            boundary_check,
            grounding,
        })
    }

    /// Generate a RAG response with streaming tokens.
    ///
    /// Same pipeline as `generate()` (steps 1-7 identical), but step 8 uses
    /// `generate_streaming()` to emit tokens progressively via `token_tx`.
    /// The full response is still returned for post-processing (citations, safety).
    pub fn generate_streaming(
        &self,
        query: &PatientQuery,
        token_tx: std::sync::mpsc::Sender<String>,
    ) -> Result<RagResponse, RagError> {
        // Steps 1-7: identical to generate()
        let query_type = query
            .query_type
            .clone()
            .unwrap_or_else(|| classify_query(&query.text));

        let params = retrieval_strategy(&query_type);

        let retrieved = retrieve(
            &query.text,
            self.embedder,
            self.vector_store,
            &params,
            self.conn,
        )?;

        let has_semantic = !retrieved.semantic_chunks.is_empty();
        let has_structured = !retrieved.structured_data.medications.is_empty()
            || !retrieved.structured_data.diagnoses.is_empty()
            || !retrieved.structured_data.allergies.is_empty()
            || !retrieved.structured_data.lab_results.is_empty()
            || !retrieved.structured_data.symptoms.is_empty()
            || !retrieved.structured_data.vital_signs.is_empty()
            || !retrieved.structured_data.screening_records.is_empty()
            || !retrieved.structured_data.entity_connections.is_empty();

        if !has_semantic && !has_structured {
            return Ok(self.no_context_result(query_type));
        }

        // Step 4.5: Enrich with clinical insights (ME-03)
        let insights = crate::invariants::enrich::enrich(
            &retrieved.structured_data.medications,
            &retrieved.structured_data.lab_results,
            &retrieved.structured_data.allergies,
            &retrieved.structured_data.vital_signs,
            self.registry,
            chrono::Local::now().date_naive(),
            self.demographics.as_ref(),
        );

        // Step 4.8: ME-01 — Score all medical items (streaming path)
        let verification_ctx = build_verification_context(self.conn, &retrieved.structured_data);
        let alert_counts = build_alert_counts(self.conn);
        let scoring_result = super::scoring_pipeline::run_scoring(
            &query.text,
            &retrieved.structured_data,
            &retrieved.structured_data.entity_connections,
            &alert_counts,
            &verification_ctx,
        );
        let scored_section = super::scored_context::format_scored_context(
            &scoring_result,
            &retrieved.structured_data.entity_connections,
        );

        let mut assembled = assemble_context(&retrieved, &query_type, &insights, self.demographics.as_ref());

        if !scored_section.is_empty() {
            assembled.text = format!("{}\n\n{}", scored_section, assembled.text);
            assembled.estimated_tokens += scored_section.len() / 4;
        }

        if assembled.text.is_empty() {
            return Ok(self.no_context_result(query_type));
        }

        let conversation_mgr = ConversationManager::new(self.conn);
        let history = conversation_mgr
            .get_history(query.conversation_id)
            .unwrap_or_default();

        let sanitized_query = sanitize_patient_input(&query.text, 2000)
            .map(|s| s.text)
            .unwrap_or_else(|_| query.text.clone());

        let prompt = build_conversation_prompt(&sanitized_query, &assembled, &history);

        // Step 8: Generate with streaming (tokens flow via channel)
        let system_prompt = conversation_system_prompt_i18n(&self.lang);
        let raw_response = self
            .generator
            .generate_streaming(&system_prompt, &prompt, token_tx)?;

        // C2: Strip thinking tokens before parsing (prevents <unused94> leaking to user)
        let sanitized_response = sanitize_llm_output(&raw_response);

        // Steps 9-13: post-processing (identical to generate())
        let (boundary_check, cleaned_response) = parse_boundary_check(&sanitized_response);
        let raw_citations = extract_citations(&cleaned_response, &assembled.chunks_included);
        let citations = validate_citations(self.conn, raw_citations);
        let display_text = clean_citations_for_display(&cleaned_response);

        let grounding = super::scored_context::compute_grounding(&scoring_result);
        let confidence = calculate_confidence(
            &boundary_check,
            grounding,
            citations.len(),
            assembled.chunks_included.len(),
        );

        let display_text = if confidence < 0.3 {
            tracing::info!(confidence, "Low RAG confidence — adding disclaimer");
            format!(
                "**Note:** This response is based on limited information from your documents. \
                Please verify with your healthcare provider.\n\n{display_text}"
            )
        } else {
            display_text
        };

        let context_used = build_context_summary(&assembled, &retrieved.structured_data);
        let guideline_citations = extract_guideline_citations(&insights);

        Ok(RagResponse {
            text: display_text,
            citations,
            guideline_citations,
            confidence,
            query_type,
            context_used,
            boundary_check,
            grounding,
        })
    }

    fn no_context_result(&self, query_type: super::types::QueryType) -> RagResponse {
        RagResponse {
            text: no_context_response_i18n(&self.lang),
            citations: vec![],
            guideline_citations: vec![],
            confidence: 0.0,
            query_type,
            context_used: ContextSummary {
                semantic_chunks_used: 0,
                structured_records_used: 0,
                total_context_tokens: 0,
            },
            boundary_check: super::types::BoundaryCheck::NoContext,
            grounding: super::scored_context::GroundingLevel::None,
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
        + structured.symptoms.len()
        + structured.vital_signs.len();

    ContextSummary {
        semantic_chunks_used: assembled.chunks_included.len(),
        structured_records_used: structured_count,
        total_context_tokens: assembled.estimated_tokens,
    }
}

/// Build verification context from DB: document_id -> (verified, confirmed).
///
/// Collects unique document IDs from structured data, then batch-looks up
/// each document's `verified` flag and `pipeline_status == Confirmed`.
fn build_verification_context(
    conn: &Connection,
    structured: &super::types::StructuredContext,
) -> super::factors::VerificationContext {
    use crate::models::enums::PipelineStatus;
    use std::collections::HashSet;

    // Collect unique document IDs from all entity types
    let mut doc_ids = HashSet::new();
    for med in &structured.medications {
        doc_ids.insert(med.document_id);
    }
    for lab in &structured.lab_results {
        doc_ids.insert(lab.document_id);
    }
    for dx in &structured.diagnoses {
        doc_ids.insert(dx.document_id);
    }
    for allergy in &structured.allergies {
        if let Some(id) = allergy.document_id {
            doc_ids.insert(id);
        }
    }

    let mut doc_status = std::collections::HashMap::new();
    for doc_id in doc_ids {
        if let Ok(Some(doc)) = crate::db::repository::get_document(conn, &doc_id) {
            doc_status.insert(
                doc_id,
                (doc.verified, doc.pipeline_status == PipelineStatus::Confirmed),
            );
        }
    }

    super::factors::VerificationContext { doc_status }
}

/// Build alert entity counts from DB: entity_id -> count of open alerts.
///
/// Loads all undismissed coherence alerts, then counts how many alerts
/// reference each entity UUID.
fn build_alert_counts(conn: &Connection) -> std::collections::HashMap<uuid::Uuid, usize> {
    let alerts = crate::db::repository::load_active_coherence_alerts(conn)
        .unwrap_or_default();

    let mut counts = std::collections::HashMap::new();
    for alert in &alerts {
        for entity_id in &alert.entity_ids {
            *counts.entry(*entity_id).or_insert(0) += 1;
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;
    use crate::invariants::InvariantRegistry;
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
        // BUG-1 fix: NoContext boundary check passes safety filter,
        // so user sees the helpful "import documents first" message.
        let conn = open_memory_database().unwrap();
        let llm = MockLlm::understanding("Some response");
        let embedder = MockEmbedder;
        let vector_store = InMemoryVectorSearch::new();

        let conv_mgr = ConversationManager::new(&conn);
        let conv_id = conv_mgr.start(Some("Test")).unwrap();

        let registry = InvariantRegistry::empty();
        let pipeline = DocumentRagPipeline::new(&llm, &embedder, &vector_store, &conn, &registry);
        let query = make_query(conv_id, "What dose of metformin?");

        let result = pipeline.query(&query).unwrap();
        assert!(result.text.contains("documents"));
        assert_eq!(result.boundary_check, BoundaryCheck::NoContext);
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

        let registry = InvariantRegistry::empty();
        let pipeline = DocumentRagPipeline::new(&llm, &embedder, &vector_store, &conn, &registry);
        let query = make_query(conv_id, "What dose of metformin?");

        let result = pipeline.query(&query).unwrap();
        assert!(result.text.contains("metformin"));
        assert_eq!(result.boundary_check, BoundaryCheck::Understanding);
        assert!(result.confidence > 0.0);
        assert!(result.context_used.semantic_chunks_used > 0);
    }

    #[test]
    fn missing_boundary_tag_defaults_to_understanding() {
        // BUG-2 fix: LLM forgetting BOUNDARY_CHECK tag is a format issue,
        // not a safety issue. Should pass through with Understanding.
        let conn = open_memory_database().unwrap();
        let llm = MockLlm::out_of_bounds(); // returns text WITHOUT BOUNDARY_CHECK: line
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

        let registry = InvariantRegistry::empty();
        let pipeline = DocumentRagPipeline::new(&llm, &embedder, &vector_store, &conn, &registry);
        let query = make_query(conv_id, "Should I stop taking my medication?");

        let result = pipeline.query(&query).unwrap();
        assert_eq!(result.boundary_check, BoundaryCheck::Understanding);
        assert!(result.confidence > 0.0);
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

        let registry = InvariantRegistry::empty();
        let pipeline = DocumentRagPipeline::new(&llm, &embedder, &vector_store, &conn, &registry);
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

        let registry = InvariantRegistry::empty();
        let pipeline = DocumentRagPipeline::new(&llm, &embedder, &vector_store, &conn, &registry);
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

        let registry = InvariantRegistry::empty();
        let pipeline = DocumentRagPipeline::new(&llm, &embedder, &vector_store, &conn, &registry);
        let query = make_query(conv_id, "What is my metformin dose?");

        let result = pipeline.query(&query).unwrap();
        assert!(result.context_used.structured_records_used >= 1);
    }
}
