//! STR-01: Markdown response parsers for element-focused extraction.
//!
//! Pure functions that convert SLM markdown list responses into typed entity
//! vectors. Each domain has its own parser that reads bullet-point responses
//! and extracts known fields.
//!
//! The SLM produces simple markdown lists. The CODE parses them into types.
//! Lenient: missing fields default to None/empty. Never errors on valid text.

use crate::pipeline::prompt_templates::DocumentDomain;
use crate::pipeline::structuring::types::{
    ExtractedAllergy, ExtractedDiagnosis, ExtractedEntities, ExtractedInstruction,
    ExtractedLabResult, ExtractedMedication, ExtractedProcedure, ExtractedReferral,
};

// ═══════════════════════════════════════════════════════════
// Domain entities enum
// ═══════════════════════════════════════════════════════════

/// Typed domain extraction result from a single domain parse.
pub enum DomainEntities {
    Medications(Vec<ExtractedMedication>),
    LabResults(Vec<ExtractedLabResult>),
    Diagnoses(Vec<ExtractedDiagnosis>),
    Allergies(Vec<ExtractedAllergy>),
    Procedures(Vec<ExtractedProcedure>),
    Referrals(Vec<ExtractedReferral>),
    Instructions(Vec<ExtractedInstruction>),
}

impl DomainEntities {
    /// Merge this domain's entities into an `ExtractedEntities` aggregate.
    pub fn merge_into(self, entities: &mut ExtractedEntities) {
        match self {
            Self::Medications(v) => entities.medications.extend(v),
            Self::LabResults(v) => entities.lab_results.extend(v),
            Self::Diagnoses(v) => entities.diagnoses.extend(v),
            Self::Allergies(v) => entities.allergies.extend(v),
            Self::Procedures(v) => entities.procedures.extend(v),
            Self::Referrals(v) => entities.referrals.extend(v),
            Self::Instructions(v) => entities.instructions.extend(v),
        }
    }
}

// ═══════════════════════════════════════════════════════════
// Dispatcher
// ═══════════════════════════════════════════════════════════

/// Parse an LLM response for a specific domain into typed entities.
pub fn parse_domain_response(domain: DocumentDomain, response: &str) -> DomainEntities {
    match domain {
        DocumentDomain::Medications => DomainEntities::Medications(parse_medications_markdown(response)),
        DocumentDomain::LabResults => DomainEntities::LabResults(parse_lab_results_markdown(response)),
        DocumentDomain::Diagnoses => DomainEntities::Diagnoses(parse_diagnoses_markdown(response)),
        DocumentDomain::Allergies => DomainEntities::Allergies(parse_allergies_markdown(response)),
        DocumentDomain::Procedures => DomainEntities::Procedures(parse_procedures_markdown(response)),
        DocumentDomain::Referrals => DomainEntities::Referrals(parse_referrals_markdown(response)),
        DocumentDomain::Instructions => DomainEntities::Instructions(parse_instructions_markdown(response)),
    }
}

// ═══════════════════════════════════════════════════════════
// Shared parsing helpers
// ═══════════════════════════════════════════════════════════

/// Split a response into top-level bullet items.
///
/// Recognizes `- `, `* `, and numbered `1. ` as item markers.
/// Sub-bullets (indented) are grouped with their parent.
fn split_bullets(response: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();

    for line in response.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Check if this is a new top-level bullet
        let is_top_level = is_top_level_bullet(line);

        if is_top_level && !current.is_empty() {
            items.push(current.trim().to_string());
            current = String::new();
        }

        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(trimmed);
    }

    if !current.trim().is_empty() {
        items.push(current.trim().to_string());
    }

    items
}

/// Check if a line is a top-level bullet (not indented).
fn is_top_level_bullet(line: &str) -> bool {
    // Must start at column 0 (no leading whitespace) or with at most 1 space
    let leading_spaces = line.len() - line.trim_start().len();
    if leading_spaces > 1 {
        return false;
    }
    let trimmed = line.trim_start();
    trimmed.starts_with("- ")
        || trimmed.starts_with("* ")
        || trimmed.starts_with("• ")
        || is_numbered_bullet(trimmed)
}

/// Check if a line starts with a numbered bullet (e.g., "1. ", "2. ").
fn is_numbered_bullet(trimmed: &str) -> bool {
    let mut chars = trimmed.chars();
    // Must start with a digit
    match chars.next() {
        Some(c) if c.is_ascii_digit() => {}
        _ => return false,
    }
    // Skip remaining digits
    for c in chars.by_ref() {
        if c == '.' {
            // Next must be a space
            return chars.next() == Some(' ');
        }
        if !c.is_ascii_digit() {
            return false;
        }
    }
    false
}

