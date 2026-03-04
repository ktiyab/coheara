//! ME-01 Brick 7: Scored context assembly for SLM consumption.
//!
//! Converts scored medical items into structured XML context that the SLM
//! can directly read and articulate. The SLM doesn't discover meaning —
//! meaning is pre-computed in the scores and relationships.

use super::domain::QueryDomain;
use super::factors::ScoredItem;
use super::medical_item::{SeveritySignal, StatusSignal};
use super::scoring_pipeline::ScoringResult;

use crate::models::entity_connection::EntityConnection;

/// Build scored context section for the SLM prompt.
///
/// Format designed for SLM low-awareness: explicit, structured, minimal.
/// Each item shows its significance level and relationships.
pub fn format_scored_context(
    result: &ScoringResult,
    connections: &[EntityConnection],
) -> String {
    if result.items.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    out.push_str(&format!(
        "<SCORED_MEDICAL_DATA domain=\"{}\" items=\"{}\">\n",
        domain_label(result.query_domain),
        result.items.len(),
    ));

    for (rank, scored) in result.items.iter().enumerate() {
        let item = &scored.item;
        let sig = significance_label(scored.s_factor);
        let uncertain = if scored.u_factor > 0.3 { " [UNCERTAIN]" } else { "" };

        let doc_attr = item
            .document_id
            .map(|id| format!(" source_doc=\"{}\"", id))
            .unwrap_or_default();
        let date_attr = item
            .relevant_date
            .map(|d| format!(" date=\"{}\"", d))
            .unwrap_or_default();

        out.push_str(&format!(
            "  <ITEM rank=\"{}\" type=\"{}\" significance=\"{}\"{}{}{}>\n",
            rank + 1,
            item.item_type.as_str(),
            sig,
            doc_attr,
            date_attr,
            uncertain,
        ));

        // Item content
        out.push_str(&format!("    {}\n", format_item_content(item)));

        // Related items via connections
        let related = find_related(item.id, connections, &result.items);
        if !related.is_empty() {
            out.push_str("    <RELATED>\n");
            for rel in &related {
                out.push_str(&format!("      {}\n", rel));
            }
            out.push_str("    </RELATED>\n");
        }

        out.push_str("  </ITEM>\n");
    }

    out.push_str("</SCORED_MEDICAL_DATA>");
    out
}

/// Format the content line for a single medical item.
fn format_item_content(item: &super::medical_item::MedicalItem) -> String {
    let status = match &item.status {
        StatusSignal::Active => "Active",
        StatusSignal::Inactive => "Inactive",
        StatusSignal::Current => "Current",
    };

    let severity = match &item.severity {
        SeveritySignal::LabFlag(flag) => {
            use crate::models::enums::AbnormalFlag;
            match flag {
                AbnormalFlag::CriticalHigh => " (Critical High)",
                AbnormalFlag::CriticalLow => " (Critical Low)",
                AbnormalFlag::High => " (High)",
                AbnormalFlag::Low => " (Low)",
                AbnormalFlag::Normal => "",
            }
        }
        SeveritySignal::AllergySeverity(sev) => {
            use crate::models::enums::AllergySeverity;
            match sev {
                AllergySeverity::LifeThreatening => " (Life-Threatening)",
                AllergySeverity::Severe => " (Severe)",
                AllergySeverity::Moderate => " (Moderate)",
                AllergySeverity::Mild => " (Mild)",
            }
        }
        SeveritySignal::Numeric(n) => {
            if *n >= 7 {
                return format!("{} - {} - Severity: {}/10", item.display_name, status, n);
            }
            return format!("{} - {} - Severity: {}/10", item.display_name, status, n);
        }
        SeveritySignal::None => "",
    };

    let date = item
        .relevant_date
        .map(|d| format!(" ({})", d))
        .unwrap_or_default();

    format!("{}{} - {}{}", item.display_name, severity, status, date)
}

