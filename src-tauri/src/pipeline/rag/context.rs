use crate::models::*;

use super::types::{AssembledContext, QueryType, RetrievedContext, ScoredChunk};

const MAX_CONTEXT_TOKENS: usize = 3000;

/// English text averages ~4 chars/token for subword tokenizers.
const CHARS_PER_TOKEN_EN: usize = 4;
/// French text averages ~3.3 chars/token due to longer words and diacritics.
/// M.7: Use a conservative estimate to avoid exceeding token budget.
const CHARS_PER_TOKEN_FR: usize = 3;

/// Detect if text is predominantly French using common French markers.
fn detect_french_content(text: &str) -> bool {
    let lower = text.to_lowercase();
    let french_markers = [
        " le ", " la ", " les ", " des ", " du ", " un ", " une ",
        " de ", " est ", " sont ", " avec ", " pour ", " dans ",
        " qui ", " que ", " ce ", " cette ", " ces ",
        "é", "è", "ê", "à", "ù", "ç", "ô", "î",
    ];
    let hits: usize = french_markers.iter().filter(|m| lower.contains(*m)).count();
    hits >= 5
}

/// Get max context chars based on content language.
fn max_context_chars(text_sample: &str) -> usize {
    let ratio = if detect_french_content(text_sample) {
        CHARS_PER_TOKEN_FR
    } else {
        CHARS_PER_TOKEN_EN
    };
    MAX_CONTEXT_TOKENS * ratio
}

/// Estimate tokens for a given text based on language.
fn estimate_tokens(text: &str) -> usize {
    let ratio = if detect_french_content(text) {
        CHARS_PER_TOKEN_FR
    } else {
        CHARS_PER_TOKEN_EN
    };
    text.len() / ratio
}

/// Assemble retrieved context into a structured prompt section.
/// Prioritizes: allergies > semantic chunks > medications > diagnoses > labs > symptoms.
/// M.7: Budget is language-aware — French text gets a tighter char limit to stay within token budget.
pub fn assemble_context(
    retrieved: &RetrievedContext,
    query_type: &QueryType,
) -> AssembledContext {
    // M.7: Sample content to detect language for token budget adjustment
    let content_sample: String = retrieved
        .semantic_chunks
        .iter()
        .take(3)
        .map(|c| c.content.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let budget = max_context_chars(&content_sample);

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
        if total_chars >= budget {
            break;
        }
        let section = format_chunk(chunk);
        if total_chars + section.len() <= budget {
            total_chars += section.len();
            sections.push(("DOCUMENT EXCERPT", section));
            chunks_included.push(chunk.clone());
        }
    }

    // Priority 3: Active medications (if room)
    if !retrieved.structured_data.medications.is_empty() && total_chars < budget {
        let section = format_medications(&retrieved.structured_data.medications);
        if total_chars + section.len() <= budget {
            total_chars += section.len();
            sections.push(("CURRENT MEDICATIONS", section));
        }
    }

    // Priority 4: Active diagnoses (if room)
    if !retrieved.structured_data.diagnoses.is_empty() && total_chars < budget {
        let section = format_diagnoses(&retrieved.structured_data.diagnoses);
        if total_chars + section.len() <= budget {
            total_chars += section.len();
            sections.push(("ACTIVE DIAGNOSES", section));
        }
    }

    // Priority 5: Lab results (if room)
    if !retrieved.structured_data.lab_results.is_empty() && total_chars < budget {
        let section = format_labs(&retrieved.structured_data.lab_results);
        if total_chars + section.len() <= budget {
            total_chars += section.len();
            sections.push(("RECENT LAB RESULTS", section));
        }
    }

    // Priority 6: Recent symptoms (for symptom queries)
    if *query_type == QueryType::Symptom
        && !retrieved.structured_data.symptoms.is_empty()
        && total_chars < budget
    {
        let section = format_symptoms(&retrieved.structured_data.symptoms);
        if total_chars + section.len() <= budget {
            let _ = total_chars + section.len(); // last section, no further budget check
            sections.push(("RECENT SYMPTOMS", section));
        }
    }

    let context_text = sections
        .iter()
        .map(|(label, content)| format!("<{label}>\n{content}\n</{label}>"))
        .collect::<Vec<_>>()
        .join("\n\n");

    let estimated_tokens = estimate_tokens(&context_text);

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
        // Allow buffer for XML tags; English content → 4 chars/token budget
        let en_budget = MAX_CONTEXT_TOKENS * CHARS_PER_TOKEN_EN;
        assert!(
            assembled.text.len() <= en_budget + 500,
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

    // ── M.7: French token ratio tests ─────────────────────────────

    #[test]
    fn detect_french_content_identifies_french() {
        let french = "Le patient présente une douleur dans la poitrine depuis \
            trois jours. Les résultats sont normaux. Il est sous traitement \
            avec du Metformine pour le diabète de type 2.";
        assert!(detect_french_content(french));
    }

    #[test]
    fn detect_french_content_rejects_english() {
        let english = "The patient presents with chest pain over the past three \
            days. Lab results are normal. Currently on Metformin for type 2 diabetes.";
        assert!(!detect_french_content(english));
    }

    #[test]
    fn french_content_gets_tighter_budget() {
        let french = "Le patient présente une douleur dans la poitrine depuis \
            trois jours. Les résultats sont normaux.";
        let english = "The patient presents with chest pain over three days.";

        let fr_budget = max_context_chars(french);
        let en_budget = max_context_chars(english);

        assert!(fr_budget < en_budget, "French budget ({fr_budget}) should be < English ({en_budget})");
        assert_eq!(fr_budget, MAX_CONTEXT_TOKENS * CHARS_PER_TOKEN_FR);
        assert_eq!(en_budget, MAX_CONTEXT_TOKENS * CHARS_PER_TOKEN_EN);
    }

    #[test]
    fn french_context_assembly_includes_fewer_chunks_than_english() {
        // French content → tighter char budget → fewer chunks fit
        let mut fr_ctx = empty_context();
        for i in 0..100 {
            fr_ctx.semantic_chunks.push(ScoredChunk {
                chunk_id: format!("fr{i}"),
                document_id: uuid::Uuid::new_v4(),
                content: format!(
                    "Le patient présente des résultats de laboratoire pour le test numéro {i}. \
                    Les valeurs sont dans la plage normale avec une légère élévation."
                ),
                score: 0.8,
                doc_type: "lab_result".into(),
                doc_date: None,
                professional_name: None,
            });
        }
        let fr_assembled = assemble_context(&fr_ctx, &QueryType::Factual);

        // Same chunk count but English content → looser budget
        let mut en_ctx = empty_context();
        for i in 0..100 {
            en_ctx.semantic_chunks.push(ScoredChunk {
                chunk_id: format!("en{i}"),
                document_id: uuid::Uuid::new_v4(),
                content: format!(
                    "The patient shows lab results for test number {i}. \
                    Values are within normal range with slight elevation noted."
                ),
                score: 0.8,
                doc_type: "lab_result".into(),
                doc_date: None,
                professional_name: None,
            });
        }
        let en_assembled = assemble_context(&en_ctx, &QueryType::Factual);

        assert!(
            fr_assembled.chunks_included.len() < en_assembled.chunks_included.len(),
            "French ({}) should fit fewer chunks than English ({})",
            fr_assembled.chunks_included.len(),
            en_assembled.chunks_included.len(),
        );
    }
}
