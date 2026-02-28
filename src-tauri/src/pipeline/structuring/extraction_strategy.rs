//! STR-01: Extraction strategy trait and output types.
//!
//! Defines the `ExtractionStrategy` trait that all extraction strategies implement.
//! Each strategy owns its LLM call pattern (7 calls, N×M calls, etc.) and returns
//! a common `StrategyOutput` that the orchestrator feeds through shared post-processing.
//!
//! Principle: The SLM does ONE thing per call. The CODE orchestrates.
//! Evidence: MF-44 (prompt complexity is the dominant degeneration factor).

use crate::pipeline::prompt_templates::PromptStrategyKind;
use crate::pipeline::strategy::PromptStrategy;
use crate::pipeline::structuring::strategy_iterative_drill::IterativeDrillStrategy;
use crate::pipeline::structuring::strategy_markdown_list::MarkdownListStrategy;
use crate::pipeline::structuring::types::{ExtractedEntities, ExtractedProfessional, LlmClient};
use crate::pipeline::structuring::StructuringError;

// ═══════════════════════════════════════════════════════════
// Strategy output
// ═══════════════════════════════════════════════════════════

/// Common output from all extraction strategies.
///
/// Carries extracted entities, combined markdown, and metadata.
/// The orchestrator applies shared post-processing (validate, classify,
/// confidence, sanitize) on this output regardless of which strategy produced it.
#[derive(Debug, Clone)]
pub struct StrategyOutput {
    /// Extracted entities across all 7 domains.
    pub entities: ExtractedEntities,
    /// Combined markdown representation of the extraction.
    pub markdown: String,
    /// Document type detected during extraction (e.g., "prescription").
    pub document_type: Option<String>,
    /// Document date detected during extraction (ISO format).
    pub document_date: Option<String>,
    /// Professional information detected during extraction.
    pub professional: Option<ExtractedProfessional>,
    /// Raw LLM responses for P.8 audit trail.
    pub raw_responses: Vec<String>,
}

// ═══════════════════════════════════════════════════════════
// Strategy trait
// ═══════════════════════════════════════════════════════════

/// Trait for document extraction strategies.
///
/// Each strategy controls how many LLM calls are made and how responses
/// are parsed into entities. The orchestrator delegates extraction to the
/// strategy, then applies shared post-processing on the output.
///
/// Implementations:
/// - `MarkdownListStrategy`: 7 calls (1 per domain), ~25-token prompts
/// - `IterativeDrillStrategy`: 7 enumerate + N×M drill calls, ~15-token prompts
pub trait ExtractionStrategy: Send + Sync {
    /// Extract entities from document text using the strategy's LLM call pattern.
    ///
    /// # Arguments
    /// * `llm` - LLM client for generating responses
    /// * `model` - Model name (e.g., "medgemma:latest")
    /// * `text` - Sanitized document text
    /// * `ocr_confidence` - OCR confidence score (0.0-1.0)
    fn extract(
        &self,
        llm: &dyn LlmClient,
        model: &str,
        text: &str,
        ocr_confidence: f32,
    ) -> Result<StrategyOutput, StructuringError>;

    /// Human-readable strategy name for logging and diagnostics.
    fn name(&self) -> &'static str;
}

// ═══════════════════════════════════════════════════════════
// Factory
// ═══════════════════════════════════════════════════════════

/// Build a concrete extraction strategy from a resolved `PromptStrategy`.
///
/// Maps `PromptStrategyKind` to the appropriate strategy implementation:
/// - `MarkdownList` → `MarkdownListStrategy` (7 calls, 1 per domain)
/// - `IterativeDrill` → `IterativeDrillStrategy` (7 enumerate + N×M drill)
pub fn build_strategy(strategy: &PromptStrategy) -> Box<dyn ExtractionStrategy> {
    match strategy.kind {
        PromptStrategyKind::MarkdownList => {
            Box::new(MarkdownListStrategy::new(strategy.max_retries))
        }
        PromptStrategyKind::IterativeDrill => {
            Box::new(IterativeDrillStrategy::new(strategy.max_retries))
        }
    }
}

// ═══════════════════════════════════════════════════════════
// Test mock (available to sibling test modules)
// ═══════════════════════════════════════════════════════════

