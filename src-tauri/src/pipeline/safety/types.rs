use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::pipeline::rag::types::{BoundaryCheck, Citation, QueryType};

/// Outcome of the safety filter pipeline.
///
/// The filter validates only the model-generated `BoundaryCheck` field.
/// Medical tone, grounding, and urgency are handled by the SLM's system prompt —
/// NOT by keyword pattern matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilteredResponse {
    /// The safe text to display (unchanged from RAG — SLM controls tone).
    pub text: String,
    /// Original citations passed through from RAG.
    pub citations: Vec<Citation>,
    /// Confidence from RAG (passed through).
    pub confidence: f32,
    /// Query type from RAG (passed through).
    pub query_type: QueryType,
    /// Validated boundary check.
    pub boundary_check: BoundaryCheck,
    /// Filter outcome summary.
    pub filter_outcome: FilterOutcome,
}

/// What the filter decided.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FilterOutcome {
    /// Response passed boundary check — safe to display.
    Passed,
    /// Response was blocked — BoundaryCheck::OutOfBounds.
    Blocked {
        violations: Vec<Violation>,
        fallback_message: String,
    },
}

/// A specific safety violation detected by the boundary check layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Violation {
    /// Which layer caught this.
    pub layer: FilterLayer,
    /// Category of violation.
    pub category: ViolationCategory,
    /// Human-readable explanation for audit log.
    pub reason: String,
}

/// Which filter layer detected the violation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FilterLayer {
    /// Layer 1: Model-generated boundary check validation.
    BoundaryCheck,
}

/// Classification of what kind of unsafe content was detected.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ViolationCategory {
    /// BoundaryCheck::OutOfBounds — response is outside medical document scope.
    BoundaryViolation,
}

/// Result of input sanitization (pre-LLM).
#[derive(Debug, Clone)]
pub struct SanitizedInput {
    /// The cleaned, safe query text.
    pub text: String,
    /// Whether any modifications were made.
    pub was_modified: bool,
    /// What was stripped (for audit, no patient data).
    pub modifications: Vec<InputModification>,
}

/// A modification made during input sanitization.
#[derive(Debug, Clone)]
pub struct InputModification {
    pub kind: InputModificationKind,
    pub description: String,
}

/// Types of input sanitization applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputModificationKind {
    InvisibleUnicodeRemoved,
    InjectionPatternRemoved,
    ExcessiveLengthTruncated,
    ControlCharacterRemoved,
}

/// Safety filter errors.
#[derive(Error, Debug)]
pub enum SafetyError {
    #[error("Input sanitization failed: {0}")]
    SanitizationFailed(String),

    #[error("Filter pipeline internal error: {0}")]
    InternalError(String),
}

/// The safety filter pipeline — validates every MedGemma response.
///
/// Only checks the model-generated `BoundaryCheck` field. Medical tone,
/// grounding, and urgency are the SLM's responsibility via its system prompt.
pub trait SafetyFilter {
    /// Check the boundary layer on a RAG response.
    fn filter_response(
        &self,
        response: &crate::pipeline::rag::types::RagResponse,
    ) -> Result<FilteredResponse, SafetyError>;

    /// Sanitize patient input before it reaches MedGemma.
    fn sanitize_input(
        &self,
        raw_query: &str,
    ) -> Result<SanitizedInput, SafetyError>;
}

/// Maximum regeneration attempts the RAG orchestrator should make
/// when Layer 1 rejects for boundary violation.
pub const MAX_BOUNDARY_REGENERATION_ATTEMPTS: usize = 2;

/// Fallback message when boundary check fails after all retries.
pub const BOUNDARY_FALLBACK_MESSAGE: &str =
    "I can help you understand what your medical documents say. \
     Could you rephrase your question about your documents?";

/// I18N-06: Get boundary fallback message in the given language.
pub fn boundary_fallback_message_i18n(lang: &str) -> &'static str {
    match lang {
        "fr" => "Je peux vous aider à comprendre vos documents médicaux. \
                 Pourriez-vous reformuler votre question concernant vos documents ?",
        "de" => "Ich kann Ihnen helfen, Ihre medizinischen Dokumente zu verstehen. \
                 Könnten Sie Ihre Frage zu Ihren Dokumenten umformulieren?",
        _ => BOUNDARY_FALLBACK_MESSAGE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_outcome_variants_serialize() {
        let passed = FilterOutcome::Passed;
        let json = serde_json::to_string(&passed).unwrap();
        assert!(json.contains("Passed"));

        let blocked = FilterOutcome::Blocked {
            violations: vec![],
            fallback_message: "test".into(),
        };
        let json = serde_json::to_string(&blocked).unwrap();
        assert!(json.contains("Blocked"));
    }

    #[test]
    fn filter_layer_equality() {
        assert_eq!(FilterLayer::BoundaryCheck, FilterLayer::BoundaryCheck);
    }

    #[test]
    fn violation_category_equality() {
        assert_eq!(ViolationCategory::BoundaryViolation, ViolationCategory::BoundaryViolation);
    }

    #[test]
    fn input_modification_kind_equality() {
        assert_eq!(
            InputModificationKind::InvisibleUnicodeRemoved,
            InputModificationKind::InvisibleUnicodeRemoved
        );
        assert_ne!(
            InputModificationKind::InjectionPatternRemoved,
            InputModificationKind::ControlCharacterRemoved
        );
    }

    // I18N-06: Boundary fallback message translations

    #[test]
    fn boundary_fallback_en_default() {
        let msg = boundary_fallback_message_i18n("en");
        assert_eq!(msg, BOUNDARY_FALLBACK_MESSAGE);
    }

    #[test]
    fn boundary_fallback_fr() {
        let msg = boundary_fallback_message_i18n("fr");
        assert!(msg.contains("documents médicaux"), "French boundary: {msg}");
        assert!(msg.contains("reformuler"));
    }

    #[test]
    fn boundary_fallback_de() {
        let msg = boundary_fallback_message_i18n("de");
        assert!(msg.contains("medizinischen Dokumente"), "German boundary: {msg}");
        assert!(msg.contains("umformulieren"));
    }

    #[test]
    fn boundary_fallback_unknown_lang() {
        let msg = boundary_fallback_message_i18n("ja");
        assert_eq!(msg, BOUNDARY_FALLBACK_MESSAGE);
    }
}
