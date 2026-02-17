use regex::Regex;
use rusqlite::Connection;
use uuid::Uuid;

use super::types::{BoundaryCheck, Citation, ScoredChunk};
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
    // No boundary check found — treat as out of bounds
    (BoundaryCheck::OutOfBounds, response.to_string())
}

/// Clean citation markers from patient-visible text.
pub fn clean_citations_for_display(text: &str) -> String {
    let doc_pattern =
        Regex::new(r"\[Doc:\s*[a-f0-9-]+(?:,\s*Date:\s*[^\]]+)?\]").unwrap();
    doc_pattern.replace_all(text, "").to_string()
}

/// Calculate response confidence based on context quality.
pub fn calculate_confidence(
    boundary_check: &BoundaryCheck,
    citations_count: usize,
    semantic_chunks_used: usize,
) -> f32 {
    let mut confidence: f32 = 0.0;

    // Boundary check contributes to confidence
    match boundary_check {
        BoundaryCheck::Understanding => confidence += 0.4,
        BoundaryCheck::Awareness => confidence += 0.3,
        BoundaryCheck::Preparation => confidence += 0.3,
        BoundaryCheck::OutOfBounds => return 0.0,
    }

    // Citations boost confidence
    confidence += (citations_count as f32 * 0.15).min(0.3);

    // Semantic context boosts confidence
    confidence += (semantic_chunks_used as f32 * 0.06).min(0.3);

    confidence.min(1.0)
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
    fn missing_boundary_check_returns_out_of_bounds() {
        let (check, text) = parse_boundary_check("Some random response without boundary");
        assert_eq!(check, BoundaryCheck::OutOfBounds);
        assert_eq!(text, "Some random response without boundary");
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
    fn confidence_understanding_with_citations() {
        let conf = calculate_confidence(&BoundaryCheck::Understanding, 2, 3);
        assert!(conf > 0.5);
        assert!(conf <= 1.0);
    }

    #[test]
    fn confidence_out_of_bounds_is_zero() {
        let conf = calculate_confidence(&BoundaryCheck::OutOfBounds, 3, 5);
        assert_eq!(conf, 0.0);
    }

    #[test]
    fn confidence_capped_at_one() {
        let conf = calculate_confidence(&BoundaryCheck::Understanding, 10, 20);
        assert!(conf <= 1.0);
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
}
