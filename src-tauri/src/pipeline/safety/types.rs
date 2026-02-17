use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::pipeline::rag::types::{BoundaryCheck, Citation, QueryType};

/// Outcome of the safety filter pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilteredResponse {
    /// The (possibly rephrased) safe text to display.
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
    /// Response passed all 3 layers without modification.
    Passed,
    /// Response had violations but was successfully rephrased.
    Rephrased {
        original_violations: Vec<Violation>,
    },
    /// Response was blocked — too many or unresolvable violations.
    Blocked {
        violations: Vec<Violation>,
        fallback_message: String,
    },
}

/// A specific safety violation detected by any layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Violation {
    /// Which layer caught this.
    pub layer: FilterLayer,
    /// Category of violation.
    pub category: ViolationCategory,
    /// The specific text span that triggered the violation.
    pub matched_text: String,
    /// Byte offset in original response where violation starts.
    pub offset: usize,
    /// Length of the matched span in bytes.
    pub length: usize,
    /// Human-readable explanation for audit log.
    pub reason: String,
}

/// Which filter layer detected the violation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FilterLayer {
    BoundaryCheck,
    KeywordScan,
    ReportingVsStating,
}

/// Classification of what kind of unsafe content was detected.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ViolationCategory {
    /// Layer 1: boundary_check field missing or invalid.
    BoundaryViolation,
    /// Layer 2: diagnostic language ("you have [condition]").
    DiagnosticLanguage,
    /// Layer 2: prescriptive language ("you should [take/stop]").
    PrescriptiveLanguage,
    /// Layer 2: alarm/emergency language ("dangerous", "immediately").
    AlarmLanguage,
    /// Layer 3: ungrounded claim (states fact without document reference).
    UngroundedClaim,
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
    #[error("Regex compilation failed: {0}")]
    RegexCompilation(String),

    #[error("Input sanitization failed: {0}")]
    SanitizationFailed(String),

    #[error("Rephrasing engine error: {0}")]
    RephrasingFailed(String),

    #[error("Filter pipeline internal error: {0}")]
    InternalError(String),
}

/// The safety filter pipeline — validates every MedGemma response.
pub trait SafetyFilter {
    /// Run all 3 filter layers on a RAG response.
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
        assert_ne!(FilterLayer::KeywordScan, FilterLayer::ReportingVsStating);
    }

    #[test]
    fn violation_category_equality() {
        assert_eq!(ViolationCategory::BoundaryViolation, ViolationCategory::BoundaryViolation);
        assert_ne!(ViolationCategory::DiagnosticLanguage, ViolationCategory::AlarmLanguage);
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
