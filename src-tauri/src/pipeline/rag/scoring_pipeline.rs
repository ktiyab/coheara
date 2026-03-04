//! ME-01 Brick 5-6: Scoring pipeline and profile maturity.
//!
//! Runs the full scoring equation over all medical items:
//! Phase 1: Bulk load → BM25 → graph walk → filter candidates
//! Phase 2: Compute V, T, S, U → final score → rank → select top N
//!
//! Returns scored items ready for context assembly.

use std::collections::HashMap;
use uuid::Uuid;

use super::domain::{QueryDomain, classify_domain};
use super::factors::{ScoredItem, ScoringConfig, UncertaintyContext, VerificationContext, score_all};
use super::graph_walk::graph_walk;
use super::medical_item::{MedicalItem, collect_items};
use super::scoring::{bm25_score, normalize_scores};
use super::types::StructuredContext;

use crate::models::entity_connection::EntityConnection;

/// Maximum items to return from scoring pipeline.
const TOP_N: usize = 15;
/// Minimum D*R score to be a candidate (Phase 1 filter).
const MIN_BRIDGE: f32 = 0.05;

/// Result of the scoring pipeline.
#[derive(Debug, Clone)]
pub struct ScoringResult {
    /// Scored items, sorted by score descending. Max TOP_N items.
    pub items: Vec<ScoredItem>,
    /// Detected query domain.
    pub query_domain: QueryDomain,
    /// Profile maturity [0.0, 1.0].
    pub maturity: f32,
    /// Scoring configuration used (adapted by maturity).
    pub config: ScoringConfig,
}

/// Run the full scoring pipeline.
///
/// This is the core ME-01 computation. Pure function — no DB, no LLM, no I/O.
/// V factor uses verification_ctx (document verified/confirmed state).
/// U factor uses alert_entity_ids (open alert counts per entity).
pub fn run_scoring(
    query: &str,
    structured: &StructuredContext,
    connections: &[EntityConnection],
    alert_entity_ids: &HashMap<Uuid, usize>,
    verification_ctx: &VerificationContext,
) -> ScoringResult {
    // Step 1: Classify query domain
    let query_domain = classify_domain(query);

    // Step 2: Collect all medical items
    let items = collect_items(structured);
    if items.is_empty() {
        return ScoringResult {
            items: vec![],
            query_domain,
            maturity: 0.0,
            config: ScoringConfig::default(),
        };
    }

    // Step 3: Profile maturity (determines BM25 vs graph weight)
    let maturity = compute_maturity(structured);
    let config = maturity_config(maturity);

    // Step 4: BM25 scoring (Phase 1 — text relevance)
    let raw_bm25 = bm25_score(query, &items);
    let bm25_normalized = normalize_scores(&raw_bm25);

    // Step 5: Graph walk (Phase 1 — relationship propagation)
    let graph_scores = graph_walk(&items, &bm25_normalized, connections);

    // Step 6: Count connections per item
    let connection_counts = count_connections(&items, connections);

    // Step 7: Build uncertainty context from alert entity IDs
    let uncertainty_ctx = UncertaintyContext {
        alert_counts: alert_entity_ids.clone(),
    };

    // Step 8: Compute full equation for all items (Phase 2)
    let today = chrono::Local::now().date_naive();
    let mut scored = score_all(
        &items,
        &bm25_normalized,
        &graph_scores,
        query_domain,
        &config,
        &connection_counts,
        today,
        &uncertainty_ctx,
        verification_ctx,
    );

    // Step 9: Filter by minimum bridge score and take top N
    scored.retain(|s| s.d_factor * s.r_factor >= MIN_BRIDGE);
    scored.truncate(TOP_N);

    ScoringResult {
        items: scored,
        query_domain,
        maturity,
        config,
    }
}

/// Compute profile maturity as a ratio of filled data categories.
///
/// Maturity determines how much to trust graph structure vs text matching.
/// Range: [0.0, 1.0]
/// - 0.0 = completely empty profile
/// - 1.0 = all 6 data categories populated with multiple items
fn compute_maturity(ctx: &StructuredContext) -> f32 {
    let categories = [
        ctx.medications.len(),
        ctx.lab_results.len(),
        ctx.diagnoses.len(),
        ctx.allergies.len(),
        ctx.symptoms.len(),
        ctx.vital_signs.len(),
    ];

    // Each category contributes up to 1/6 maturity.
    // Within each category: 1 item = 0.5, 3+ items = 1.0 of its share.
    let total: f32 = categories
        .iter()
        .map(|&count| {
            if count == 0 {
                0.0
            } else if count < 3 {
                0.5 / 6.0
            } else {
                1.0 / 6.0
            }
        })
        .sum();

    total.clamp(0.0, 1.0)
}

