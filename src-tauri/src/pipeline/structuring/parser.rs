use serde::Deserialize;

use super::types::{ExtractedEntities, ExtractedProfessional};
use super::StructuringError;

/// Parse MedGemma's response into structured entities and Markdown.
pub fn parse_structuring_response(
    response: &str,
) -> Result<(ExtractedEntities, String, Option<RawDocumentMeta>), StructuringError> {
    let (json_str, markdown) = extract_json_and_markdown(response)?;
    let (entities, meta) = parse_entities_json(&json_str)?;
    Ok((entities, markdown, Some(meta)))
}

/// Metadata extracted from the top-level JSON (document_type, date, professional)
#[derive(Debug, Clone)]
pub struct RawDocumentMeta {
    pub document_type: Option<String>,
    pub document_date: Option<String>,
    pub professional: Option<ExtractedProfessional>,
}

/// Extract the JSON block and Markdown from MedGemma's response.
/// Case-insensitive fence detection (handles ```JSON, ```Json, ```json).
fn extract_json_and_markdown(response: &str) -> Result<(String, String), StructuringError> {
    let lower = response.to_lowercase();
    let json_start = lower
        .find("```json")
        .ok_or_else(|| StructuringError::MalformedResponse("No JSON block found".into()))?;
    let json_content_start = json_start + 7;

    let json_end = response[json_content_start..]
        .find("```")
        .ok_or_else(|| StructuringError::MalformedResponse("Unclosed JSON block".into()))?;

    let json_str = response[json_content_start..json_content_start + json_end]
        .trim()
        .to_string();

    let markdown_start = json_content_start + json_end + 3;
    let markdown = if markdown_start < response.len() {
        response[markdown_start..].trim().to_string()
    } else {
        String::new()
    };

    Ok((json_str, markdown))
}

/// Parse the JSON string into ExtractedEntities and raw document metadata.
fn parse_entities_json(
    json_str: &str,
) -> Result<(ExtractedEntities, RawDocumentMeta), StructuringError> {
    #[derive(Deserialize)]
    struct RawResponse {
        document_type: Option<String>,
        document_date: Option<String>,
        professional: Option<serde_json::Value>,
        medications: Option<Vec<serde_json::Value>>,
        lab_results: Option<Vec<serde_json::Value>>,
        diagnoses: Option<Vec<serde_json::Value>>,
        allergies: Option<Vec<serde_json::Value>>,
        procedures: Option<Vec<serde_json::Value>>,
        referrals: Option<Vec<serde_json::Value>>,
        instructions: Option<Vec<serde_json::Value>>,
    }

    let raw: RawResponse = serde_json::from_str(json_str)
        .map_err(|e| StructuringError::JsonParsing(e.to_string()))?;

    let professional: Option<ExtractedProfessional> =
        raw.professional.and_then(|v| serde_json::from_value(v).ok());

    let meta = RawDocumentMeta {
        document_type: raw.document_type,
        document_date: raw.document_date,
        professional: professional.clone(),
    };

    let (medications, d1) = parse_array_lenient(raw.medications.as_deref(), "medication");
    let (lab_results, d2) = parse_array_lenient(raw.lab_results.as_deref(), "lab_result");
    let (diagnoses, d3) = parse_array_lenient(raw.diagnoses.as_deref(), "diagnosis");
    let (allergies, d4) = parse_array_lenient(raw.allergies.as_deref(), "allergy");
    let (procedures, d5) = parse_array_lenient(raw.procedures.as_deref(), "procedure");
    let (referrals, d6) = parse_array_lenient(raw.referrals.as_deref(), "referral");
    let (instructions, d7) = parse_array_lenient(raw.instructions.as_deref(), "instruction");

    let total_dropped = d1 + d2 + d3 + d4 + d5 + d6 + d7;
    if total_dropped > 0 {
        tracing::warn!(
            total_dropped = total_dropped,
            "Entities dropped during lenient parse"
        );
    }

    let entities = ExtractedEntities {
        medications,
        lab_results,
        diagnoses,
        allergies,
        procedures,
        referrals,
        instructions,
    };

    Ok((entities, meta))
}

