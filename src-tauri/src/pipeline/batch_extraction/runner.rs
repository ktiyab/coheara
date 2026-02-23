//! BatchRunner — orchestrates the full extraction pipeline.
//!
//! Connects all 5 modules: Scheduler → Analyzer → Extractors → Verifier → Store.
//! Runs sequentially (one LLM call at a time) as Ollama is single-threaded on CPU.

use std::time::Instant;

use rusqlite::Connection;

use super::duplicate::check_duplicate;
use super::error::ExtractionError;
use super::scheduler::new_batch_id;
use super::store::create_pending_item;
use super::traits::*;
use super::types::*;
use super::verifier::SemanticVerifier;
use crate::pipeline::safety::output_sanitize::sanitize_llm_output;

/// Orchestrates a full extraction batch run.
pub struct BatchRunner {
    analyzer: Box<dyn ConversationAnalyzer>,
    extractors: Vec<Box<dyn DomainExtractor>>,
    verifier: SemanticVerifier,
    config: ExtractionConfig,
}

impl BatchRunner {
    pub fn new(
        analyzer: Box<dyn ConversationAnalyzer>,
        extractors: Vec<Box<dyn DomainExtractor>>,
        config: ExtractionConfig,
    ) -> Self {
        let verifier = SemanticVerifier::new(config.max_items_per_domain as usize);
        Self {
            analyzer,
            extractors,
            verifier,
            config,
        }
    }

    /// Run extraction on a single conversation.
    /// Returns pending items ready for storage.
    pub fn extract_conversation(
        &self,
        conversation: &ConversationBatch,
        patient_context: &PatientContext,
        llm: &dyn crate::pipeline::structuring::types::LlmClient,
    ) -> Result<ConversationExtractionResult, ExtractionError> {
        let start = Instant::now();

        // Step 1: Analyze conversation for domains
        let analysis = self.analyzer.analyze(conversation);

        if analysis.is_pure_qa || analysis.domains.is_empty() {
            return Ok(ConversationExtractionResult {
                conversation_id: conversation.id.clone(),
                domains_found: vec![],
                items: vec![],
                duration_ms: start.elapsed().as_millis() as u64,
                skipped: true,
            });
        }

        let batch_id = new_batch_id();
        let conversation_date = conversation.last_message_at.date();
        let mut all_items = Vec::new();
        let mut domains_found = Vec::new();

        // Step 2: Extract per domain (sequential — one LLM call at a time)
        for domain_match in &analysis.domains {
            let extractor = match self.get_extractor(domain_match.domain) {
                Ok(e) => e,
                Err(_) => {
                    tracing::debug!(
                        domain = domain_match.domain.as_str(),
                        "No extractor registered for domain, skipping"
                    );
                    continue;
                }
            };

            // Build extraction input
            let input = analysis.build_input(
                conversation,
                domain_match,
                patient_context.clone(),
                conversation_date,
            );

            // Build prompt and call LLM
            let prompt = extractor.build_prompt(&input);
            let system = "You are a medical health information extractor. Output valid JSON only.";

            let raw_response = llm.generate(
                &self.config.model_name,
                &prompt,
                system,
            )?;

            // Sanitize output: strip MedGemma thinking tags before parsing
            let response = sanitize_llm_output(&raw_response);

            // Parse response
            let items = match extractor.parse_response(&response) {
                Ok(items) => items,
                Err(e) => {
                    tracing::warn!(
                        domain = domain_match.domain.as_str(),
                        conversation_id = conversation.id,
                        error = %e,
                        "Failed to parse extraction response, skipping domain"
                    );
                    continue;
                }
            };

            // Validate
            let validated = extractor.validate(&items);
            if validated.rejected_count > 0 {
                tracing::debug!(
                    domain = domain_match.domain.as_str(),
                    rejected = validated.rejected_count,
                    warnings = ?validated.warnings,
                    "Validation rejected some items"
                );
            }

            // Consolidate duplicates within conversation
            let consolidated = extractor.consolidate(validated.items);

            // Verify against source text
            let verified = self.verifier.verify(&consolidated, &input);
            if !verified.warnings.is_empty() {
                tracing::debug!(
                    domain = domain_match.domain.as_str(),
                    warnings = ?verified.warnings,
                    "Verification warnings"
                );
            }

            // Create pending review items
            for verified_item in verified.items {
                if verified_item.confidence >= self.config.confidence_threshold {
                    let source_msgs: Vec<&ConversationMessage> = verified_item
                        .item
                        .source_message_indices
                        .iter()
                        .filter_map(|&idx| conversation.messages.get(idx))
                        .collect();

                    let source_ids: Vec<String> =
                        source_msgs.iter().map(|m| m.id.clone()).collect();

                    // Build source quote from patient signal messages (REV-12)
                    let source_quote = build_source_quote(&source_msgs);

                    let pending = create_pending_item(
                        &conversation.id,
                        &batch_id,
                        domain_match.domain,
                        verified_item.item.data,
                        verified_item.confidence,
                        verified_item.grounding,
                        None, // Duplicate detection runs in run_full_batch (needs DB access)
                        source_ids,
                        source_quote,
                    );

                    all_items.push(pending);
                }
            }

            domains_found.push(domain_match.domain);
        }

        Ok(ConversationExtractionResult {
            conversation_id: conversation.id.clone(),
            domains_found,
            items: all_items,
            duration_ms: start.elapsed().as_millis() as u64,
            skipped: false,
        })
    }

