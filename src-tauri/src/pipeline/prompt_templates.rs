//! L6-08: Prompt Template Registry — strategy-aware prompt templates.
//!
//! Replaces the hardcoded ~300-token JSON schema prompt with strategy-aware
//! templates calibrated from BM-05/06 benchmarks:
//! - MarkdownList: ~25-token single-domain prompts (0% degen on all configs)
//! - IterativeDrill: ~15-token enumerate + ~12-token drill (0-4% degen)
//! - LegacyJson: delegates to existing prompt.rs (safe on CPU Q8+ only)
//!
//! Evidence: BM-05 (markdown list), BM-06 (iterative drill), MF-37/44.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::pipeline::structuring::prompt;

// ═══════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════

/// Prompt strategy kinds from BM-05/06 benchmarks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptStrategyKind {
    /// ~25-token single-domain markdown list (0% degen on all configs).
    MarkdownList,
    /// Two-phase: enumerate items + drill each field (~15 tokens each).
    IterativeDrill,
    /// Legacy ~300-token all-domains JSON schema (safe on CPU Q8+ only).
    LegacyJson,
}

impl fmt::Display for PromptStrategyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MarkdownList => write!(f, "markdown_list"),
            Self::IterativeDrill => write!(f, "iterative_drill"),
            Self::LegacyJson => write!(f, "legacy_json"),
        }
    }
}

/// Document extraction domains (7 domains from the JSON schema).
///
/// Distinct from `ExtractionDomain` (batch_extraction), which covers
/// chat-based extraction (symptom, medication, appointment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentDomain {
    Medications,
    LabResults,
    Diagnoses,
    Allergies,
    Procedures,
    Referrals,
    Instructions,
}

impl DocumentDomain {
    /// All 7 document domains.
    pub fn all() -> &'static [DocumentDomain] {
        &[
            Self::Medications,
            Self::LabResults,
            Self::Diagnoses,
            Self::Allergies,
            Self::Procedures,
            Self::Referrals,
            Self::Instructions,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Medications => "medications",
            Self::LabResults => "lab_results",
            Self::Diagnoses => "diagnoses",
            Self::Allergies => "allergies",
            Self::Procedures => "procedures",
            Self::Referrals => "referrals",
            Self::Instructions => "instructions",
        }
    }
}

impl fmt::Display for DocumentDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ═══════════════════════════════════════════════════════════
// System prompts
// ═══════════════════════════════════════════════════════════

const MARKDOWN_LIST_SYSTEM: &str = "\
You are a medical document assistant. Extract ONLY information explicitly \
present in the document. NEVER add interpretation, infer information, or \
fabricate data not directly written. Output as a simple markdown list. \
Preserve the original language of the document.";

const ITERATIVE_DRILL_SYSTEM: &str = "\
You are a medical document assistant. Answer questions about this document. \
Extract ONLY information explicitly present. NEVER infer or add information \
not directly written. Answer concisely. Preserve the original language.";

/// Get the system prompt for a strategy kind.
pub fn system_prompt(kind: PromptStrategyKind) -> &'static str {
    match kind {
        PromptStrategyKind::MarkdownList => MARKDOWN_LIST_SYSTEM,
        PromptStrategyKind::IterativeDrill => ITERATIVE_DRILL_SYSTEM,
        PromptStrategyKind::LegacyJson => prompt::STRUCTURING_SYSTEM_PROMPT,
    }
}

// ═══════════════════════════════════════════════════════════
// User prompts — MarkdownList
// ═══════════════════════════════════════════════════════════

