//! STR-01: MarkdownList extraction strategy.
//!
//! Makes 7 LLM calls (one per domain), each with a ~25-token prompt.
//! The SLM extracts one domain at a time as a markdown list.
//! The CODE parses each response and builds the structured entities.
//!
//! 0% degeneration on all configurations (BM-05).

use crate::pipeline::prompt_templates::{
    self, DocumentDomain, PromptStrategyKind,
};
use crate::pipeline::structuring::extraction_strategy::{ExtractionStrategy, StrategyOutput};
use crate::pipeline::structuring::markdown_parser;
use crate::pipeline::structuring::types::{ExtractedEntities, LlmClient};
use crate::pipeline::structuring::StructuringError;

// ═══════════════════════════════════════════════════════════
// Strategy implementation
// ═══════════════════════════════════════════════════════════

/// MarkdownList extraction strategy.
///
/// Iterates over all 7 document domains, makes one LLM call per domain,
/// parses the markdown response, and merges into `ExtractedEntities`.
pub struct MarkdownListStrategy {
    max_retries: u32,
}

impl MarkdownListStrategy {
    pub fn new(max_retries: u32) -> Self {
        Self { max_retries }
    }
}

impl ExtractionStrategy for MarkdownListStrategy {
    fn extract(
        &self,
        llm: &dyn LlmClient,
        model: &str,
        text: &str,
        ocr_confidence: f32,
    ) -> Result<StrategyOutput, StructuringError> {
        let system = prompt_templates::system_prompt(PromptStrategyKind::MarkdownList);
        let domains = DocumentDomain::all();

        let mut entities = ExtractedEntities::default();
        let mut markdown_sections = Vec::with_capacity(domains.len());
        let mut raw_responses = Vec::with_capacity(domains.len());

        for &domain in domains {
            let prompt = prompt_templates::markdown_list_prompt(domain, text, ocr_confidence);

            let response = match call_with_retry(llm, model, &prompt, system, self.max_retries) {
                Ok(resp) => resp,
                Err(e) => {
                    tracing::warn!(
                        domain = %domain,
                        error = %e,
                        "MarkdownList: domain extraction failed, continuing with partial results"
                    );
                    continue;
                }
            };

            raw_responses.push(response.clone());

            // Parse and merge
            let domain_entities = markdown_parser::parse_domain_response(domain, &response);
            domain_entities.merge_into(&mut entities);

            // Build markdown section
            let header = domain_header(domain);
            if !response.trim().is_empty() {
                markdown_sections.push(format!("## {header}\n{response}"));
            }
        }

        let markdown = markdown_sections.join("\n\n");

        Ok(StrategyOutput {
            entities,
            markdown,
            document_type: None,  // Orchestrator classifies from entities
            document_date: None,  // Orchestrator parses from metadata
            professional: None,   // Not extracted in domain prompts
            raw_responses,
        })
    }

    fn name(&self) -> &'static str {
        "markdown_list"
    }
}

// ═══════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════

/// Call LLM with retry on failure.
fn call_with_retry(
    llm: &dyn LlmClient,
    model: &str,
    prompt: &str,
    system: &str,
    max_retries: u32,
) -> Result<String, StructuringError> {
    let mut last_error = None;

    for attempt in 0..=max_retries {
        match llm.generate(model, prompt, system) {
            Ok(response) => return Ok(response),
            Err(e) => {
                if attempt < max_retries {
                    tracing::debug!(attempt, error = %e, "MarkdownList: retrying");
                }
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        StructuringError::MalformedResponse("No attempts made".into())
    }))
}