/// Find related items via entity connections.
fn find_related(
    item_id: uuid::Uuid,
    connections: &[EntityConnection],
    scored_items: &[ScoredItem],
) -> Vec<String> {
    let item_names: std::collections::HashMap<uuid::Uuid, &str> = scored_items
        .iter()
        .map(|s| (s.item.id, s.item.display_name.as_str()))
        .collect();

    let mut related = Vec::new();

    for conn in connections {
        if conn.source_id == item_id {
            if let Some(target_name) = item_names.get(&conn.target_id) {
                related.push(format!(
                    "{} -> {}",
                    conn.relationship_type.as_str(),
                    target_name,
                ));
            }
        } else if conn.target_id == item_id {
            if let Some(source_name) = item_names.get(&conn.source_id) {
                related.push(format!(
                    "{} <- {}",
                    conn.relationship_type.as_str(),
                    source_name,
                ));
            }
        }
    }

    related
}

fn domain_label(domain: QueryDomain) -> &'static str {
    match domain {
        QueryDomain::Medication => "medication",
        QueryDomain::Lab => "lab",
        QueryDomain::Symptom => "symptom",
        QueryDomain::Diagnosis => "diagnosis",
        QueryDomain::Allergy => "allergy",
        QueryDomain::Procedure => "procedure",
        QueryDomain::Timeline => "timeline",
        QueryDomain::General => "general",
    }
}

fn significance_label(s_factor: f32) -> &'static str {
    if s_factor >= 1.8 {
        "critical"
    } else if s_factor >= 1.3 {
        "high"
    } else if s_factor >= 0.8 {
        "normal"
    } else {
        "low"
    }
}

/// Compute grounding level from scored items (replaces BOUNDARY_CHECK).
///
/// Grounding is a property of the data, not an LLM self-report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GroundingLevel {
    /// Multiple verified items with high scores.
    High,
    /// Some items found but low confidence or few sources.
    Moderate,
    /// Very few items, low scores.
    Low,
    /// No items scored above threshold.
    None,
}