/// Adapt scoring config based on profile maturity.
///
/// Raw profiles (< 20% maturity): trust text matching more.
/// Mature profiles (>= 80% maturity): trust graph structure more.
fn maturity_config(maturity: f32) -> ScoringConfig {
    // Linear interpolation between raw and mature weights.
    // Raw:    w_lex=0.85, w_graph=0.15
    // Mature: w_lex=0.55, w_graph=0.45
    let t = maturity.clamp(0.0, 1.0);
    ScoringConfig {
        w_lexical: 0.85 - 0.30 * t,
        w_graph: 0.15 + 0.30 * t,
    }
}

/// Count how many connections reference each item.
fn count_connections(items: &[MedicalItem], connections: &[EntityConnection]) -> Vec<usize> {
    let id_set: HashMap<Uuid, usize> = items
        .iter()
        .enumerate()
        .map(|(i, item)| (item.id, i))
        .collect();

    let mut counts = vec![0usize; items.len()];
    for conn in connections {
        if let Some(&idx) = id_set.get(&conn.source_id) {
            counts[idx] += 1;
        }
        if let Some(&idx) = id_set.get(&conn.target_id) {
            counts[idx] += 1;
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::enums::*;
    use crate::models::entity_connection::{EntityConnection, EntityType, RelationshipType};
    use crate::models::*;
    use uuid::Uuid;

    fn make_structured_context() -> StructuredContext {
        let doc_id = Uuid::new_v4();
        StructuredContext {
            medications: vec![Medication {
                id: Uuid::new_v4(),
                generic_name: "Metformin".into(),
                brand_name: Some("Glucophage".into()),
                dose: "500mg".into(),
                frequency: "twice daily".into(),
                frequency_type: FrequencyType::Scheduled,
                route: "oral".into(),
                prescriber_id: None,
                start_date: Some(chrono::NaiveDate::from_ymd_opt(2025, 6, 1).unwrap()),
                end_date: None,
                reason_start: None,
                reason_stop: None,
                is_otc: false,
                status: MedicationStatus::Active,
                administration_instructions: None,
                max_daily_dose: None,
                condition: Some("Type 2 Diabetes".into()),
                dose_type: DoseType::Fixed,
                is_compound: false,
                document_id: doc_id,
            }],
            diagnoses: vec![Diagnosis {
                id: Uuid::new_v4(),
                name: "Type 2 Diabetes".into(),
                icd_code: Some("E11".into()),
                date_diagnosed: Some(chrono::NaiveDate::from_ymd_opt(2024, 3, 15).unwrap()),
                diagnosing_professional_id: None,
                status: DiagnosisStatus::Active,
                document_id: doc_id,
            }],
            lab_results: vec![LabResult {
                id: Uuid::new_v4(),
                test_name: "HbA1c".into(),
                test_code: Some("4548-4".into()),
                value: Some(7.2),
                value_text: None,
                unit: Some("%".into()),
                reference_range_low: Some(4.0),
                reference_range_high: Some(5.6),
                abnormal_flag: AbnormalFlag::High,
                collection_date: chrono::NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
                lab_facility: None,
                ordering_physician_id: None,
                document_id: doc_id,
            }],
            allergies: vec![Allergy {
                id: Uuid::new_v4(),
                allergen: "Penicillin".into(),
                reaction: Some("Anaphylaxis".into()),
                severity: AllergySeverity::LifeThreatening,
                allergen_category: None,
                date_identified: None,
                source: AllergySource::DocumentExtracted,
                document_id: Some(doc_id),
                verified: true,
            }],
            symptoms: vec![],
            vital_signs: vec![],
            recent_conversations: vec![],
            screening_records: vec![],
            entity_connections: vec![],
        }
    }

    #[test]
    fn scoring_returns_results_for_medication_query() {
        let ctx = make_structured_context();
        let result = run_scoring("What dose of metformin?", &ctx, &[], &HashMap::new(), &VerificationContext::empty());

        assert_eq!(result.query_domain, QueryDomain::Medication);
        assert!(!result.items.is_empty(), "Should return scored items");

        // Metformin should be ranked first
        assert_eq!(result.items[0].item.display_name, "Metformin");
        assert!(result.items[0].score > 0.0);
    }

    #[test]
    fn empty_context_returns_empty() {
        let ctx = StructuredContext::default();
        let result = run_scoring("What medications?", &ctx, &[], &HashMap::new(), &VerificationContext::empty());
        assert!(result.items.is_empty());
    }

    #[test]
    fn allergy_surfaces_for_medication_query() {
        let ctx = make_structured_context();
        let result = run_scoring("Can I take amoxicillin?", &ctx, &[], &HashMap::new(), &VerificationContext::empty());

        // "allergy" domain detected due to "can i take"
        // Penicillin allergy should surface
        let has_allergy = result
            .items
            .iter()
            .any(|s| s.item.display_name == "Penicillin");
        assert!(has_allergy, "Penicillin allergy should surface for 'can i take' query");
    }

    #[test]
    fn connections_boost_related_items() {
        let ctx = make_structured_context();
        let med_id = ctx.medications[0].id;
        let dx_id = ctx.diagnoses[0].id;

        let connections = vec![EntityConnection {
            id: Uuid::new_v4(),
            source_type: EntityType::Medication,
            source_id: med_id,
            target_type: EntityType::Diagnosis,
            target_id: dx_id,
            relationship_type: RelationshipType::PrescribedFor,
            confidence: 0.95,
            document_id: Uuid::new_v4(),
            created_at: "2026-01-01".into(),
        }];

        let with_conn = run_scoring("metformin", &ctx, &connections, &HashMap::new(), &VerificationContext::empty());
        let without_conn = run_scoring("metformin", &ctx, &[], &HashMap::new(), &VerificationContext::empty());

        // Diabetes should rank higher with the PrescribedFor connection
        let dx_score_with = with_conn
            .items
            .iter()
            .find(|s| s.item.display_name == "Type 2 Diabetes")
            .map(|s| s.score)
            .unwrap_or(0.0);
        let dx_score_without = without_conn
            .items
            .iter()
            .find(|s| s.item.display_name == "Type 2 Diabetes")
            .map(|s| s.score)
            .unwrap_or(0.0);

        assert!(
            dx_score_with > dx_score_without,
            "Connection should boost diabetes score: {} > {}", dx_score_with, dx_score_without
        );
    }

    #[test]
    fn maturity_affects_config() {
        let config_raw = maturity_config(0.0);
        let config_mature = maturity_config(1.0);

        assert!(config_raw.w_lexical > config_mature.w_lexical);
        assert!(config_raw.w_graph < config_mature.w_graph);
        assert!((config_raw.w_lexical - 0.85).abs() < 0.01);
        assert!((config_mature.w_graph - 0.45).abs() < 0.01);
    }

    #[test]
    fn maturity_computation() {
        let empty = StructuredContext::default();
        assert_eq!(compute_maturity(&empty), 0.0);

        let partial = make_structured_context(); // has meds, dx, labs, allergies (4 of 6)
        let m = compute_maturity(&partial);
        assert!(m > 0.0 && m < 1.0, "Partial context maturity: {}", m);
    }

    #[test]
    fn top_n_limit_respected() {
        let mut ctx = make_structured_context();
        // Add 20 medications to exceed TOP_N
        let doc_id = ctx.medications[0].document_id;
        for i in 0..20 {
            ctx.medications.push(Medication {
                id: Uuid::new_v4(),
                generic_name: format!("Drug{}", i),
                brand_name: None,
                dose: "10mg".into(),
                frequency: "daily".into(),
                frequency_type: FrequencyType::Scheduled,
                route: "oral".into(),
                prescriber_id: None,
                start_date: Some(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()),
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
                document_id: doc_id,
            });
        }

        let result = run_scoring("medications", &ctx, &[], &HashMap::new(), &VerificationContext::empty());
        assert!(result.items.len() <= TOP_N, "Should cap at {} items", TOP_N);
    }

    #[test]
    fn french_query_domain_classification() {
        let ctx = make_structured_context();
        let result = run_scoring("Quelle dose de metformine je prends?", &ctx, &[], &HashMap::new(), &VerificationContext::empty());
        assert_eq!(result.query_domain, QueryDomain::Medication);
    }
}
