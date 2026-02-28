//! STR-01: IterativeDrill extraction strategy.
//!
//! Two-phase extraction:
//! 1. Enumerate: 7 calls (one per domain) to list item names
//! 2. Drill: N×M calls to extract each field for each item
//!
//! Most thorough strategy (12/12 lab tests vs 5/12 for MarkdownList in BM-06).
//! 0-4% degeneration. Best for NightBatch where time is not constrained.
//!
//! Evidence: BM-06 (iterative drill), MF-44 (prompt complexity dominant).

use std::collections::HashMap;

use crate::pipeline::prompt_templates::{
    self, DocumentDomain, PromptStrategyKind,
};
use crate::pipeline::structuring::extraction_strategy::{ExtractionStrategy, StrategyOutput};
use crate::pipeline::structuring::types::{
    ExtractedAllergy, ExtractedDiagnosis, ExtractedEntities, ExtractedInstruction,
    ExtractedLabResult, ExtractedMedication, ExtractedProcedure, ExtractedReferral,
    LlmClient,
};
use crate::pipeline::structuring::StructuringError;

// ═══════════════════════════════════════════════════════════
// Strategy implementation
// ═══════════════════════════════════════════════════════════

/// IterativeDrill extraction strategy.
///
/// Phase 1: Enumerate item names per domain (7 calls).
/// Phase 2: Drill each field for each item (N × M calls per domain).
/// Phase 3: Assemble typed entities from collected field values.
pub struct IterativeDrillStrategy {
    max_retries: u32,
}

impl IterativeDrillStrategy {
    pub fn new(max_retries: u32) -> Self {
        Self { max_retries }
    }
}

impl ExtractionStrategy for IterativeDrillStrategy {
    fn extract(
        &self,
        llm: &dyn LlmClient,
        model: &str,
        text: &str,
        _ocr_confidence: f32,
    ) -> Result<StrategyOutput, StructuringError> {
        let system = prompt_templates::system_prompt(PromptStrategyKind::IterativeDrill);
        let domains = DocumentDomain::all();

        let mut entities = ExtractedEntities::default();
        let mut markdown_sections = Vec::new();
        let mut raw_responses = Vec::new();

        for &domain in domains {
            // Phase 1: Enumerate items for this domain
            let enumerate_prompt = prompt_templates::enumerate_prompt(domain, text);
            let enumerate_response = match call_with_retry(
                llm, model, &enumerate_prompt, system, self.max_retries,
            ) {
                Ok(resp) => resp,
                Err(e) => {
                    tracing::warn!(
                        domain = %domain,
                        error = %e,
                        "IterativeDrill: enumerate failed, skipping domain"
                    );
                    continue;
                }
            };

            raw_responses.push(enumerate_response.clone());
            let item_names = parse_enumerate_response(&enumerate_response);

            if item_names.is_empty() {
                continue;
            }

            // Phase 2: Drill each item × each field
            let fields = prompt_templates::drill_fields(domain);
            let mut domain_markdown = Vec::new();

            for item_name in &item_names {
                let mut field_values: HashMap<String, String> = HashMap::new();
                let mut item_markdown = format!("- **{item_name}**");

                for &field in fields {
                    let drill = prompt_templates::drill_prompt(domain, item_name, field);
                    match call_with_retry(llm, model, &drill, system, self.max_retries) {
                        Ok(resp) => {
                            raw_responses.push(resp.clone());
                            let value = resp.trim().to_string();
                            if !value.is_empty() && !is_not_specified(&value) {
                                field_values.insert(field.to_string(), value.clone());
                                item_markdown.push_str(&format!("\n  - {field}: {value}"));
                            }
                        }
                        Err(e) => {
                            tracing::debug!(
                                domain = %domain,
                                item = %item_name,
                                field = %field,
                                error = %e,
                                "IterativeDrill: drill failed, skipping field"
                            );
                        }
                    }
                }

                domain_markdown.push(item_markdown);

                // Assemble entity from collected fields
                assemble_entity(domain, item_name, &field_values, &mut entities);
            }

            if !domain_markdown.is_empty() {
                let header = domain_header(domain);
                markdown_sections.push(format!("## {header}\n{}", domain_markdown.join("\n")));
            }
        }

        let markdown = markdown_sections.join("\n\n");

        Ok(StrategyOutput {
            entities,
            markdown,
            document_type: None,
            document_date: None,
            professional: None,
            raw_responses,
        })
    }

    fn name(&self) -> &'static str {
        "iterative_drill"
    }
}