pub fn compute_grounding(result: &ScoringResult) -> GroundingLevel {
    if result.items.is_empty() {
        return GroundingLevel::None;
    }

    let top_score = result.items[0].score;
    let count = result.items.len();

    if count >= 3 && top_score >= 1.0 {
        GroundingLevel::High
    } else if count >= 1 && top_score >= 0.3 {
        GroundingLevel::Moderate
    } else {
        GroundingLevel::Low
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::rag::medical_item::{ItemType, MedicalItem, SeveritySignal, StatusSignal};
    use chrono::NaiveDate;
    use uuid::Uuid;

    fn make_scored(name: &str, item_type: ItemType, score: f32, s_factor: f32) -> ScoredItem {
        ScoredItem {
            item: MedicalItem {
                id: Uuid::new_v4(),
                item_type,
                display_name: name.into(),
                searchable_text: name.into(),
                document_id: Some(Uuid::new_v4()),
                relevant_date: Some(NaiveDate::from_ymd_opt(2026, 2, 1).unwrap()),
                severity: SeveritySignal::None,
                status: StatusSignal::Active,
            },
            d_factor: 2.0,
            r_factor: 0.8,
            v_factor: 0.7,
            t_factor: 1.0,
            s_factor,
            u_factor: 0.0,
            score,
        }
    }

    #[test]
    fn format_produces_xml_structure() {
        let result = ScoringResult {
            items: vec![make_scored("Metformin", ItemType::Medication, 1.5, 1.0)],
            query_domain: QueryDomain::Medication,
            maturity: 0.5,
            config: super::super::factors::ScoringConfig::default(),
        };

        let text = format_scored_context(&result, &[]);
        assert!(text.contains("<SCORED_MEDICAL_DATA"));
        assert!(text.contains("domain=\"medication\""));
        assert!(text.contains("rank=\"1\""));
        assert!(text.contains("Metformin"));
        assert!(text.contains("</SCORED_MEDICAL_DATA>"));
    }

    #[test]
    fn empty_result_returns_empty_string() {
        let result = ScoringResult {
            items: vec![],
            query_domain: QueryDomain::General,
            maturity: 0.0,
            config: super::super::factors::ScoringConfig::default(),
        };
        assert!(format_scored_context(&result, &[]).is_empty());
    }

    #[test]
    fn uncertain_items_flagged() {
        let mut item = make_scored("Problematic", ItemType::Medication, 0.5, 1.0);
        item.u_factor = 0.5; // > 0.3 threshold

        let result = ScoringResult {
            items: vec![item],
            query_domain: QueryDomain::Medication,
            maturity: 0.5,
            config: super::super::factors::ScoringConfig::default(),
        };

        let text = format_scored_context(&result, &[]);
        assert!(text.contains("[UNCERTAIN]"));
    }

    #[test]
    fn critical_severity_shows_significance() {
        let item = make_scored("Potassium", ItemType::LabResult, 2.0, 2.0);
        let result = ScoringResult {
            items: vec![item],
            query_domain: QueryDomain::Lab,
            maturity: 0.5,
            config: super::super::factors::ScoringConfig::default(),
        };

        let text = format_scored_context(&result, &[]);
        assert!(text.contains("significance=\"critical\""));
    }

    #[test]
    fn related_items_via_connections() {
        let med_id = Uuid::new_v4();
        let dx_id = Uuid::new_v4();

        let mut med_item = make_scored("Metformin", ItemType::Medication, 1.5, 1.0);
        med_item.item.id = med_id;

        let mut dx_item = make_scored("Type 2 Diabetes", ItemType::Diagnosis, 0.8, 1.0);
        dx_item.item.id = dx_id;

        let connections = vec![EntityConnection {
            id: Uuid::new_v4(),
            source_type: crate::models::entity_connection::EntityType::Medication,
            source_id: med_id,
            target_type: crate::models::entity_connection::EntityType::Diagnosis,
            target_id: dx_id,
            relationship_type: crate::models::entity_connection::RelationshipType::PrescribedFor,
            confidence: 0.95,
            document_id: Uuid::new_v4(),
            created_at: "2026-01-01".into(),
        }];

        let result = ScoringResult {
            items: vec![med_item, dx_item],
            query_domain: QueryDomain::Medication,
            maturity: 0.5,
            config: super::super::factors::ScoringConfig::default(),
        };

        let text = format_scored_context(&result, &connections);
        assert!(text.contains("<RELATED>"));
        assert!(text.contains("PrescribedFor -> Type 2 Diabetes"));
    }

    // ── Grounding level ───────────────────────────────────────────

    #[test]
    fn grounding_none_for_empty() {
        let result = ScoringResult {
            items: vec![],
            query_domain: QueryDomain::General,
            maturity: 0.0,
            config: super::super::factors::ScoringConfig::default(),
        };
        assert_eq!(compute_grounding(&result), GroundingLevel::None);
    }

    #[test]
    fn grounding_high_for_multiple_strong_items() {
        let result = ScoringResult {
            items: vec![
                make_scored("A", ItemType::Medication, 1.5, 1.0),
                make_scored("B", ItemType::Diagnosis, 1.2, 1.0),
                make_scored("C", ItemType::LabResult, 1.0, 1.0),
            ],
            query_domain: QueryDomain::Medication,
            maturity: 0.5,
            config: super::super::factors::ScoringConfig::default(),
        };
        assert_eq!(compute_grounding(&result), GroundingLevel::High);
    }

    #[test]
    fn grounding_moderate_for_single_item() {
        let result = ScoringResult {
            items: vec![make_scored("A", ItemType::Medication, 0.5, 1.0)],
            query_domain: QueryDomain::Medication,
            maturity: 0.5,
            config: super::super::factors::ScoringConfig::default(),
        };
        assert_eq!(compute_grounding(&result), GroundingLevel::Moderate);
    }
}
