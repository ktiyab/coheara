use crate::crypto::profile::PatientDemographics;
use crate::invariants::types::{ClinicalInsight, InsightSeverity};
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
/// Prioritizes: blood type > allergies > clinical insights > semantic chunks > medications > diagnoses > labs > vitals > screenings > symptoms.
/// M.7: Budget is language-aware — French text gets a tighter char limit to stay within token budget.
/// ME-03: Clinical insights inserted at Priority 1.5 (after allergies, before semantic chunks).
/// BT-01: Blood type at Priority 0.5 (before allergies — identity-level context).
pub fn assemble_context(
    retrieved: &RetrievedContext,
    query_type: &QueryType,
    insights: &[ClinicalInsight],
    demographics: Option<&PatientDemographics>,
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

    // Priority 0.5: Blood type (identity-level, ultra-compact ~10 tokens)
    if let Some(bt_section) = format_blood_type(demographics) {
        total_chars += bt_section.len();
        sections.push(("PATIENT BLOOD TYPE", bt_section));
    }

    // Priority 1: Allergies (always include — safety critical)
    if !retrieved.structured_data.allergies.is_empty() {
        let section = format_allergies(&retrieved.structured_data.allergies);
        total_chars += section.len();
        sections.push(("KNOWN ALLERGIES", section));
    }

    // Priority 1.5: Clinical insights (ME-03 enrichment — deterministic, high value)
    // I18N: Use detected content language for insight descriptions.
    let insight_lang = if detect_french_content(&content_sample) { "fr" } else { "en" };
    if !insights.is_empty() {
        let section = format_clinical_insights(insights, insight_lang);
        if total_chars + section.len() <= budget {
            total_chars += section.len();
            sections.push(("PUBLISHED GUIDELINE REFERENCES", section));
        }
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

    // Priority 4.5: Entity connections (semantic graph edges)
    if !retrieved.structured_data.entity_connections.is_empty() && total_chars < budget {
        let section = format_entity_connections(
            &retrieved.structured_data.entity_connections,
            &retrieved.structured_data,
        );
        if !section.is_empty() && total_chars + section.len() <= budget {
            total_chars += section.len();
            sections.push(("ENTITY RELATIONSHIPS", section));
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

    // Priority 6: Vital signs (if room)
    if !retrieved.structured_data.vital_signs.is_empty() && total_chars < budget {
        let section = format_vital_signs(&retrieved.structured_data.vital_signs);
        if total_chars + section.len() <= budget {
            total_chars += section.len();
            sections.push(("VITAL SIGNS", section));
        }
    }

    // Priority 7: Screening and vaccination records (if room)
    if !retrieved.structured_data.screening_records.is_empty() && total_chars < budget {
        let section = format_screening_records(&retrieved.structured_data.screening_records);
        if total_chars + section.len() <= budget {
            total_chars += section.len();
            sections.push(("PREVENTIVE SCREENINGS AND VACCINATIONS", section));
        }
    }

    // Priority 8: Recent symptoms (for symptom queries)
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

/// BT-01: Format blood type for RAG context (~10 tokens).
fn format_blood_type(demographics: Option<&PatientDemographics>) -> Option<String> {
    let bt = demographics?.blood_type.as_ref()?;
    let info = crate::invariants::blood_types::find_blood_type(bt.as_str())?;
    let rh = if info.rh_positive { "Rh-positive" } else { "Rh-negative" };
    Some(format!("Blood type: {} ({} {}, {})", info.display, info.abo_group, rh, info.source))
}

fn format_allergies(allergies: &[Allergy]) -> String {
    allergies
        .iter()
        .map(|a| {
            let cat_tag = a
                .allergen_category
                .as_ref()
                .map(|c| format!("[{}] ", c.as_str().to_uppercase()))
                .unwrap_or_default();
            let verified_tag = if a.verified { " [verified]" } else { "" };
            format!(
                "- {}{} (severity: {}, reaction: {}){}",
                cat_tag,
                a.allergen,
                a.severity.as_str(),
                a.reaction.as_deref().unwrap_or("not specified"),
                verified_tag,
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Map internal severity to regulatory-neutral LLM context tags.
/// Internal enum values (Critical/Warning/Info) are preserved — only
/// the text emitted into the LLM context changes.
fn context_severity_tag(severity: InsightSeverity) -> &'static str {
    match severity {
        InsightSeverity::Critical => "NOTABLE",
        InsightSeverity::Warning => "ELEVATED",
        InsightSeverity::Info => "REFERENCE",
    }
}

/// Format clinical insights for the SLM context.
///
/// Format: `[TAG] description — summary_key (source: X)`
/// Insights are pre-sorted by severity (Critical first) from the enrichment engine.
/// I18N: Uses detected content language for insight descriptions.
/// REG-01: Preamble instructs SLM to present as reference data, not diagnosis.
fn format_clinical_insights(insights: &[ClinicalInsight], lang: &str) -> String {
    let mut lines = vec![
        "The following are published guideline thresholds compared to the patient's data. Present as reference information, not as your assessment.".to_string(),
    ];
    lines.extend(insights.iter().map(|i| {
        format!(
            "[{}] {} - {} (source: {})",
            context_severity_tag(i.severity),
            i.description.get(lang),
            i.summary_key,
            i.source,
        )
    }));
    lines.join("\n")
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

fn format_vital_signs(vitals: &[VitalSign]) -> String {
    vitals
        .iter()
        .take(10)
        .map(|v| {
            let value_str = if let Some(secondary) = v.value_secondary {
                format!("{}/{}", v.value_primary, secondary)
            } else {
                format!("{}", v.value_primary)
            };
            format!(
                "- {} {} {} ({})",
                v.vital_type.as_str(),
                value_str,
                v.unit,
                v.recorded_at.format("%Y-%m-%d"),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// ME-06/G5: Format screening and vaccination records for LLM context.
fn format_screening_records(records: &[crate::db::repository::ScreeningRecord]) -> String {
    records
        .iter()
        .map(|r| {
            let category = if r.screening_key.starts_with("vaccine_") {
                "Vaccine"
            } else {
                "Screening"
            };
            let provider_str = r
                .provider
                .as_deref()
                .map(|p| format!(" by {p}"))
                .unwrap_or_default();
            let dose_str = if r.dose_number > 1 || r.screening_key.starts_with("vaccine_") {
                format!(" (dose {})", r.dose_number)
            } else {
                String::new()
            };
            format!(
                "- [{}] {}{} - completed {}{}", category, r.screening_key, dose_str, r.completed_at, provider_str
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// B2-G6: Format entity connections as human-readable relationship lines.
///
/// Resolves entity UUIDs to names from the already-loaded structured data.
/// Format: `- Source (Type) → RelationshipType → Target (Type) [confidence: X]`
fn format_entity_connections(
    connections: &[crate::models::entity_connection::EntityConnection],
    ctx: &super::types::StructuredContext,
) -> String {
    use crate::models::entity_connection::EntityType;

    let resolve_name = |etype: &EntityType, eid: &uuid::Uuid| -> Option<String> {
        match etype {
            EntityType::Medication => ctx
                .medications
                .iter()
                .find(|m| m.id == *eid)
                .map(|m| m.generic_name.clone()),
            EntityType::Diagnosis => ctx
                .diagnoses
                .iter()
                .find(|d| d.id == *eid)
                .map(|d| d.name.clone()),
            EntityType::LabResult => ctx
                .lab_results
                .iter()
                .find(|l| l.id == *eid)
                .map(|l| l.test_name.clone()),
            EntityType::Allergy => ctx
                .allergies
                .iter()
                .find(|a| a.id == *eid)
                .map(|a| a.allergen.clone()),
            _ => None,
        }
    };

    let lines: Vec<String> = connections
        .iter()
        .filter_map(|c| {
            let source_name = resolve_name(&c.source_type, &c.source_id)?;
            let target_name = resolve_name(&c.target_type, &c.target_id)?;
            Some(format!(
                "- {} ({}) -> {} -> {} ({})",
                source_name,
                c.source_type.as_str(),
                c.relationship_type.as_str(),
                target_name,
                c.target_type.as_str(),
            ))
        })
        .collect();

    lines.join("\n")
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
        let assembled = assemble_context(&ctx, &QueryType::General, &[], None);
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
            allergen_category: None,
            date_identified: None,
            source: AllergySource::DocumentExtracted,
            document_id: None,
            verified: true,
        });

        let assembled = assemble_context(&ctx, &QueryType::General, &[], None);
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

        let assembled = assemble_context(&ctx, &QueryType::Factual, &[], None);
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

        let assembled = assemble_context(&ctx, &QueryType::Factual, &[], None);
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

        let assembled = assemble_context(&ctx, &QueryType::Factual, &[], None);
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
        let fr_assembled = assemble_context(&fr_ctx, &QueryType::Factual, &[], None);

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
        let en_assembled = assemble_context(&en_ctx, &QueryType::Factual, &[], None);

        assert!(
            fr_assembled.chunks_included.len() < en_assembled.chunks_included.len(),
            "French ({}) should fit fewer chunks than English ({})",
            fr_assembled.chunks_included.len(),
            en_assembled.chunks_included.len(),
        );
    }

    // ── ME-03: Clinical insights in context assembly ─────────

    #[test]
    fn clinical_insights_appear_before_semantic_chunks() {
        use crate::invariants::types::*;

        let mut ctx = empty_context();
        ctx.semantic_chunks.push(ScoredChunk {
            chunk_id: "c1".into(),
            document_id: uuid::Uuid::new_v4(),
            content: "Some document content here.".into(),
            score: 0.8,
            doc_type: "note".into(),
            doc_date: None,
            professional_name: None,
        });

        let insights = vec![ClinicalInsight {
            kind: InsightKind::Classification,
            severity: InsightSeverity::Warning,
            summary_key: "BP 145/92 mmHg".to_string(),
            description: InvariantLabel {
                key: "bp_grade_1_htn",
                en: "Grade 1 Hypertension",
                fr: "Hypertension de grade 1",
                de: "Hypertonie Grad 1",
            },
            source: "ISH 2020".to_string(),
            related_entities: vec![],
            meaning_factors: MeaningFactors::default(),
        }];

        let assembled = assemble_context(&ctx, &QueryType::Factual, &insights, None);
        assert!(assembled.text.contains("PUBLISHED GUIDELINE REFERENCES"));
        assert!(assembled.text.contains("Grade 1 Hypertension"));
        assert!(assembled.text.contains("BP 145/92 mmHg"));
        assert!(assembled.text.contains("ISH 2020"));

        // Insights section should appear before document excerpts
        let insights_pos = assembled.text.find("PUBLISHED GUIDELINE REFERENCES").unwrap();
        let doc_pos = assembled.text.find("DOCUMENT EXCERPT").unwrap();
        assert!(insights_pos < doc_pos);
    }

    #[test]
    fn empty_insights_produce_no_section() {
        let ctx = empty_context();
        let assembled = assemble_context(&ctx, &QueryType::General, &[], None);
        assert!(!assembled.text.contains("PUBLISHED GUIDELINE REFERENCES"));
    }

    #[test]
    fn insights_consume_budget() {
        use crate::invariants::types::*;

        let mut ctx = empty_context();
        // Fill with many chunks
        for i in 0..100 {
            ctx.semantic_chunks.push(ScoredChunk {
                chunk_id: format!("c{i}"),
                document_id: uuid::Uuid::new_v4(),
                content: "A ".repeat(200),
                score: 0.8,
                doc_type: "note".into(),
                doc_date: None,
                professional_name: None,
            });
        }

        // Assembly without insights
        let without = assemble_context(&ctx, &QueryType::Factual, &[], None);

        // Create some insights
        let insights = vec![
            ClinicalInsight {
                kind: InsightKind::Classification,
                severity: InsightSeverity::Critical,
                summary_key: "eGFR 28 mL/min".to_string(),
                description: InvariantLabel {
                    key: "ckd_g4",
                    en: "CKD G4 - Severely decreased",
                    fr: "MRC G4",
                    de: "CKD G4",
                },
                source: "KDIGO 2024".to_string(),
                related_entities: vec![],
                meaning_factors: MeaningFactors::default(),
            },
            ClinicalInsight {
                kind: InsightKind::Interaction,
                severity: InsightSeverity::Critical,
                summary_key: "warfarin + ibuprofen: bleeding risk".to_string(),
                description: InvariantLabel {
                    key: "drug_interaction",
                    en: "Drug-drug interaction",
                    fr: "Interaction",
                    de: "Interaktion",
                },
                source: "WHO EML".to_string(),
                related_entities: vec![],
                meaning_factors: MeaningFactors::default(),
            },
        ];

        // Assembly with insights — fewer chunks should fit
        let with = assemble_context(&ctx, &QueryType::Factual, &insights, None);
        assert!(
            with.chunks_included.len() <= without.chunks_included.len(),
            "Insights should consume budget, reducing chunk count"
        );
    }

    // ── ALLERGY-01 B9: Allergy format tests ─────────────────

    #[test]
    fn allergy_with_category_shows_tag() {
        let mut ctx = empty_context();
        ctx.structured_data.allergies.push(Allergy {
            id: uuid::Uuid::new_v4(),
            allergen: "Peanut".into(),
            reaction: Some("Anaphylaxis".into()),
            severity: AllergySeverity::LifeThreatening,
            allergen_category: Some(AllergenCategory::Food),
            date_identified: None,
            source: AllergySource::PatientReported,
            document_id: None,
            verified: false,
        });

        let assembled = assemble_context(&ctx, &QueryType::General, &[], None);
        assert!(assembled.text.contains("[FOOD]"), "Should include category tag");
        assert!(assembled.text.contains("Peanut"));
    }

    #[test]
    fn allergy_verified_shows_tag() {
        let mut ctx = empty_context();
        ctx.structured_data.allergies.push(Allergy {
            id: uuid::Uuid::new_v4(),
            allergen: "Aspirin".into(),
            reaction: None,
            severity: AllergySeverity::Moderate,
            allergen_category: Some(AllergenCategory::Drug),
            date_identified: None,
            source: AllergySource::DocumentExtracted,
            document_id: None,
            verified: true,
        });

        let assembled = assemble_context(&ctx, &QueryType::General, &[], None);
        assert!(assembled.text.contains("[verified]"), "Should include verified tag");
        assert!(assembled.text.contains("[DRUG]"), "Should include category tag");
        assert!(assembled.text.contains("Aspirin"));
    }

    #[test]
    fn allergy_without_category_no_tag() {
        let mut ctx = empty_context();
        ctx.structured_data.allergies.push(Allergy {
            id: uuid::Uuid::new_v4(),
            allergen: "Latex".into(),
            reaction: Some("Contact dermatitis".into()),
            severity: AllergySeverity::Mild,
            allergen_category: None,
            date_identified: None,
            source: AllergySource::DocumentExtracted,
            document_id: None,
            verified: false,
        });

        let assembled = assemble_context(&ctx, &QueryType::General, &[], None);
        assert!(assembled.text.contains("Latex"));
        assert!(!assembled.text.contains("[verified]"));
        // No category tag prefix before allergen name
        assert!(assembled.text.contains("- Latex (severity:"));
    }

    #[test]
    fn allergy_no_reaction_shows_not_specified() {
        let mut ctx = empty_context();
        ctx.structured_data.allergies.push(Allergy {
            id: uuid::Uuid::new_v4(),
            allergen: "Sulfonamide".into(),
            reaction: None,
            severity: AllergySeverity::Severe,
            allergen_category: None,
            date_identified: None,
            source: AllergySource::DocumentExtracted,
            document_id: None,
            verified: false,
        });

        let assembled = assemble_context(&ctx, &QueryType::General, &[], None);
        assert!(assembled.text.contains("not specified"));
    }

    // ── BT-01: Blood type in RAG context ─────────────────

    #[test]
    fn blood_type_in_context_when_known() {
        use crate::crypto::profile::PatientDemographics;
        use crate::models::enums::BloodType;

        let ctx = empty_context();
        let demo = PatientDemographics {
            sex: None,
            ethnicities: vec![],
            age_context: None,
            age_years: None,
            blood_type: Some(BloodType::OPositive),
        };

        let assembled = assemble_context(&ctx, &QueryType::General, &[], Some(&demo));
        assert!(assembled.text.contains("PATIENT BLOOD TYPE"));
        assert!(assembled.text.contains("O+"));
        assert!(assembled.text.contains("Rh-positive"));
    }

    #[test]
    fn no_blood_type_section_when_unknown() {
        use crate::crypto::profile::PatientDemographics;

        let ctx = empty_context();
        let demo = PatientDemographics {
            sex: None,
            ethnicities: vec![],
            age_context: None,
            age_years: None,
            blood_type: None,
        };

        let assembled = assemble_context(&ctx, &QueryType::General, &[], Some(&demo));
        assert!(!assembled.text.contains("PATIENT BLOOD TYPE"));
    }
}