// ═══════════════════════════════════════════════════════════
// Enumerate response parser
// ═══════════════════════════════════════════════════════════

/// Parse enumerate response into a list of item names.
///
/// Handles bullet lists, comma-separated lists, and newline-separated lists.
/// Filters out empty items and "not specified"/"none" responses.
pub fn parse_enumerate_response(response: &str) -> Vec<String> {
    let trimmed = response.trim();
    if trimmed.is_empty() || is_not_specified(trimmed) {
        return vec![];
    }

    let mut items = Vec::new();

    for line in trimmed.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Strip bullet markers
        let cleaned = strip_bullet_marker(line);

        // Handle comma-separated on a single line
        if cleaned.contains(',') && !cleaned.contains('\n') {
            for part in cleaned.split(',') {
                let name = part.trim().replace("**", "");
                if !name.is_empty() && !is_not_specified(&name) {
                    items.push(name);
                }
            }
        } else {
            let name = cleaned.replace("**", "").trim().to_string();
            if !name.is_empty() && !is_not_specified(&name) {
                items.push(name);
            }
        }
    }

    items
}

/// Strip bullet marker from a line.
fn strip_bullet_marker(line: &str) -> &str {
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix("- ") {
        return rest;
    }
    if let Some(rest) = trimmed.strip_prefix("* ") {
        return rest;
    }
    if let Some(rest) = trimmed.strip_prefix("• ") {
        return rest;
    }
    // Numbered: "1. text"
    if let Some(dot_pos) = trimmed.find(". ") {
        let prefix = &trimmed[..dot_pos];
        if prefix.chars().all(|c| c.is_ascii_digit()) {
            return &trimmed[dot_pos + 2..];
        }
    }
    trimmed
}

// ═══════════════════════════════════════════════════════════
// Entity assemblers
// ═══════════════════════════════════════════════════════════

/// Assemble a typed entity from drilled field values and add to entities.
fn assemble_entity(
    domain: DocumentDomain,
    name: &str,
    fields: &HashMap<String, String>,
    entities: &mut ExtractedEntities,
) {
    match domain {
        DocumentDomain::Medications => {
            entities.medications.push(assemble_medication(name, fields));
        }
        DocumentDomain::LabResults => {
            entities.lab_results.push(assemble_lab_result(name, fields));
        }
        DocumentDomain::Diagnoses => {
            entities.diagnoses.push(assemble_diagnosis(name, fields));
        }
        DocumentDomain::Allergies => {
            entities.allergies.push(assemble_allergy(name, fields));
        }
        DocumentDomain::Procedures => {
            entities.procedures.push(assemble_procedure(name, fields));
        }
        DocumentDomain::Referrals => {
            entities.referrals.push(assemble_referral(name, fields));
        }
        DocumentDomain::Instructions => {
            entities.instructions.push(assemble_instruction(name, fields));
        }
    }
}

fn assemble_medication(name: &str, fields: &HashMap<String, String>) -> ExtractedMedication {
    ExtractedMedication {
        generic_name: Some(name.to_string()),
        brand_name: None,
        dose: fields.get("dose").cloned().unwrap_or_default(),
        frequency: fields.get("frequency").cloned().unwrap_or_default(),
        frequency_type: String::new(),
        route: fields.get("route").cloned().unwrap_or_default(),
        reason: None,
        instructions: fields
            .get("instructions")
            .map(|i| vec![i.clone()])
            .unwrap_or_default(),
        is_compound: false,
        compound_ingredients: vec![],
        tapering_steps: vec![],
        max_daily_dose: None,
        condition: None,
        confidence: 0.0,
    }
}

fn assemble_lab_result(name: &str, fields: &HashMap<String, String>) -> ExtractedLabResult {
    let value_str = fields.get("value").cloned();
    let value = value_str
        .as_deref()
        .and_then(|s| s.replace(',', ".").parse::<f64>().ok());
    let value_text = if value.is_none() { value_str } else { None };

    ExtractedLabResult {
        test_name: name.to_string(),
        test_code: None,
        value,
        value_text,
        unit: fields.get("unit").cloned(),
        reference_range_low: None,
        reference_range_high: None,
        reference_range_text: fields.get("reference_range").cloned(),
        abnormal_flag: fields.get("abnormal_flag").cloned(),
        collection_date: None,
        confidence: 0.0,
    }
}

fn assemble_diagnosis(name: &str, fields: &HashMap<String, String>) -> ExtractedDiagnosis {
    ExtractedDiagnosis {
        name: name.to_string(),
        icd_code: None,
        date: fields.get("date").cloned(),
        status: fields.get("status").cloned().unwrap_or_default(),
        confidence: 0.0,
    }
}

