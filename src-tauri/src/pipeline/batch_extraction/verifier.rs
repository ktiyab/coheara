//! Semantic Verifier — validates extracted items against source text.
//!
//! Four verification checks (LP-01 Section 7):
//! 1. Token overlap: key terms from extracted item exist in source messages
//! 2. Date reasonableness: dates within ±1 year of conversation date
//! 3. Entity count sanity: max items per domain per conversation
//! 4. Grounding assessment: grounded / partial / ungrounded

use chrono::NaiveDate;

use super::types::*;

/// Verifies extracted items against their source conversation messages.
pub struct SemanticVerifier {
    max_items_per_domain: usize,
}

impl SemanticVerifier {
    pub fn new(max_items_per_domain: usize) -> Self {
        Self { max_items_per_domain }
    }

    /// Verify a batch of extracted items from a single conversation.
    pub fn verify(
        &self,
        items: &[ExtractedItem],
        input: &ExtractionInput,
    ) -> VerificationResult {
        let source_text = self.build_source_text(&input.messages);
        let mut verified = Vec::new();
        let mut warnings = Vec::new();

        // Entity count sanity check
        if items.len() > self.max_items_per_domain {
            warnings.push(format!(
                "Too many items ({}) for domain, keeping first {}",
                items.len(),
                self.max_items_per_domain
            ));
        }

        let items_to_check = if items.len() > self.max_items_per_domain {
            &items[..self.max_items_per_domain]
        } else {
            items
        };

        for item in items_to_check {
            let grounding = self.assess_grounding(item, &source_text);
            let date_ok = self.check_date_reasonableness(item, input.conversation_date);

            if !date_ok {
                warnings.push(format!(
                    "Item has unreasonable date (>1 year from conversation)"
                ));
                continue;
            }

            let confidence = self.compute_confidence(&grounding, item);

            verified.push(VerifiedItem {
                item: item.clone(),
                grounding,
                confidence,
            });
        }

        VerificationResult { items: verified, warnings }
    }