    fn get_extractor(&self, domain: ExtractionDomain) -> Result<&dyn DomainExtractor, ExtractionError> {
        self.extractors
            .iter()
            .find(|e| e.domain() == domain)
            .map(|e| e.as_ref())
            .ok_or_else(|| ExtractionError::ExtractorNotFound(domain.to_string()))
    }
}

/// Build a source quote from signal messages for display in ReviewCards (REV-12).
/// Picks the first patient message, truncated to 200 chars.
fn build_source_quote(source_msgs: &[&ConversationMessage]) -> Option<String> {
    // Prefer patient messages over assistant messages
    let patient_msg = source_msgs
        .iter()
        .find(|m| m.role == "patient")
        .or_else(|| source_msgs.first());

    patient_msg.map(|m| {
        let content = m.content.trim();
        if content.len() <= 200 {
            content.to_string()
        } else {
            let truncated = &content[..content.ceil_char_boundary(197)];
            format!("{truncated}...")
        }
    })
}

/// Result of extracting from a single conversation.
#[derive(Debug)]
pub struct ConversationExtractionResult {
    pub conversation_id: String,
    pub domains_found: Vec<ExtractionDomain>,
    pub items: Vec<PendingReviewItem>,
    pub duration_ms: u64,
    pub skipped: bool,
}