/// Strip bullet marker from the first line of an item.
fn strip_bullet(text: &str) -> &str {
    let trimmed = text.trim_start();
    if let Some(rest) = trimmed.strip_prefix("- ") {
        return rest;
    }
    if let Some(rest) = trimmed.strip_prefix("* ") {
        return rest;
    }
    if let Some(rest) = trimmed.strip_prefix("• ") {
        return rest;
    }
    // Numbered bullet: skip "N. "
    if let Some(dot_pos) = trimmed.find(". ") {
        let prefix = &trimmed[..dot_pos];
        if prefix.chars().all(|c| c.is_ascii_digit()) {
            return &trimmed[dot_pos + 2..];
        }
    }
    trimmed
}

/// Extract a field value from a bullet item by label (case-insensitive).
///
/// Searches for patterns like "dose: 500mg", "Dose: 500mg", "dose — 500mg".
fn extract_field(item: &str, label: &str) -> Option<String> {
    let lower = item.to_lowercase();
    let label_lower = label.to_lowercase();

    // Try "label: value" and "label — value"
    for sep in [":", "—", "-"] {
        let pattern = format!("{label_lower}{sep}");
        if let Some(pos) = lower.find(&pattern) {
            let value_start = pos + pattern.len();
            let value = &item[value_start..];
            // Take until newline or next field marker
            let value = value
                .lines()
                .next()
                .unwrap_or("")
                .trim();
            if !value.is_empty() && !is_not_specified(value) {
                return Some(value.to_string());
            }
        }
    }

    // Try sub-bullet "- label: value"
    for line in item.lines() {
        let line_trimmed = line.trim();
        let line_lower = line_trimmed.to_lowercase();
        if line_lower.starts_with(&format!("- {label_lower}"))
            || line_lower.starts_with(&format!("* {label_lower}"))
        {
            let after_label = if let Some(colon_pos) = line_trimmed.find(':') {
                line_trimmed[colon_pos + 1..].trim()
            } else {
                let skip = line_trimmed.find(' ').map(|p| p + 1).unwrap_or(0);
                line_trimmed[skip..].trim()
            };
            if !after_label.is_empty() && !is_not_specified(after_label) {
                return Some(after_label.to_string());
            }
        }
    }

    None
}

/// Check if a value indicates "not specified" / "none" / "N/A".
fn is_not_specified(value: &str) -> bool {
    let lower = value.to_lowercase().trim().to_string();
    matches!(
        lower.as_str(),
        "not specified" | "n/a" | "none" | "not mentioned" | "not available"
            | "not provided" | "unknown" | "non spécifié" | "aucun" | "néant"
    )
}

/// Extract the entity name from the first line of a bullet item.
fn extract_name(item: &str) -> String {
    let first_line = strip_bullet(item.lines().next().unwrap_or(""));
    // Clean: remove bold markers, trailing field markers
    let name = first_line
        .replace("**", "")
        .trim()
        .to_string();
    // If the name contains a colon or dash separator, take just the name part
    // e.g., "Metformin - 500mg twice daily" → "Metformin"
    // But keep "Metformin 500mg" as-is (no separator)
    if let Some(pos) = name.find(" - ") {
        return name[..pos].trim().to_string();
    }
    if let Some(pos) = name.find(" — ") {
        return name[..pos].trim().to_string();
    }
    // If name has inline fields after a colon, keep just what's before
    // e.g., "Metformin: dose 500mg" → "Metformin"
    // But not "Metformin 500mg" (no colon)
    if let Some(pos) = name.find(':') {
        let before = name[..pos].trim();
        if !before.is_empty() {
            return before.to_string();
        }
    }
    name
}

/// Try to parse a string as f64.
fn parse_f64(s: &str) -> Option<f64> {
    // Handle comma as decimal separator (French)
    let normalized = s.replace(',', ".");
    normalized.trim().parse().ok()
}

// ═══════════════════════════════════════════════════════════
// Per-domain parsers
// ═══════════════════════════════════════════════════════════