/// Mock strategy for testing orchestrator post-processing.
///
/// Returns a preset `StrategyOutput` or error. Used by orchestrator tests,
/// security tests, and processor tests that need to bypass real LLM extraction.
#[cfg(test)]
pub struct MockExtractionStrategy {
    pub output: Option<StrategyOutput>,
    pub error: Option<StructuringError>,
}

#[cfg(test)]
impl MockExtractionStrategy {
    pub fn with_output(output: StrategyOutput) -> Self {
        Self {
            output: Some(output),
            error: None,
        }
    }

    pub fn with_error(error: StructuringError) -> Self {
        Self {
            output: None,
            error: Some(error),
        }
    }
}

#[cfg(test)]
impl ExtractionStrategy for MockExtractionStrategy {
    fn extract(
        &self,
        _llm: &dyn LlmClient,
        _model: &str,
        _text: &str,
        _ocr_confidence: f32,
    ) -> Result<StrategyOutput, StructuringError> {
        if let Some(ref err) = self.error {
            return Err(StructuringError::MalformedResponse(format!("{err}")));
        }
        Ok(self.output.clone().expect("MockExtractionStrategy: no output configured"))
    }

    fn name(&self) -> &'static str {
        "mock"
    }
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strategy_output_default_construction() {
        let output = StrategyOutput {
            entities: ExtractedEntities::default(),
            markdown: String::new(),
            document_type: None,
            document_date: None,
            professional: None,
            raw_responses: vec![],
        };
        assert!(output.entities.medications.is_empty());
        assert!(output.markdown.is_empty());
        assert!(output.raw_responses.is_empty());
    }

    #[test]
    fn strategy_output_with_data() {
        let output = StrategyOutput {
            entities: ExtractedEntities::default(),
            markdown: "## Medications\n- Metformin 500mg".into(),
            document_type: Some("prescription".into()),
            document_date: Some("2024-01-15".into()),
            professional: Some(ExtractedProfessional {
                name: "Dr. Chen".into(),
                specialty: Some("GP".into()),
                institution: None,
            }),
            raw_responses: vec!["response1".into(), "response2".into()],
        };
        assert_eq!(output.document_type.as_deref(), Some("prescription"));
        assert_eq!(output.raw_responses.len(), 2);
        assert!(output.markdown.contains("Metformin"));
    }

    #[test]
    fn mock_strategy_returns_output() {
        use crate::pipeline::structuring::ollama::MockLlmClient;

        let output = StrategyOutput {
            entities: ExtractedEntities::default(),
            markdown: "test".into(),
            document_type: None,
            document_date: None,
            professional: None,
            raw_responses: vec![],
        };
        let strategy = MockExtractionStrategy::with_output(output);
        let llm = MockLlmClient::new("unused");

        let result = strategy.extract(&llm, "model", "text", 0.9);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().markdown, "test");
    }

    #[test]
    fn mock_strategy_returns_error() {
        use crate::pipeline::structuring::ollama::MockLlmClient;

        let strategy = MockExtractionStrategy::with_error(
            StructuringError::MalformedResponse("test error".into()),
        );
        let llm = MockLlmClient::new("unused");

        let result = strategy.extract(&llm, "model", "text", 0.9);
        assert!(result.is_err());
    }

    #[test]
    fn mock_strategy_name() {
        let strategy = MockExtractionStrategy::with_output(StrategyOutput {
            entities: ExtractedEntities::default(),
            markdown: String::new(),
            document_type: None,
            document_date: None,
            professional: None,
            raw_responses: vec![],
        });
        assert_eq!(strategy.name(), "mock");
    }

    // ── Factory tests ─────────────────────────────────────

    #[test]
    fn build_strategy_markdown_list() {
        let ps = PromptStrategy {
            kind: PromptStrategyKind::MarkdownList,
            temperature: 0.1,
            max_tokens: 4096,
            max_retries: 1,
            streaming: true,
        };
        let s = build_strategy(&ps);
        assert_eq!(s.name(), "markdown_list");
    }

    #[test]
    fn build_strategy_iterative_drill() {
        let ps = PromptStrategy {
            kind: PromptStrategyKind::IterativeDrill,
            temperature: 0.1,
            max_tokens: 1024,
            max_retries: 1,
            streaming: true,
        };
        let s = build_strategy(&ps);
        assert_eq!(s.name(), "iterative_drill");
    }
}
