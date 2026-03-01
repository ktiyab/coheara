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
use std::path::Path;

use crate::butler_service::{SessionError, ValidatedOutput, VisionSession};
use crate::pipeline::domain_contracts::{contract_for_document_domain, InputMode};
use crate::pipeline::prompt_templates::{
    self, DocumentDomain, PromptStrategyKind,
};
use crate::pipeline::safety::output_sanitize::sanitize_llm_output;
use crate::pipeline::structuring::extraction_strategy::{ExtractionStrategy, StrategyOutput};
use crate::pipeline::structuring::types::{
    ExtractedAllergy, ExtractedDiagnosis, ExtractedEntities, ExtractedInstruction,
    ExtractedLabResult, ExtractedMedication, ExtractedProcedure, ExtractedReferral,
    LlmClient, VisionClient,
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

impl IterativeDrillStrategy {
    /// Extract entities from an image using iterative vision Q&A.
    ///
    /// Phase 0: For each domain, enumerate items visible in the image.
    /// Phase 1: For each item, drill each field with focused single-question prompts.
    ///
    /// Uses `VisionSession` for enforced sanitize + quality gate on every call.
    /// Handles degeneration gracefully: uses partial output if available, skips field otherwise.
    ///
    /// C4: When `progress_path` is Some, writes a JSONL entry after each LLM call.
    /// This makes extraction progression visible in real-time (`tail -f`).
    ///
    /// 09-CAE: When `user_doc_type` is Some, only relevant domains are queried
    /// (e.g., LabReport → LabResults + Diagnoses). When None, all 7 domains
    /// run (legacy/chat fallback).
    pub fn extract_from_image(
        &self,
        session: &dyn VisionSession,
        client: &dyn VisionClient,
        images: &[String],
        system_prompt: &str,
        progress_path: Option<&Path>,
        user_doc_type: Option<crate::pipeline::extraction::vision_classifier::UserDocumentType>,
    ) -> Result<StrategyOutput, SessionError> {
        use crate::pipeline::diagnostic;

        // 09-CAE: Filter domains by user category, or run all 7 (legacy/chat fallback)
        let domains: &[DocumentDomain] = match user_doc_type {
            Some(dt) => crate::pipeline::domain_contracts::domains_for_document_type(dt),
            None => DocumentDomain::all(),
        };
        let mut entities = ExtractedEntities::default();
        let mut markdown_sections = Vec::new();
        let mut raw_responses = Vec::new();
        let mut call_num: u32 = 0;
        let extraction_start = std::time::Instant::now();

        for (domain_idx, &domain) in domains.iter().enumerate() {
            let contract = contract_for_document_domain(domain);

            // Phase 0: Enumerate items for this domain via vision
            // 09-CAE: Category context for focused prompts
            let cat_ctx = user_doc_type
                .map(crate::pipeline::domain_contracts::category_context)
                .unwrap_or("");
            let enumerate_prompt = contract.enumerate_prompt_for(InputMode::Vision, "", cat_ctx);
            call_num += 1;
            let call_start = std::time::Instant::now();

            let enumerate_result = match session.chat_with_images(
                client,
                &enumerate_prompt,
                images,
                Some(system_prompt),
            ) {
                Ok(result) => {
                    Self::log_progress(progress_path, call_num, "enumerate", domain_idx, &domain.to_string(), "", "", "ok", &result.text, result.tokens_generated, call_start.elapsed().as_millis());
                    result
                }
                Err(SessionError::Degeneration { partial_output, tokens_before_abort, pattern, .. }) => {
                    Self::log_progress(progress_path, call_num, "enumerate", domain_idx, &domain.to_string(), "", "", &format!("degen:{pattern}"), &partial_output, tokens_before_abort, call_start.elapsed().as_millis());
                    if partial_output.trim().is_empty() {
                        tracing::warn!(domain = %domain, "Vision enumerate degenerated with no output, skipping");
                        continue;
                    }
                    tracing::debug!(domain = %domain, "Vision enumerate degenerated, using partial output");
                    ValidatedOutput {
                        text: partial_output,
                        tokens_generated: 0,
                        model: session.model().to_string(),
                    }
                }
                Err(SessionError::QualityGate { raw_output, reason, .. }) => {
                    Self::log_progress(progress_path, call_num, "enumerate", domain_idx, &domain.to_string(), "", "", &format!("quality_gate:{reason}"), &raw_output, 0, call_start.elapsed().as_millis());
                    if raw_output.trim().is_empty() {
                        continue;
                    }
                    ValidatedOutput {
                        text: raw_output,
                        tokens_generated: 0,
                        model: session.model().to_string(),
                    }
                }
                Err(e) => {
                    Self::log_progress(progress_path, call_num, "enumerate", domain_idx, &domain.to_string(), "", "", &format!("error:{e}"), "", 0, call_start.elapsed().as_millis());
                    return Err(e);
                }
            };

            raw_responses.push(enumerate_result.text.clone());
            let item_names = parse_enumerate_response(&enumerate_result.text);

            if item_names.is_empty() {
                continue;
            }

            // Phase 1: Drill each item × each field via vision
            let mut domain_markdown = Vec::new();

            for item_name in &item_names {
                let mut field_values: HashMap<String, String> = HashMap::new();
                let mut item_markdown = format!("- **{item_name}**");

                for field_desc in contract.fields {
                    let drill_prompt =
                        contract.drill_prompt_for(InputMode::Vision, item_name, field_desc, "");
                    call_num += 1;
                    let call_start = std::time::Instant::now();

                    match session.chat_with_images(
                        client,
                        &drill_prompt,
                        images,
                        Some(system_prompt),
                    ) {
                        Ok(result) => {
                            Self::log_progress(progress_path, call_num, "drill", domain_idx, &domain.to_string(), item_name, field_desc.name, "ok", &result.text, result.tokens_generated, call_start.elapsed().as_millis());
                            raw_responses.push(result.text.clone());
                            let value = result.text.trim().to_string();
                            if !value.is_empty() && !is_not_specified(&value) {
                                field_values
                                    .insert(field_desc.name.to_string(), value.clone());
                                item_markdown
                                    .push_str(&format!("\n  - {}: {value}", field_desc.name));
                            }
                        }
                        Err(SessionError::Degeneration {
                            partial_output, tokens_before_abort, pattern, ..
                        }) => {
                            Self::log_progress(progress_path, call_num, "drill", domain_idx, &domain.to_string(), item_name, field_desc.name, &format!("degen:{pattern}"), &partial_output, tokens_before_abort, call_start.elapsed().as_millis());
                            let trimmed = partial_output.trim().to_string();
                            if !trimmed.is_empty() && !is_not_specified(&trimmed) {
                                field_values
                                    .insert(field_desc.name.to_string(), trimmed.clone());
                                item_markdown
                                    .push_str(&format!("\n  - {}: {trimmed}", field_desc.name));
                            }
                        }
                        Err(SessionError::QualityGate { reason, .. }) => {
                            Self::log_progress(progress_path, call_num, "drill", domain_idx, &domain.to_string(), item_name, field_desc.name, &format!("quality_gate:{reason}"), "", 0, call_start.elapsed().as_millis());
                            tracing::debug!(
                                domain = %domain,
                                item = %item_name,
                                field = %field_desc.name,
                                "Vision drill quality gate rejected, skipping field"
                            );
                        }
                        Err(e) => {
                            Self::log_progress(progress_path, call_num, "drill", domain_idx, &domain.to_string(), item_name, field_desc.name, &format!("error:{e}"), "", 0, call_start.elapsed().as_millis());
                            return Err(e);
                        }
                    }
                }

                domain_markdown.push(item_markdown);
                assemble_entity(domain, item_name, &field_values, &mut entities);
            }

            if !domain_markdown.is_empty() {
                let header = domain_header(domain);
                markdown_sections
                    .push(format!("## {header}\n{}", domain_markdown.join("\n")));
            }
        }

        let markdown = markdown_sections.join("\n\n");

        // C4: Write summary entry
        if let Some(path) = progress_path {
            if let Some(dir) = path.parent() {
                diagnostic::dump_jsonl_append(dir, path.file_name().unwrap().to_str().unwrap_or("progress.jsonl"), &serde_json::json!({
                    "call": call_num + 1,
                    "phase": "summary",
                    "total_calls": call_num,
                    "total_elapsed_ms": extraction_start.elapsed().as_millis() as u64,
                    "lab_results": entities.lab_results.len(),
                    "medications": entities.medications.len(),
                    "diagnoses": entities.diagnoses.len(),
                    "allergies": entities.allergies.len(),
                    "procedures": entities.procedures.len(),
                    "referrals": entities.referrals.len(),
                    "instructions": entities.instructions.len(),
                }));
            }
        }

        Ok(StrategyOutput {
            entities,
            markdown,
            document_type: None,
            document_date: None,
            professional: None,
            raw_responses,
        })
    }

    /// C4: Append a progress entry to the JSONL diagnostic file.
    #[allow(clippy::too_many_arguments)]
    fn log_progress(
        progress_path: Option<&Path>,
        call: u32,
        phase: &str,
        domain_idx: usize,
        domain: &str,
        item: &str,
        field: &str,
        status: &str,
        response: &str,
        tokens: usize,
        elapsed_ms: u128,
    ) {
        let Some(path) = progress_path else { return };
        let Some(dir) = path.parent() else { return };
        let Some(filename) = path.file_name().and_then(|f| f.to_str()) else { return };

        use crate::pipeline::diagnostic;

        let mut entry = serde_json::json!({
            "call": call,
            "phase": phase,
            "domain_idx": domain_idx,
            "domain": domain,
            "status": status,
            "tokens": tokens,
            "elapsed_ms": elapsed_ms as u64,
        });

        if !item.is_empty() {
            entry["item"] = serde_json::json!(item);
        }
        if !field.is_empty() {
            entry["field"] = serde_json::json!(field);
        }
        // Truncate response for readability (drill answers are short, but enumerate can be longer)
        let truncated: String = response.chars().take(200).collect();
        entry["response"] = serde_json::json!(truncated);

        diagnostic::dump_jsonl_append(dir, filename, &entry);
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
            let contract = contract_for_document_domain(domain);

            // Phase 1: Enumerate items for this domain
            let enumerate_prompt = contract.build_enumerate_prompt(text);
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

            // Phase 2: Drill each item × each field (from DomainContract)
            let mut domain_markdown = Vec::new();

            for item_name in &item_names {
                let mut field_values: HashMap<String, String> = HashMap::new();
                let mut item_markdown = format!("- **{item_name}**");

                for field_desc in contract.fields {
                    let drill = contract.drill_instruction(item_name, field_desc);
                    match call_with_retry(llm, model, &drill, system, self.max_retries) {
                        Ok(resp) => {
                            raw_responses.push(resp.clone());
                            let value = resp.trim().to_string();
                            if !value.is_empty() && !is_not_specified(&value) {
                                field_values.insert(field_desc.name.to_string(), value.clone());
                                item_markdown.push_str(&format!("\n  - {}: {value}", field_desc.name));
                            }
                        }
                        Err(e) => {
                            tracing::debug!(
                                domain = %domain,
                                item = %item_name,
                                field = %field_desc.name,
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

/// Maximum length for a medical item name (test, medication, diagnosis, etc.).
/// No real name exceeds 60 characters. Chain-of-thought sentences are 80+ chars.
const MAX_ITEM_NAME_LEN: usize = 60;

/// Maximum number of items from a single enumerate response.
/// No single page of a medical document has > 30 distinct items.
const MAX_ITEMS: usize = 30;

/// Parse enumerate response into a list of item names.
///
/// Handles bullet lists, comma-separated lists, and newline-separated lists.
/// Filters out empty items and "not specified"/"none" responses.
///
/// Hardening (09-CAE):
/// - Explicit "NONE" response → empty vec (deterministic empty-case handling)
/// - Lines > 60 chars skipped (chain-of-thought sentences, not item names)
/// - Max 30 items returned (no single page has more)
pub fn parse_enumerate_response(response: &str) -> Vec<String> {
    let trimmed = response.trim();
    if trimmed.is_empty() || is_not_specified(trimmed) {
        return vec![];
    }

    // 09-CAE: Explicit empty response from category-aware prompt
    if trimmed.eq_ignore_ascii_case("NONE") {
        return vec![];
    }

    let mut items = Vec::new();

    for line in trimmed.lines() {
        if items.len() >= MAX_ITEMS {
            break;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Strip bullet markers
        let cleaned = strip_bullet_marker(line);

        // Handle comma-separated on a single line
        if cleaned.contains(',') && !cleaned.contains('\n') {
            for part in cleaned.split(',') {
                if items.len() >= MAX_ITEMS {
                    break;
                }
                let name = part.trim().replace("**", "");
                if !name.is_empty()
                    && !is_not_specified(&name)
                    && name.len() <= MAX_ITEM_NAME_LEN
                {
                    items.push(name);
                }
            }
        } else {
            let name = cleaned.replace("**", "").trim().to_string();
            if !name.is_empty()
                && !is_not_specified(&name)
                && name.len() <= MAX_ITEM_NAME_LEN
            {
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
            Ok(response) => {
                // C2: Strip thinking tokens before parsing
                return Ok(sanitize_llm_output(&response));
            }
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

    // ── 09-CAE: Parser hardening tests ──────────────────

    #[test]
    fn parse_enumerate_none_explicit() {
        assert!(parse_enumerate_response("NONE").is_empty());
    }

    #[test]
    fn parse_enumerate_none_case_insensitive() {
        assert!(parse_enumerate_response("none").is_empty());
        assert!(parse_enumerate_response("None").is_empty());
        assert!(parse_enumerate_response("  NONE  ").is_empty());
    }

    #[test]
    fn parse_enumerate_long_lines_filtered() {
        let long_line = "Here's my thought process for extracting the medication names from the provided image:";
        let short_line = "Hemoglobin";
        let input = format!("{long_line}\n{short_line}");
        let items = parse_enumerate_response(&input);
        assert_eq!(items, vec!["Hemoglobin"]);
    }

    #[test]
    fn parse_enumerate_max_items_capped() {
        let lines: Vec<String> = (1..=35).map(|i| format!("Item{i}")).collect();
        let input = lines.join("\n");
        let items = parse_enumerate_response(&input);
        assert_eq!(items.len(), MAX_ITEMS);
        assert_eq!(items[0], "Item1");
        assert_eq!(items[29], "Item30");
    }

    #[test]
    fn parse_enumerate_thinking_text_filtered() {
        // Actual chain-of-thought from 002fd7e3 diagnostic — all lines > 60 chars
        let thinking = "\
Here's my thought process for extracting the medication names from the provided image:\n\
Understand the Goal: The request asks for a list of *all visible medication names*\n\
presented one per line.\n\
Analyze the Image: I need to scan the image carefully\n\
looking for text that appears to be drug names. I'll pay attention to:\n\
Context: Is the text associated with a specific condition\n\
or treatment?\n\
Formatting: Are there specific labels like \"Medication:\"\n\
Common Drug Names: Do I recognize any common medication names?";
        let items = parse_enumerate_response(thinking);
        // Most lines are > 60 chars (thinking text). Some short fragments like
        // "or treatment?" (14 chars) pass the length filter but get through
        // as noise. The key assertion: no 80+ char reasoning sentences pass.
        for item in &items {
            assert!(
                item.len() <= MAX_ITEM_NAME_LEN,
                "Item '{}' ({} chars) exceeds MAX_ITEM_NAME_LEN",
                item,
                item.len(),
            );
        }
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
                // DomainContract prompt_label: "how often to take it ..."
                if lower.contains("frequency") || lower.contains("how often") {
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

    // ── Vision IterativeDrill tests ──────────────────────

    use crate::butler_service::{FallbackSession, SessionError};
    use crate::pipeline::strategy::ContextType;
    use crate::pipeline::structuring::ollama_types::OllamaError;

    /// Vision mock that responds contextually to enumerate vs drill prompts.
    struct VisionDrillMock;

    impl VisionClient for VisionDrillMock {
        fn generate_with_images(
            &self,
            _model: &str,
            _prompt: &str,
            _images: &[String],
            _system: Option<&str>,
        ) -> Result<String, OllamaError> {
            Ok(String::new())
        }

        fn chat_with_images(
            &self,
            _model: &str,
            prompt: &str,
            _images: &[String],
            _system: Option<&str>,
        ) -> Result<String, OllamaError> {
            let lower = prompt.to_lowercase();

            // Enumerate prompts contain "what ... are visible"
            if lower.contains("are visible") {
                // LAB_RESULTS: "What tests are visible..."
                if lower.contains("tests") {
                    return Ok("- Hemoglobine\n- Leucocytes".into());
                }
                // All other domains: nothing found
                return Ok("None".into());
            }

            // Drill prompts: "In this document image, for the test '{name}': what is the {prompt_label}?"
            if lower.contains("what is the") {
                if lower.contains("hemoglobine") {
                    // prompt_label: "result value (number)"
                    if lower.contains("result value") {
                        return Ok("11.2".into());
                    }
                    // prompt_label: "unit of measurement"
                    if lower.contains("unit of measurement") {
                        return Ok("g/dL".into());
                    }
                    // prompt_label: "whether the result is normal, low, high..."
                    if lower.contains("whether the result") {
                        return Ok("low".into());
                    }
                    // prompt_label: "low end of the normal range"
                    if lower.contains("low end") {
                        return Ok("12.0".into());
                    }
                    // prompt_label: "high end of the normal range"
                    if lower.contains("high end") {
                        return Ok("16.0".into());
                    }
                    return Ok("not specified".into());
                }
                if lower.contains("leucocytes") {
                    if lower.contains("result value") {
                        return Ok("7.2".into());
                    }
                    if lower.contains("unit of measurement") {
                        return Ok("10^9/L".into());
                    }
                    if lower.contains("whether the result") {
                        return Ok("normal".into());
                    }
                    return Ok("not specified".into());
                }
                return Ok("not specified".into());
            }

            Ok(String::new())
        }
    }

    /// Helper to run vision drill with a given mock and return the result.
    fn run_vision_drill(
        vision: &dyn VisionClient,
    ) -> Result<StrategyOutput, SessionError> {
        run_vision_drill_with_doc_type(vision, None)
    }

    fn run_vision_drill_with_doc_type(
        vision: &dyn VisionClient,
        doc_type: Option<crate::pipeline::extraction::vision_classifier::UserDocumentType>,
    ) -> Result<StrategyOutput, SessionError> {
        let session = FallbackSession::new("medgemma:4b", ContextType::NightBatch, false);
        let strategy = IterativeDrillStrategy::new(0);
        let images = vec!["base64_image_data".to_string()];
        strategy.extract_from_image(&session, vision, &images, "You are a medical document extractor.", None, doc_type)
    }

    #[test]
    fn vision_drill_extracts_lab_results() {
        let mock = VisionDrillMock;
        let output = run_vision_drill(&mock).unwrap();

        assert_eq!(output.entities.lab_results.len(), 2);
        assert_eq!(output.entities.lab_results[0].test_name, "Hemoglobine");
        assert_eq!(output.entities.lab_results[0].value, Some(11.2));
        assert_eq!(output.entities.lab_results[0].unit.as_deref(), Some("g/dL"));
        assert_eq!(output.entities.lab_results[0].abnormal_flag.as_deref(), Some("low"));

        assert_eq!(output.entities.lab_results[1].test_name, "Leucocytes");
        assert_eq!(output.entities.lab_results[1].value, Some(7.2));
    }

    #[test]
    fn vision_drill_skips_empty_domains() {
        let mock = VisionDrillMock;
        let output = run_vision_drill(&mock).unwrap();

        // Only lab results should have items, other domains return "None"
        assert!(output.entities.medications.is_empty());
        assert!(output.entities.diagnoses.is_empty());
        assert!(output.entities.allergies.is_empty());
        assert!(output.entities.procedures.is_empty());
        assert!(output.entities.referrals.is_empty());
        assert!(output.entities.instructions.is_empty());
    }

    #[test]
    fn vision_drill_not_specified_skipped() {
        let mock = VisionDrillMock;
        let output = run_vision_drill(&mock).unwrap();

        // Leucocytes: mock returns "not specified" for fields not explicitly handled
        // (collection_date, reference ranges). These should not appear in the entity.
        let leuco = &output.entities.lab_results[1];
        assert!(leuco.collection_date.is_none());
        // "normal" is a valid abnormal_flag value, so it should be present
        assert_eq!(leuco.abnormal_flag.as_deref(), Some("normal"));
    }

    #[test]
    fn vision_drill_markdown_output() {
        let mock = VisionDrillMock;
        let output = run_vision_drill(&mock).unwrap();

        assert!(output.markdown.contains("## Lab Results"));
        assert!(output.markdown.contains("Hemoglobine"));
        assert!(output.markdown.contains("Leucocytes"));
    }

    #[test]
    fn vision_drill_raw_responses_collected() {
        let mock = VisionDrillMock;
        let output = run_vision_drill(&mock).unwrap();

        // At least: 7 enumerate + drill responses for 2 items
        assert!(output.raw_responses.len() >= 7);
    }

    /// Mock that degenerates on enumerate for all domains.
    struct VisionAlwaysDegenerate;

    impl VisionClient for VisionAlwaysDegenerate {
        fn generate_with_images(
            &self,
            _model: &str,
            _prompt: &str,
            _images: &[String],
            _system: Option<&str>,
        ) -> Result<String, OllamaError> {
            Ok(String::new())
        }

        fn chat_with_images(
            &self,
            _model: &str,
            _prompt: &str,
            _images: &[String],
            _system: Option<&str>,
        ) -> Result<String, OllamaError> {
            Err(OllamaError::VisionDegeneration {
                pattern: "token_repeat".into(),
                tokens_before_abort: 100,
                partial_output: String::new(),
            })
        }
    }

    #[test]
    fn vision_drill_handles_degeneration_gracefully() {
        let mock = VisionAlwaysDegenerate;
        let output = run_vision_drill(&mock).unwrap();

        // All domains degenerate with empty partial — nothing extracted, but no error
        assert!(output.entities.lab_results.is_empty());
        assert!(output.entities.medications.is_empty());
    }

    /// Mock that degenerates on enumerate but provides partial item names.
    struct VisionPartialDegenerate;

    impl VisionClient for VisionPartialDegenerate {
        fn generate_with_images(
            &self,
            _model: &str,
            _prompt: &str,
            _images: &[String],
            _system: Option<&str>,
        ) -> Result<String, OllamaError> {
            Ok(String::new())
        }

        fn chat_with_images(
            &self,
            _model: &str,
            prompt: &str,
            _images: &[String],
            _system: Option<&str>,
        ) -> Result<String, OllamaError> {
            let lower = prompt.to_lowercase();

            if lower.contains("are visible") {
                if lower.contains("tests") {
                    // Degenerate but with useful partial
                    return Err(OllamaError::VisionDegeneration {
                        pattern: "low_diversity".into(),
                        tokens_before_abort: 50,
                        partial_output: "- Glucose".into(),
                    });
                }
                return Ok("None".into());
            }

            // Drill: return value for Glucose
            if lower.contains("what is the") && lower.contains("glucose") {
                if lower.contains("result value") {
                    return Ok("5.4".into());
                }
                if lower.contains("unit of measurement") {
                    return Ok("mmol/L".into());
                }
            }

            Ok("not specified".into())
        }
    }

    #[test]
    fn vision_drill_partial_output_used_on_degeneration() {
        let mock = VisionPartialDegenerate;
        let output = run_vision_drill(&mock).unwrap();

        // Should have extracted Glucose from partial degeneration output
        assert_eq!(output.entities.lab_results.len(), 1);
        assert_eq!(output.entities.lab_results[0].test_name, "Glucose");
        assert_eq!(output.entities.lab_results[0].value, Some(5.4));
        assert_eq!(output.entities.lab_results[0].unit.as_deref(), Some("mmol/L"));
    }

    /// Mock that returns a network error (non-degeneration).
    struct VisionNetworkError;

    impl VisionClient for VisionNetworkError {
        fn generate_with_images(
            &self,
            _model: &str,
            _prompt: &str,
            _images: &[String],
            _system: Option<&str>,
        ) -> Result<String, OllamaError> {
            Ok(String::new())
        }

        fn chat_with_images(
            &self,
            _model: &str,
            _prompt: &str,
            _images: &[String],
            _system: Option<&str>,
        ) -> Result<String, OllamaError> {
            Err(OllamaError::NotReachable)
        }
    }

    #[test]
    fn vision_drill_session_error_propagated() {
        let mock = VisionNetworkError;
        let result = run_vision_drill(&mock);
        assert!(result.is_err());
        match result.unwrap_err() {
            SessionError::Llm(msg) => assert!(msg.contains("not reachable") || msg.contains("NotReachable") || !msg.is_empty()),
            other => panic!("Expected SessionError::Llm, got: {:?}", other),
        }
    }

    #[test]
    fn vision_drill_all_7_domains_checked() {
        // Use a counting mock to verify all 7 domains get an enumerate call
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CountingVision {
            enumerate_count: AtomicUsize,
        }

        impl VisionClient for CountingVision {
            fn generate_with_images(
                &self,
                _model: &str,
                _prompt: &str,
                _images: &[String],
                _system: Option<&str>,
            ) -> Result<String, OllamaError> {
                Ok(String::new())
            }

            fn chat_with_images(
                &self,
                _model: &str,
                prompt: &str,
                _images: &[String],
                _system: Option<&str>,
            ) -> Result<String, OllamaError> {
                if prompt.to_lowercase().contains("are visible") {
                    self.enumerate_count.fetch_add(1, Ordering::Relaxed);
                }
                Ok("None".into())
            }
        }

        let mock = CountingVision {
            enumerate_count: AtomicUsize::new(0),
        };
        let _ = run_vision_drill(&mock).unwrap();

        assert_eq!(mock.enumerate_count.load(Ordering::Relaxed), 7);
    }

    // ── 09-CAE: Domain filtering integration tests ──────

    #[test]
    fn vision_drill_filters_domains_lab_report() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use crate::pipeline::extraction::vision_classifier::UserDocumentType;

        struct CountingVision {
            enumerate_count: AtomicUsize,
        }

        impl VisionClient for CountingVision {
            fn generate_with_images(&self, _: &str, _: &str, _: &[String], _: Option<&str>) -> Result<String, OllamaError> {
                Ok(String::new())
            }
            fn chat_with_images(&self, _: &str, prompt: &str, _: &[String], _: Option<&str>) -> Result<String, OllamaError> {
                if prompt.to_lowercase().contains("are visible") {
                    self.enumerate_count.fetch_add(1, Ordering::Relaxed);
                }
                Ok("NONE".into())
            }
        }

        let mock = CountingVision { enumerate_count: AtomicUsize::new(0) };
        let _ = run_vision_drill_with_doc_type(&mock, Some(UserDocumentType::LabReport)).unwrap();

        // LabReport → [LabResults, Diagnoses] → 2 enumerate calls
        assert_eq!(mock.enumerate_count.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn vision_drill_filters_domains_prescription() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use crate::pipeline::extraction::vision_classifier::UserDocumentType;

        struct CountingVision {
            enumerate_count: AtomicUsize,
        }

        impl VisionClient for CountingVision {
            fn generate_with_images(&self, _: &str, _: &str, _: &[String], _: Option<&str>) -> Result<String, OllamaError> {
                Ok(String::new())
            }
            fn chat_with_images(&self, _: &str, prompt: &str, _: &[String], _: Option<&str>) -> Result<String, OllamaError> {
                if prompt.to_lowercase().contains("are visible") {
                    self.enumerate_count.fetch_add(1, Ordering::Relaxed);
                }
                Ok("NONE".into())
            }
        }

        let mock = CountingVision { enumerate_count: AtomicUsize::new(0) };
        let _ = run_vision_drill_with_doc_type(&mock, Some(UserDocumentType::Prescription)).unwrap();

        // Prescription → [Medications, Instructions, Diagnoses] → 3 enumerate calls
        assert_eq!(mock.enumerate_count.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn vision_drill_no_filter_when_none() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CountingVision {
            enumerate_count: AtomicUsize,
        }

        impl VisionClient for CountingVision {
            fn generate_with_images(&self, _: &str, _: &str, _: &[String], _: Option<&str>) -> Result<String, OllamaError> {
                Ok(String::new())
            }
            fn chat_with_images(&self, _: &str, prompt: &str, _: &[String], _: Option<&str>) -> Result<String, OllamaError> {
                if prompt.to_lowercase().contains("are visible") {
                    self.enumerate_count.fetch_add(1, Ordering::Relaxed);
                }
                Ok("NONE".into())
            }
        }

        let mock = CountingVision { enumerate_count: AtomicUsize::new(0) };
        let _ = run_vision_drill_with_doc_type(&mock, None).unwrap();

        // None (legacy fallback) → all 7 domains
        assert_eq!(mock.enumerate_count.load(Ordering::Relaxed), 7);
    }

    #[test]
    fn vision_drill_writes_progress_diagnostic() {
        let tmp = tempfile::tempdir().unwrap();
        let progress_path = tmp.path().join("drill-progress.jsonl");

        let session = FallbackSession::new("medgemma:4b", ContextType::NightBatch, false);
        let strategy = IterativeDrillStrategy::new(0);
        let images = vec!["base64_image_data".to_string()];
        let mock = VisionDrillMock;

        let output = strategy.extract_from_image(
            &session,
            &mock,
            &images,
            "You are a medical document extractor.",
            Some(&progress_path),
            None,
        ).unwrap();

        assert!(!output.entities.lab_results.is_empty());

        // Verify JSONL file was written
        let content = std::fs::read_to_string(&progress_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        // 7 enumerate + N drill calls + 1 summary
        assert!(lines.len() > 7, "Expected > 7 lines, got {}", lines.len());

        // First line should be an enumerate call
        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["phase"], "enumerate");
        assert_eq!(first["call"], 1);

        // Last line should be summary
        let last: serde_json::Value = serde_json::from_str(lines[lines.len() - 1]).unwrap();
        assert_eq!(last["phase"], "summary");
        assert!(last["total_calls"].as_u64().unwrap() > 0);
        assert!(last["lab_results"].as_u64().unwrap() >= 2);

        // Verify a drill entry has item and field
        let has_drill = lines.iter().any(|line| {
            let v: serde_json::Value = serde_json::from_str(line).unwrap_or_default();
            v["phase"] == "drill" && v.get("item").is_some() && v.get("field").is_some()
        });
        assert!(has_drill, "Expected at least one drill entry with item+field");
    }
}