/// Parse an array leniently — skip items that fail to deserialize.
/// Logs a warning for each dropped entity (STR-02-G03).
fn parse_array_lenient<T: for<'de> Deserialize<'de>>(
    items: Option<&[serde_json::Value]>,
    entity_type: &str,
) -> (Vec<T>, usize) {
    match items {
        None => (vec![], 0),
        Some(arr) => {
            let mut results = Vec::with_capacity(arr.len());
            let mut dropped = 0;

            for (i, v) in arr.iter().enumerate() {
                match serde_json::from_value(v.clone()) {
                    Ok(item) => results.push(item),
                    Err(e) => {
                        dropped += 1;
                        tracing::warn!(
                            entity_type = entity_type,
                            index = i,
                            error = %e,
                            "Dropped entity during lenient parse"
                        );
                    }
                }
            }

            (results, dropped)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::structuring::types::ExtractedDiagnosis;

    fn sample_response() -> String {
        r#"Here is the extraction:

```json
{
  "document_type": "prescription",
  "document_date": "2024-01-15",
  "professional": {"name": "Dr. Chen", "specialty": "GP", "institution": null},
  "medications": [
    {
      "generic_name": "Metformin",
      "brand_name": "Glucophage",
      "dose": "500mg",
      "frequency": "twice daily",
      "frequency_type": "scheduled",
      "route": "oral",
      "reason": "Type 2 diabetes",
      "instructions": ["Take with food"],
      "is_compound": false,
      "compound_ingredients": [],
      "tapering_steps": [],
      "max_daily_dose": "2000mg",
      "condition": "Type 2 diabetes",
      "confidence": 0.92
    }
  ],
  "lab_results": [],
  "diagnoses": [
    {
      "name": "Type 2 Diabetes",
      "icd_code": "E11",
      "date": "2024-01-15",
      "status": "active",
      "confidence": 0.90
    }
  ],
  "allergies": [
    {
      "allergen": "Penicillin",
      "reaction": "rash",
      "severity": "moderate",
      "confidence": 0.85
    }
  ],
  "procedures": [],
  "referrals": [],
  "instructions": [
    {
      "text": "Return in 3 months for HbA1c check",
      "category": "follow_up"
    }
  ]
}
```

# Prescription — Dr. Chen, GP
**Date:** January 15, 2024

## Medications
- **Metformin (Glucophage)** 500mg — twice daily, oral
  - Take with food
  - For: Type 2 diabetes

## Allergies
- Penicillin (rash, moderate)
"#
        .to_string()
    }

    #[test]
    fn parse_full_response() {
        let response = sample_response();
        let (entities, markdown, meta) = parse_structuring_response(&response).unwrap();

        assert_eq!(entities.medications.len(), 1);
        assert_eq!(
            entities.medications[0].generic_name.as_deref(),
            Some("Metformin")
        );
        assert_eq!(entities.medications[0].dose, "500mg");

        assert_eq!(entities.diagnoses.len(), 1);
        assert_eq!(entities.diagnoses[0].name, "Type 2 Diabetes");

        assert_eq!(entities.allergies.len(), 1);
        assert_eq!(entities.allergies[0].allergen, "Penicillin");

        assert_eq!(entities.instructions.len(), 1);
        assert_eq!(entities.instructions[0].category, "follow_up");

        assert!(markdown.contains("Prescription"));
        assert!(markdown.contains("Metformin"));

        let meta = meta.unwrap();
        assert_eq!(meta.document_type.as_deref(), Some("prescription"));
        assert_eq!(meta.document_date.as_deref(), Some("2024-01-15"));
        assert_eq!(meta.professional.as_ref().unwrap().name, "Dr. Chen");
    }

    #[test]
    fn parse_empty_arrays() {
        let response = r#"```json
{
  "document_type": "other",
  "document_date": null,
  "professional": null,
  "medications": [],
  "lab_results": [],
  "diagnoses": [],
  "allergies": [],
  "procedures": [],
  "referrals": [],
  "instructions": []
}
```

No structured content.
"#;
        let (entities, _, _) = parse_structuring_response(response).unwrap();
        assert!(entities.medications.is_empty());
        assert!(entities.lab_results.is_empty());
    }

    #[test]
    fn parse_missing_json_block_returns_error() {
        let result = parse_structuring_response("No JSON here, just text.");
        assert!(matches!(result, Err(StructuringError::MalformedResponse(_))));
    }

    #[test]
    fn parse_invalid_json_returns_error() {
        let response = "```json\n{invalid json}\n```\nSome markdown";
        let result = parse_structuring_response(response);
        assert!(matches!(result, Err(StructuringError::JsonParsing(_))));
    }

    #[test]
    fn lenient_parsing_skips_bad_items() {
        let items = vec![
            serde_json::json!({
                "name": "Valid Diagnosis",
                "status": "active",
                "confidence": 0.9
            }),
            serde_json::json!({"invalid_field_only": "bad data"}),
            serde_json::json!({
                "name": "Another Diagnosis",
                "status": "monitoring",
                "confidence": 0.8
            }),
        ];
        let (parsed, dropped): (Vec<ExtractedDiagnosis>, usize) =
            parse_array_lenient(Some(&items), "diagnosis");
        assert_eq!(parsed.len(), 2);
        assert_eq!(dropped, 1);
        assert_eq!(parsed[0].name, "Valid Diagnosis");
        assert_eq!(parsed[1].name, "Another Diagnosis");
    }

    #[test]
    fn case_insensitive_json_fence() {
        let response = "```JSON\n{\n  \"document_type\": \"other\",\n  \"document_date\": null,\n  \"professional\": null,\n  \"medications\": [],\n  \"lab_results\": [],\n  \"diagnoses\": [],\n  \"allergies\": [],\n  \"procedures\": [],\n  \"referrals\": [],\n  \"instructions\": []\n}\n```\n\nSome markdown.";
        let (entities, markdown, _) = parse_structuring_response(response).unwrap();
        assert!(entities.medications.is_empty());
        assert!(markdown.contains("Some markdown"));
    }

    #[test]
    fn mixed_case_json_fence() {
        let response = "```Json\n{\n  \"document_type\": \"prescription\",\n  \"document_date\": null,\n  \"professional\": null,\n  \"medications\": [],\n  \"lab_results\": [],\n  \"diagnoses\": [],\n  \"allergies\": [],\n  \"procedures\": [],\n  \"referrals\": [],\n  \"instructions\": []\n}\n```\n\nMarkdown content.";
        let (_, markdown, _) = parse_structuring_response(response).unwrap();
        assert!(markdown.contains("Markdown content"));
    }

    #[test]
    fn parse_lab_results() {
        let response = r#"```json
{
  "document_type": "lab_result",
  "document_date": "2024-02-01",
  "professional": null,
  "medications": [],
  "lab_results": [
    {
      "test_name": "Potassium",
      "test_code": null,
      "value": 4.2,
      "value_text": null,
      "unit": "mmol/L",
      "reference_range_low": 3.5,
      "reference_range_high": 5.0,
      "abnormal_flag": "normal",
      "collection_date": "2024-02-01",
      "confidence": 0.95
    }
  ],
  "diagnoses": [],
  "allergies": [],
  "procedures": [],
  "referrals": [],
  "instructions": []
}
```

# Lab Results
"#;
        let (entities, _, _) = parse_structuring_response(response).unwrap();
        assert_eq!(entities.lab_results.len(), 1);
        assert_eq!(entities.lab_results[0].test_name, "Potassium");
        assert!((entities.lab_results[0].value.unwrap() - 4.2).abs() < f64::EPSILON);
        assert_eq!(entities.lab_results[0].unit.as_deref(), Some("mmol/L"));
    }

    #[test]
    fn parse_compound_medication() {
        let response = r#"```json
{
  "document_type": "prescription",
  "document_date": null,
  "professional": null,
  "medications": [
    {
      "generic_name": "Amoxicillin/Clavulanate",
      "brand_name": "Augmentin",
      "dose": "875/125mg",
      "frequency": "twice daily",
      "frequency_type": "scheduled",
      "route": "oral",
      "reason": "Infection",
      "instructions": [],
      "is_compound": true,
      "compound_ingredients": [
        {"name": "Amoxicillin", "dose": "875mg"},
        {"name": "Clavulanate", "dose": "125mg"}
      ],
      "tapering_steps": [],
      "max_daily_dose": null,
      "condition": null,
      "confidence": 0.88
    }
  ],
  "lab_results": [],
  "diagnoses": [],
  "allergies": [],
  "procedures": [],
  "referrals": [],
  "instructions": []
}
```

Markdown here
"#;
        let (entities, _, _) = parse_structuring_response(response).unwrap();
        assert!(entities.medications[0].is_compound);
        assert_eq!(entities.medications[0].compound_ingredients.len(), 2);
        assert_eq!(
            entities.medications[0].compound_ingredients[0].name,
            "Amoxicillin"
        );
    }

    #[test]
    fn missing_optional_fields_default_gracefully() {
        // LLM omits dose, frequency, route — should default to empty string, not fail
        let response = r#"```json
{
  "document_type": "prescription",
  "document_date": null,
  "professional": null,
  "medications": [
    {
      "generic_name": "Aspirin",
      "brand_name": null,
      "instructions": [],
      "is_compound": false,
      "compound_ingredients": [],
      "tapering_steps": [],
      "max_daily_dose": null,
      "condition": null
    }
  ],
  "lab_results": [],
  "diagnoses": [
    {
      "name": "Headache"
    }
  ],
  "allergies": [],
  "procedures": [],
  "referrals": [],
  "instructions": [
    {
      "text": "Rest well"
    }
  ]
}
```

Markdown.
"#;
        let (entities, _, _) = parse_structuring_response(response).unwrap();
        assert_eq!(entities.medications.len(), 1);
        assert_eq!(
            entities.medications[0].generic_name.as_deref(),
            Some("Aspirin")
        );
        // Missing fields default to empty string
        assert_eq!(entities.medications[0].dose, "");
        assert_eq!(entities.medications[0].frequency, "");
        assert_eq!(entities.medications[0].route, "");
        assert_eq!(entities.medications[0].frequency_type, "");

        // Diagnosis with missing status defaults to empty string
        assert_eq!(entities.diagnoses.len(), 1);
        assert_eq!(entities.diagnoses[0].name, "Headache");
        assert_eq!(entities.diagnoses[0].status, "");

        // Instruction with missing category defaults to empty string
        assert_eq!(entities.instructions.len(), 1);
        assert_eq!(entities.instructions[0].text, "Rest well");
        assert_eq!(entities.instructions[0].category, "");
    }
}
