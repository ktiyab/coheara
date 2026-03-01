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

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::butler_service::{SessionError, ValidatedOutput, VisionSession};
use crate::pipeline::domain_contracts::{contract_for_document_domain, locale_for_domain};
use crate::pipeline::prompt_templates::{
    self, DocumentDomain, PromptStrategyKind,
};
use crate::pipeline::safety::output_sanitize::sanitize_llm_output;
use crate::pipeline::structuring::extraction_strategy::{ExtractionStrategy, StrategyOutput};
use crate::pipeline::structuring::types::{
    ExtractedAllergy, ExtractedDiagnosis, ExtractedEntities, ExtractedInstruction,
    ExtractedLabResult, ExtractedMedication, ExtractedProcedure, ExtractedProfessional,
    ExtractedReferral, LlmClient, VisionClient,
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
    ///
    /// 10-LDC: `lang` selects language-matched prompts via `locale_for_domain()`.
    /// Falls back to English when no locale is available for the given language.
    pub fn extract_from_image(
        &self,
        session: &dyn VisionSession,
        client: &dyn VisionClient,
        images: &[String],
        _system_prompt: &str,
        progress_path: Option<&Path>,
        user_doc_type: Option<crate::pipeline::extraction::vision_classifier::UserDocumentType>,
        lang: &str,
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

        // 12-ERC B4: Pre-domain metadata extraction (professional + date)
        let meta_sys = crate::pipeline::domain_contracts::meta_system_prompt(lang);

        // Pre-domain: professional
        let mut professional: Option<ExtractedProfessional> = None;
        call_num += 1;
        let call_start = std::time::Instant::now();
        let prof_prompt = crate::pipeline::domain_contracts::professional_prompt(lang);
        match Self::vision_drill_call(session, client, prof_prompt, images, meta_sys) {
            Ok((text, tokens)) => {
                Self::log_progress(
                    progress_path, call_num, "meta", 0, "professional", "", "", "ok",
                    &text, tokens, call_start.elapsed().as_millis(),
                );
                raw_responses.push(text.clone());
                professional = parse_professional_response(&text);
            }
            Err(DrillError::Degeneration { .. } | DrillError::QualityGate { .. }) => {
                tracing::debug!("Professional extraction: degeneration/quality gate, skipping");
            }
            Err(DrillError::Fatal(e)) => return Err(e),
        }

        // Pre-domain: document date
        let mut document_date: Option<String> = None;
        call_num += 1;
        let call_start = std::time::Instant::now();
        let date_prompt = crate::pipeline::domain_contracts::document_date_prompt(lang);
        match Self::vision_drill_call(session, client, date_prompt, images, meta_sys) {
            Ok((text, tokens)) => {
                Self::log_progress(
                    progress_path, call_num, "meta", 0, "date", "", "", "ok",
                    &text, tokens, call_start.elapsed().as_millis(),
                );
                raw_responses.push(text.clone());
                document_date = parse_date_response(&text);
            }
            Err(DrillError::Degeneration { .. } | DrillError::QualityGate { .. }) => {
                tracing::debug!("Date extraction: degeneration/quality gate, skipping");
            }
            Err(DrillError::Fatal(e)) => return Err(e),
        }

        for (domain_idx, &domain) in domains.iter().enumerate() {
            let contract = contract_for_document_domain(domain);
            // 10-LDC: Resolve language-matched locale for this domain
            let locale = locale_for_domain(contract.domain, lang);

            // Phase 0: Enumerate items for this domain via vision
            // 10-LDC: Use locale-specific enumerate prompt (replaces English-only contract prompt)
            let enumerate_prompt = locale.vision_enumerate.to_string();
            call_num += 1;
            let call_start = std::time::Instant::now();

            // 10-LDC: Use locale system prompt (focused extraction, not monolithic OCR)
            let locale_system = locale.system_prompt;
            let enumerate_result = match session.chat_with_images(
                client,
                &enumerate_prompt,
                images,
                Some(locale_system),
            ) {
                Ok(result) => {
                    Self::log_progress(progress_path, call_num, "enumerate", domain_idx, &domain.to_string(), "", "", "ok", &result.text, result.tokens_generated, call_start.elapsed().as_millis());
                    result
                }
                Err(SessionError::Degeneration { partial_output, tokens_before_abort, pattern, .. }) => {
                    Self::log_progress(progress_path, call_num, "enumerate", domain_idx, &domain.to_string(), "", "", &format!("degen:{pattern}"), &partial_output, tokens_before_abort, call_start.elapsed().as_millis());
                    // 11-SRP B5: Check if model produced <answer> before degenerating.
                    // If yes, extract it (model answered, then degenerated — data is valid).
                    // If no, sanitize and try legacy parser (model degenerated mid-output).
                    let sanitized = sanitize_llm_output(&partial_output);
                    match extract_answer(&sanitized, locale.answer_start, locale.answer_end) {
                        Some(answer) if !answer.is_empty() => {
                            tracing::info!(domain = %domain, "Degeneration after answer — using answer content");
                            ValidatedOutput {
                                text: format!("{}{}{}", locale.answer_start, answer, locale.answer_end),
                                tokens_generated: 0,
                                model: session.model().to_string(),
                            }
                        }
                        _ => {
                            if sanitized.trim().is_empty() {
                                tracing::warn!(domain = %domain, "Vision enumerate degenerated with no output, skipping");
                                continue;
                            }
                            tracing::debug!(domain = %domain, "Vision enumerate degenerated before answer, using sanitized output");
                            ValidatedOutput {
                                text: sanitized,
                                tokens_generated: 0,
                                model: session.model().to_string(),
                            }
                        }
                    }
                }
                Err(SessionError::QualityGate { raw_output, reason, .. }) => {
                    Self::log_progress(progress_path, call_num, "enumerate", domain_idx, &domain.to_string(), "", "", &format!("quality_gate:{reason}"), &raw_output, 0, call_start.elapsed().as_millis());
                    // 11-SRP B5: Same structural extraction for quality gate output
                    let sanitized = sanitize_llm_output(&raw_output);
                    match extract_answer(&sanitized, locale.answer_start, locale.answer_end) {
                        Some(answer) if !answer.is_empty() => {
                            ValidatedOutput {
                                text: format!("{}{}{}", locale.answer_start, answer, locale.answer_end),
                                tokens_generated: 0,
                                model: session.model().to_string(),
                            }
                        }
                        _ => {
                            if sanitized.trim().is_empty() {
                                continue;
                            }
                            ValidatedOutput {
                                text: sanitized,
                                tokens_generated: 0,
                                model: session.model().to_string(),
                            }
                        }
                    }
                }
                Err(e) => {
                    Self::log_progress(progress_path, call_num, "enumerate", domain_idx, &domain.to_string(), "", "", &format!("error:{e}"), "", 0, call_start.elapsed().as_millis());
                    return Err(e);
                }
            };

            raw_responses.push(enumerate_result.text.clone());
            // 11-SRP: Use structural parser for vision mode (answer tokens)
            let item_names = parse_enumerate_structured(
                &enumerate_result.text,
                locale.answer_start,
                locale.answer_end,
                locale.none_keyword,
            );

            if item_names.is_empty() {
                continue;
            }

            // Phase 1: Drill each item via locale-based grouped prompts
            // 10-LDC: Instead of 6 calls per lab test, use 2 locale drill templates:
            //   - vision_drill_value → value+unit (or dosage for medications)
            //   - vision_drill_range → reference range (lab results only)
            // Then compute abnormal_flag from parsed values (no model call needed).
            let mut domain_markdown = Vec::new();

            for item_name in &item_names {
                let mut field_values: HashMap<String, String> = HashMap::new();
                let mut item_markdown = format!("- **{item_name}**");

                // Drill group 1: value (always present if locale has drill_value)
                if !locale.vision_drill_value.is_empty() {
                    let drill_prompt = locale.vision_drill_value.replace("{item}", item_name);
                    call_num += 1;
                    let call_start = std::time::Instant::now();

                    match Self::vision_drill_call(session, client, &drill_prompt, images, locale_system) {
                        Ok((text, tokens)) => {
                            Self::log_progress(progress_path, call_num, "drill", domain_idx, &domain.to_string(), item_name, "value", "ok", &text, tokens, call_start.elapsed().as_millis());
                            raw_responses.push(text.clone());
                            // 11-SRP: Extract from answer tokens, fallback to raw response
                            let value = extract_answer(&text, locale.answer_start, locale.answer_end)
                                .unwrap_or_else(|| text.trim().to_string());
                            if !value.is_empty() && !is_not_specified(&value) {
                                // 10-LDC: Parse combined value+unit for lab results
                                if contract.domain == "lab_results" {
                                    if let Some((v, u)) = parse_value_with_unit(&value) {
                                        field_values.insert("value".into(), v.clone());
                                        field_values.insert("unit".into(), u.clone());
                                        item_markdown.push_str(&format!("\n  - value: {v} {u}"));
                                    } else {
                                        // Non-numeric value (e.g., "negative") — store as text
                                        field_values.insert("value".into(), value.clone());
                                        item_markdown.push_str(&format!("\n  - value: {value}"));
                                    }
                                } else {
                                    // Other domains: store raw response as primary field
                                    field_values.insert("value".into(), value.clone());
                                    item_markdown.push_str(&format!("\n  - value: {value}"));
                                }
                            }
                        }
                        Err(DrillError::Degeneration { partial, tokens, pattern }) => {
                            Self::log_progress(progress_path, call_num, "drill", domain_idx, &domain.to_string(), item_name, "value", &format!("degen:{pattern}"), &partial, tokens, call_start.elapsed().as_millis());
                            // 11-SRP B5: Try extracting answer from partial degeneration output
                            let value = extract_answer(&partial, locale.answer_start, locale.answer_end)
                                .unwrap_or(partial);
                            if !value.is_empty() && !is_not_specified(&value) {
                                field_values.insert("value".into(), value.clone());
                                item_markdown.push_str(&format!("\n  - value: {value}"));
                            }
                        }
                        Err(DrillError::QualityGate { reason }) => {
                            Self::log_progress(progress_path, call_num, "drill", domain_idx, &domain.to_string(), item_name, "value", &format!("quality_gate:{reason}"), "", 0, call_start.elapsed().as_millis());
                        }
                        Err(DrillError::Fatal(e)) => {
                            Self::log_progress(progress_path, call_num, "drill", domain_idx, &domain.to_string(), item_name, "value", &format!("error:{e}"), "", 0, call_start.elapsed().as_millis());
                            return Err(e);
                        }
                    }
                }

                // Drill group 2: reference range (lab results only — locale has drill_range)
                if !locale.vision_drill_range.is_empty() {
                    let drill_prompt = locale.vision_drill_range.replace("{item}", item_name);
                    call_num += 1;
                    let call_start = std::time::Instant::now();

                    match Self::vision_drill_call(session, client, &drill_prompt, images, locale_system) {
                        Ok((text, tokens)) => {
                            Self::log_progress(progress_path, call_num, "drill", domain_idx, &domain.to_string(), item_name, "range", "ok", &text, tokens, call_start.elapsed().as_millis());
                            raw_responses.push(text.clone());
                            // 11-SRP: Extract from answer tokens, fallback to raw response
                            let value = extract_answer(&text, locale.answer_start, locale.answer_end)
                                .unwrap_or_else(|| text.trim().to_string());
                            if !value.is_empty() && !is_not_specified(&value) {
                                if let Some((low, high)) = parse_reference_range(&value) {
                                    field_values.insert("reference_range_low".into(), low.to_string());
                                    field_values.insert("reference_range_high".into(), high.to_string());
                                    item_markdown.push_str(&format!("\n  - range: {value}"));
                                } else {
                                    field_values.insert("reference_range".into(), value.clone());
                                    item_markdown.push_str(&format!("\n  - range: {value}"));
                                }
                            }
                        }
                        Err(DrillError::Degeneration { partial, tokens, pattern }) => {
                            Self::log_progress(progress_path, call_num, "drill", domain_idx, &domain.to_string(), item_name, "range", &format!("degen:{pattern}"), &partial, tokens, call_start.elapsed().as_millis());
                            // 11-SRP B5: Try extracting answer from partial degeneration output
                            let value = extract_answer(&partial, locale.answer_start, locale.answer_end)
                                .unwrap_or(partial);
                            if !value.is_empty() && !is_not_specified(&value) {
                                if let Some((low, high)) = parse_reference_range(&value) {
                                    field_values.insert("reference_range_low".into(), low.to_string());
                                    field_values.insert("reference_range_high".into(), high.to_string());
                                    item_markdown.push_str(&format!("\n  - range: {value}"));
                                }
                            }
                        }
                        Err(DrillError::QualityGate { reason }) => {
                            Self::log_progress(progress_path, call_num, "drill", domain_idx, &domain.to_string(), item_name, "range", &format!("quality_gate:{reason}"), "", 0, call_start.elapsed().as_millis());
                        }
                        Err(DrillError::Fatal(e)) => {
                            Self::log_progress(progress_path, call_num, "drill", domain_idx, &domain.to_string(), item_name, "range", &format!("error:{e}"), "", 0, call_start.elapsed().as_millis());
                            return Err(e);
                        }
                    }
                }

                // 10-LDC: Compute abnormal_flag from parsed value vs range (no model call)
                if let (Some(val_str), Some(low_str), Some(high_str)) = (
                    field_values.get("value"),
                    field_values.get("reference_range_low"),
                    field_values.get("reference_range_high"),
                ) {
                    if let (Some(val), Some(low), Some(high)) = (
                        parse_french_number(val_str),
                        parse_french_number(low_str),
                        parse_french_number(high_str),
                    ) {
                        field_values.insert("abnormal_flag".into(), compute_abnormal_flag(val, low, high).to_string());
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
            document_date,
            professional,
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

        // 12-ERC B4: Pre-domain metadata extraction (professional + date)
        let meta_sys = crate::pipeline::domain_contracts::META_SYSTEM_PROMPT_EN;

        let mut professional: Option<ExtractedProfessional> = None;
        let prof_prompt = crate::pipeline::domain_contracts::professional_prompt("en");
        match llm.generate(model, &format!("{}\n\n{}", prof_prompt, text), meta_sys) {
            Ok(response) => {
                raw_responses.push(response.clone());
                professional = parse_professional_response(&response);
            }
            Err(e) => tracing::debug!("Professional extraction failed: {e}"),
        }

        let mut document_date: Option<String> = None;
        let date_prompt_str = crate::pipeline::domain_contracts::document_date_prompt("en");
        match llm.generate(model, &format!("{}\n\n{}", date_prompt_str, text), meta_sys) {
            Ok(response) => {
                raw_responses.push(response.clone());
                document_date = parse_date_response(&response);
            }
            Err(e) => tracing::debug!("Date extraction failed: {e}"),
        }

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
            document_date,
            professional,
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
///
/// 10-LDC additions:
/// - Multilingual NONE detection (AUCUN, KEINE, etc.)
/// - Thinking-line detection (numbered reasoning, meta-commentary)
/// - Case-insensitive deduplication (first occurrence wins)
pub fn parse_enumerate_response(response: &str) -> Vec<String> {
    let trimmed = response.trim();
    if trimmed.is_empty() || is_not_specified(trimmed) {
        return vec![];
    }

    // 10-LDC: Multilingual NONE detection (09-CAE was English-only)
    if is_none_response(trimmed) {
        return vec![];
    }

    let mut items = Vec::new();
    let mut seen = HashSet::new();

    for line in trimmed.lines() {
        if items.len() >= MAX_ITEMS {
            break;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // 10-LDC: Skip thinking-text lines (chain-of-thought reasoning)
        if is_thinking_line(line) {
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
                    && seen.insert(name.to_lowercase())
                {
                    items.push(name);
                }
            }
        } else {
            let name = cleaned.replace("**", "").trim().to_string();
            if !name.is_empty()
                && !is_not_specified(&name)
                && name.len() <= MAX_ITEM_NAME_LEN
                && seen.insert(name.to_lowercase())
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
// 10-LDC: Drill call helper + error type
// ═══════════════════════════════════════════════════════════

/// Internal error type for drill calls — separates recoverable from fatal.
enum DrillError {
    Degeneration { partial: String, tokens: usize, pattern: String },
    QualityGate { reason: String },
    Fatal(SessionError),
}

impl IterativeDrillStrategy {
    /// Execute a single vision drill call, normalizing all error variants.
    fn vision_drill_call(
        session: &dyn VisionSession,
        client: &dyn VisionClient,
        prompt: &str,
        images: &[String],
        system_prompt: &str,
    ) -> Result<(String, usize), DrillError> {
        match session.chat_with_images(client, prompt, images, Some(system_prompt)) {
            // 11-SRP B2: Sanitize drill responses — strip thinking tokens before field parsing
            Ok(result) => Ok((sanitize_llm_output(&result.text), result.tokens_generated)),
            Err(SessionError::Degeneration { partial_output, tokens_before_abort, pattern, .. }) => {
                let partial = sanitize_llm_output(&partial_output).trim().to_string();
                Err(DrillError::Degeneration { partial, tokens: tokens_before_abort, pattern })
            }
            Err(SessionError::QualityGate { reason, .. }) => {
                Err(DrillError::QualityGate { reason })
            }
            Err(e) => Err(DrillError::Fatal(e)),
        }
    }
}

// ═══════════════════════════════════════════════════════════
// 10-LDC: Thinking-line detection
// ═══════════════════════════════════════════════════════════

/// Prefixes that indicate chain-of-thought reasoning, not item names.
const THINKING_PREFIXES: &[&str] = &[
    "the user", "i need", "let me", "check ", "final ",
    "scan ", "identify", "look for", "extract the",
    "note:", "observe", "this is", "here's my",
    "understand", "analyze", "context:",
];

/// Detect lines that are model thinking/reasoning, not medical item names.
fn is_thinking_line(line: &str) -> bool {
    let lower = line.trim_start().to_lowercase();
    THINKING_PREFIXES.iter().any(|p| lower.starts_with(p))
}

// ═══════════════════════════════════════════════════════════
// 10-LDC: Multilingual NONE detection
// ═══════════════════════════════════════════════════════════

/// Check if a response means "nothing found" in any supported language.
///
/// Used for enumerate responses. Distinct from `is_not_specified()` which
/// handles drill field values.
pub fn is_none_response(text: &str) -> bool {
    let trimmed = text.trim().to_lowercase();
    matches!(
        trimmed.as_str(),
        "none" | "aucun" | "aucune" | "keine" | "keiner"
            | "no results" | "aucun résultat" | "keine ergebnisse"
    )
}

// ═══════════════════════════════════════════════════════════
// 11-SRP: Structural answer extraction
// ═══════════════════════════════════════════════════════════

/// Extract content between answer tokens from model output.
///
/// Returns None if answer tokens are not found (model didn't follow structure).
/// Returns Some("") for empty answers.
/// Returns Some(content) for the trimmed text between start and end tokens.
///
/// The `<think>...</think>` section is ignored entirely — it never reaches
/// the item/value parsing logic.
pub fn extract_answer(response: &str, start_token: &str, end_token: &str) -> Option<String> {
    let start_idx = response.find(start_token)?;
    let content_start = start_idx + start_token.len();
    let end_idx = response[content_start..].find(end_token)?;
    Some(response[content_start..content_start + end_idx].trim().to_string())
}

/// Parse enumerate response using structural answer tokens with legacy fallback.
///
/// 1. Try to extract content between `<answer>...</answer>` tokens
/// 2. If NONE keyword found inside answer, return empty
/// 3. Parse clean item list from answer content
/// 4. Fallback to `parse_enumerate_response()` if no answer tokens found
pub fn parse_enumerate_structured(
    response: &str,
    answer_start: &str,
    answer_end: &str,
    none_keyword: &str,
) -> Vec<String> {
    // Step 1: Extract answer content
    let answer = match extract_answer(response, answer_start, answer_end) {
        Some(a) => a,
        None => {
            // Fallback: model didn't use tokens — apply legacy parser
            tracing::warn!("Model did not produce answer tokens, falling back to legacy parser");
            return parse_enumerate_response(response);
        }
    };

    // Step 2: Check for NONE
    if answer.trim().eq_ignore_ascii_case(none_keyword)
        || answer.trim().eq_ignore_ascii_case("none")
    {
        return vec![];
    }

    // Step 3: Parse clean item list (no thinking-line heuristics needed — content is inside <answer>)
    let mut items = Vec::new();
    let mut seen = HashSet::new();

    for line in answer.lines() {
        if items.len() >= MAX_ITEMS {
            break;
        }
        let name = line.trim().replace("**", "");
        let name = strip_bullet_marker(&name).trim().to_string();
        if !name.is_empty()
            && !is_not_specified(&name)
            && name.len() <= MAX_ITEM_NAME_LEN
            && seen.insert(name.to_lowercase())
        {
            items.push(name);
        }
    }

    items
}

// ═══════════════════════════════════════════════════════════
// 10-LDC: Value and range parsers
// ═══════════════════════════════════════════════════════════

/// Parse a combined "value unit" response into separate components.
///
/// Examples:
/// - "4.88 T/l" → Some(("4.88", "T/l"))
/// - "13.3 g/dL" → Some(("13.3", "g/dL"))
/// - "6 160 /mm3" → Some(("6160", "/mm3"))
/// - "negative" → None (non-numeric, store as text)
pub fn parse_value_with_unit(response: &str) -> Option<(String, String)> {
    let trimmed = response.trim();
    if trimmed.is_empty() || is_not_specified(trimmed) {
        return None;
    }

    // Strategy: find the boundary between the numeric part and the unit part.
    // The unit starts at the first character that is not a digit, space, comma,
    // or dot AFTER we've seen at least one digit. This handles:
    // "4.88 T/l" → ("4.88", "T/l")
    // "6 160 /mm3" → ("6160", "/mm3")
    // "13.3 g/dL" → ("13.3", "g/dL")
    let mut saw_digit = false;
    let mut split_pos = None;

    for (i, c) in trimmed.char_indices() {
        if c.is_ascii_digit() {
            saw_digit = true;
        } else if saw_digit && c != ' ' && c != ',' && c != '.' {
            split_pos = Some(i);
            break;
        }
    }

    if let Some(pos) = split_pos {
        let value_part = trimmed[..pos].trim();
        let unit_part = trimmed[pos..].trim();
        if !unit_part.is_empty() && !value_part.is_empty() {
            let value_clean = value_part.replace(' ', "");
            return Some((value_clean, unit_part.to_string()));
        }
    }
    None
}

/// Parse a reference range response into (low, high) numeric bounds.
///
/// Examples:
/// - "(4,28-6,00)" → Some((4.28, 6.00))
/// - "(1 800-6 900)" → Some((1800.0, 6900.0))
/// - "(<630)" → Some((0.0, 630.0))
/// - "13.4-16.7" → Some((13.4, 16.7))
pub fn parse_reference_range(response: &str) -> Option<(f64, f64)> {
    let cleaned = response
        .trim()
        .trim_start_matches(['(', '['])
        .trim_end_matches([')', ']'])
        .trim();

    if cleaned.is_empty() || is_not_specified(cleaned) {
        return None;
    }

    // Handle "<N" format (upper bound only)
    if let Some(upper) = cleaned.strip_prefix('<') {
        let val = parse_french_number(upper.trim())?;
        return Some((0.0, val));
    }

    // Handle ">N" format (lower bound only)
    if let Some(lower) = cleaned.strip_prefix('>') {
        let val = parse_french_number(lower.trim())?;
        return Some((val, f64::INFINITY));
    }

    // Split on dash/en-dash
    let parts: Vec<&str> = cleaned.splitn(2, |c| c == '-' || c == '\u{2013}').collect();
    if parts.len() == 2 {
        let low = parse_french_number(parts[0].trim())?;
        let high = parse_french_number(parts[1].trim())?;
        return Some((low, high));
    }
    None
}

/// Parse a number that may use French conventions.
///
/// French: comma decimal ("4,28"), space thousands ("6 160").
/// International: dot decimal ("4.28"), no thousands separator.
pub fn parse_french_number(s: &str) -> Option<f64> {
    let normalized = s.replace(' ', "").replace(',', ".");
    normalized.parse::<f64>().ok()
}

/// Compute abnormal flag from value and reference range bounds.
///
/// Returns a flag string matching the DB CHECK constraint.
/// Critical flags: value < 50% of low or > 200% of high.
pub fn compute_abnormal_flag(value: f64, low: f64, high: f64) -> &'static str {
    if value < low * 0.5 {
        "critical_low"
    } else if high.is_finite() && value > high * 2.0 {
        "critical_high"
    } else if value < low {
        "low"
    } else if high.is_finite() && value > high {
        "high"
    } else {
        "normal"
    }
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
    // 10-LDC: "value" field contains combined dosage from locale drill.
    // Fall back to individual fields for backward compat with text-mode extraction.
    let combined_dosage = fields.get("value").cloned().unwrap_or_default();
    let dose = fields.get("dose").cloned().unwrap_or(combined_dosage);
    ExtractedMedication {
        generic_name: Some(name.to_string()),
        brand_name: None,
        dose,
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
        .and_then(|s| parse_french_number(s));
    let value_text = if value.is_none() { value_str } else { None };

    // 10-LDC: reference_range_low/high now parsed by drill loop
    let ref_low = fields.get("reference_range_low")
        .and_then(|s| parse_french_number(s));
    let ref_high = fields.get("reference_range_high")
        .and_then(|s| parse_french_number(s));

    ExtractedLabResult {
        test_name: name.to_string(),
        test_code: None,
        value,
        value_text,
        unit: fields.get("unit").cloned(),
        reference_range_low: ref_low,
        reference_range_high: ref_high,
        reference_range_text: fields.get("reference_range").cloned(),
        abnormal_flag: fields.get("abnormal_flag").cloned(),
        collection_date: None,
        confidence: 0.0,
    }
}

fn assemble_diagnosis(name: &str, fields: &HashMap<String, String>) -> ExtractedDiagnosis {
    // 10-LDC: "value" field contains status from locale drill.
    // Fall back to "status" for backward compat with text-mode extraction.
    let status = fields.get("status")
        .or_else(|| fields.get("value"))
        .cloned()
        .unwrap_or_default();
    ExtractedDiagnosis {
        name: name.to_string(),
        icd_code: None,
        date: fields.get("date").cloned(),
        status,
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
// 12-ERC B4: Pre-domain metadata parsers
// ═══════════════════════════════════════════════════════════

/// Parse professional name and specialty from LLM response.
///
/// Expected format inside `<answer>`: `Name | Specialty` or just `Name`.
/// Returns `None` if the response indicates no professional found.
pub fn parse_professional_response(response: &str) -> Option<ExtractedProfessional> {
    let answer = extract_answer(response, "<answer>", "</answer>")
        .unwrap_or_else(|| response.trim().to_string());
    let answer = answer.trim();

    if answer.is_empty() || is_not_specified(answer) {
        return None;
    }

    let parts: Vec<&str> = answer.splitn(2, '|').map(|s| s.trim()).collect();
    let name = parts[0].to_string();
    if name.is_empty() {
        return None;
    }

    let specialty = parts.get(1).filter(|s| !s.is_empty()).map(|s| s.to_string());

    Some(ExtractedProfessional {
        name,
        specialty,
        institution: None,
    })
}

/// Parse document date from LLM response.
///
/// Expected format inside `<answer>`: `YYYY-MM-DD`.
/// Returns `None` if the response indicates no date found or date is unparseable.
pub fn parse_date_response(response: &str) -> Option<String> {
    let answer = extract_answer(response, "<answer>", "</answer>")
        .unwrap_or_else(|| response.trim().to_string());
    let answer = answer.trim();

    if answer.is_empty() || is_not_specified(answer) {
        return None;
    }

    // Validate ISO date format (YYYY-MM-DD)
    let date_re = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}$").ok()?;
    if date_re.is_match(answer) {
        Some(answer.to_string())
    } else {
        // Try to extract a date pattern from a longer response
        let extract_re = regex::Regex::new(r"\d{4}-\d{2}-\d{2}").ok()?;
        extract_re.find(answer).map(|m| m.as_str().to_string())
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
    /// 10-LDC: Updated to recognize locale-based prompt patterns ("analytes" not just "tests").
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

            // Enumerate prompts: detect via "one name per line" or "one per line" or "are visible"
            // 10-LDC: Locale prompts say "analytes" not "tests", "one name per line"
            if lower.contains("one name per line")
                || lower.contains("one per line")
                || lower.contains("per line")
                || lower.contains("are visible")
            {
                // LAB_RESULTS: locale says "analytes" or legacy "tests"
                if lower.contains("analytes") || lower.contains("tests") {
                    return Ok("- Hemoglobine\n- Leucocytes".into());
                }
                // All other domains: nothing found
                return Ok("NONE".into());
            }

            // 10-LDC: Drill prompts use locale patterns.
            // Value drill: "What is the measured value and unit for the analyte 'X'?"
            // Range drill: "What is the reference range (normal values) for 'X'?"
            if lower.contains("value and unit") || lower.contains("measured value") {
                if lower.contains("hemoglobine") {
                    return Ok("11.2 g/dL".into());
                }
                if lower.contains("leucocytes") {
                    return Ok("7.2 G/L".into());
                }
                return Ok("not specified".into());
            }
            if lower.contains("reference range") || lower.contains("normal values") {
                if lower.contains("hemoglobine") {
                    return Ok("(12.0-16.0)".into());
                }
                return Ok("not specified".into());
            }
            // Legacy drill prompts (text-mode, backward compat)
            if lower.contains("what is the") {
                if lower.contains("hemoglobine") && lower.contains("result value") {
                    return Ok("11.2".into());
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
        strategy.extract_from_image(&session, vision, &images, "You are a medical document extractor.", None, doc_type, "en")
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

        // Leucocytes: mock returns "not specified" for range drill.
        // 10-LDC: abnormal_flag only computed when reference range is available.
        let leuco = &output.entities.lab_results[1];
        assert!(leuco.collection_date.is_none());
        assert!(leuco.abnormal_flag.is_none()); // No reference range → no flag
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

            // 10-LDC: Recognize locale enumerate prompts
            if lower.contains("per line") || lower.contains("are visible") {
                if lower.contains("analytes") || lower.contains("tests") {
                    // Degenerate but with useful partial
                    return Err(OllamaError::VisionDegeneration {
                        pattern: "low_diversity".into(),
                        tokens_before_abort: 50,
                        partial_output: "- Glucose".into(),
                    });
                }
                return Ok("NONE".into());
            }

            // 10-LDC: Drill prompts use locale patterns (value+unit combined)
            if (lower.contains("value and unit") || lower.contains("measured value"))
                && lower.contains("glucose")
            {
                return Ok("5.4 mmol/L".into());
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
            "en",
        ).unwrap();

        assert!(!output.entities.lab_results.is_empty());

        // Verify JSONL file was written
        let content = std::fs::read_to_string(&progress_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        // 2 meta + 7 enumerate + N drill calls + 1 summary
        assert!(lines.len() > 9, "Expected > 9 lines, got {}", lines.len());

        // First lines are pre-domain meta calls (12-ERC B4: professional + date)
        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["phase"], "meta");
        assert_eq!(first["call"], 1);

        // First enumerate call follows the 2 meta calls
        let first_enum = lines.iter().position(|line| {
            let v: serde_json::Value = serde_json::from_str(line).unwrap_or_default();
            v["phase"] == "enumerate"
        });
        assert!(first_enum.is_some(), "Expected at least one enumerate entry");

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

    // ── 10-LDC: Parser improvement tests ────────────────────

    #[test]
    fn dedup_case_insensitive() {
        let input = "Hématocrite\nhématocrite\nHÉMATOCRITE\nHémoglobine";
        let items = parse_enumerate_response(input);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0], "Hématocrite"); // first occurrence wins
        assert_eq!(items[1], "Hémoglobine");
    }

    #[test]
    fn dedup_preserves_first_occurrence() {
        let input = "- Glucose\n- glucose\n- GLUCOSE";
        let items = parse_enumerate_response(input);
        assert_eq!(items, vec!["Glucose"]);
    }

    #[test]
    fn thinking_line_detected_numbered() {
        assert!(is_thinking_line("The user asked for lab results"));
        assert!(is_thinking_line("Let me scan the image carefully"));
        assert!(is_thinking_line("I need to identify the tests"));
    }

    #[test]
    fn thinking_line_detected_meta() {
        assert!(is_thinking_line("Check the lab values section"));
        assert!(is_thinking_line("Observe the hematology panel"));
        assert!(is_thinking_line("This is a French lab report"));
    }

    #[test]
    fn real_test_name_not_filtered() {
        assert!(!is_thinking_line("Hémoglobine"));
        assert!(!is_thinking_line("Leucocytes"));
        assert!(!is_thinking_line("V.G.M."));
        assert!(!is_thinking_line("Polynucléaires neutrophiles"));
    }

    #[test]
    fn thinking_lines_removed_from_enumerate() {
        let input = "Here's my analysis of the document:\n\
                     Hémoglobine\n\
                     Let me check for more items\n\
                     Leucocytes\n\
                     Note: some values may be uncertain";
        let items = parse_enumerate_response(input);
        assert_eq!(items, vec!["Hémoglobine", "Leucocytes"]);
    }

    #[test]
    fn none_detection_en() {
        assert!(is_none_response("NONE"));
        assert!(is_none_response("none"));
        assert!(is_none_response("  None  "));
    }

    #[test]
    fn none_detection_fr() {
        assert!(is_none_response("AUCUN"));
        assert!(is_none_response("aucun"));
        assert!(is_none_response("AUCUNE"));
        assert!(is_none_response("aucun résultat"));
    }

    #[test]
    fn none_detection_de() {
        assert!(is_none_response("KEINE"));
        assert!(is_none_response("keine"));
        assert!(is_none_response("keine ergebnisse"));
    }

    #[test]
    fn none_response_returns_empty_enumerate() {
        assert!(parse_enumerate_response("AUCUN").is_empty());
        assert!(parse_enumerate_response("KEINE").is_empty());
    }

    #[test]
    fn parse_value_unit_simple() {
        let (val, unit) = parse_value_with_unit("4.88 T/l").unwrap();
        assert_eq!(val, "4.88");
        assert_eq!(unit, "T/l");
    }

    #[test]
    fn parse_value_unit_gdl() {
        let (val, unit) = parse_value_with_unit("13.3 g/dL").unwrap();
        assert_eq!(val, "13.3");
        assert_eq!(unit, "g/dL");
    }

    #[test]
    fn parse_value_unit_spaces_in_value() {
        let (val, unit) = parse_value_with_unit("6 160 /mm3").unwrap();
        assert_eq!(val, "6160");
        assert_eq!(unit, "/mm3");
    }

    #[test]
    fn parse_value_unit_no_unit() {
        // Pure number — no unit found
        assert!(parse_value_with_unit("4.88").is_none());
    }

    #[test]
    fn parse_value_unit_not_specified() {
        assert!(parse_value_with_unit("not specified").is_none());
    }

    #[test]
    fn parse_range_parens() {
        let (low, high) = parse_reference_range("(4.28-6.00)").unwrap();
        assert!((low - 4.28).abs() < 0.001);
        assert!((high - 6.00).abs() < 0.001);
    }

    #[test]
    fn parse_range_french_comma() {
        let (low, high) = parse_reference_range("(4,28-6,00)").unwrap();
        assert!((low - 4.28).abs() < 0.001);
        assert!((high - 6.00).abs() < 0.001);
    }

    #[test]
    fn parse_range_less_than() {
        let (low, high) = parse_reference_range("(<630)").unwrap();
        assert!((low - 0.0).abs() < 0.001);
        assert!((high - 630.0).abs() < 0.001);
    }

    #[test]
    fn parse_range_spaces_in_numbers() {
        let (low, high) = parse_reference_range("(1 800-6 900)").unwrap();
        assert!((low - 1800.0).abs() < 0.001);
        assert!((high - 6900.0).abs() < 0.001);
    }

    #[test]
    fn parse_range_no_parens() {
        let (low, high) = parse_reference_range("13.4-16.7").unwrap();
        assert!((low - 13.4).abs() < 0.001);
        assert!((high - 16.7).abs() < 0.001);
    }

    #[test]
    fn parse_range_not_specified() {
        assert!(parse_reference_range("not specified").is_none());
    }

    #[test]
    fn french_number_comma_decimal() {
        assert!((parse_french_number("4,28").unwrap() - 4.28).abs() < 0.001);
    }

    #[test]
    fn french_number_space_thousands() {
        assert!((parse_french_number("6 160").unwrap() - 6160.0).abs() < 0.001);
    }

    #[test]
    fn french_number_both() {
        assert!((parse_french_number("1 234,56").unwrap() - 1234.56).abs() < 0.001);
    }

    #[test]
    fn compute_flag_normal() {
        assert_eq!(compute_abnormal_flag(5.0, 4.0, 6.0), "normal");
    }

    #[test]
    fn compute_flag_low() {
        assert_eq!(compute_abnormal_flag(3.5, 4.0, 6.0), "low");
    }

    #[test]
    fn compute_flag_high() {
        assert_eq!(compute_abnormal_flag(7.0, 4.0, 6.0), "high");
    }

    #[test]
    fn compute_flag_critical_low() {
        assert_eq!(compute_abnormal_flag(1.5, 4.0, 6.0), "critical_low");
    }

    #[test]
    fn compute_flag_critical_high() {
        assert_eq!(compute_abnormal_flag(13.0, 4.0, 6.0), "critical_high");
    }

    #[test]
    fn compute_flag_boundary_normal() {
        // Exactly at low boundary → normal (not low)
        assert_eq!(compute_abnormal_flag(4.0, 4.0, 6.0), "normal");
        // Exactly at high boundary → normal (not high)
        assert_eq!(compute_abnormal_flag(6.0, 4.0, 6.0), "normal");
    }

    // ── 10-LDC: Language integration tests ──────────────

    /// Helper to run drill with a specific language code.
    fn run_vision_drill_with_lang(
        vision: &dyn VisionClient,
        lang: &str,
    ) -> Result<StrategyOutput, SessionError> {
        let session = FallbackSession::new("medgemma:4b", ContextType::NightBatch, false);
        let strategy = IterativeDrillStrategy::new(0);
        let images = vec!["base64_image_data".to_string()];
        strategy.extract_from_image(&session, vision, &images, "", None, None, lang)
    }

    /// Mock that verifies French prompts are used and responds in French.
    struct FrenchPromptVerifier;

    impl VisionClient for FrenchPromptVerifier {
        fn generate_with_images(&self, _: &str, _: &str, _: &[String], _: Option<&str>) -> Result<String, OllamaError> {
            Ok(String::new())
        }
        fn chat_with_images(&self, _: &str, prompt: &str, _: &[String], system: Option<&str>) -> Result<String, OllamaError> {
            let lower = prompt.to_lowercase();

            // 12-ERC B4: pre-domain meta calls (professional, date) — respond AUCUN
            if lower.contains("professionnel") || lower.contains("professional")
                || lower.contains("date de ce") || lower.contains("date of this") {
                return Ok("AUCUN".into());
            }

            // Enumerate: verify French prompt pattern
            if lower.contains("par ligne") || lower.contains("per line") || lower.contains("are visible") {
                if lower.contains("analyses biologiques") {
                    // French lab results enumerate prompt recognized
                    return Ok("- Hémoglobine\n- Créatinine".into());
                }
                return Ok("AUCUN".into());
            }

            // Drill: verify French prompt and return French-style results
            if lower.contains("valeur mesurée") || lower.contains("value and unit") {
                if lower.contains("hémoglobine") {
                    return Ok("13,5 g/dL".into());
                }
                if lower.contains("créatinine") {
                    return Ok("72 µmol/L".into());
                }
                return Ok("non spécifié".into());
            }
            if lower.contains("plage de référence") || lower.contains("reference range") {
                if lower.contains("hémoglobine") {
                    return Ok("(12,0-16,0)".into());
                }
                return Ok("non spécifié".into());
            }

            // Verify system prompt is French
            if let Some(sys) = system {
                assert!(
                    sys.contains("extraction") || sys.contains("valeur"),
                    "System prompt should be French, got: {sys}"
                );
            }

            Ok("non spécifié".into())
        }
    }

    #[test]
    fn drill_uses_locale_fr() {
        let mock = FrenchPromptVerifier;
        let output = run_vision_drill_with_lang(&mock, "fr").unwrap();

        // French enumerate returned French test names
        assert_eq!(output.entities.lab_results.len(), 2);
        assert_eq!(output.entities.lab_results[0].test_name, "Hémoglobine");
        assert_eq!(output.entities.lab_results[1].test_name, "Créatinine");

        // French comma decimal parsed correctly
        assert_eq!(output.entities.lab_results[0].value, Some(13.5));
        assert_eq!(output.entities.lab_results[0].unit.as_deref(), Some("g/dL"));

        // Reference range parsed from French format
        assert_eq!(output.entities.lab_results[0].reference_range_low, Some(12.0));
        assert_eq!(output.entities.lab_results[0].reference_range_high, Some(16.0));
        assert_eq!(output.entities.lab_results[0].abnormal_flag.as_deref(), Some("normal"));
    }

    #[test]
    fn drill_uses_locale_en() {
        // VisionDrillMock already uses EN locale patterns
        let mock = VisionDrillMock;
        let output = run_vision_drill_with_lang(&mock, "en").unwrap();

        // EN locale returns English-style analyte names
        assert_eq!(output.entities.lab_results.len(), 2);
        assert_eq!(output.entities.lab_results[0].test_name, "Hemoglobine");
        assert_eq!(output.entities.lab_results[0].value, Some(11.2));
        assert_eq!(output.entities.lab_results[0].unit.as_deref(), Some("g/dL"));
    }

    #[test]
    fn drill_field_reduction_2_groups_per_lab() {
        // Verify that lab results use exactly 2 drill groups (value+unit, range)
        // not the old 6 per-field drills
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct DrillCountingVision {
            enumerate_count: AtomicUsize,
            drill_count: AtomicUsize,
        }

        impl VisionClient for DrillCountingVision {
            fn generate_with_images(&self, _: &str, _: &str, _: &[String], _: Option<&str>) -> Result<String, OllamaError> {
                Ok(String::new())
            }
            fn chat_with_images(&self, _: &str, prompt: &str, _: &[String], _: Option<&str>) -> Result<String, OllamaError> {
                let lower = prompt.to_lowercase();
                if lower.contains("per line") || lower.contains("are visible") {
                    self.enumerate_count.fetch_add(1, Ordering::Relaxed);
                    if lower.contains("analytes") || lower.contains("tests") {
                        return Ok("Glucose".into());
                    }
                    return Ok("NONE".into());
                }
                // 12-ERC B4: pre-domain meta calls (professional, date) — not counted as drill
                if lower.contains("professional") || lower.contains("specialty")
                    || lower.contains("date of this") || lower.contains("date de ce") {
                    return Ok("NONE".into());
                }
                // Count domain drill calls
                self.drill_count.fetch_add(1, Ordering::Relaxed);
                if lower.contains("value and unit") || lower.contains("measured value") {
                    return Ok("5.4 mmol/L".into());
                }
                if lower.contains("reference range") || lower.contains("normal values") {
                    return Ok("(3.9-5.5)".into());
                }
                Ok("not specified".into())
            }
        }

        let mock = DrillCountingVision {
            enumerate_count: AtomicUsize::new(0),
            drill_count: AtomicUsize::new(0),
        };
        let output = run_vision_drill_with_lang(&mock, "en").unwrap();

        // 1 lab result extracted
        assert_eq!(output.entities.lab_results.len(), 1);
        assert_eq!(output.entities.lab_results[0].test_name, "Glucose");
        assert_eq!(output.entities.lab_results[0].value, Some(5.4));

        // 10-LDC field reduction: exactly 2 drill calls per lab item (value+unit, range)
        assert_eq!(mock.drill_count.load(Ordering::Relaxed), 2,
            "Expected 2 drill groups (value+unit, range), got {}",
            mock.drill_count.load(Ordering::Relaxed));
    }

    // ═══════════════════════════════════════════════════════════
    // 11-SRP Brick 1: Sanitization bug fix tests
    // ═══════════════════════════════════════════════════════════

    /// Mock that returns degeneration with <unused94>thought in partial output.
    struct VisionDegenWithThinking;

    impl VisionClient for VisionDegenWithThinking {
        fn generate_with_images(&self, _: &str, _: &str, _: &[String], _: Option<&str>) -> Result<String, OllamaError> {
            Ok(String::new())
        }
        fn chat_with_images(&self, _: &str, prompt: &str, _: &[String], _: Option<&str>) -> Result<String, OllamaError> {
            let lower = prompt.to_lowercase();
            if lower.contains("per line") || lower.contains("are visible") {
                if lower.contains("analytes") || lower.contains("tests") {
                    // B1: Degeneration with <unused94>thought prefix — must be sanitized
                    return Err(OllamaError::VisionDegeneration {
                        pattern: "sequence_repeat".into(),
                        tokens_before_abort: 65,
                        partial_output: "<unused94>thought\nThe user wants\nHémoglobine\nGlucose".into(),
                    });
                }
                return Ok("NONE".into());
            }
            // Drill calls — return clean values
            if lower.contains("value and unit") || lower.contains("measured value") {
                return Ok("5.4 mmol/L".into());
            }
            Ok("not specified".into())
        }
    }

    #[test]
    fn degeneration_partial_output_sanitized() {
        // B1: <unused94>thought in degeneration partial output must be stripped
        // Before fix: "<unused94>thought" becomes an item name (phantom)
        // After fix: sanitize_llm_output() strips it, leaving real item names
        let mock = VisionDegenWithThinking;
        let output = run_vision_drill(&mock).unwrap();

        // Should extract Hémoglobine and/or Glucose (real names), not "<unused94>thought"
        for lab in &output.entities.lab_results {
            assert!(
                !lab.test_name.contains("<unused"),
                "Unsanitized thinking token leaked as item name: {}",
                lab.test_name
            );
            assert!(
                !lab.test_name.contains("thought"),
                "Thinking marker leaked as item name: {}",
                lab.test_name
            );
            assert!(
                !lab.test_name.to_lowercase().contains("the user"),
                "Thinking text leaked as item name: {}",
                lab.test_name
            );
        }
    }

    #[test]
    fn quality_gate_raw_output_sanitized() {
        // B3: Quality gate raw output also goes through sanitize_llm_output
        // Verify by checking that sanitize_llm_output strips thinking prefix
        use crate::pipeline::safety::output_sanitize::sanitize_llm_output;
        let raw = "<unused94>thought\nLet me analyze this...\nHémoglobine";
        let sanitized = sanitize_llm_output(raw);
        assert!(!sanitized.contains("<unused94>"));
        assert!(!sanitized.starts_with("thought"));
        assert!(sanitized.contains("Hémoglobine"));
    }

    #[test]
    fn drill_response_sanitized() {
        // B2: Drill response should not contain <unused> tokens
        use crate::pipeline::safety::output_sanitize::sanitize_llm_output;
        let raw = "<unused94>thought\nThe value is 13.3\n13.3 g/dl";
        let sanitized = sanitize_llm_output(raw);
        assert!(!sanitized.contains("<unused"));
        assert!(sanitized.contains("13.3 g/dl"));
    }

    // ═══════════════════════════════════════════════════════════
    // 11-SRP Brick 4: Structural parser tests
    // ═══════════════════════════════════════════════════════════

    #[test]
    fn extract_answer_basic() {
        let response = "<think>\nI see Hemoglobin and WBC.\n</think>\n<answer>\nHemoglobin\nWBC\n</answer>";
        let result = extract_answer(response, "<answer>", "</answer>");
        assert_eq!(result.unwrap(), "Hemoglobin\nWBC");
    }

    #[test]
    fn extract_answer_empty() {
        let response = "<think>nothing here</think>\n<answer></answer>";
        let result = extract_answer(response, "<answer>", "</answer>");
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn extract_answer_none_when_missing() {
        let response = "I see Hemoglobin and WBC. No answer tokens used.";
        let result = extract_answer(response, "<answer>", "</answer>");
        assert!(result.is_none());
    }

    #[test]
    fn extract_answer_only_start_token() {
        let response = "<answer>Hemoglobin\nWBC";
        let result = extract_answer(response, "<answer>", "</answer>");
        assert!(result.is_none(), "Should return None when end token is missing");
    }

    #[test]
    fn extract_answer_inline() {
        let response = "<think>quick check</think><answer>13.3 g/dl</answer>";
        let result = extract_answer(response, "<answer>", "</answer>");
        assert_eq!(result.unwrap(), "13.3 g/dl");
    }

    #[test]
    fn parse_enumerate_structured_with_tokens() {
        let response = "<think>\nI see a hematology section.\n</think>\n<answer>\nHémoglobine\nHématocrite\nLeucocytes\n</answer>";
        let items = parse_enumerate_structured(response, "<answer>", "</answer>", "NONE");
        assert_eq!(items, vec!["Hémoglobine", "Hématocrite", "Leucocytes"]);
    }

    #[test]
    fn parse_enumerate_structured_none_response() {
        let response = "<think>\nNo lab results visible.\n</think>\n<answer>NONE</answer>";
        let items = parse_enumerate_structured(response, "<answer>", "</answer>", "NONE");
        assert!(items.is_empty());
    }

    #[test]
    fn parse_enumerate_structured_aucun_response() {
        let response = "<think>\nAucune analyse visible.\n</think>\n<answer>AUCUN</answer>";
        let items = parse_enumerate_structured(response, "<answer>", "</answer>", "AUCUN");
        assert!(items.is_empty());
    }

    #[test]
    fn parse_enumerate_structured_keine_response() {
        let response = "<think>\nKeine Parameter sichtbar.\n</think>\n<answer>KEINE</answer>";
        let items = parse_enumerate_structured(response, "<answer>", "</answer>", "KEINE");
        assert!(items.is_empty());
    }

    #[test]
    fn parse_enumerate_structured_fallback_without_tokens() {
        // When model doesn't produce answer tokens, falls back to legacy parser
        let response = "- Hemoglobin\n- WBC\n- Platelets";
        let items = parse_enumerate_structured(response, "<answer>", "</answer>", "NONE");
        assert_eq!(items, vec!["Hemoglobin", "WBC", "Platelets"]);
    }

    #[test]
    fn parse_enumerate_structured_dedup() {
        let response = "<answer>\nHemoglobin\nhemoglobin\nWBC\n</answer>";
        let items = parse_enumerate_structured(response, "<answer>", "</answer>", "NONE");
        assert_eq!(items, vec!["Hemoglobin", "WBC"]);
    }

    #[test]
    fn parse_enumerate_structured_strips_bullets() {
        let response = "<answer>\n- Hémoglobine\n- Hématocrite\n</answer>";
        let items = parse_enumerate_structured(response, "<answer>", "</answer>", "NONE");
        assert_eq!(items, vec!["Hémoglobine", "Hématocrite"]);
    }

    #[test]
    fn parse_enumerate_structured_max_items() {
        let lines: Vec<String> = (0..35).map(|i| format!("Item{i}")).collect();
        let response = format!("<answer>\n{}\n</answer>", lines.join("\n"));
        let items = parse_enumerate_structured(&response, "<answer>", "</answer>", "NONE");
        assert_eq!(items.len(), MAX_ITEMS);
    }

    #[test]
    fn parse_enumerate_structured_long_lines_filtered() {
        let long_line = "A".repeat(61);
        let response = format!("<answer>\nHemoglobin\n{long_line}\nWBC\n</answer>");
        let items = parse_enumerate_structured(&response, "<answer>", "</answer>", "NONE");
        assert_eq!(items, vec!["Hemoglobin", "WBC"]);
    }

    #[test]
    fn parse_enumerate_structured_thinking_ignored() {
        // Chain-of-thought in <think> section never reaches item parsing
        let response = "<think>\nThe user wants me to extract lab test names.\nI see HEMATIES 4.88 T/l.\nI see Hémoglobine 13.3 g/dl.\n</think>\n<answer>\nHématies\nHémoglobine\n</answer>";
        let items = parse_enumerate_structured(response, "<answer>", "</answer>", "NONE");
        assert_eq!(items, vec!["Hématies", "Hémoglobine"]);
        // Verify thinking text was not included
        for item in &items {
            assert!(!item.to_lowercase().contains("user wants"));
            assert!(!item.contains("4.88"));
        }
    }

    #[test]
    fn extract_answer_drill_value_with_thinking() {
        // Simulates a drill response where model reasons then answers
        let response = "<think>\nThe measured value is 13.3 g/dl for Hémoglobine.\n</think>\n<answer>13.3 g/dl</answer>";
        let value = extract_answer(response, "<answer>", "</answer>");
        assert_eq!(value.unwrap(), "13.3 g/dl");
    }

    #[test]
    fn extract_answer_drill_range_with_thinking() {
        let response = "<think>\nThe reference range is 4.28 to 6.00.\n</think>\n<answer>4,28-6,00</answer>";
        let value = extract_answer(response, "<answer>", "</answer>");
        assert_eq!(value.unwrap(), "4,28-6,00");
    }

    // ═══════════════════════════════════════════════════════════
    // 11-SRP Brick 5: Degeneration with answer token tests
    // ═══════════════════════════════════════════════════════════

    #[test]
    fn degeneration_with_answer_extracted() {
        // Model produced <answer> then degenerated — answer content is valid
        let partial = "<think>\nI see tests.\n</think>\n<answer>\nHémoglobine\nHématocrite\n</answer>\nloop loop loop";
        let result = extract_answer(partial, "<answer>", "</answer>");
        assert_eq!(result.unwrap(), "Hémoglobine\nHématocrite");
    }

    #[test]
    fn degeneration_without_answer_returns_none() {
        // Model degenerated inside <think> before reaching <answer>
        let partial = "<think>\nThe user wants me to loop loop loop loop loop";
        let result = extract_answer(partial, "<answer>", "</answer>");
        assert!(result.is_none());
    }

    #[test]
    fn degeneration_answer_then_repeat() {
        // Model answered correctly, then degenerated in post-answer text
        let partial = "<think>analysis</think><answer>Glucose</answer><think>loop loop loop loop";
        let result = extract_answer(partial, "<answer>", "</answer>");
        assert_eq!(result.unwrap(), "Glucose");
    }

    #[test]
    fn drill_degeneration_with_answer() {
        // Drill partial output with valid <answer> — value should be extracted
        let partial = "<think>The value is</think><answer>13.3 g/dl</answer> loop loop";
        let value = extract_answer(partial, "<answer>", "</answer>")
            .unwrap_or(partial.to_string());
        assert_eq!(value, "13.3 g/dl");
    }

    // ═══════════════════════════════════════════════════════════
    // 11-SRP Brick 6: Integration tests (structured mock)
    // ═══════════════════════════════════════════════════════════

    /// Mock that returns structured <think>/<answer> responses like a model
    /// trained with 11-SRP system prompts.
    struct StructuredVisionMock;

    impl VisionClient for StructuredVisionMock {
        fn generate_with_images(&self, _: &str, _: &str, _: &[String], _: Option<&str>) -> Result<String, OllamaError> {
            Ok(String::new())
        }
        fn chat_with_images(&self, _: &str, prompt: &str, _: &[String], _system: Option<&str>) -> Result<String, OllamaError> {
            let lower = prompt.to_lowercase();

            // Enumerate: detect via "per line"
            if lower.contains("per line") || lower.contains("par ligne") {
                if lower.contains("analytes") || lower.contains("analyses biologiques") {
                    return Ok(
                        "<think>\nI see HEMATOLOGIE section with several tests.\n</think>\n\
                         <answer>\nHématies\nHémoglobine\nHématocrite\n</answer>".into()
                    );
                }
                if lower.contains("diagnos") {
                    return Ok("<answer>NONE</answer>".into());
                }
                if lower.contains("médicaments") || lower.contains("medications") {
                    return Ok("<answer>NONE</answer>".into());
                }
                return Ok("<answer>NONE</answer>".into());
            }

            // Value drill
            if lower.contains("value and unit") || lower.contains("measured value")
                || lower.contains("valeur mesurée")
            {
                if lower.contains("hématies") {
                    return Ok("<think>I see 4.88 T/l</think><answer>4.88 T/l</answer>".into());
                }
                if lower.contains("hémoglobine") {
                    return Ok("<think>I see 13.3 g/dl</think><answer>13.3 g/dl</answer>".into());
                }
                if lower.contains("hématocrite") {
                    return Ok("<think>I see 41.0 %</think><answer>41.0 %</answer>".into());
                }
                return Ok("<answer>not specified</answer>".into());
            }

            // Range drill
            if lower.contains("reference range") || lower.contains("normal values")
                || lower.contains("plage de référence")
            {
                if lower.contains("hématies") {
                    return Ok("<think>Range is 4.28 to 6.00</think><answer>(4,28-6,00)</answer>".into());
                }
                if lower.contains("hémoglobine") {
                    return Ok("<think>Range is 13.4 to 16.7</think><answer>(13,4-16,7)</answer>".into());
                }
                if lower.contains("hématocrite") {
                    return Ok("<think>Range is 38 to 49</think><answer>(38-49)</answer>".into());
                }
                return Ok("<answer>not specified</answer>".into());
            }

            Ok("<answer>not specified</answer>".into())
        }
    }

    #[test]
    fn structured_enumerate_extracts_items() {
        let mock = StructuredVisionMock;
        let output = run_vision_drill(&mock).unwrap();

        // Should extract 3 items from the <answer> block
        assert_eq!(output.entities.lab_results.len(), 3,
            "Expected 3 lab results, got {}: {:?}",
            output.entities.lab_results.len(),
            output.entities.lab_results.iter().map(|l| &l.test_name).collect::<Vec<_>>()
        );
        assert_eq!(output.entities.lab_results[0].test_name, "Hématies");
        assert_eq!(output.entities.lab_results[1].test_name, "Hémoglobine");
        assert_eq!(output.entities.lab_results[2].test_name, "Hématocrite");
    }

    #[test]
    fn structured_none_skips_domains() {
        let mock = StructuredVisionMock;
        let output = run_vision_drill(&mock).unwrap();

        // Diagnoses returned <answer>NONE</answer> → 0 items
        assert!(output.entities.diagnoses.is_empty());
        assert!(output.entities.medications.is_empty());
    }

    #[test]
    fn structured_drill_extracts_values() {
        let mock = StructuredVisionMock;
        let output = run_vision_drill(&mock).unwrap();

        // Hémoglobine: <answer>13.3 g/dl</answer> → parsed correctly
        let hb = &output.entities.lab_results[1];
        assert_eq!(hb.test_name, "Hémoglobine");
        assert_eq!(hb.value, Some(13.3));
        assert_eq!(hb.unit.as_deref(), Some("g/dl"));
    }

    #[test]
    fn structured_drill_extracts_ranges() {
        let mock = StructuredVisionMock;
        let output = run_vision_drill(&mock).unwrap();

        // Hémoglobine: <answer>(13,4-16,7)</answer> → parsed as range
        let hb = &output.entities.lab_results[1];
        assert_eq!(hb.reference_range_low, Some(13.4));
        assert_eq!(hb.reference_range_high, Some(16.7));
    }

    #[test]
    fn structured_drill_computes_abnormal_flag() {
        let mock = StructuredVisionMock;
        let output = run_vision_drill(&mock).unwrap();

        // Hémoglobine: value 13.3, range 13.4-16.7 → low (below range)
        let hb = &output.entities.lab_results[1];
        assert_eq!(hb.abnormal_flag.as_deref(), Some("low"),
            "13.3 is below range 13.4-16.7, expected 'low' but got {:?}",
            hb.abnormal_flag
        );
    }

    #[test]
    fn structured_no_phantom_items() {
        let mock = StructuredVisionMock;
        let output = run_vision_drill(&mock).unwrap();

        // No thinking text should appear as item names
        for lab in &output.entities.lab_results {
            assert!(!lab.test_name.to_lowercase().contains("i see"),
                "Thinking text leaked as item name: {}", lab.test_name);
            assert!(!lab.test_name.to_lowercase().contains("section"),
                "Thinking text leaked as item name: {}", lab.test_name);
        }
        // No phantom diagnoses
        assert!(output.entities.diagnoses.is_empty());
    }

    // --- 12-ERC Brick 4: parse_professional_response ---

    #[test]
    fn parse_professional_with_specialty() {
        let response = "<think>I see a doctor name.</think><answer>Dr. Martin | Cardiology</answer>";
        let prof = parse_professional_response(response).unwrap();
        assert_eq!(prof.name, "Dr. Martin");
        assert_eq!(prof.specialty, Some("Cardiology".into()));
    }

    #[test]
    fn parse_professional_name_only() {
        let response = "<answer>Dr. Dupont</answer>";
        let prof = parse_professional_response(response).unwrap();
        assert_eq!(prof.name, "Dr. Dupont");
        assert_eq!(prof.specialty, None);
    }

    #[test]
    fn parse_professional_none() {
        assert!(parse_professional_response("<answer>NONE</answer>").is_none());
        assert!(parse_professional_response("<answer>Not specified</answer>").is_none());
        assert!(parse_professional_response("<answer></answer>").is_none());
    }

    #[test]
    fn parse_professional_raw_no_tags() {
        // Fallback: no tags, raw text
        let prof = parse_professional_response("Dr. Smith | Radiology").unwrap();
        assert_eq!(prof.name, "Dr. Smith");
        assert_eq!(prof.specialty, Some("Radiology".into()));
    }

    // --- 12-ERC Brick 4: parse_date_response ---

    #[test]
    fn parse_date_iso() {
        let response = "<think>The date is visible.</think><answer>2024-03-15</answer>";
        let date = parse_date_response(response).unwrap();
        assert_eq!(date, "2024-03-15");
    }

    #[test]
    fn parse_date_none() {
        assert!(parse_date_response("<answer>NONE</answer>").is_none());
        assert!(parse_date_response("<answer>Not specified</answer>").is_none());
        assert!(parse_date_response("<answer>AUCUN</answer>").is_none());
    }
}