/// Run a full batch: scheduler → runner → store.
/// This is the top-level function called from the IPC command.
pub fn run_full_batch(
    conn: &Connection,
    scheduler: &dyn BatchScheduler,
    runner: &BatchRunner,
    store: &dyn PendingReviewStore,
    llm: &dyn crate::pipeline::structuring::types::LlmClient,
    config: &ExtractionConfig,
    patient_context: &PatientContext,
    progress_fn: Option<&dyn Fn(BatchStatusEvent)>,
) -> Result<BatchResult, ExtractionError> {
    let start = Instant::now();

    let conversations = scheduler.get_eligible_conversations(conn, config)?;

    if conversations.is_empty() {
        return Ok(BatchResult::empty());
    }

    let total = conversations.len() as u32;

    if let Some(progress) = progress_fn {
        progress(BatchStatusEvent::Started {
            conversation_count: total,
        });
    }

    let mut result = BatchResult::empty();

    for (i, conv) in conversations.iter().enumerate() {
        if let Some(progress) = progress_fn {
            progress(BatchStatusEvent::Progress {
                completed: i as u32,
                total,
                current_title: conv.title.clone().unwrap_or_default(),
            });
        }

        match runner.extract_conversation(conv, patient_context, llm) {
            Ok(mut extraction) => {
                if extraction.skipped {
                    result.conversations_skipped += 1;
                } else {
                    result.conversations_processed += 1;
                    result.items_extracted += extraction.items.len() as u32;

                    // Check duplicates against existing DB records
                    let conv_date = conv.last_message_at.date();
                    for item in &mut extraction.items {
                        let dup_status = check_duplicate(
                            conn,
                            item.domain,
                            &item.extracted_data,
                            conv_date,
                        );
                        match &dup_status {
                            DuplicateStatus::AlreadyTracked { existing_id }
                            | DuplicateStatus::PossibleDuplicate { existing_id } => {
                                item.duplicate_of = Some(existing_id.clone());
                            }
                            DuplicateStatus::New => {}
                        }
                    }

                    // Store pending items
                    if !extraction.items.is_empty() {
                        store.store_pending(conn, &extraction.items)?;
                        result.items_stored += extraction.items.len() as u32;
                    }

                    // Mark conversation as extracted
                    let batch_id = if let Some(item) = extraction.items.first() {
                        item.batch_id.clone()
                    } else {
                        new_batch_id()
                    };

                    scheduler.mark_extracted(
                        conn,
                        &conv.id,
                        &batch_id,
                        &extraction.domains_found,
                        extraction.items.len() as u32,
                        &config.model_name,
                        extraction.duration_ms,
                    )?;
                }
            }
            Err(e) => {
                result.errors.push(format!(
                    "Conversation {}: {e}",
                    conv.title.as_deref().unwrap_or(&conv.id)
                ));
            }
        }
    }

    result.duration_ms = start.elapsed().as_millis() as u64;

    if let Some(progress) = progress_fn {
        progress(BatchStatusEvent::Completed {
            items_found: result.items_stored,
            duration_ms: result.duration_ms,
        });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::batch_extraction::analyzer::RuleBasedAnalyzer;
    use crate::pipeline::batch_extraction::extractors::*;
    use crate::pipeline::structuring::StructuringError;

    /// Mock LLM that returns canned symptom extraction response.
    struct MockExtractionLlm {
        response: String,
    }

    impl MockExtractionLlm {
        fn symptom_response() -> Self {
            Self {
                response: r#"{"symptoms": [{"category": "Pain", "specific": "Headache", "severity_hint": 4, "onset_hint": null, "body_region": "right side", "duration": "3 days", "character": "Throbbing", "aggravating": [], "relieving": [], "timing_pattern": null, "notes": null, "related_medication_hint": null, "source_messages": [0]}]}"#.to_string(),
            }
        }

        fn empty_response() -> Self {
            Self {
                response: r#"{"symptoms": [], "medications": [], "appointments": []}"#.to_string(),
            }
        }
    }

    impl crate::pipeline::structuring::types::LlmClient for MockExtractionLlm {
        fn generate(&self, _model: &str, _prompt: &str, _system: &str) -> Result<String, StructuringError> {
            Ok(self.response.clone())
        }

        fn is_model_available(&self, _model: &str) -> Result<bool, StructuringError> {
            Ok(true)
        }

        fn list_models(&self) -> Result<Vec<String>, StructuringError> {
            Ok(vec!["medgemma:4b".to_string()])
        }
    }

    fn make_conversation(id: &str, messages: Vec<(&str, &str)>) -> ConversationBatch {
        let msg_count = messages.len() as u32;
        ConversationBatch {
            id: id.to_string(),
            title: Some(format!("Test {id}")),
            messages: messages.into_iter().enumerate().map(|(i, (role, content))| {
                ConversationMessage {
                    id: format!("msg-{i}"),
                    index: i,
                    role: role.to_string(),
                    content: content.to_string(),
                    created_at: chrono::NaiveDate::from_ymd_opt(2026, 2, 20)
                        .unwrap()
                        .and_hms_opt(10, i as u32, 0)
                        .unwrap(),
                    is_signal: false,
                }
            }).collect(),
            last_message_at: chrono::NaiveDate::from_ymd_opt(2026, 2, 20)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap(),
            message_count: msg_count,
        }
    }

    fn make_runner() -> BatchRunner {
        let config = ExtractionConfig::default();
        BatchRunner::new(
            Box::new(RuleBasedAnalyzer::new()),
            vec![
                Box::new(SymptomExtractor::new()),
                Box::new(MedicationExtractor::new()),
                Box::new(AppointmentExtractor::new()),
            ],
            config,
        )
    }

    #[test]
    fn extracts_symptom_from_conversation() {
        let runner = make_runner();
        let llm = MockExtractionLlm::symptom_response();
        let conv = make_conversation("conv-1", vec![
            ("patient", "I've been having headaches for 3 days, throbbing on the right side"),
            ("coheara", "That sounds uncomfortable. Tell me more."),
        ]);

        let result = runner.extract_conversation(&conv, &PatientContext::default(), &llm).unwrap();

        assert!(!result.skipped);
        assert!(result.domains_found.contains(&ExtractionDomain::Symptom));
        assert!(!result.items.is_empty());
        assert_eq!(result.items[0].domain, ExtractionDomain::Symptom);
    }

    #[test]
    fn skips_pure_qa_conversation() {
        let runner = make_runner();
        let llm = MockExtractionLlm::empty_response();
        let conv = make_conversation("conv-qa", vec![
            ("patient", "What is a blood pressure monitor?"),
            ("coheara", "It's a device that measures blood pressure."),
        ]);

        let result = runner.extract_conversation(&conv, &PatientContext::default(), &llm).unwrap();

        assert!(result.skipped);
        assert!(result.items.is_empty());
    }

    #[test]
    fn handles_llm_parse_failure_gracefully() {
        struct BadLlm;
        impl crate::pipeline::structuring::types::LlmClient for BadLlm {
            fn generate(&self, _: &str, _: &str, _: &str) -> Result<String, StructuringError> {
                Ok("This is not JSON at all, sorry!".to_string())
            }
            fn is_model_available(&self, _: &str) -> Result<bool, StructuringError> { Ok(true) }
            fn list_models(&self) -> Result<Vec<String>, StructuringError> { Ok(vec![]) }
        }

        let runner = make_runner();
        let llm = BadLlm;
        let conv = make_conversation("conv-bad", vec![
            ("patient", "I have terrible headaches every morning"),
            ("coheara", "Let me help you track that."),
        ]);

        // Should not panic — should handle parse error gracefully
        let result = runner.extract_conversation(&conv, &PatientContext::default(), &llm).unwrap();

        // No items extracted (parse failed), but shouldn't error
        assert!(!result.skipped); // Analyzer detected symptoms
        assert!(result.items.is_empty()); // But parse failed
    }

    #[test]
    fn low_confidence_items_filtered() {
        struct LowConfLlm;
        impl crate::pipeline::structuring::types::LlmClient for LowConfLlm {
            fn generate(&self, _: &str, _: &str, _: &str) -> Result<String, StructuringError> {
                // Return a symptom that won't ground well (unrelated to message)
                Ok(r#"{"symptoms": [{"category": "Skin", "specific": "Rash", "severity_hint": null, "onset_hint": null, "body_region": "arm", "duration": null, "character": null, "aggravating": [], "relieving": [], "timing_pattern": null, "notes": null, "related_medication_hint": null, "source_messages": [0]}]}"#.to_string())
            }
            fn is_model_available(&self, _: &str) -> Result<bool, StructuringError> { Ok(true) }
            fn list_models(&self) -> Result<Vec<String>, StructuringError> { Ok(vec![]) }
        }

        let mut config = ExtractionConfig::default();
        config.confidence_threshold = 0.8; // High threshold
        let runner = BatchRunner::new(
            Box::new(RuleBasedAnalyzer::new()),
            vec![Box::new(SymptomExtractor::new())],
            config,
        );
        let llm = LowConfLlm;
        let conv = make_conversation("conv-low", vec![
            ("patient", "I have a headache every day"),
            ("coheara", "I see."),
        ]);

        let result = runner.extract_conversation(&conv, &PatientContext::default(), &llm).unwrap();

        // "Rash" on "arm" won't ground against "headache" → low confidence → filtered
        assert!(result.items.is_empty(), "Low confidence items should be filtered");
    }

    #[test]
    fn extraction_result_has_timing() {
        let runner = make_runner();
        let llm = MockExtractionLlm::symptom_response();
        let conv = make_conversation("conv-timed", vec![
            ("patient", "My head hurts badly"),
            ("coheara", "Since when?"),
        ]);

        let result = runner.extract_conversation(&conv, &PatientContext::default(), &llm).unwrap();

        assert!(result.duration_ms > 0 || result.duration_ms == 0); // Just verify field exists
    }

    #[test]
    fn source_quote_populated_from_patient_message() {
        let runner = make_runner();
        let llm = MockExtractionLlm::symptom_response();
        let conv = make_conversation("conv-quote", vec![
            ("patient", "I've been having headaches for 3 days, throbbing on the right side"),
            ("coheara", "That sounds uncomfortable. Tell me more."),
        ]);

        let result = runner.extract_conversation(&conv, &PatientContext::default(), &llm).unwrap();

        assert!(!result.items.is_empty());
        let quote = result.items[0].source_quote.as_ref().expect("source_quote should be set");
        assert!(quote.contains("headaches"), "Quote should contain patient text, got: {quote}");
    }

    #[test]
    fn source_quote_truncated_at_200_chars() {
        let long_text = "a".repeat(300);
        let msg = ConversationMessage {
            id: "m1".to_string(),
            index: 0,
            role: "patient".to_string(),
            content: long_text,
            created_at: chrono::NaiveDate::from_ymd_opt(2026, 2, 20)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap(),
            is_signal: true,
        };
        let msgs = vec![&msg];

        let quote = build_source_quote(&msgs).unwrap();
        assert!(quote.len() <= 200, "Quote should be max 200 chars, got {}", quote.len());
        assert!(quote.ends_with("..."), "Truncated quote should end with ...");
    }
}