    /// Build combined source text from all messages (lowercased for matching).
    fn build_source_text(&self, messages: &[ConversationMessage]) -> String {
        messages
            .iter()
            .map(|m| m.content.to_lowercase())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Check how well the extracted item's key terms appear in source text.
    fn assess_grounding(&self, item: &ExtractedItem, source_text: &str) -> Grounding {
        let key_terms = self.extract_key_terms(item);

        if key_terms.is_empty() {
            return Grounding::Ungrounded;
        }

        let matched = key_terms
            .iter()
            .filter(|term| source_text.contains(&term.to_lowercase()))
            .count();

        let ratio = matched as f32 / key_terms.len() as f32;

        if ratio >= 0.7 {
            Grounding::Grounded
        } else if ratio >= 0.3 {
            Grounding::Partial
        } else {
            Grounding::Ungrounded
        }
    }

    /// Extract key terms from an extracted item for grounding verification.
    fn extract_key_terms(&self, item: &ExtractedItem) -> Vec<String> {
        let mut terms = Vec::new();

        match item.domain {
            ExtractionDomain::Symptom => {
                if let Some(specific) = item.data.get("specific").and_then(|v| v.as_str()) {
                    terms.push(specific.to_string());
                }
                if let Some(region) = item.data.get("body_region").and_then(|v| v.as_str()) {
                    terms.push(region.to_string());
                }
                if let Some(character) = item.data.get("character").and_then(|v| v.as_str()) {
                    terms.push(character.to_string());
                }
            }
            ExtractionDomain::Medication => {
                if let Some(name) = item.data.get("name").and_then(|v| v.as_str()) {
                    terms.push(name.to_string());
                }
                if let Some(dose) = item.data.get("dose").and_then(|v| v.as_str()) {
                    terms.push(dose.to_string());
                }
            }
            ExtractionDomain::Appointment => {
                if let Some(name) = item.data.get("professional_name").and_then(|v| v.as_str()) {
                    terms.push(name.to_string());
                }
                if let Some(specialty) = item.data.get("specialty").and_then(|v| v.as_str()) {
                    terms.push(specialty.to_string());
                }
            }
        }

        terms
    }

    /// Check if dates in the extracted item are within ±1 year of the conversation.
    fn check_date_reasonableness(&self, item: &ExtractedItem, conversation_date: NaiveDate) -> bool {
        let date_fields = ["onset_hint", "start_date_hint", "date_hint"];

        for field in &date_fields {
            if let Some(date_str) = item.data.get(*field).and_then(|v| v.as_str()) {
                if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                    let diff_days = (date - conversation_date).num_days().abs();
                    if diff_days > 365 {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Compute confidence score based on grounding and data completeness.
    fn compute_confidence(&self, grounding: &Grounding, item: &ExtractedItem) -> f32 {
        let base = match grounding {
            Grounding::Grounded => 0.8,
            Grounding::Partial => 0.5,
            Grounding::Ungrounded => 0.2,
        };

        // Bonus for data completeness
        let completeness = item.data.as_object()
            .map(|o| {
                let total = o.len() as f32;
                let non_null = o.values().filter(|v| !v.is_null()).count() as f32;
                if total > 0.0 { non_null / total } else { 0.0 }
            })
            .unwrap_or(0.0);

        // Bonus for having source message references
        let has_sources = !item.source_message_indices.is_empty();

        let score = base + (completeness * 0.15) + if has_sources { 0.05 } else { 0.0 };
        score.min(1.0)
    }
}

/// Result of verification for a single item.
#[derive(Debug, Clone)]
pub struct VerifiedItem {
    pub item: ExtractedItem,
    pub grounding: Grounding,
    pub confidence: f32,
}

/// Result of verifying a batch of items.
#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub items: Vec<VerifiedItem>,
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_input(messages: Vec<(&str, &str)>) -> ExtractionInput {
        ExtractionInput {
            conversation_id: "conv-1".to_string(),
            messages: messages
                .into_iter()
                .enumerate()
                .map(|(i, (role, content))| ConversationMessage {
                    id: format!("msg-{i}"),
                    index: i,
                    role: role.to_string(),
                    content: content.to_string(),
                    created_at: chrono::NaiveDate::from_ymd_opt(2026, 2, 20)
                        .unwrap()
                        .and_hms_opt(10, 0, 0)
                        .unwrap(),
                    is_signal: role == "patient",
                })
                .collect(),
            patient_context: PatientContext::default(),
            conversation_date: chrono::NaiveDate::from_ymd_opt(2026, 2, 20).unwrap(),
        }
    }

    #[test]
    fn grounded_item_high_confidence() {
        let input = make_input(vec![
            ("patient", "I have a terrible headache, throbbing on the right side"),
        ]);

        let items = vec![ExtractedItem {
            domain: ExtractionDomain::Symptom,
            data: serde_json::json!({
                "category": "Pain",
                "specific": "headache",
                "body_region": "right side",
                "character": "throbbing",
                "severity_hint": 4
            }),
            confidence: 0.0,
            source_message_indices: vec![0],
        }];

        let verifier = SemanticVerifier::new(5);
        let result = verifier.verify(&items, &input);

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].grounding, Grounding::Grounded);
        assert!(result.items[0].confidence > 0.7);
    }

    #[test]
    fn ungrounded_item_low_confidence() {
        let input = make_input(vec![
            ("patient", "I feel fine today, just asking a question"),
        ]);

        let items = vec![ExtractedItem {
            domain: ExtractionDomain::Symptom,
            data: serde_json::json!({
                "category": "Pain",
                "specific": "migraine",
                "body_region": "left temple",
                "character": "stabbing"
            }),
            confidence: 0.0,
            source_message_indices: vec![0],
        }];

        let verifier = SemanticVerifier::new(5);
        let result = verifier.verify(&items, &input);

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].grounding, Grounding::Ungrounded);
        assert!(result.items[0].confidence < 0.5);
    }

    #[test]
    fn partial_grounding() {
        let input = make_input(vec![
            ("patient", "I'm taking ibuprofen for my pain"),
        ]);

        let items = vec![ExtractedItem {
            domain: ExtractionDomain::Medication,
            data: serde_json::json!({
                "name": "ibuprofen",
                "dose": "400mg",  // 400mg not in source
            }),
            confidence: 0.0,
            source_message_indices: vec![0],
        }];

        let verifier = SemanticVerifier::new(5);
        let result = verifier.verify(&items, &input);

        assert_eq!(result.items.len(), 1);
        // "ibuprofen" found but "400mg" not found → partial
        assert_eq!(result.items[0].grounding, Grounding::Partial);
    }