fn assemble_allergy(name: &str, fields: &HashMap<String, String>) -> ExtractedAllergy {
    ExtractedAllergy {
        allergen: name.to_string(),
        reaction: fields.get("reaction").cloned(),
        severity: fields.get("severity").cloned(),
        confidence: 0.0,
    }
}

fn assemble_procedure(name: &str, fields: &HashMap<String, String>) -> ExtractedProcedure {
    let follow_up = fields.get("follow_up").cloned();
    let follow_up_required = follow_up.as_ref().map_or(false, |v| {
        let lower = v.to_lowercase();
        lower.contains("yes") || lower.contains("required") || lower.contains("oui")
    });

    ExtractedProcedure {
        name: name.to_string(),
        date: fields.get("date").cloned(),
        outcome: fields.get("outcome").cloned(),
        follow_up_required,
        follow_up_date: None,
        confidence: 0.0,
    }
}

fn assemble_referral(name: &str, fields: &HashMap<String, String>) -> ExtractedReferral {
    ExtractedReferral {
        referred_to: name.to_string(),
        specialty: fields.get("specialty").cloned(),
        reason: fields.get("reason").cloned(),
        confidence: 0.0,
    }
}

fn assemble_instruction(name: &str, fields: &HashMap<String, String>) -> ExtractedInstruction {
    ExtractedInstruction {
        text: name.to_string(),
        category: fields.get("category").cloned().unwrap_or_default(),
    }
}

// ═══════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════

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
                    tracing::debug!(attempt, error = %e, "IterativeDrill: retrying");
                }
                last_error = Some(e);
            }
        }
    }
    Err(last_error.unwrap_or_else(|| {
        StructuringError::MalformedResponse("No attempts made".into())
    }))
}