/// Parse medications from markdown list response.
pub fn parse_medications_markdown(response: &str) -> Vec<ExtractedMedication> {
    let bullets = split_bullets(response);
    bullets
        .iter()
        .filter(|b| !b.trim().is_empty())
        .map(|item| {
            let name = extract_name(item);
            ExtractedMedication {
                generic_name: if name.is_empty() { None } else { Some(name) },
                brand_name: extract_field(item, "brand"),
                dose: extract_field(item, "dose").unwrap_or_default(),
                frequency: extract_field(item, "frequency").unwrap_or_default(),
                frequency_type: String::new(),
                route: extract_field(item, "route").unwrap_or_default(),
                reason: extract_field(item, "reason").or_else(|| extract_field(item, "for")),
                instructions: extract_field(item, "instructions")
                    .map(|i| vec![i])
                    .unwrap_or_default(),
                is_compound: false,
                compound_ingredients: vec![],
                tapering_steps: vec![],
                max_daily_dose: extract_field(item, "max"),
                condition: extract_field(item, "condition"),
                confidence: 0.0,
            }
        })
        .collect()
}

/// Parse lab results from markdown list response.
pub fn parse_lab_results_markdown(response: &str) -> Vec<ExtractedLabResult> {
    let bullets = split_bullets(response);
    bullets
        .iter()
        .filter(|b| !b.trim().is_empty())
        .map(|item| {
            let name = extract_name(item);
            let value_str = extract_field(item, "value");
            let value = value_str.as_deref().and_then(parse_f64);
            let value_text = if value.is_none() { value_str } else { None };

            ExtractedLabResult {
                test_name: name,
                test_code: extract_field(item, "code"),
                value,
                value_text,
                unit: extract_field(item, "unit"),
                reference_range_low: extract_field(item, "reference_range_low").as_deref().and_then(parse_f64),
                reference_range_high: extract_field(item, "reference_range_high").as_deref().and_then(parse_f64),
                reference_range_text: extract_field(item, "reference_range")
                    .or_else(|| extract_field(item, "reference")),
                abnormal_flag: extract_field(item, "abnormal")
                    .or_else(|| extract_field(item, "flag")),
                collection_date: extract_field(item, "date"),
                confidence: 0.0,
            }
        })
        .collect()
}

/// Parse diagnoses from markdown list response.
pub fn parse_diagnoses_markdown(response: &str) -> Vec<ExtractedDiagnosis> {
    let bullets = split_bullets(response);
    bullets
        .iter()
        .filter(|b| !b.trim().is_empty())
        .map(|item| {
            let name = extract_name(item);
            ExtractedDiagnosis {
                name,
                icd_code: extract_field(item, "icd")
                    .or_else(|| extract_field(item, "code")),
                date: extract_field(item, "date"),
                status: extract_field(item, "status").unwrap_or_default(),
                confidence: 0.0,
            }
        })
        .collect()
}

/// Parse allergies from markdown list response.
pub fn parse_allergies_markdown(response: &str) -> Vec<ExtractedAllergy> {
    let bullets = split_bullets(response);
    bullets
        .iter()
        .filter(|b| !b.trim().is_empty())
        .map(|item| {
            let name = extract_name(item);
            ExtractedAllergy {
                allergen: name,
                reaction: extract_field(item, "reaction"),
                severity: extract_field(item, "severity"),
                confidence: 0.0,
            }
        })
        .collect()
}

/// Parse procedures from markdown list response.
pub fn parse_procedures_markdown(response: &str) -> Vec<ExtractedProcedure> {
    let bullets = split_bullets(response);
    bullets
        .iter()
        .filter(|b| !b.trim().is_empty())
        .map(|item| {
            let name = extract_name(item);
            let follow_up = extract_field(item, "follow_up")
                .or_else(|| extract_field(item, "follow-up"))
                .or_else(|| extract_field(item, "followup"));
            let follow_up_required = follow_up.as_ref().map_or(false, |v| {
                let lower = v.to_lowercase();
                lower.contains("yes") || lower.contains("required") || lower.contains("oui")
            });
            ExtractedProcedure {
                name,
                date: extract_field(item, "date"),
                outcome: extract_field(item, "outcome"),
                follow_up_required,
                follow_up_date: extract_field(item, "follow_up_date")
                    .or_else(|| extract_field(item, "follow-up date")),
                confidence: 0.0,
            }
        })
        .collect()
}

/// Parse referrals from markdown list response.
pub fn parse_referrals_markdown(response: &str) -> Vec<ExtractedReferral> {
    let bullets = split_bullets(response);
    bullets
        .iter()
        .filter(|b| !b.trim().is_empty())
        .map(|item| {
            let name = extract_name(item);
            ExtractedReferral {
                referred_to: name,
                specialty: extract_field(item, "specialty")
                    .or_else(|| extract_field(item, "spécialité")),
                reason: extract_field(item, "reason")
                    .or_else(|| extract_field(item, "motif")),
                confidence: 0.0,
            }
        })
        .collect()
}