    #[test]
    fn rejects_unreasonable_dates() {
        let input = make_input(vec![
            ("patient", "I started having headaches"),
        ]);

        let items = vec![ExtractedItem {
            domain: ExtractionDomain::Symptom,
            data: serde_json::json!({
                "category": "Pain",
                "specific": "headaches",
                "onset_hint": "2020-01-01"  // >1 year before conversation
            }),
            confidence: 0.0,
            source_message_indices: vec![0],
        }];

        let verifier = SemanticVerifier::new(5);
        let result = verifier.verify(&items, &input);

        assert!(result.items.is_empty(), "Should reject item with unreasonable date");
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn accepts_reasonable_dates() {
        let input = make_input(vec![
            ("patient", "Headaches started 3 days ago"),
        ]);

        let items = vec![ExtractedItem {
            domain: ExtractionDomain::Symptom,
            data: serde_json::json!({
                "category": "Pain",
                "specific": "headaches",
                "onset_hint": "2026-02-17"  // 3 days before conversation
            }),
            confidence: 0.0,
            source_message_indices: vec![0],
        }];

        let verifier = SemanticVerifier::new(5);
        let result = verifier.verify(&items, &input);

        assert_eq!(result.items.len(), 1);
    }

    #[test]
    fn entity_count_sanity() {
        let input = make_input(vec![
            ("patient", "I have pain everywhere - head, back, neck, shoulders, knees, ankles, wrists"),
        ]);

        // 7 items but max is 3
        let items: Vec<ExtractedItem> = (0..7)
            .map(|i| ExtractedItem {
                domain: ExtractionDomain::Symptom,
                data: serde_json::json!({
                    "category": "Pain",
                    "specific": format!("pain-{i}"),
                }),
                confidence: 0.0,
                source_message_indices: vec![0],
            })
            .collect();

        let verifier = SemanticVerifier::new(3);
        let result = verifier.verify(&items, &input);

        assert!(result.items.len() <= 3, "Should cap at max_items_per_domain");
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn medication_grounding_checks_name_and_dose() {
        let input = make_input(vec![
            ("patient", "I take lisinopril 10mg every morning"),
        ]);

        let items = vec![ExtractedItem {
            domain: ExtractionDomain::Medication,
            data: serde_json::json!({
                "name": "lisinopril",
                "dose": "10mg",
            }),
            confidence: 0.0,
            source_message_indices: vec![0],
        }];

        let verifier = SemanticVerifier::new(5);
        let result = verifier.verify(&items, &input);

        assert_eq!(result.items[0].grounding, Grounding::Grounded);
    }

    #[test]
    fn appointment_grounding_checks_name_and_specialty() {
        let input = make_input(vec![
            ("patient", "I'm seeing Dr. Martin, my neurologist, next Tuesday"),
        ]);

        let items = vec![ExtractedItem {
            domain: ExtractionDomain::Appointment,
            data: serde_json::json!({
                "professional_name": "Dr. Martin",
                "specialty": "neurologist",
                "date_hint": "2026-02-25",
            }),
            confidence: 0.0,
            source_message_indices: vec![0],
        }];

        let verifier = SemanticVerifier::new(5);
        let result = verifier.verify(&items, &input);

        assert_eq!(result.items[0].grounding, Grounding::Grounded);
    }

    #[test]
    fn empty_items_returns_empty() {
        let input = make_input(vec![("patient", "Hello")]);
        let verifier = SemanticVerifier::new(5);
        let result = verifier.verify(&[], &input);
        assert!(result.items.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn confidence_includes_source_bonus() {
        let input = make_input(vec![
            ("patient", "I have a headache"),
        ]);

        let with_sources = ExtractedItem {
            domain: ExtractionDomain::Symptom,
            data: serde_json::json!({"category": "Pain", "specific": "headache"}),
            confidence: 0.0,
            source_message_indices: vec![0],
        };

        let without_sources = ExtractedItem {
            domain: ExtractionDomain::Symptom,
            data: serde_json::json!({"category": "Pain", "specific": "headache"}),
            confidence: 0.0,
            source_message_indices: vec![],
        };

        let verifier = SemanticVerifier::new(5);
        let r1 = verifier.verify(&[with_sources], &input);
        let r2 = verifier.verify(&[without_sources], &input);

        assert!(
            r1.items[0].confidence > r2.items[0].confidence,
            "Having source references should give higher confidence"
        );
    }
}