fn is_not_specified(value: &str) -> bool {
    let lower = value.to_lowercase().trim().to_string();
    matches!(
        lower.as_str(),
        "not specified" | "n/a" | "none" | "not mentioned" | "not available"
            | "not provided" | "unknown" | "non spécifié" | "aucun" | "néant"
    )
}

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
    use std::sync::atomic::{AtomicUsize, Ordering};

    // ── Enumerate parser tests ───────────────────────────

    #[test]
    fn enumerate_bullet_list() {
        let items = parse_enumerate_response("- Metformin\n- Lisinopril\n- Aspirin");
        assert_eq!(items, vec!["Metformin", "Lisinopril", "Aspirin"]);
    }

    #[test]
    fn enumerate_numbered_list() {
        let items = parse_enumerate_response("1. Potassium\n2. Glucose");
        assert_eq!(items, vec!["Potassium", "Glucose"]);
    }

    #[test]
    fn enumerate_comma_separated() {
        let items = parse_enumerate_response("Metformin, Lisinopril, Aspirin");
        assert_eq!(items, vec!["Metformin", "Lisinopril", "Aspirin"]);
    }

    #[test]
    fn enumerate_none_response() {
        assert!(parse_enumerate_response("none").is_empty());
        assert!(parse_enumerate_response("Not specified").is_empty());
        assert!(parse_enumerate_response("").is_empty());
    }

    #[test]
    fn enumerate_bold_names() {
        let items = parse_enumerate_response("- **Metformin**\n- **Aspirin**");
        assert_eq!(items, vec!["Metformin", "Aspirin"]);
    }

    // ── Assembler tests ──────────────────────────────────

    #[test]
    fn assemble_medication_full() {
        let mut fields = HashMap::new();
        fields.insert("dose".into(), "500mg".into());
        fields.insert("frequency".into(), "twice daily".into());
        fields.insert("route".into(), "oral".into());

        let med = assemble_medication("Metformin", &fields);
        assert_eq!(med.generic_name.as_deref(), Some("Metformin"));
        assert_eq!(med.dose, "500mg");
        assert_eq!(med.frequency, "twice daily");
        assert_eq!(med.route, "oral");
    }

    #[test]
    fn assemble_medication_minimal() {
        let med = assemble_medication("Aspirin", &HashMap::new());
        assert_eq!(med.generic_name.as_deref(), Some("Aspirin"));
        assert!(med.dose.is_empty());
    }

    #[test]
    fn assemble_lab_result_numeric() {
        let mut fields = HashMap::new();
        fields.insert("value".into(), "4.2".into());
        fields.insert("unit".into(), "mmol/L".into());

        let lab = assemble_lab_result("Potassium", &fields);
        assert_eq!(lab.test_name, "Potassium");
        assert_eq!(lab.value, Some(4.2));
        assert!(lab.value_text.is_none());
        assert_eq!(lab.unit.as_deref(), Some("mmol/L"));
    }

    #[test]
    fn assemble_lab_result_text_value() {
        let mut fields = HashMap::new();
        fields.insert("value".into(), "negative".into());

        let lab = assemble_lab_result("Culture", &fields);
        assert!(lab.value.is_none());
        assert_eq!(lab.value_text.as_deref(), Some("negative"));
    }

    // ── Full strategy tests ──────────────────────────────

    /// Mock LLM that responds appropriately to enumerate vs drill prompts.
    struct DrillAwareMockLlm {
        call_count: AtomicUsize,
    }

    impl LlmClient for DrillAwareMockLlm {
        fn generate(
            &self,
            _model: &str,
            prompt: &str,
            _system: &str,
        ) -> Result<String, StructuringError> {
            self.call_count.fetch_add(1, Ordering::Relaxed);
            let lower = prompt.to_lowercase();

            // Enumerate prompts contain "list only" or "what ... mentioned"
            if lower.contains("list only") || lower.contains("what ") && lower.contains("mentioned") {
                if lower.contains("medication") {
                    return Ok("- Metformin".into());
                }
                // All other domains: no items
                return Ok("None".into());
            }

            // Drill prompts contain "what is the"
            if lower.contains("what is the") {
                if lower.contains("dose") {
                    return Ok("500mg".into());
                }
                if lower.contains("frequency") {
                    return Ok("twice daily".into());
                }
                if lower.contains("route") {
                    return Ok("oral".into());
                }
                if lower.contains("instructions") {
                    return Ok("Take with food".into());
                }
                return Ok("not specified".into());
            }

            Ok(String::new())
        }

        fn is_model_available(&self, _: &str) -> Result<bool, StructuringError> {
            Ok(true)
        }
        fn list_models(&self) -> Result<Vec<String>, StructuringError> {
            Ok(vec![])
        }
    }

    #[test]
    fn full_iterative_drill() {
        let llm = DrillAwareMockLlm {
            call_count: AtomicUsize::new(0),
        };
        let strategy = IterativeDrillStrategy::new(0);

        let output = strategy
            .extract(&llm, "medgemma:latest", "Test document about Metformin", 0.90)
            .unwrap();

        // Only medications domain should have items (others return "None")
        assert_eq!(output.entities.medications.len(), 1);
        assert_eq!(
            output.entities.medications[0].generic_name.as_deref(),
            Some("Metformin")
        );
        assert_eq!(output.entities.medications[0].dose, "500mg");
        assert_eq!(output.entities.medications[0].frequency, "twice daily");

        // Other domains should be empty
        assert!(output.entities.lab_results.is_empty());
        assert!(output.entities.diagnoses.is_empty());

        // Markdown should have Medications section
        assert!(output.markdown.contains("## Medications"));
        assert!(output.markdown.contains("Metformin"));
    }

    #[test]
    fn empty_enumerate_skips_drill() {
        struct EmptyEnumerateLlm;
        impl LlmClient for EmptyEnumerateLlm {
            fn generate(
                &self,
                _model: &str,
                _prompt: &str,
                _system: &str,
            ) -> Result<String, StructuringError> {
                Ok("None".into())
            }
            fn is_model_available(&self, _: &str) -> Result<bool, StructuringError> {
                Ok(true)
            }
            fn list_models(&self) -> Result<Vec<String>, StructuringError> {
                Ok(vec![])
            }
        }

        let strategy = IterativeDrillStrategy::new(0);
        let output = strategy
            .extract(&EmptyEnumerateLlm, "model", "text", 0.90)
            .unwrap();

        assert!(output.entities.medications.is_empty());
        assert!(output.entities.lab_results.is_empty());
    }

    #[test]
    fn strategy_name() {
        let strategy = IterativeDrillStrategy::new(1);
        assert_eq!(strategy.name(), "iterative_drill");
    }

    #[test]
    fn not_specified_drill_values_excluded() {
        let mut fields = HashMap::new();
        fields.insert("dose".into(), "500mg".into());
        // "not specified" values should not be in fields because the strategy filters them

        let med = assemble_medication("Test", &fields);
        assert_eq!(med.dose, "500mg");
        assert!(med.route.is_empty()); // Not in fields → default empty
    }
}
