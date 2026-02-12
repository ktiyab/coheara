pub const STRUCTURING_SYSTEM_PROMPT: &str = r#"
You are a medical document structuring assistant. Your ONLY role is to convert
raw medical document text into a structured format. You extract and organize
information that is explicitly present in the document.

RULES — ABSOLUTE, NO EXCEPTIONS:
1. Extract ONLY information explicitly stated in the document.
2. NEVER add interpretation, diagnosis, advice, or clinical opinion.
3. NEVER infer information that is not directly written.
4. If a field is unclear or missing, output null for that field.
5. Preserve exact values (doses, lab values, dates) verbatim from the document.
6. Output MUST be valid JSON followed by structured Markdown.
7. For compound medications (e.g., Augmentin), list each ingredient separately.
8. For tapering schedules, list each step with dose and duration.

OUTPUT FORMAT:
First, output a JSON block wrapped in ```json``` fences containing extracted entities.
Then, output structured Markdown of the full document content.
"#;

/// Build the structuring prompt for a specific document.
pub fn build_structuring_prompt(raw_text: &str, ocr_confidence: f32) -> String {
    let confidence_note = if ocr_confidence < 0.70 {
        "NOTE: This text was extracted with LOW confidence. Some characters may be misread. \
         Mark uncertain values with confidence: 'low'.\n"
    } else {
        ""
    };

    format!(
        r#"{confidence_note}
<document>
{raw_text}
</document>

Extract ALL medical information from the above document into the following JSON structure.
For any field not present in the document, use null.

```json
{{
  "document_type": "prescription | lab_result | clinical_note | discharge_summary | radiology_report | pharmacy_record | other",
  "document_date": "YYYY-MM-DD or null",
  "professional": {{
    "name": "Full name or null",
    "specialty": "Specialty or null",
    "institution": "Institution or null"
  }},
  "medications": [
    {{
      "generic_name": "name or null",
      "brand_name": "name or null",
      "dose": "e.g., 500mg",
      "frequency": "e.g., twice daily",
      "frequency_type": "scheduled | as_needed | tapering",
      "route": "oral | topical | injection | inhaled | other",
      "reason": "why prescribed or null",
      "instructions": ["instruction1", "instruction2"],
      "is_compound": false,
      "compound_ingredients": [
        {{"name": "ingredient", "dose": "dose or null"}}
      ],
      "tapering_steps": [
        {{"step_number": 1, "dose": "dose", "duration_days": 7}}
      ],
      "max_daily_dose": "max dose or null",
      "condition": "condition treated or null",
      "confidence": 0.0
    }}
  ],
  "lab_results": [
    {{
      "test_name": "name",
      "test_code": "LOINC code or null",
      "value": 0.0,
      "value_text": "non-numeric result or null",
      "unit": "unit or null",
      "reference_range_low": 0.0,
      "reference_range_high": 0.0,
      "abnormal_flag": "normal | low | high | critical_low | critical_high | null",
      "collection_date": "YYYY-MM-DD or null",
      "confidence": 0.0
    }}
  ],
  "diagnoses": [
    {{
      "name": "diagnosis name",
      "icd_code": "code or null",
      "date": "YYYY-MM-DD or null",
      "status": "active | resolved | monitoring",
      "confidence": 0.0
    }}
  ],
  "allergies": [
    {{
      "allergen": "substance",
      "reaction": "reaction type or null",
      "severity": "mild | moderate | severe | life_threatening | null",
      "confidence": 0.0
    }}
  ],
  "procedures": [
    {{
      "name": "procedure name",
      "date": "YYYY-MM-DD or null",
      "outcome": "outcome or null",
      "follow_up_required": false,
      "follow_up_date": "YYYY-MM-DD or null",
      "confidence": 0.0
    }}
  ],
  "referrals": [
    {{
      "referred_to": "name or specialty",
      "specialty": "specialty or null",
      "reason": "reason or null",
      "confidence": 0.0
    }}
  ],
  "instructions": [
    {{
      "text": "instruction text",
      "category": "follow_up | lifestyle | monitoring | other"
    }}
  ]
}}
```

Now write the COMPLETE document as structured Markdown, preserving ALL information:
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_contains_document_text() {
        let prompt = build_structuring_prompt("Metformin 500mg", 0.90);
        assert!(prompt.contains("Metformin 500mg"));
        assert!(prompt.contains("<document>"));
        assert!(prompt.contains("</document>"));
    }

    #[test]
    fn low_confidence_adds_warning() {
        let prompt = build_structuring_prompt("some text", 0.50);
        assert!(prompt.contains("LOW confidence"));
    }

    #[test]
    fn high_confidence_no_warning() {
        let prompt = build_structuring_prompt("some text", 0.90);
        assert!(!prompt.contains("LOW confidence"));
    }

    #[test]
    fn system_prompt_enforces_extraction_only() {
        assert!(STRUCTURING_SYSTEM_PROMPT.contains("NEVER add interpretation"));
        assert!(STRUCTURING_SYSTEM_PROMPT.contains("ONLY"));
        assert!(STRUCTURING_SYSTEM_PROMPT.contains("valid JSON"));
    }
}
