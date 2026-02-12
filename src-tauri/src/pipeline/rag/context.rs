use crate::models::*;

use super::types::{AssembledContext, QueryType, RetrievedContext, ScoredChunk};

const MAX_CONTEXT_TOKENS: usize = 3000;
const APPROX_CHARS_PER_TOKEN: usize = 4;
const MAX_CONTEXT_CHARS: usize = MAX_CONTEXT_TOKENS * APPROX_CHARS_PER_TOKEN;

/// Assemble retrieved context into a structured prompt section.
/// Prioritizes: allergies > semantic chunks > medications > diagnoses > labs > symptoms.
pub fn assemble_context(
    retrieved: &RetrievedContext,
    query_type: &QueryType,
) -> AssembledContext {
    let mut sections = Vec::new();
    let mut total_chars = 0;

    // Priority 1: Allergies (always include — safety critical)
    if !retrieved.structured_data.allergies.is_empty() {
        let section = format_allergies(&retrieved.structured_data.allergies);
        total_chars += section.len();
        sections.push(("KNOWN ALLERGIES", section));
    }

    // Priority 2: Most relevant semantic chunks (ordered by score)
    let mut chunks = retrieved.semantic_chunks.clone();
    chunks.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut chunks_included = Vec::new();
    for chunk in &chunks {
        if total_chars >= MAX_CONTEXT_CHARS {
            break;
        }
        let section = format_chunk(chunk);
        if total_chars + section.len() <= MAX_CONTEXT_CHARS {
            total_chars += section.len();
            sections.push(("DOCUMENT EXCERPT", section));
            chunks_included.push(chunk.clone());
        }
    }

    // Priority 3: Active medications (if room)
    if !retrieved.structured_data.medications.is_empty() && total_chars < MAX_CONTEXT_CHARS {
        let section = format_medications(&retrieved.structured_data.medications);
        if total_chars + section.len() <= MAX_CONTEXT_CHARS {
            total_chars += section.len();
            sections.push(("CURRENT MEDICATIONS", section));
        }
    }

    // Priority 4: Active diagnoses (if room)
    if !retrieved.structured_data.diagnoses.is_empty() && total_chars < MAX_CONTEXT_CHARS {
        let section = format_diagnoses(&retrieved.structured_data.diagnoses);
        if total_chars + section.len() <= MAX_CONTEXT_CHARS {
            total_chars += section.len();
            sections.push(("ACTIVE DIAGNOSES", section));
        }
    }

    // Priority 5: Lab results (if room)
    if !retrieved.structured_data.lab_results.is_empty() && total_chars < MAX_CONTEXT_CHARS {
        let section = format_labs(&retrieved.structured_data.lab_results);
        if total_chars + section.len() <= MAX_CONTEXT_CHARS {
            total_chars += section.len();
            sections.push(("RECENT LAB RESULTS", section));
        }
    }

    // Priority 6: Recent symptoms (for symptom queries)
    if *query_type == QueryType::Symptom
        && !retrieved.structured_data.symptoms.is_empty()
        && total_chars < MAX_CONTEXT_CHARS
    {
        let section = format_symptoms(&retrieved.structured_data.symptoms);
        if total_chars + section.len() <= MAX_CONTEXT_CHARS {
            let _ = total_chars + section.len(); // last section, no further budget check
            sections.push(("RECENT SYMPTOMS", section));
        }
    }

    let context_text = sections
        .iter()
        .map(|(label, content)| format!("<{label}>\n{content}\n</{label}>"))
        .collect::<Vec<_>>()
        .join("\n\n");

    let estimated_tokens = context_text.len() / APPROX_CHARS_PER_TOKEN;

    AssembledContext {
        text: context_text,
        estimated_tokens,
        chunks_included,
    }
}

