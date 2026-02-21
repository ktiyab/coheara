//! M3: Domain-specific extractors for symptoms, medications, and appointments.
//!
//! Each extractor:
//! 1. Builds a domain-specific prompt with patient context + conversation
//! 2. Parses the LLM JSON response into ExtractedItems
//! 3. Validates plausibility
//! 4. Consolidates duplicates within a single conversation

use tracing::warn;

use super::error::ExtractionError;
use super::traits::DomainExtractor;
use super::types::*;

// ═══════════════════════════════════════════
// Shared helpers
// ═══════════════════════════════════════════

/// Format messages for inclusion in an extraction prompt.
/// Signal messages are marked with [S] prefix.
fn format_messages(messages: &[ConversationMessage]) -> String {
    messages
        .iter()
        .map(|m| {
            let signal_marker = if m.is_signal { "[S] " } else { "" };
            let role = if m.role == "patient" { "PATIENT" } else { "ASSISTANT" };
            format!("[Msg {}] {signal_marker}{role}: {}", m.index, m.content)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Format active medications list for context.
fn format_medications(meds: &[ActiveMedicationSummary]) -> String {
    if meds.is_empty() {
        return "None recorded".to_string();
    }
    meds.iter()
        .map(|m| format!("{} {} {}", m.name, m.dose, m.frequency))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Format recent symptoms for context.
fn format_symptoms(symptoms: &[RecentSymptomSummary]) -> String {
    if symptoms.is_empty() {
        return "None recorded".to_string();
    }
    symptoms
        .iter()
        .map(|s| format!("{}/{} (severity {})", s.category, s.specific, s.severity))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Format known professionals for context.
fn format_professionals(pros: &[ProfessionalSummary]) -> String {
    if pros.is_empty() {
        return "None recorded".to_string();
    }
    pros.iter()
        .map(|p| {
            if let Some(ref spec) = p.specialty {
                format!("{} ({})", p.name, spec)
            } else {
                p.name.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Format known allergies for context.
fn format_allergies(allergies: &[String]) -> String {
    if allergies.is_empty() {
        return "None recorded".to_string();
    }
    allergies.join(", ")
}

/// Extract a JSON block from LLM response text.
/// Handles responses that include text before/after the JSON.
fn extract_json_block(response: &str) -> Result<&str, ExtractionError> {
    // Try to find JSON between ```json and ``` or { ... }
    let trimmed = response.trim();

    // Strip markdown code fences if present
    if let Some(start) = trimmed.find("```json") {
        let after_fence = &trimmed[start + 7..];
        if let Some(end) = after_fence.find("```") {
            return Ok(after_fence[..end].trim());
        }
    }

    if let Some(start) = trimmed.find("```") {
        let after_fence = &trimmed[start + 3..];
        if let Some(end) = after_fence.find("```") {
            let block = after_fence[..end].trim();
            if block.starts_with('{') || block.starts_with('[') {
                return Ok(block);
            }
        }
    }

    // Find the first { and last }
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if start < end {
            return Ok(&trimmed[start..=end]);
        }
    }

    Err(ExtractionError::JsonParsing(
        "No JSON block found in LLM response".to_string(),
    ))
}

// ═══════════════════════════════════════════
// Symptom Extractor
// ═══════════════════════════════════════════

pub struct SymptomExtractor;

impl SymptomExtractor {
    pub fn new() -> Self {
        Self
    }
}

impl DomainExtractor for SymptomExtractor {
    fn domain(&self) -> ExtractionDomain {
        ExtractionDomain::Symptom
    }

    fn build_prompt(&self, input: &ExtractionInput) -> String {
        let context = &input.patient_context;
        format!(
            "You are a health information extractor. Read the patient's conversation \
and extract ALL symptom information explicitly mentioned. Output valid JSON.\n\
Consolidate: if the same symptom appears in multiple messages, produce ONE entry \
with the most complete and recent information.\n\n\
RULES:\n\
1. Extract ONLY what the patient explicitly states across the conversation.\n\
2. NEVER add symptoms not mentioned.\n\
3. For missing fields, use null.\n\
4. Preserve original language for text fields.\n\
5. Severity: extract hints (e.g., \"terrible\" → 4, \"mild\" → 2) or null.\n\
6. Dates: resolve relative references (\"yesterday\", \"3 days ago\") to ISO dates.\n\
   TODAY is {date}.\n\
7. If the same symptom is described across multiple messages, merge into ONE entry.\n\n\
PATIENT CONTEXT:\n\
- Active medications: {meds}\n\
- Recent logged symptoms: {symptoms}\n\n\
CONVERSATION (messages marked [S] contain symptom information):\n\
{messages}\n\n\
OUTPUT FORMAT:\n\
```json\n\
{{\n\
  \"symptoms\": [\n\
    {{\n\
      \"category\": \"Pain|Digestive|Respiratory|Neurological|General|Mood|Skin|Other\",\n\
      \"specific\": \"symptom name in original language\",\n\
      \"severity_hint\": 1-5 or null,\n\
      \"onset_hint\": \"ISO date or null\",\n\
      \"body_region\": \"body region or null\",\n\
      \"duration\": \"duration description or null\",\n\
      \"character\": \"Sharp|Dull|Burning|Pressure|Throbbing or null\",\n\
      \"aggravating\": [\"factor1\", \"factor2\"] or [],\n\
      \"relieving\": [\"factor1\"] or [],\n\
      \"timing_pattern\": \"Morning|Night|AfterMeals|Random|AllTheTime or null\",\n\
      \"notes\": \"additional context from conversation or null\",\n\
      \"related_medication_hint\": \"medication name if correlation mentioned or null\",\n\
      \"source_messages\": [1, 2, 7]\n\
    }}\n\
  ]\n\
}}\n\
```",
            date = input.conversation_date,
            meds = format_medications(&context.active_medications),
            symptoms = format_symptoms(&context.recent_symptoms),
            messages = format_messages(&input.messages),
        )
    }

    fn parse_response(&self, response: &str) -> Result<Vec<ExtractedItem>, ExtractionError> {
        let json_str = extract_json_block(response)?;

        #[derive(serde::Deserialize)]
        struct SymptomResponse {
            #[serde(default)]
            symptoms: Vec<ExtractedSymptomData>,
        }

        let parsed: SymptomResponse = serde_json::from_str(json_str)
            .map_err(|e| ExtractionError::JsonParsing(format!("Symptom parse error: {e}")))?;

        Ok(parsed
            .symptoms
            .into_iter()
            .map(|s| {
                let source_msgs = s.source_messages.clone();
                ExtractedItem {
                    domain: ExtractionDomain::Symptom,
                    data: serde_json::to_value(&s).unwrap_or_default(),
                    confidence: 0.0, // Set by verifier
                    source_message_indices: source_msgs,
                }
            })
            .collect())
    }

    fn validate(&self, items: &[ExtractedItem]) -> ValidationResult {
        let mut valid = Vec::new();
        let mut warnings = Vec::new();
        let mut rejected = 0;

        for item in items {
            // Must have category and specific
            let category = item.data.get("category").and_then(|v| v.as_str());
            let has_specific = item.data.get("specific").and_then(|v| v.as_str()).is_some();

            if category.is_none() || !has_specific {
                warnings.push("Symptom missing category or specific name".to_string());
                rejected += 1;
                continue;
            }

            // Validate category against CATEGORIES whitelist
            let cat = category.unwrap();
            if !crate::journal::CATEGORIES.contains(&cat) {
                warnings.push(format!("Symptom category '{cat}' not in allowed list, will be clamped to Other"));
                // Don't reject — dispatch will clamp to "Other"
            }

            // Warn about body_region if not in whitelist (don't reject)
            if let Some(br) = item.data.get("body_region").and_then(|v| v.as_str()) {
                if !crate::journal::BODY_REGIONS.contains(&br) {
                    warn!(body_region = br, "Extracted body_region not in BODY_REGIONS whitelist");
                    warnings.push(format!("Body region '{br}' not in allowed list, will be discarded"));
                }
            }

            // Severity must be 1-5 if present
            if let Some(sev) = item.data.get("severity_hint").and_then(|v| v.as_i64()) {
                if !(1..=5).contains(&sev) {
                    warnings.push(format!("Symptom severity {sev} out of range 1-5"));
                    rejected += 1;
                    continue;
                }
            }

            valid.push(item.clone());
        }

        ValidationResult {
            items: valid,
            warnings,
            rejected_count: rejected,
        }
    }

    fn consolidate(&self, items: Vec<ExtractedItem>) -> Vec<ExtractedItem> {
        // Group by (category, specific) and keep the most complete entry
        use std::collections::HashMap;
        let mut groups: HashMap<(String, String), Vec<ExtractedItem>> = HashMap::new();

        for item in items {
            let category = item.data.get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            let specific = item.data.get("specific")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            groups.entry((category, specific)).or_default().push(item);
        }

        groups
            .into_values()
            .map(|group| {
                if group.len() == 1 {
                    return group.into_iter().next().unwrap();
                }
                // Keep the entry with the most non-null fields
                group
                    .into_iter()
                    .max_by_key(|item| {
                        item.data.as_object()
                            .map(|o| o.values().filter(|v| !v.is_null()).count())
                            .unwrap_or(0)
                    })
                    .unwrap()
            })
            .collect()
    }
}

// ═══════════════════════════════════════════
// Medication Extractor
// ═══════════════════════════════════════════

pub struct MedicationExtractor;

impl MedicationExtractor {
    pub fn new() -> Self {
        Self
    }
}

impl DomainExtractor for MedicationExtractor {
    fn domain(&self) -> ExtractionDomain {
        ExtractionDomain::Medication
    }

    fn build_prompt(&self, input: &ExtractionInput) -> String {
        let context = &input.patient_context;
        format!(
            "You are a health information extractor. Read the patient's conversation \
and extract ALL medication information explicitly mentioned. Output valid JSON.\n\
Consolidate: if the same medication is mentioned in multiple messages, produce \
ONE entry with the most complete information.\n\n\
RULES:\n\
1. Extract ONLY medications the patient explicitly mentions.\n\
2. NEVER add medications not in the conversation.\n\
3. Distinguish between \"taking\" (active) and \"stopped\" (discontinued).\n\
4. For missing fields, use null.\n\
5. Preserve original language for medication names and instructions.\n\
6. Dates: resolve relative references to ISO dates. TODAY is {date}.\n\
7. If the same medication appears in multiple messages, merge into ONE entry.\n\n\
PATIENT CONTEXT:\n\
- Known active medications: {meds}\n\
- Known allergies: {allergies}\n\n\
CONVERSATION (messages marked [S] contain medication information):\n\
{messages}\n\n\
OUTPUT FORMAT:\n\
```json\n\
{{\n\
  \"medications\": [\n\
    {{\n\
      \"name\": \"medication name in original language\",\n\
      \"dose\": \"e.g., 400mg or null\",\n\
      \"frequency\": \"e.g., twice daily or null\",\n\
      \"route\": \"oral|topical|injection|inhaled|other or null\",\n\
      \"reason\": \"why taking or null\",\n\
      \"is_otc\": true or false,\n\
      \"start_date_hint\": \"ISO date or null\",\n\
      \"status_hint\": \"active|stopped|changed or null\",\n\
      \"adherence_note\": \"e.g., forgot morning dose or null\",\n\
      \"source_messages\": [3, 4]\n\
    }}\n\
  ]\n\
}}\n\
```",
            date = input.conversation_date,
            meds = format_medications(&context.active_medications),
            allergies = format_allergies(&context.known_allergies),
            messages = format_messages(&input.messages),
        )
    }

    fn parse_response(&self, response: &str) -> Result<Vec<ExtractedItem>, ExtractionError> {
        let json_str = extract_json_block(response)?;

        #[derive(serde::Deserialize)]
        struct MedicationResponse {
            #[serde(default)]
            medications: Vec<ExtractedMedicationData>,
        }

        let parsed: MedicationResponse = serde_json::from_str(json_str)
            .map_err(|e| ExtractionError::JsonParsing(format!("Medication parse error: {e}")))?;

        Ok(parsed
            .medications
            .into_iter()
            .map(|m| {
                let source_msgs = m.source_messages.clone();
                ExtractedItem {
                    domain: ExtractionDomain::Medication,
                    data: serde_json::to_value(&m).unwrap_or_default(),
                    confidence: 0.0,
                    source_message_indices: source_msgs,
                }
            })
            .collect())
    }

    fn validate(&self, items: &[ExtractedItem]) -> ValidationResult {
        let mut valid = Vec::new();
        let mut warnings = Vec::new();
        let mut rejected = 0;

        for item in items {
            let name = item.data.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if name.is_empty() {
                warnings.push("Medication missing name".to_string());
                rejected += 1;
                continue;
            }

            // Reject medication names that are excessively long (likely garbage)
            if name.len() > 200 {
                warnings.push(format!("Medication name too long ({} chars), will be truncated", name.len()));
                // Don't reject — dispatch will truncate
            }

            valid.push(item.clone());
        }

        ValidationResult {
            items: valid,
            warnings,
            rejected_count: rejected,
        }
    }

    fn consolidate(&self, items: Vec<ExtractedItem>) -> Vec<ExtractedItem> {
        use std::collections::HashMap;
        let mut groups: HashMap<String, Vec<ExtractedItem>> = HashMap::new();

        for item in items {
            let name = item.data.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            groups.entry(name).or_default().push(item);
        }

        groups
            .into_values()
            .map(|group| {
                if group.len() == 1 {
                    return group.into_iter().next().unwrap();
                }
                group
                    .into_iter()
                    .max_by_key(|item| {
                        item.data.as_object()
                            .map(|o| o.values().filter(|v| !v.is_null()).count())
                            .unwrap_or(0)
                    })
                    .unwrap()
            })
            .collect()
    }
}

// ═══════════════════════════════════════════
// Appointment Extractor
// ═══════════════════════════════════════════

pub struct AppointmentExtractor;

impl AppointmentExtractor {
    pub fn new() -> Self {
        Self
    }
}

impl DomainExtractor for AppointmentExtractor {
    fn domain(&self) -> ExtractionDomain {
        ExtractionDomain::Appointment
    }

    fn build_prompt(&self, input: &ExtractionInput) -> String {
        let context = &input.patient_context;
        format!(
            "You are a health information extractor. Read the patient's conversation \
and extract ALL appointment/visit information explicitly mentioned. Output valid JSON.\n\n\
RULES:\n\
1. Extract ONLY appointments the patient explicitly mentions.\n\
2. NEVER invent appointment details.\n\
3. Parse relative dates (\"next Tuesday\", \"in two weeks\") to ISO dates.\n\
   TODAY is {date}.\n\
4. For missing fields, use null.\n\
5. Preserve original language for names and reasons.\n\n\
PATIENT CONTEXT:\n\
- Known professionals: {pros}\n\
- Recent symptoms: {symptoms}\n\n\
CONVERSATION (messages marked [S] contain appointment information):\n\
{messages}\n\n\
OUTPUT FORMAT:\n\
```json\n\
{{\n\
  \"appointments\": [\n\
    {{\n\
      \"professional_name\": \"doctor name or null\",\n\
      \"specialty\": \"specialty or null\",\n\
      \"date_hint\": \"ISO date or null\",\n\
      \"time_hint\": \"HH:MM or null\",\n\
      \"location\": \"location or null\",\n\
      \"reason\": \"visit reason or null\",\n\
      \"notes\": \"additional notes or null\",\n\
      \"source_messages\": [5]\n\
    }}\n\
  ]\n\
}}\n\
```",
            date = input.conversation_date,
            pros = format_professionals(&context.known_professionals),
            symptoms = format_symptoms(&context.recent_symptoms),
            messages = format_messages(&input.messages),
        )
    }

    fn parse_response(&self, response: &str) -> Result<Vec<ExtractedItem>, ExtractionError> {
        let json_str = extract_json_block(response)?;

        #[derive(serde::Deserialize)]
        struct AppointmentResponse {
            #[serde(default)]
            appointments: Vec<ExtractedAppointmentData>,
        }

        let parsed: AppointmentResponse = serde_json::from_str(json_str)
            .map_err(|e| ExtractionError::JsonParsing(format!("Appointment parse error: {e}")))?;

        Ok(parsed
            .appointments
            .into_iter()
            .map(|a| {
                let source_msgs = a.source_messages.clone();
                ExtractedItem {
                    domain: ExtractionDomain::Appointment,
                    data: serde_json::to_value(&a).unwrap_or_default(),
                    confidence: 0.0,
                    source_message_indices: source_msgs,
                }
            })
            .collect())
    }

    fn validate(&self, items: &[ExtractedItem]) -> ValidationResult {
        let mut valid = Vec::new();
        let mut warnings = Vec::new();
        let mut rejected = 0;

        for item in items {
            // Must have at least a date_hint or professional_name
            let has_date = item.data.get("date_hint")
                .and_then(|v| v.as_str())
                .map(|s| !s.is_empty())
                .unwrap_or(false);
            let has_professional = item.data.get("professional_name")
                .and_then(|v| v.as_str())
                .map(|s| !s.is_empty())
                .unwrap_or(false);

            if !has_date && !has_professional {
                warnings.push("Appointment missing both date and professional name".to_string());
                rejected += 1;
                continue;
            }

            // Warn about invalid specialty (dispatch will normalize to "Other")
            if let Some(spec) = item.data.get("specialty").and_then(|v| v.as_str()) {
                if !crate::appointment::SPECIALTIES.contains(&spec) && spec != "Other" {
                    warnings.push(format!("Specialty '{spec}' not in allowed list, will default to Other"));
                }
            }

            valid.push(item.clone());
        }

        ValidationResult {
            items: valid,
            warnings,
            rejected_count: rejected,
        }
    }

    fn consolidate(&self, items: Vec<ExtractedItem>) -> Vec<ExtractedItem> {
        use std::collections::HashMap;
        let mut groups: HashMap<(String, String), Vec<ExtractedItem>> = HashMap::new();

        for item in items {
            let professional = item.data.get("professional_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            let date = item.data.get("date_hint")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            groups.entry((professional, date)).or_default().push(item);
        }

        groups
            .into_values()
            .map(|group| {
                if group.len() == 1 {
                    return group.into_iter().next().unwrap();
                }
                group
                    .into_iter()
                    .max_by_key(|item| {
                        item.data.as_object()
                            .map(|o| o.values().filter(|v| !v.is_null()).count())
                            .unwrap_or(0)
                    })
                    .unwrap()
            })
            .collect()
    }
}

// ═══════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_input() -> ExtractionInput {
        ExtractionInput {
            conversation_id: "conv-1".to_string(),
            messages: vec![
                ConversationMessage {
                    id: "msg-0".to_string(),
                    index: 0,
                    role: "patient".to_string(),
                    content: "I've been having headaches for 3 days".to_string(),
                    created_at: chrono::NaiveDate::from_ymd_opt(2026, 2, 20)
                        .unwrap()
                        .and_hms_opt(10, 0, 0)
                        .unwrap(),
                    is_signal: true,
                },
                ConversationMessage {
                    id: "msg-1".to_string(),
                    index: 1,
                    role: "coheara".to_string(),
                    content: "Tell me more about the pain.".to_string(),
                    created_at: chrono::NaiveDate::from_ymd_opt(2026, 2, 20)
                        .unwrap()
                        .and_hms_opt(10, 1, 0)
                        .unwrap(),
                    is_signal: false,
                },
                ConversationMessage {
                    id: "msg-2".to_string(),
                    index: 2,
                    role: "patient".to_string(),
                    content: "Throbbing, right side, about 6/10".to_string(),
                    created_at: chrono::NaiveDate::from_ymd_opt(2026, 2, 20)
                        .unwrap()
                        .and_hms_opt(10, 2, 0)
                        .unwrap(),
                    is_signal: true,
                },
            ],
            patient_context: PatientContext {
                active_medications: vec![ActiveMedicationSummary {
                    name: "Lisinopril".to_string(),
                    dose: "10mg".to_string(),
                    frequency: "daily".to_string(),
                }],
                ..Default::default()
            },
            conversation_date: chrono::NaiveDate::from_ymd_opt(2026, 2, 20).unwrap(),
        }
    }

    // ── Prompt building ──

    #[test]
    fn symptom_prompt_contains_conversation_date() {
        let extractor = SymptomExtractor::new();
        let prompt = extractor.build_prompt(&sample_input());
        assert!(prompt.contains("2026-02-20"), "Prompt should contain conversation date");
    }

    #[test]
    fn symptom_prompt_contains_messages() {
        let extractor = SymptomExtractor::new();
        let prompt = extractor.build_prompt(&sample_input());
        assert!(prompt.contains("headaches for 3 days"), "Prompt should contain message content");
        assert!(prompt.contains("[S]"), "Signal messages should be marked");
    }

    #[test]
    fn symptom_prompt_contains_patient_context() {
        let extractor = SymptomExtractor::new();
        let prompt = extractor.build_prompt(&sample_input());
        assert!(prompt.contains("Lisinopril"), "Prompt should contain active medications");
    }

    #[test]
    fn medication_prompt_contains_rules() {
        let extractor = MedicationExtractor::new();
        let prompt = extractor.build_prompt(&sample_input());
        assert!(prompt.contains("RULES:"), "Prompt should contain extraction rules");
        assert!(prompt.contains("NEVER add medications"), "Prompt should contain safety rule");
    }

    #[test]
    fn appointment_prompt_contains_professionals() {
        let mut input = sample_input();
        input.patient_context.known_professionals = vec![ProfessionalSummary {
            name: "Dr. Martin".to_string(),
            specialty: Some("Neurologist".to_string()),
        }];

        let extractor = AppointmentExtractor::new();
        let prompt = extractor.build_prompt(&input);
        assert!(prompt.contains("Dr. Martin"), "Prompt should contain known professionals");
    }

    // ── Response parsing ──

    #[test]
    fn parse_symptom_json() {
        let response = r#"```json
{
  "symptoms": [
    {
      "category": "Pain",
      "specific": "Headache",
      "severity_hint": 4,
      "onset_hint": "2026-02-17",
      "body_region": "right side of head",
      "duration": "3 days",
      "character": "Throbbing",
      "aggravating": ["screen use"],
      "relieving": [],
      "timing_pattern": "Morning",
      "notes": null,
      "related_medication_hint": "Lisinopril",
      "source_messages": [0, 2]
    }
  ]
}
```"#;

        let extractor = SymptomExtractor::new();
        let items = extractor.parse_response(response).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].domain, ExtractionDomain::Symptom);
        assert_eq!(items[0].data["category"], "Pain");
        assert_eq!(items[0].data["specific"], "Headache");
        assert_eq!(items[0].source_message_indices, vec![0, 2]);
    }

    #[test]
    fn parse_medication_json() {
        let response = r#"{
  "medications": [
    {
      "name": "Ibuprofen",
      "dose": "400mg",
      "frequency": "twice daily",
      "route": "oral",
      "reason": "headaches",
      "is_otc": true,
      "start_date_hint": "2026-02-19",
      "status_hint": "active",
      "adherence_note": null,
      "source_messages": [3]
    }
  ]
}"#;

        let extractor = MedicationExtractor::new();
        let items = extractor.parse_response(response).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].domain, ExtractionDomain::Medication);
        assert_eq!(items[0].data["name"], "Ibuprofen");
        assert_eq!(items[0].data["is_otc"], true);
    }

    #[test]
    fn parse_appointment_json() {
        let response = r#"{
  "appointments": [
    {
      "professional_name": "Dr. Martin",
      "specialty": "Neurologist",
      "date_hint": "2026-02-25",
      "time_hint": "14:00",
      "location": null,
      "reason": "headache consultation",
      "notes": null,
      "source_messages": [4]
    }
  ]
}"#;

        let extractor = AppointmentExtractor::new();
        let items = extractor.parse_response(response).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].domain, ExtractionDomain::Appointment);
        assert_eq!(items[0].data["professional_name"], "Dr. Martin");
    }

    #[test]
    fn parse_response_without_code_fences() {
        let response = r#"{"symptoms": [{"category": "Pain", "specific": "Headache", "severity_hint": 3, "onset_hint": null, "body_region": null, "duration": null, "character": null, "aggravating": [], "relieving": [], "timing_pattern": null, "notes": null, "related_medication_hint": null, "source_messages": [0]}]}"#;

        let extractor = SymptomExtractor::new();
        let items = extractor.parse_response(response).unwrap();
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn parse_response_with_preamble() {
        let response = r#"Based on the conversation, here is the extracted data:

{
  "symptoms": [
    {
      "category": "Pain",
      "specific": "Headache",
      "severity_hint": null,
      "onset_hint": null,
      "body_region": null,
      "duration": null,
      "character": null,
      "aggravating": [],
      "relieving": [],
      "timing_pattern": null,
      "notes": null,
      "related_medication_hint": null,
      "source_messages": [0]
    }
  ]
}"#;

        let extractor = SymptomExtractor::new();
        let items = extractor.parse_response(response).unwrap();
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn parse_empty_response() {
        let response = r#"{"symptoms": []}"#;
        let extractor = SymptomExtractor::new();
        let items = extractor.parse_response(response).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn parse_invalid_json() {
        let response = "This is not JSON at all, just text.";
        let extractor = SymptomExtractor::new();
        let result = extractor.parse_response(response);
        assert!(result.is_err());
    }

    // ── Validation ──

    #[test]
    fn validate_rejects_symptom_without_category() {
        let items = vec![ExtractedItem {
            domain: ExtractionDomain::Symptom,
            data: serde_json::json!({"specific": "Headache"}),
            confidence: 0.0,
            source_message_indices: vec![0],
        }];

        let extractor = SymptomExtractor::new();
        let result = extractor.validate(&items);
        assert_eq!(result.rejected_count, 1);
        assert!(result.items.is_empty());
    }

    #[test]
    fn validate_rejects_symptom_with_invalid_severity() {
        let items = vec![ExtractedItem {
            domain: ExtractionDomain::Symptom,
            data: serde_json::json!({"category": "Pain", "specific": "Headache", "severity_hint": 10}),
            confidence: 0.0,
            source_message_indices: vec![0],
        }];

        let extractor = SymptomExtractor::new();
        let result = extractor.validate(&items);
        assert_eq!(result.rejected_count, 1);
    }

    #[test]
    fn validate_accepts_valid_symptom() {
        let items = vec![ExtractedItem {
            domain: ExtractionDomain::Symptom,
            data: serde_json::json!({"category": "Pain", "specific": "Headache", "severity_hint": 4}),
            confidence: 0.0,
            source_message_indices: vec![0],
        }];

        let extractor = SymptomExtractor::new();
        let result = extractor.validate(&items);
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.rejected_count, 0);
    }

    #[test]
    fn validate_rejects_medication_without_name() {
        let items = vec![ExtractedItem {
            domain: ExtractionDomain::Medication,
            data: serde_json::json!({"dose": "400mg"}),
            confidence: 0.0,
            source_message_indices: vec![0],
        }];

        let extractor = MedicationExtractor::new();
        let result = extractor.validate(&items);
        assert_eq!(result.rejected_count, 1);
    }

    #[test]
    fn validate_rejects_appointment_without_date_or_professional() {
        let items = vec![ExtractedItem {
            domain: ExtractionDomain::Appointment,
            data: serde_json::json!({"reason": "check-up"}),
            confidence: 0.0,
            source_message_indices: vec![0],
        }];

        let extractor = AppointmentExtractor::new();
        let result = extractor.validate(&items);
        assert_eq!(result.rejected_count, 1);
    }

    #[test]
    fn validate_accepts_appointment_with_professional_only() {
        let items = vec![ExtractedItem {
            domain: ExtractionDomain::Appointment,
            data: serde_json::json!({"professional_name": "Dr. Martin"}),
            confidence: 0.0,
            source_message_indices: vec![0],
        }];

        let extractor = AppointmentExtractor::new();
        let result = extractor.validate(&items);
        assert_eq!(result.items.len(), 1);
    }

    // ── Consolidation ──

    #[test]
    fn consolidate_merges_duplicate_symptoms() {
        let items = vec![
            ExtractedItem {
                domain: ExtractionDomain::Symptom,
                data: serde_json::json!({"category": "Pain", "specific": "Headache", "severity_hint": null}),
                confidence: 0.0,
                source_message_indices: vec![0],
            },
            ExtractedItem {
                domain: ExtractionDomain::Symptom,
                data: serde_json::json!({"category": "Pain", "specific": "Headache", "severity_hint": 4, "body_region": "right side"}),
                confidence: 0.0,
                source_message_indices: vec![2],
            },
        ];

        let extractor = SymptomExtractor::new();
        let consolidated = extractor.consolidate(items);
        assert_eq!(consolidated.len(), 1, "Should merge two headache entries");
        // Should keep the more complete entry (with severity + body_region)
        assert_eq!(consolidated[0].data["severity_hint"], 4);
    }

    #[test]
    fn consolidate_keeps_different_symptoms_separate() {
        let items = vec![
            ExtractedItem {
                domain: ExtractionDomain::Symptom,
                data: serde_json::json!({"category": "Pain", "specific": "Headache"}),
                confidence: 0.0,
                source_message_indices: vec![0],
            },
            ExtractedItem {
                domain: ExtractionDomain::Symptom,
                data: serde_json::json!({"category": "General", "specific": "Fatigue"}),
                confidence: 0.0,
                source_message_indices: vec![2],
            },
        ];

        let extractor = SymptomExtractor::new();
        let consolidated = extractor.consolidate(items);
        assert_eq!(consolidated.len(), 2, "Different symptoms should stay separate");
    }

    #[test]
    fn consolidate_merges_duplicate_medications() {
        let items = vec![
            ExtractedItem {
                domain: ExtractionDomain::Medication,
                data: serde_json::json!({"name": "Ibuprofen", "dose": null}),
                confidence: 0.0,
                source_message_indices: vec![3],
            },
            ExtractedItem {
                domain: ExtractionDomain::Medication,
                data: serde_json::json!({"name": "Ibuprofen", "dose": "400mg", "frequency": "twice daily"}),
                confidence: 0.0,
                source_message_indices: vec![6],
            },
        ];

        let extractor = MedicationExtractor::new();
        let consolidated = extractor.consolidate(items);
        assert_eq!(consolidated.len(), 1);
        assert_eq!(consolidated[0].data["dose"], "400mg");
    }

    // ── Domain trait conformance ──

    #[test]
    fn symptom_extractor_domain() {
        assert_eq!(SymptomExtractor::new().domain(), ExtractionDomain::Symptom);
    }

    #[test]
    fn medication_extractor_domain() {
        assert_eq!(MedicationExtractor::new().domain(), ExtractionDomain::Medication);
    }

    #[test]
    fn appointment_extractor_domain() {
        assert_eq!(AppointmentExtractor::new().domain(), ExtractionDomain::Appointment);
    }

    // ── JSON extraction helper ──

    #[test]
    fn extract_json_block_from_fenced() {
        let text = "Here is the result:\n```json\n{\"key\": \"value\"}\n```\nDone.";
        let result = extract_json_block(text).unwrap();
        assert_eq!(result, "{\"key\": \"value\"}");
    }

    #[test]
    fn extract_json_block_from_bare() {
        let text = "Result: {\"key\": \"value\"}";
        let result = extract_json_block(text).unwrap();
        assert_eq!(result, "{\"key\": \"value\"}");
    }

    #[test]
    fn extract_json_block_no_json() {
        let text = "No JSON here at all.";
        let result = extract_json_block(text);
        assert!(result.is_err());
    }

    // ── Validation tightening tests ──

    #[test]
    fn validate_symptom_warns_on_invalid_category() {
        let items = vec![ExtractedItem {
            domain: ExtractionDomain::Symptom,
            data: serde_json::json!({"category": "FakeCategory", "specific": "Something"}),
            confidence: 0.0,
            source_message_indices: vec![0],
        }];

        let extractor = SymptomExtractor::new();
        let result = extractor.validate(&items);
        // Should still pass (dispatch will clamp), but with a warning
        assert_eq!(result.items.len(), 1);
        assert!(!result.warnings.is_empty(), "Should warn about invalid category");
    }

    #[test]
    fn validate_medication_warns_on_long_name() {
        let long_name = "A".repeat(250);
        let items = vec![ExtractedItem {
            domain: ExtractionDomain::Medication,
            data: serde_json::json!({"name": long_name}),
            confidence: 0.0,
            source_message_indices: vec![0],
        }];

        let extractor = MedicationExtractor::new();
        let result = extractor.validate(&items);
        // Should still pass (dispatch will truncate), but with a warning
        assert_eq!(result.items.len(), 1);
        assert!(!result.warnings.is_empty(), "Should warn about long name");
    }

    #[test]
    fn validate_appointment_warns_on_invalid_specialty() {
        let items = vec![ExtractedItem {
            domain: ExtractionDomain::Appointment,
            data: serde_json::json!({"professional_name": "Dr. Test", "specialty": "Podiatrist"}),
            confidence: 0.0,
            source_message_indices: vec![0],
        }];

        let extractor = AppointmentExtractor::new();
        let result = extractor.validate(&items);
        // Should still pass (dispatch will normalize), but with a warning
        assert_eq!(result.items.len(), 1);
        assert!(!result.warnings.is_empty(), "Should warn about invalid specialty");
    }
}