/// Parse instructions from markdown list response.
pub fn parse_instructions_markdown(response: &str) -> Vec<ExtractedInstruction> {
    let bullets = split_bullets(response);
    bullets
        .iter()
        .filter(|b| !b.trim().is_empty())
        .map(|item| {
            let text = strip_bullet(item.lines().next().unwrap_or(""))
                .replace("**", "")
                .trim()
                .to_string();
            ExtractedInstruction {
                text,
                category: extract_field(item, "category").unwrap_or_default(),
            }
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Bullet splitting ─────────────────────────────────

    #[test]
    fn split_simple_bullets() {
        let items = split_bullets("- Item 1\n- Item 2\n- Item 3");
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], "- Item 1");
    }

    #[test]
    fn split_with_sub_bullets() {
        let response = "- Item 1\n  - Sub 1\n  - Sub 2\n- Item 2";
        let items = split_bullets(response);
        assert_eq!(items.len(), 2);
        assert!(items[0].contains("Sub 1"));
    }

    #[test]
    fn split_numbered_bullets() {
        let items = split_bullets("1. First\n2. Second");
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn split_empty_response() {
        let items = split_bullets("");
        assert!(items.is_empty());
    }

    #[test]
    fn split_asterisk_bullets() {
        let items = split_bullets("* Item A\n* Item B");
        assert_eq!(items.len(), 2);
    }

    // ── Field extraction ─────────────────────────────────

    #[test]
    fn extract_field_colon_format() {
        let item = "- Metformin\n  - dose: 500mg\n  - frequency: twice daily";
        assert_eq!(extract_field(item, "dose"), Some("500mg".into()));
        assert_eq!(extract_field(item, "frequency"), Some("twice daily".into()));
    }

    #[test]
    fn extract_field_case_insensitive() {
        let item = "- Test\n  - Dose: 250mg";
        assert_eq!(extract_field(item, "dose"), Some("250mg".into()));
    }

    #[test]
    fn extract_field_missing() {
        let item = "- Metformin";
        assert_eq!(extract_field(item, "dose"), None);
    }

    #[test]
    fn extract_field_not_specified_returns_none() {
        let item = "- Test\n  - dose: not specified";
        assert_eq!(extract_field(item, "dose"), None);
    }

    // ── Name extraction ──────────────────────────────────

    #[test]
    fn extract_name_simple() {
        assert_eq!(extract_name("- Metformin"), "Metformin");
    }

    #[test]
    fn extract_name_bold() {
        assert_eq!(extract_name("- **Metformin**"), "Metformin");
    }

    #[test]
    fn extract_name_with_dash_separator() {
        assert_eq!(extract_name("- Metformin - 500mg"), "Metformin");
    }

    // ── Medications parser ───────────────────────────────

    #[test]
    fn medications_happy_path() {
        let response = "\
- Metformin
  - dose: 500mg
  - frequency: twice daily
  - route: oral
  - instructions: Take with food";
        let meds = parse_medications_markdown(response);
        assert_eq!(meds.len(), 1);
        assert_eq!(meds[0].generic_name.as_deref(), Some("Metformin"));
        assert_eq!(meds[0].dose, "500mg");
        assert_eq!(meds[0].frequency, "twice daily");
        assert_eq!(meds[0].route, "oral");
    }

    #[test]
    fn medications_multi_item() {
        let response = "\
- Metformin
  - dose: 500mg
- Lisinopril
  - dose: 10mg";
        let meds = parse_medications_markdown(response);
        assert_eq!(meds.len(), 2);
        assert_eq!(meds[0].generic_name.as_deref(), Some("Metformin"));
        assert_eq!(meds[1].generic_name.as_deref(), Some("Lisinopril"));
    }

    #[test]
    fn medications_missing_fields() {
        let response = "- Aspirin";
        let meds = parse_medications_markdown(response);
        assert_eq!(meds.len(), 1);
        assert_eq!(meds[0].generic_name.as_deref(), Some("Aspirin"));
        assert!(meds[0].dose.is_empty());
    }

    #[test]
    fn medications_empty_response() {
        let meds = parse_medications_markdown("");
        assert!(meds.is_empty());
    }

    #[test]
    fn medications_with_reason() {
        let response = "- Metformin\n  - dose: 500mg\n  - reason: Type 2 diabetes";
        let meds = parse_medications_markdown(response);
        assert_eq!(meds[0].reason.as_deref(), Some("Type 2 diabetes"));
    }

    // ── Lab results parser ───────────────────────────────

    #[test]
    fn lab_results_happy_path() {
        let response = "\
- Potassium
  - value: 4.2
  - unit: mmol/L
  - reference_range: 3.5-5.0
  - abnormal: normal";
        let labs = parse_lab_results_markdown(response);
        assert_eq!(labs.len(), 1);
        assert_eq!(labs[0].test_name, "Potassium");
        assert_eq!(labs[0].value, Some(4.2));
        assert_eq!(labs[0].unit.as_deref(), Some("mmol/L"));
    }

    #[test]
    fn lab_results_text_value() {
        let response = "- Urine culture\n  - value: negative";
        let labs = parse_lab_results_markdown(response);
        assert_eq!(labs.len(), 1);
        assert!(labs[0].value.is_none());
        assert_eq!(labs[0].value_text.as_deref(), Some("negative"));
    }

    #[test]
    fn lab_results_multi() {
        let response = "- Glucose\n  - value: 5.8\n- HbA1c\n  - value: 6.2";
        let labs = parse_lab_results_markdown(response);
        assert_eq!(labs.len(), 2);
    }

    #[test]
    fn lab_results_empty() {
        assert!(parse_lab_results_markdown("").is_empty());
    }

    // ── Diagnoses parser ─────────────────────────────────

    #[test]
    fn diagnoses_happy_path() {
        let response = "\
- Type 2 Diabetes
  - date: 2024-01-15
  - status: active";
        let dx = parse_diagnoses_markdown(response);
        assert_eq!(dx.len(), 1);
        assert_eq!(dx[0].name, "Type 2 Diabetes");
        assert_eq!(dx[0].date.as_deref(), Some("2024-01-15"));
        assert_eq!(dx[0].status, "active");
    }

    #[test]
    fn diagnoses_name_only() {
        let response = "- Hypertension";
        let dx = parse_diagnoses_markdown(response);
        assert_eq!(dx.len(), 1);
        assert_eq!(dx[0].name, "Hypertension");
        assert!(dx[0].status.is_empty());
    }

    #[test]
    fn diagnoses_multi() {
        let response = "- Diabetes\n- Hypertension";
        let dx = parse_diagnoses_markdown(response);
        assert_eq!(dx.len(), 2);
    }

    #[test]
    fn diagnoses_empty() {
        assert!(parse_diagnoses_markdown("").is_empty());
    }

    // ── Allergies parser ─────────────────────────────────

    #[test]
    fn allergies_happy_path() {
        let response = "\
- Penicillin
  - reaction: rash
  - severity: moderate";
        let allergies = parse_allergies_markdown(response);
        assert_eq!(allergies.len(), 1);
        assert_eq!(allergies[0].allergen, "Penicillin");
        assert_eq!(allergies[0].reaction.as_deref(), Some("rash"));
        assert_eq!(allergies[0].severity.as_deref(), Some("moderate"));
    }

    #[test]
    fn allergies_name_only() {
        let response = "- Sulfa drugs";
        let allergies = parse_allergies_markdown(response);
        assert_eq!(allergies.len(), 1);
        assert_eq!(allergies[0].allergen, "Sulfa drugs");
        assert!(allergies[0].reaction.is_none());
    }

    #[test]
    fn allergies_empty() {
        assert!(parse_allergies_markdown("").is_empty());
    }

    // ── Procedures parser ────────────────────────────────

    #[test]
    fn procedures_happy_path() {
        let response = "\
- Appendectomy
  - date: 2023-06-10
  - outcome: successful
  - follow_up: yes, 2 weeks";
        let procs = parse_procedures_markdown(response);
        assert_eq!(procs.len(), 1);
        assert_eq!(procs[0].name, "Appendectomy");
        assert_eq!(procs[0].date.as_deref(), Some("2023-06-10"));
        assert!(procs[0].follow_up_required);
    }

    #[test]
    fn procedures_no_follow_up() {
        let response = "- Blood draw\n  - date: 2024-01-01";
        let procs = parse_procedures_markdown(response);
        assert_eq!(procs.len(), 1);
        assert!(!procs[0].follow_up_required);
    }

    #[test]
    fn procedures_empty() {
        assert!(parse_procedures_markdown("").is_empty());
    }

    // ── Referrals parser ─────────────────────────────────

    #[test]
    fn referrals_happy_path() {
        let response = "\
- Dr. Smith
  - specialty: Cardiology
  - reason: chest pain evaluation";
        let refs = parse_referrals_markdown(response);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].referred_to, "Dr. Smith");
        assert_eq!(refs[0].specialty.as_deref(), Some("Cardiology"));
        assert_eq!(refs[0].reason.as_deref(), Some("chest pain evaluation"));
    }

    #[test]
    fn referrals_name_only() {
        let response = "- Endocrinologist";
        let refs = parse_referrals_markdown(response);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].referred_to, "Endocrinologist");
    }

    #[test]
    fn referrals_empty() {
        assert!(parse_referrals_markdown("").is_empty());
    }

    // ── Instructions parser ──────────────────────────────

    #[test]
    fn instructions_happy_path() {
        let response = "\
- Take medication with food
- Return for follow-up in 2 weeks
- Monitor blood sugar daily";
        let instr = parse_instructions_markdown(response);
        assert_eq!(instr.len(), 3);
        assert!(instr[0].text.contains("medication"));
        assert!(instr[1].text.contains("follow-up"));
    }

    #[test]
    fn instructions_with_category() {
        let response = "- Exercise 30 minutes daily\n  - category: lifestyle";
        let instr = parse_instructions_markdown(response);
        assert_eq!(instr.len(), 1);
        assert_eq!(instr[0].category, "lifestyle");
    }

    #[test]
    fn instructions_empty() {
        assert!(parse_instructions_markdown("").is_empty());
    }

    // ── Domain dispatcher ────────────────────────────────

    #[test]
    fn dispatcher_medications() {
        let result = parse_domain_response(
            DocumentDomain::Medications,
            "- Metformin\n  - dose: 500mg",
        );
        let mut entities = ExtractedEntities::default();
        result.merge_into(&mut entities);
        assert_eq!(entities.medications.len(), 1);
    }

    #[test]
    fn dispatcher_lab_results() {
        let result = parse_domain_response(
            DocumentDomain::LabResults,
            "- Glucose\n  - value: 5.8",
        );
        let mut entities = ExtractedEntities::default();
        result.merge_into(&mut entities);
        assert_eq!(entities.lab_results.len(), 1);
    }

    #[test]
    fn dispatcher_all_7_domains() {
        for domain in DocumentDomain::all() {
            let result = parse_domain_response(*domain, "- Test item");
            let mut entities = ExtractedEntities::default();
            result.merge_into(&mut entities);
            // At least one entity should be produced per domain
            let total = entities.medications.len()
                + entities.lab_results.len()
                + entities.diagnoses.len()
                + entities.allergies.len()
                + entities.procedures.len()
                + entities.referrals.len()
                + entities.instructions.len();
            assert_eq!(total, 1, "Domain {domain} should produce 1 entity");
        }
    }

    #[test]
    fn merge_into_accumulates() {
        let mut entities = ExtractedEntities::default();
        parse_domain_response(DocumentDomain::Medications, "- Med1")
            .merge_into(&mut entities);
        parse_domain_response(DocumentDomain::Medications, "- Med2")
            .merge_into(&mut entities);
        assert_eq!(entities.medications.len(), 2);
    }

    // ── French language support ──────────────────────────

    #[test]
    fn medications_french() {
        let response = "\
- Paracétamol
  - dose: 1g
  - frequency: 3 fois par jour
  - route: oral";
        let meds = parse_medications_markdown(response);
        assert_eq!(meds.len(), 1);
        assert_eq!(meds[0].generic_name.as_deref(), Some("Paracétamol"));
        assert_eq!(meds[0].dose, "1g");
    }

    #[test]
    fn lab_results_comma_decimal() {
        let response = "- Glycémie\n  - value: 5,8\n  - unit: mmol/L";
        let labs = parse_lab_results_markdown(response);
        assert_eq!(labs.len(), 1);
        assert_eq!(labs[0].value, Some(5.8));
    }

    // ── Not-specified handling ────────────────────────────

    #[test]
    fn not_specified_returns_none_fr() {
        assert!(is_not_specified("non spécifié"));
        assert!(is_not_specified("aucun"));
    }

    #[test]
    fn not_specified_returns_none_en() {
        assert!(is_not_specified("not specified"));
        assert!(is_not_specified("N/A"));
        assert!(is_not_specified("none"));
    }

    #[test]
    fn real_value_not_flagged() {
        assert!(!is_not_specified("500mg"));
        assert!(!is_not_specified("twice daily"));
    }
}