fn format_allergies(allergies: &[Allergy]) -> String {
    allergies
        .iter()
        .map(|a| {
            format!(
                "- {} (severity: {}, reaction: {})",
                a.allergen,
                a.severity.as_str(),
                a.reaction.as_deref().unwrap_or("not specified")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_chunk(chunk: &ScoredChunk) -> String {
    let mut text = String::new();
    if let Some(ref date) = chunk.doc_date {
        text.push_str(&format!("[Date: {date}] "));
    }
    if let Some(ref prof) = chunk.professional_name {
        text.push_str(&format!("[From: {prof}] "));
    }
    text.push_str(&format!("[Doc ID: {}]\n", chunk.document_id));
    text.push_str(&chunk.content);
    text
}

fn format_medications(meds: &[Medication]) -> String {
    meds.iter()
        .map(|m| {
            format!(
                "- {} {} {} ({})",
                m.generic_name,
                m.dose,
                m.frequency,
                m.status.as_str(),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_diagnoses(diagnoses: &[Diagnosis]) -> String {
    diagnoses
        .iter()
        .map(|d| format!("- {} (status: {})", d.name, d.status.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_labs(labs: &[LabResult]) -> String {
    labs.iter()
        .take(10)
        .map(|l| {
            format!(
                "- {}: {} {} (flag: {})",
                l.test_name,
                l.value
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| l.value_text.clone().unwrap_or_default()),
                l.unit.as_deref().unwrap_or(""),
                l.abnormal_flag.as_str(),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_symptoms(symptoms: &[Symptom]) -> String {
    symptoms
        .iter()
        .take(5)
        .map(|s| {
            format!(
                "- {} ({}): severity {}/5, since {}",
                s.specific, s.category, s.severity, s.onset_date
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::StructuredContext;
    use crate::models::enums::*;

    fn empty_context() -> RetrievedContext {
        RetrievedContext {
            semantic_chunks: vec![],
            structured_data: StructuredContext::default(),
            dismissed_alerts: vec![],
        }
    }

    #[test]
    fn empty_context_produces_empty_text() {
        let ctx = empty_context();
        let assembled = assemble_context(&ctx, &QueryType::General);
        assert!(assembled.text.is_empty());
        assert_eq!(assembled.estimated_tokens, 0);
    }

    #[test]
    fn allergies_always_included_first() {
        let mut ctx = empty_context();
        ctx.structured_data.allergies.push(Allergy {
            id: uuid::Uuid::new_v4(),
            allergen: "Penicillin".into(),
            reaction: Some("Anaphylaxis".into()),
            severity: AllergySeverity::LifeThreatening,
            date_identified: None,
            source: AllergySource::DocumentExtracted,
            document_id: None,
            verified: true,
        });

        let assembled = assemble_context(&ctx, &QueryType::General);
        assert!(assembled.text.contains("KNOWN ALLERGIES"));
        assert!(assembled.text.contains("Penicillin"));
        assert!(assembled.text.contains("life_threatening"));
    }

    #[test]
    fn context_respects_token_budget() {
        let mut ctx = empty_context();

        // Add 100 large chunks — should not all fit
        for i in 0..100 {
            ctx.semantic_chunks.push(ScoredChunk {
                chunk_id: format!("c{i}"),
                document_id: uuid::Uuid::new_v4(),
                content: "A ".repeat(500),
                score: 0.8,
                doc_type: "prescription".into(),
                doc_date: None,
                professional_name: None,
            });
        }

        let assembled = assemble_context(&ctx, &QueryType::Factual);
        // Allow buffer for XML tags
        assert!(
            assembled.text.len() <= MAX_CONTEXT_CHARS + 500,
            "Context too large: {} chars",
            assembled.text.len()
        );
        assert!(assembled.chunks_included.len() < 100);
    }

    #[test]
    fn chunks_sorted_by_score() {
        let mut ctx = empty_context();
        let doc_id = uuid::Uuid::new_v4();

        ctx.semantic_chunks.push(ScoredChunk {
            chunk_id: "low".into(),
            document_id: doc_id,
            content: "Low relevance content that is long enough.".into(),
            score: 0.3,
            doc_type: "note".into(),
            doc_date: None,
            professional_name: None,
        });
        ctx.semantic_chunks.push(ScoredChunk {
            chunk_id: "high".into(),
            document_id: doc_id,
            content: "High relevance content that is long enough.".into(),
            score: 0.9,
            doc_type: "prescription".into(),
            doc_date: None,
            professional_name: None,
        });

        let assembled = assemble_context(&ctx, &QueryType::Factual);
        // High score chunk should appear before low score
        let high_pos = assembled.text.find("High relevance").unwrap();
        let low_pos = assembled.text.find("Low relevance").unwrap();
        assert!(high_pos < low_pos);
    }

    #[test]
    fn medications_included_when_present() {
        let mut ctx = empty_context();
        ctx.structured_data.medications.push(Medication {
            id: uuid::Uuid::new_v4(),
            generic_name: "Metformin".into(),
            brand_name: None,
            dose: "500mg".into(),
            frequency: "twice daily".into(),
            frequency_type: FrequencyType::Scheduled,
            route: "oral".into(),
            prescriber_id: None,
            start_date: None,
            end_date: None,
            reason_start: None,
            reason_stop: None,
            is_otc: false,
            status: MedicationStatus::Active,
            administration_instructions: None,
            max_daily_dose: None,
            condition: None,
            dose_type: DoseType::Fixed,
            is_compound: false,
            document_id: uuid::Uuid::new_v4(),
        });

        let assembled = assemble_context(&ctx, &QueryType::Factual);
        assert!(assembled.text.contains("CURRENT MEDICATIONS"));
        assert!(assembled.text.contains("Metformin"));
    }
}