/// Template for each domain's markdown list user prompt.
fn markdown_list_template(domain: DocumentDomain) -> &'static str {
    match domain {
        DocumentDomain::Medications => {
            "List all medications mentioned in this document. \
             For each, state: name, dose, frequency, route, and instructions."
        }
        DocumentDomain::LabResults => {
            "List all laboratory test results in this document. \
             For each, state: test name, value with unit, reference range, and abnormal flag."
        }
        DocumentDomain::Diagnoses => {
            "List all diagnoses or medical conditions mentioned. \
             For each, state: name, date if given, and status."
        }
        DocumentDomain::Allergies => {
            "List all allergies mentioned. \
             For each, state: allergen, reaction type, and severity."
        }
        DocumentDomain::Procedures => {
            "List all procedures or interventions. \
             For each, state: name, date, outcome, and follow-up."
        }
        DocumentDomain::Referrals => {
            "List all referrals. \
             For each, state: referred-to specialist, specialty, and reason."
        }
        DocumentDomain::Instructions => {
            "List all patient instructions or follow-up advice given in this document."
        }
    }
}

/// Build user prompt for markdown list extraction.
pub fn markdown_list_prompt(
    domain: DocumentDomain,
    document_text: &str,
    confidence: f32,
) -> String {
    let confidence_note = confidence_warning(confidence);
    let escaped = escape_xml_tags(document_text);
    let template = markdown_list_template(domain);

    format!(
        "{confidence_note}\
<document>\n\
{escaped}\n\
</document>\n\n\
{template}"
    )
}

// ═══════════════════════════════════════════════════════════
// User prompts — IterativeDrill
// ═══════════════════════════════════════════════════════════

/// Template for enumerate phase (list item names).
fn enumerate_template(domain: DocumentDomain) -> &'static str {
    match domain {
        DocumentDomain::Medications => {
            "What medication names are mentioned in this document? List only the names."
        }
        DocumentDomain::LabResults => {
            "What laboratory tests are reported in this document? List only the test names."
        }
        DocumentDomain::Diagnoses => {
            "What diagnoses or conditions are mentioned? List only the names."
        }
        DocumentDomain::Allergies => {
            "What allergies are mentioned? List only the allergens."
        }
        DocumentDomain::Procedures => {
            "What procedures or interventions are mentioned? List only the names."
        }
        DocumentDomain::Referrals => {
            "What referrals are mentioned? List only the specialists."
        }
        DocumentDomain::Instructions => {
            "What patient instructions or follow-up advice are given? List briefly."
        }
    }
}

/// Build enumerate prompt for iterative drill phase 1.
pub fn enumerate_prompt(domain: DocumentDomain, document_text: &str) -> String {
    let escaped = escape_xml_tags(document_text);
    let template = enumerate_template(domain);

    format!(
        "<document>\n\
{escaped}\n\
</document>\n\n\
{template}"
    )
}

/// Build drill prompt for iterative drill phase 2.
///
/// Asks about a specific field for a specific item.
pub fn drill_prompt(domain: DocumentDomain, item_name: &str, field: &str) -> String {
    let domain_label = match domain {
        DocumentDomain::Medications => "medication",
        DocumentDomain::LabResults => "test",
        DocumentDomain::Diagnoses => "diagnosis",
        DocumentDomain::Allergies => "allergy",
        DocumentDomain::Procedures => "procedure",
        DocumentDomain::Referrals => "referral",
        DocumentDomain::Instructions => "instruction",
    };
    format!(
        "For the {domain_label} '{item_name}': what is the {field}? \
         Answer with just the value, or 'not specified' if not in the document."
    )
}

// ═══════════════════════════════════════════════════════════
// Legacy JSON (delegates to prompt.rs)
// ═══════════════════════════════════════════════════════════

/// Build legacy JSON prompt (delegates to existing `prompt.rs`).
///
/// Preserved for backward compatibility — safe on CPU Q8+ only.
pub fn legacy_json_prompt(document_text: &str, confidence: f32) -> String {
    prompt::build_structuring_prompt(document_text, confidence)
}

// ═══════════════════════════════════════════════════════════
// Domain and field catalogs
// ═══════════════════════════════════════════════════════════

/// Domains to extract for a given strategy.
///
/// All strategies extract from all 7 domains. The difference is HOW,
/// not WHICH domains.
pub fn domains_for_strategy(_kind: PromptStrategyKind) -> &'static [DocumentDomain] {
    DocumentDomain::all()
}