/// Human-readable domain header for markdown output.
fn domain_header(domain: DocumentDomain) -> &'static str {
    match domain {
        DocumentDomain::Medications => "Medications",
        DocumentDomain::LabResults => "Lab Results",
        DocumentDomain::Diagnoses => "Diagnoses",
        DocumentDomain::Allergies => "Allergies",
        DocumentDomain::Procedures => "Procedures",
        DocumentDomain::Referrals => "Referrals",
        DocumentDomain::Instructions => "Instructions",
    }
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::structuring::ollama::MockLlmClient;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // ── Mock that returns domain-specific responses ──────

    struct DomainAwareMockLlm {
        call_count: AtomicUsize,
    }

    impl DomainAwareMockLlm {
        fn new() -> Self {
            Self { call_count: AtomicUsize::new(0) }
        }
    }

    impl LlmClient for DomainAwareMockLlm {
        fn generate(
            &self,
            _model: &str,
            prompt: &str,
            _system: &str,
        ) -> Result<String, StructuringError> {
            self.call_count.fetch_add(1, Ordering::Relaxed);
            let lower = prompt.to_lowercase();

            if lower.contains("medication") {
                Ok("- Metformin\n  - dose: 500mg\n  - frequency: twice daily".into())
            } else if lower.contains("laboratory") || lower.contains("lab") {
                Ok("- Potassium\n  - value: 4.2\n  - unit: mmol/L".into())
            } else if lower.contains("diagnos") || lower.contains("condition") {
                Ok("- Type 2 Diabetes\n  - status: active".into())
            } else if lower.contains("allerg") {
                Ok("- Penicillin\n  - reaction: rash".into())
            } else if lower.contains("procedure") || lower.contains("intervention") {
                Ok("- Blood draw\n  - date: 2024-01-15".into())
            } else if lower.contains("referral") {
                Ok("- Endocrinologist\n  - reason: diabetes management".into())
            } else if lower.contains("instruction") || lower.contains("advice") {
                Ok("- Monitor blood sugar daily".into())
            } else {
                Ok(String::new())
            }
        }

        fn is_model_available(&self, _model: &str) -> Result<bool, StructuringError> {
            Ok(true)
        }

        fn list_models(&self) -> Result<Vec<String>, StructuringError> {
            Ok(vec!["medgemma:latest".into()])
        }
    }

    // ── Strategy tests ───────────────────────────────────

    #[test]
    fn full_extraction_all_domains() {
        let llm = DomainAwareMockLlm::new();
        let strategy = MarkdownListStrategy::new(1);

        let output = strategy
            .extract(&llm, "medgemma:latest", "Test document text here", 0.90)
            .unwrap();

        assert_eq!(output.entities.medications.len(), 1);
        assert_eq!(output.entities.lab_results.len(), 1);
        assert_eq!(output.entities.diagnoses.len(), 1);
        assert_eq!(output.entities.allergies.len(), 1);
        assert_eq!(output.entities.procedures.len(), 1);
        assert_eq!(output.entities.referrals.len(), 1);
        assert_eq!(output.entities.instructions.len(), 1);

        // 7 LLM calls (one per domain)
        assert_eq!(llm.call_count.load(Ordering::Relaxed), 7);
    }

    #[test]
    fn markdown_contains_domain_headers() {
        let llm = DomainAwareMockLlm::new();
        let strategy = MarkdownListStrategy::new(1);

        let output = strategy
            .extract(&llm, "medgemma:latest", "Test doc", 0.90)
            .unwrap();

        assert!(output.markdown.contains("## Medications"));
        assert!(output.markdown.contains("## Lab Results"));
        assert!(output.markdown.contains("## Diagnoses"));
    }

    #[test]
    fn raw_responses_collected() {
        let llm = DomainAwareMockLlm::new();
        let strategy = MarkdownListStrategy::new(1);

        let output = strategy
            .extract(&llm, "medgemma:latest", "Test doc", 0.90)
            .unwrap();

        assert_eq!(output.raw_responses.len(), 7);
    }

    #[test]
    fn empty_response_handled() {
        let llm = MockLlmClient::new("");
        let strategy = MarkdownListStrategy::new(1);

        let output = strategy
            .extract(&llm, "medgemma:latest", "Test doc", 0.90)
            .unwrap();

        // All domains should have empty entities
        assert!(output.entities.medications.is_empty());
        assert!(output.entities.lab_results.is_empty());
    }

    #[test]
    fn partial_failure_continues() {
        // LLM that fails on odd calls
        struct AlternatingMockLlm {
            call_count: AtomicUsize,
        }
        impl LlmClient for AlternatingMockLlm {
            fn generate(
                &self,
                _model: &str,
                _prompt: &str,
                _system: &str,
            ) -> Result<String, StructuringError> {
                let n = self.call_count.fetch_add(1, Ordering::Relaxed);
                if n % 2 == 0 {
                    Ok("- Test item".into())
                } else {
                    Err(StructuringError::OllamaConnection("test".into()))
                }
            }
            fn is_model_available(&self, _: &str) -> Result<bool, StructuringError> {
                Ok(true)
            }
            fn list_models(&self) -> Result<Vec<String>, StructuringError> {
                Ok(vec![])
            }
        }

        let llm = AlternatingMockLlm {
            call_count: AtomicUsize::new(0),
        };
        let strategy = MarkdownListStrategy::new(0); // No retries

        let output = strategy
            .extract(&llm, "medgemma:latest", "Test doc", 0.90)
            .unwrap();

        // Some domains should have entities, some shouldn't
        // (first call succeeds = medications, second fails = lab_results, etc.)
        assert!(!output.raw_responses.is_empty());
    }

    #[test]
    fn strategy_name() {
        let strategy = MarkdownListStrategy::new(1);
        assert_eq!(strategy.name(), "markdown_list");
    }

    #[test]
    fn metadata_not_extracted() {
        let llm = DomainAwareMockLlm::new();
        let strategy = MarkdownListStrategy::new(1);

        let output = strategy
            .extract(&llm, "medgemma:latest", "Test doc", 0.90)
            .unwrap();

        // MarkdownList doesn't extract metadata — orchestrator handles it
        assert!(output.document_type.is_none());
        assert!(output.document_date.is_none());
        assert!(output.professional.is_none());
    }

    #[test]
    fn retry_on_failure() {
        struct FailOnceLlm {
            call_count: AtomicUsize,
        }
        impl LlmClient for FailOnceLlm {
            fn generate(
                &self,
                _model: &str,
                _prompt: &str,
                _system: &str,
            ) -> Result<String, StructuringError> {
                let n = self.call_count.fetch_add(1, Ordering::Relaxed);
                if n == 0 {
                    Err(StructuringError::HttpClient("timeout".into()))
                } else {
                    Ok("- Item".into())
                }
            }
            fn is_model_available(&self, _: &str) -> Result<bool, StructuringError> {
                Ok(true)
            }
            fn list_models(&self) -> Result<Vec<String>, StructuringError> {
                Ok(vec![])
            }
        }

        let llm = FailOnceLlm {
            call_count: AtomicUsize::new(0),
        };
        // With 1 retry, the first domain fails then succeeds on retry
        let result = call_with_retry(&llm, "model", "prompt", "system", 1);
        assert!(result.is_ok());
    }
}
