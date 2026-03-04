use regex::Regex;
use rusqlite::Connection;
use uuid::Uuid;

use super::scored_context::GroundingLevel;
use super::types::{BoundaryCheck, Citation, GuidelineCitation, ScoredChunk};
use crate::invariants::types::ClinicalInsight;
use crate::db::repository::get_document;

/// Extract citations from MedGemma's response and match to source chunks.
pub fn extract_citations(response_text: &str, context_chunks: &[ScoredChunk]) -> Vec<Citation> {
    let mut citations = Vec::new();

    // Pattern 1: Explicit [Doc: uuid, Date: date] citations from MedGemma
    let doc_pattern =
        Regex::new(r"\[Doc:\s*([a-f0-9-]+)(?:,\s*Date:\s*([^\]]+))?\]").unwrap();

    for cap in doc_pattern.captures_iter(response_text) {
        let doc_id_str = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let date = cap.get(2).map(|m| m.as_str().to_string());

        if let Ok(doc_id) = Uuid::parse_str(doc_id_str) {
            if let Some(chunk) = context_chunks.iter().find(|c| c.document_id == doc_id) {
                citations.push(Citation {
                    document_id: doc_id,
                    document_title: format!(
                        "Document from {}",
                        chunk.doc_date.as_deref().unwrap_or("unknown date")
                    ),
                    document_date: date.or_else(|| chunk.doc_date.clone()),
                    professional_name: chunk.professional_name.clone(),
                    chunk_text: chunk.content.chars().take(200).collect(),
                    relevance_score: chunk.score,
                });
            }
        }
    }

    // Pattern 2: If MedGemma didn't cite explicitly, attach top-scoring chunks
    if citations.is_empty() && !context_chunks.is_empty() {
        for chunk in context_chunks.iter().take(3) {
            if chunk.score > 0.5 {
                citations.push(Citation {
                    document_id: chunk.document_id,
                    document_title: format!(
                        "Document from {}",
                        chunk.doc_date.as_deref().unwrap_or("unknown date")
                    ),
                    document_date: chunk.doc_date.clone(),
                    professional_name: chunk.professional_name.clone(),
                    chunk_text: chunk.content.chars().take(200).collect(),
                    relevance_score: chunk.score,
                });
            }
        }
    }

    // Deduplicate by document_id
    citations.sort_by(|a, b| {
        b.relevance_score
            .partial_cmp(&a.relevance_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    citations.dedup_by(|a, b| a.document_id == b.document_id);

    citations
}

/// Validate citations against the patient's document database (RS-L2-01-002).
///
/// Removes any citation whose `document_id` does not exist in the documents table.
/// This guards against hallucinated or stale UUIDs reaching the frontend.
pub fn validate_citations(conn: &Connection, citations: Vec<Citation>) -> Vec<Citation> {
    citations
        .into_iter()
        .filter(|c| {
            match get_document(conn, &c.document_id) {
                Ok(Some(_)) => true,
                Ok(None) => {
                    tracing::warn!(
                        document_id = %c.document_id,
                        "Citation references non-existent document — removed"
                    );
                    false
                }
                Err(e) => {
                    tracing::warn!(
                        document_id = %c.document_id,
                        error = %e,
                        "Failed to validate citation document — removed"
                    );
                    false
                }
            }
        })
        .collect()
}

/// Parse the BOUNDARY_CHECK from MedGemma's response.
/// Returns the boundary check and the cleaned response text.
pub fn parse_boundary_check(response: &str) -> (BoundaryCheck, String) {
    if let Some(first_line_end) = response.find('\n') {
        let first_line = response[..first_line_end].trim();
        if first_line.starts_with("BOUNDARY_CHECK:") {
            let check_str = first_line
                .strip_prefix("BOUNDARY_CHECK:")
                .unwrap()
                .trim();
            let check = match check_str.to_lowercase().as_str() {
                "understanding" => BoundaryCheck::Understanding,
                "awareness" => BoundaryCheck::Awareness,
                "preparation" => BoundaryCheck::Preparation,
                _ => BoundaryCheck::OutOfBounds,
            };
            let cleaned_response = response[first_line_end + 1..].trim().to_string();
            return (check, cleaned_response);
        }
    }
    // No boundary check found — default to Understanding.
    // The LLM was given context-only grounding via system prompt;
    // a missing tag is a format issue, not a safety issue.
    (BoundaryCheck::Understanding, response.to_string())
}

/// Clean citation markers from patient-visible text.
pub fn clean_citations_for_display(text: &str) -> String {
    let doc_pattern =
        Regex::new(r"\[Doc:\s*[a-f0-9-]+(?:,\s*Date:\s*[^\]]+)?\]").unwrap();
    doc_pattern.replace_all(text, "").to_string()
}

/// Calculate response confidence based on context quality.
///
/// ME-01: Confidence is now data-driven via GroundingLevel (computed from scored
/// medical items). BoundaryCheck still gates safety (OutOfBounds = blocked),
/// but the base confidence comes from the data, not the SLM's self-report.
pub fn calculate_confidence(
    boundary_check: &BoundaryCheck,
    grounding: GroundingLevel,
    citations_count: usize,
    semantic_chunks_used: usize,
) -> f32 {
    // Safety gate: NoContext and OutOfBounds always zero
    match boundary_check {
        BoundaryCheck::NoContext | BoundaryCheck::OutOfBounds => return 0.0,
        _ => {}
    }

    let mut confidence: f32 = 0.0;

    // Data-driven grounding (replaces LLM self-report as base score)
    match grounding {
        GroundingLevel::High => confidence += 0.45,
        GroundingLevel::Moderate => confidence += 0.30,
        GroundingLevel::Low => confidence += 0.15,
        GroundingLevel::None => confidence += 0.05,
    }

    // Citations boost confidence
    confidence += (citations_count as f32 * 0.15).min(0.3);

    // Semantic context boosts confidence
    confidence += (semantic_chunks_used as f32 * 0.06).min(0.3);

    confidence.min(1.0)
}

/// ME-03: Extract unique guideline citations from clinical insights.
///
/// Groups insights by source, counts how many reference each guideline,
/// and returns deduplicated citations sorted by count (most-cited first).
pub fn extract_guideline_citations(insights: &[ClinicalInsight]) -> Vec<GuidelineCitation> {
    let mut source_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for insight in insights {
        *source_counts.entry(&insight.source).or_insert(0) += 1;
    }

    let mut citations: Vec<GuidelineCitation> = source_counts
        .into_iter()
        .map(|(source, count)| GuidelineCitation {
            source: source.to_string(),
            insight_count: count,
        })
        .collect();

    citations.sort_by(|a, b| b.insight_count.cmp(&a.insight_count));
    citations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_understanding_boundary() {
        let (check, text) =
            parse_boundary_check("BOUNDARY_CHECK: understanding\nYour documents show that...");
        assert_eq!(check, BoundaryCheck::Understanding);
        assert!(text.starts_with("Your documents"));
    }

    #[test]
    fn parse_awareness_boundary() {
        let (check, _) = parse_boundary_check("BOUNDARY_CHECK: awareness\nSome response");
        assert_eq!(check, BoundaryCheck::Awareness);
    }

    #[test]
    fn parse_preparation_boundary() {
        let (check, _) = parse_boundary_check("BOUNDARY_CHECK: preparation\nPrepare for...");
        assert_eq!(check, BoundaryCheck::Preparation);
    }

    #[test]
    fn missing_boundary_check_defaults_to_understanding() {
        // BUG-2 fix: missing tag is a format issue, not a safety issue.
        // The LLM was given context-only grounding via system prompt.
        let (check, text) = parse_boundary_check("Some random response without boundary");
        assert_eq!(check, BoundaryCheck::Understanding);
        assert_eq!(text, "Some random response without boundary");
    }

    #[test]
    fn no_context_confidence_is_zero() {
        assert_eq!(calculate_confidence(&BoundaryCheck::NoContext, GroundingLevel::None, 0, 0), 0.0);
    }

    #[test]
    fn extract_explicit_citations() {
        let doc_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let response = "Your doctor prescribed this [Doc: 550e8400-e29b-41d4-a716-446655440000, Date: 2024-01-15].";
        let chunks = vec![ScoredChunk {
            chunk_id: "c1".into(),
            document_id: doc_id,
            content: "Metformin 500mg twice daily".into(),
            score: 0.9,
            doc_type: "prescription".into(),
            doc_date: Some("2024-01-15".into()),
            professional_name: Some("Dr. Chen".into()),
        }];

        let citations = extract_citations(response, &chunks);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].document_id, doc_id);
        assert_eq!(citations[0].professional_name.as_deref(), Some("Dr. Chen"));
    }

    #[test]
    fn fallback_citations_when_no_explicit_refs() {
        let doc_id = Uuid::new_v4();
        let response = "Based on your records, you take metformin.";
        let chunks = vec![ScoredChunk {
            chunk_id: "c1".into(),
            document_id: doc_id,
            content: "Metformin 500mg".into(),
            score: 0.85,
            doc_type: "prescription".into(),
            doc_date: None,
            professional_name: None,
        }];

        let citations = extract_citations(response, &chunks);
        assert_eq!(citations.len(), 1);
    }

    #[test]
    fn no_fallback_citations_for_low_scores() {
        let response = "Based on your records...";
        let chunks = vec![ScoredChunk {
            chunk_id: "c1".into(),
            document_id: Uuid::new_v4(),
            content: "Irrelevant chunk".into(),
            score: 0.2,
            doc_type: "note".into(),
            doc_date: None,
            professional_name: None,
        }];

        let citations = extract_citations(response, &chunks);
        assert!(citations.is_empty());
    }

    #[test]
    fn clean_citations_removes_doc_markers() {
        let text =
            "Your doctor [Doc: 550e8400-e29b-41d4-a716-446655440000, Date: 2024-01-15] prescribed this.";
        let clean = clean_citations_for_display(text);
        assert!(!clean.contains("[Doc:"));
        assert!(clean.contains("Your doctor"));
        assert!(clean.contains("prescribed this"));
    }

    #[test]
    fn confidence_high_grounding_with_citations() {
        let conf = calculate_confidence(&BoundaryCheck::Understanding, GroundingLevel::High, 2, 3);
        // High grounding (0.45) + 2 citations (0.30) + 3 chunks (0.18) = 0.93
        assert!(conf > 0.7);
        assert!(conf <= 1.0);
    }

    #[test]
    fn confidence_out_of_bounds_is_zero() {
        let conf = calculate_confidence(&BoundaryCheck::OutOfBounds, GroundingLevel::High, 3, 5);
        assert_eq!(conf, 0.0);
    }

    #[test]
    fn confidence_capped_at_one() {
        let conf = calculate_confidence(&BoundaryCheck::Understanding, GroundingLevel::High, 10, 20);
        assert!(conf <= 1.0);
    }

    #[test]
    fn confidence_grounding_levels_ordered() {
        let high = calculate_confidence(&BoundaryCheck::Understanding, GroundingLevel::High, 1, 1);
        let moderate = calculate_confidence(&BoundaryCheck::Understanding, GroundingLevel::Moderate, 1, 1);
        let low = calculate_confidence(&BoundaryCheck::Understanding, GroundingLevel::Low, 1, 1);
        let none = calculate_confidence(&BoundaryCheck::Understanding, GroundingLevel::None, 1, 1);
        assert!(high > moderate, "High > Moderate: {} > {}", high, moderate);
        assert!(moderate > low, "Moderate > Low: {} > {}", moderate, low);
        assert!(low > none, "Low > None: {} > {}", low, none);
    }

    // --- validate_citations ---

    #[test]
    fn validate_citations_keeps_valid_documents() {
        use crate::db::repository::insert_document;
        use crate::db::sqlite::open_memory_database;
        use crate::models::document::Document;
        use crate::models::enums::{DocumentType, PipelineStatus};
        use chrono::NaiveDateTime;

        let conn = open_memory_database().unwrap();
        let doc_id = Uuid::new_v4();
        let doc = Document {
            id: doc_id,
            doc_type: DocumentType::Prescription,
            title: "Test Doc".into(),
            document_date: None,
            ingestion_date: NaiveDateTime::parse_from_str(
                "2024-01-15 10:00:00",
                "%Y-%m-%d %H:%M:%S",
            )
            .unwrap(),
            professional_id: None,
            source_file: "/test/doc.enc".into(),
            markdown_file: None,
            ocr_confidence: Some(0.9),
            verified: false,
            source_deleted: false,
            perceptual_hash: None,
            notes: None,
            pipeline_status: PipelineStatus::Imported,
        };
        insert_document(&conn, &doc).unwrap();

        let citations = vec![Citation {
            document_id: doc_id,
            document_title: "Test".into(),
            document_date: None,
            professional_name: None,
            chunk_text: "Some text".into(),
            relevance_score: 0.9,
        }];

        let validated = validate_citations(&conn, citations);
        assert_eq!(validated.len(), 1);
        assert_eq!(validated[0].document_id, doc_id);
    }

    #[test]
    fn validate_citations_removes_nonexistent_documents() {
        use crate::db::sqlite::open_memory_database;

        let conn = open_memory_database().unwrap();
        let fake_id = Uuid::new_v4();

        let citations = vec![Citation {
            document_id: fake_id,
            document_title: "Fake".into(),
            document_date: None,
            professional_name: None,
            chunk_text: "Hallucinated".into(),
            relevance_score: 0.8,
        }];

        let validated = validate_citations(&conn, citations);
        assert!(validated.is_empty());
    }

    // --- ME-03: Guideline citations ---

    #[test]
    fn guideline_citations_from_empty_insights() {
        let citations = extract_guideline_citations(&[]);
        assert!(citations.is_empty());
    }

    #[test]
    fn guideline_citations_deduplicates_sources() {
        use crate::invariants::types::*;

        let insights = vec![
            ClinicalInsight {
                kind: InsightKind::Classification,
                severity: InsightSeverity::Warning,
                summary_key: "bp_grade_1".into(),
                description: InvariantLabel { key: "bp", en: "Grade 1 HTN", fr: "HTA1", de: "HTN1" },
                source: "ISH 2020".into(),
                related_entities: vec![],
                meaning_factors: MeaningFactors::default(),
            },
            ClinicalInsight {
                kind: InsightKind::Classification,
                severity: InsightSeverity::Critical,
                summary_key: "egfr_g4".into(),
                description: InvariantLabel { key: "ckd", en: "CKD G4", fr: "MRC G4", de: "CKD G4" },
                source: "KDIGO 2024".into(),
                related_entities: vec![],
                meaning_factors: MeaningFactors::default(),
            },
            ClinicalInsight {
                kind: InsightKind::Classification,
                severity: InsightSeverity::Warning,
                summary_key: "hr_brady".into(),
                description: InvariantLabel { key: "hr", en: "Brady", fr: "Brady", de: "Brady" },
                source: "ISH 2020".into(), // Duplicate source
                related_entities: vec![],
                meaning_factors: MeaningFactors::default(),
            },
        ];

        let citations = extract_guideline_citations(&insights);
        assert_eq!(citations.len(), 2);
        // ISH 2020 has 2 insights, KDIGO 2024 has 1 — sorted by count descending
        assert_eq!(citations[0].source, "ISH 2020");
        assert_eq!(citations[0].insight_count, 2);
        assert_eq!(citations[1].source, "KDIGO 2024");
        assert_eq!(citations[1].insight_count, 1);
    }

    #[test]
    fn guideline_citations_single_source() {
        use crate::invariants::types::*;

        let insights = vec![ClinicalInsight {
            kind: InsightKind::Interaction,
            severity: InsightSeverity::Critical,
            summary_key: "warfarin_aspirin".into(),
            description: InvariantLabel { key: "int", en: "Interaction", fr: "Interaction", de: "Interaktion" },
            source: "WHO EML".into(),
            related_entities: vec![],
            meaning_factors: MeaningFactors::default(),
        }];

        let citations = extract_guideline_citations(&insights);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].source, "WHO EML");
        assert_eq!(citations[0].insight_count, 1);
    }
}