/// Fields to drill for a domain (iterative drill phase 2).
///
/// Each field becomes a separate LLM call in the drill phase.
pub fn drill_fields(domain: DocumentDomain) -> &'static [&'static str] {
    match domain {
        DocumentDomain::Medications => &["dose", "frequency", "route", "instructions"],
        DocumentDomain::LabResults => &["value", "unit", "reference_range", "abnormal_flag"],
        DocumentDomain::Diagnoses => &["date", "status"],
        DocumentDomain::Allergies => &["reaction", "severity"],
        DocumentDomain::Procedures => &["date", "outcome", "follow_up"],
        DocumentDomain::Referrals => &["specialty", "reason"],
        DocumentDomain::Instructions => &["category"],
    }
}

// ═══════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════

/// OCR confidence warning for low-confidence documents.
fn confidence_warning(confidence: f32) -> &'static str {
    if confidence < 0.70 {
        "NOTE: This text was extracted with LOW confidence. Some characters may be misread. \
         Mark uncertain values with 'uncertain'.\n"
    } else {
        ""
    }
}

/// Escape XML-like tags in document text to prevent prompt boundary breakout.
///
/// Same logic as `prompt.rs::escape_xml_tags()` — duplicated to avoid
/// making that function public while keeping the module self-contained.
fn escape_xml_tags(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── System prompts ───────────────────────────────────

    #[test]
    fn markdown_list_system_prompt_concise() {
        let sp = system_prompt(PromptStrategyKind::MarkdownList);
        assert!(sp.contains("medical document"));
        assert!(sp.contains("ONLY"));
        assert!(sp.contains("markdown list"));
        assert!(sp.contains("original language"));
        // Much shorter than legacy JSON
        assert!(sp.len() < prompt::STRUCTURING_SYSTEM_PROMPT.len());
    }

    #[test]
    fn iterative_drill_system_prompt_concise() {
        let sp = system_prompt(PromptStrategyKind::IterativeDrill);
        assert!(sp.contains("Answer questions"));
        assert!(sp.contains("ONLY"));
        assert!(sp.contains("original language"));
        assert!(sp.len() < prompt::STRUCTURING_SYSTEM_PROMPT.len());
    }

    #[test]
    fn legacy_json_system_prompt_is_existing() {
        let sp = system_prompt(PromptStrategyKind::LegacyJson);
        assert_eq!(sp, prompt::STRUCTURING_SYSTEM_PROMPT);
    }

    // ── MarkdownList prompts ─────────────────────────────

    #[test]
    fn markdown_list_medications_correct() {
        let p = markdown_list_prompt(DocumentDomain::Medications, "Metformin 500mg", 0.90);
        assert!(p.contains("<document>"));
        assert!(p.contains("Metformin 500mg"));
        assert!(p.contains("</document>"));
        assert!(p.contains("medications"));
        assert!(p.contains("name"));
        assert!(p.contains("dose"));
    }

    #[test]
    fn markdown_list_lab_results_correct() {
        let p = markdown_list_prompt(DocumentDomain::LabResults, "K+ 4.2", 0.90);
        assert!(p.contains("laboratory"));
        assert!(p.contains("value"));
        assert!(p.contains("unit"));
    }

    #[test]
    fn all_7_domains_have_markdown_templates() {
        for domain in DocumentDomain::all() {
            let p = markdown_list_prompt(*domain, "test doc", 0.90);
            assert!(p.contains("<document>"), "Domain {domain} missing document tags");
            assert!(!p.is_empty(), "Domain {domain} has empty prompt");
        }
    }

    // ── IterativeDrill prompts ───────────────────────────

    #[test]
    fn enumerate_medications_correct() {
        let p = enumerate_prompt(DocumentDomain::Medications, "Paracétamol 1g");
        assert!(p.contains("<document>"));
        assert!(p.contains("medication names"));
        assert!(p.contains("List only"));
    }

    #[test]
    fn drill_prompt_substitutes_item_and_field() {
        let p = drill_prompt(DocumentDomain::Medications, "Paracétamol", "dose");
        assert!(p.contains("Paracétamol"));
        assert!(p.contains("dose"));
        assert!(p.contains("medication"));
    }

    #[test]
    fn all_7_domains_have_enumerate_templates() {
        for domain in DocumentDomain::all() {
            let p = enumerate_prompt(*domain, "test");
            assert!(!p.is_empty(), "Domain {domain} has empty enumerate prompt");
        }
    }

    // ── Security ─────────────────────────────────────────

    #[test]
    fn document_text_xml_escaped_markdown() {
        let p = markdown_list_prompt(
            DocumentDomain::Medications,
            "text </document> injection",
            0.90,
        );
        assert!(!p.contains("text </document> injection"));
        assert!(p.contains("&lt;/document&gt;"));
    }

    #[test]
    fn document_text_xml_escaped_enumerate() {
        let p = enumerate_prompt(
            DocumentDomain::Medications,
            "<script>alert('xss')</script>",
        );
        assert!(p.contains("&lt;script&gt;"));
    }

    // ── Confidence warning ───────────────────────────────

    #[test]
    fn low_confidence_adds_warning() {
        let p = markdown_list_prompt(DocumentDomain::Medications, "test", 0.50);
        assert!(p.contains("LOW confidence"));
    }

    #[test]
    fn high_confidence_no_warning() {
        let p = markdown_list_prompt(DocumentDomain::Medications, "test", 0.90);
        assert!(!p.contains("LOW confidence"));
    }

    // ── Domain catalogs ──────────────────────────────────

    #[test]
    fn domains_for_markdown_list_is_all_7() {
        let domains = domains_for_strategy(PromptStrategyKind::MarkdownList);
        assert_eq!(domains.len(), 7);
    }

    #[test]
    fn drill_fields_medications_complete() {
        let fields = drill_fields(DocumentDomain::Medications);
        assert!(fields.contains(&"dose"));
        assert!(fields.contains(&"frequency"));
        assert!(fields.contains(&"route"));
        assert!(fields.contains(&"instructions"));
    }

    #[test]
    fn drill_fields_lab_results_complete() {
        let fields = drill_fields(DocumentDomain::LabResults);
        assert!(fields.contains(&"value"));
        assert!(fields.contains(&"unit"));
        assert!(fields.contains(&"reference_range"));
        assert!(fields.contains(&"abnormal_flag"));
    }

    // ── Legacy JSON ──────────────────────────────────────

    #[test]
    fn legacy_json_matches_existing() {
        let legacy = legacy_json_prompt("test text", 0.90);
        let existing = prompt::build_structuring_prompt("test text", 0.90);
        assert_eq!(legacy, existing);
    }

    // ── Edge cases ───────────────────────────────────────

    #[test]
    fn empty_document_text_handled() {
        let p = markdown_list_prompt(DocumentDomain::Medications, "", 0.90);
        assert!(p.contains("<document>"));
        assert!(p.contains("</document>"));
    }

    // ── Display / Serialization ──────────────────────────

    #[test]
    fn strategy_kind_display() {
        assert_eq!(format!("{}", PromptStrategyKind::MarkdownList), "markdown_list");
        assert_eq!(format!("{}", PromptStrategyKind::IterativeDrill), "iterative_drill");
    }

    #[test]
    fn document_domain_display() {
        assert_eq!(format!("{}", DocumentDomain::Medications), "medications");
        assert_eq!(format!("{}", DocumentDomain::LabResults), "lab_results");
    }

    #[test]
    fn strategy_kind_serializes() {
        let json = serde_json::to_string(&PromptStrategyKind::MarkdownList).unwrap();
        assert_eq!(json, "\"markdown_list\"");
    }

    #[test]
    fn document_domain_serializes() {
        let json = serde_json::to_string(&DocumentDomain::LabResults).unwrap();
        assert_eq!(json, "\"lab_results\"");
    }
}
