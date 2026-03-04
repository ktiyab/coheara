//! ME-01 Brick 4: Six scoring factors and final equation.
//!
//! M(item, query) = D · R · V · T · S · (1 - U)
//!
//! - D: Domain relevance (from cross-domain matrix)
//! - R: Relevance bridge (BM25 + graph walk)
//! - V: Verification state (how trustworthy the data is)
//! - T: Temporal decay (recency with type-specific half-lives)
//! - S: Clinical significance (severity + connection count)
//! - U: Uncertainty (open alerts + missing critical fields)

use chrono::NaiveDate;
use uuid::Uuid;

use super::domain::{QueryDomain, domain_relevance};
use super::medical_item::{ItemType, MedicalItem, SeveritySignal, StatusSignal};

/// A scored medical item ready for ranking and context assembly.
#[derive(Debug, Clone)]
pub struct ScoredItem {
    pub item: MedicalItem,
    /// Domain relevance factor [0.2, 2.0].
    pub d_factor: f32,
    /// Relevance bridge factor [0, 1].
    pub r_factor: f32,
    /// Verification factor [0.05, 1.0].
    pub v_factor: f32,
    /// Temporal decay factor [0.1, 1.0].
    pub t_factor: f32,
    /// Clinical significance factor [0.5, 2.0].
    pub s_factor: f32,
    /// Uncertainty factor [0, 0.8].
    pub u_factor: f32,
    /// Final score: D * R * V * T * S * (1 - U).
    pub score: f32,
}

/// Parameters for scoring (determined by profile maturity).
#[derive(Debug, Clone)]
pub struct ScoringConfig {
    /// Weight for BM25 text relevance in R factor.
    pub w_lexical: f32,
    /// Weight for graph walk relevance in R factor.
    pub w_graph: f32,
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            w_lexical: 0.70,
            w_graph: 0.30,
        }
    }
}

/// Context for uncertainty computation: which item IDs have open alerts.
pub struct UncertaintyContext {
    /// Map of entity UUID -> count of open (undismissed) alerts.
    pub alert_counts: std::collections::HashMap<Uuid, usize>,
}

impl UncertaintyContext {
    pub fn empty() -> Self {
        Self {
            alert_counts: std::collections::HashMap::new(),
        }
    }
}

/// Context for verification computation: document_id -> (verified, confirmed).
pub struct VerificationContext {
    /// Map of document UUID -> (doc_verified, pipeline_confirmed).
    pub doc_status: std::collections::HashMap<Uuid, (bool, bool)>,
}

impl VerificationContext {
    pub fn empty() -> Self {
        Self {
            doc_status: std::collections::HashMap::new(),
        }
    }

    /// Look up verification state for a document.
    /// Returns (doc_verified, user_confirmed).
    pub fn lookup(&self, doc_id: Option<Uuid>) -> (bool, bool) {
        doc_id
            .and_then(|id| self.doc_status.get(&id).copied())
            .unwrap_or((false, false))
    }
}

// ═══════════════════════════════════════════════════════════════════
// FACTOR COMPUTATIONS
// ═══════════════════════════════════════════════════════════════════

/// D factor: Domain relevance from cross-domain matrix.
pub fn compute_d(item_type: ItemType, query_domain: QueryDomain) -> f32 {
    domain_relevance(item_type, query_domain)
}

/// R factor: Combined BM25 + graph relevance.
///
/// R = w_lex * bm25_normalized + w_graph * graph_normalized
/// Range: [0, 1]
pub fn compute_r(bm25_score: f32, graph_score: f32, config: &ScoringConfig) -> f32 {
    let r = config.w_lexical * bm25_score + config.w_graph * graph_score;
    r.clamp(0.0, 1.0)
}

/// V factor: Verification state.
///
/// Based on document pipeline status:
/// - Extracted (pending review): 0.4
/// - User confirmed: 0.7
/// - Doctor verified document: 1.0
/// - Dismissed: 0.05
///
/// Special: severe allergies have a floor of 0.5.
pub fn compute_v(item: &MedicalItem, doc_verified: bool, user_confirmed: bool) -> f32 {
    let base: f32 = if doc_verified {
        1.0
    } else if user_confirmed {
        0.7
    } else {
        0.4 // Extracted but not reviewed
    };

    // Severe allergy floor.
    if item.item_type == ItemType::Allergy {
        if let SeveritySignal::AllergySeverity(ref sev) = item.severity {
            use crate::models::enums::AllergySeverity;
            if matches!(sev, AllergySeverity::Severe | AllergySeverity::LifeThreatening) {
                return base.max(0.5);
            }
        }
    }

    base
}

/// T factor: Temporal decay with type-specific decay rates.
///
/// T = max(0.1, e^(-lambda * days_since))
///
/// Decay rates (lambda):
/// - Allergies: 0.0001 (quasi-permanent, half-life ~19 years)
/// - Active medications: 0.0005 (half-life ~3.8 years, resets on refill)
/// - Diagnoses: 0.0003 (half-life ~6.3 years)
/// - Lab results: 0.005 (half-life ~138 days)
/// - Symptoms: 0.02 (half-life ~35 days, active ones don't decay)
/// - Vital signs: 0.01 (half-life ~69 days)
pub fn compute_t(item: &MedicalItem, today: NaiveDate) -> f32 {
    let days_since = item
        .relevant_date
        .map(|d| (today - d).num_days().max(0) as f32)
        .unwrap_or(365.0); // Unknown date → assume 1 year old

    // Active items with no end date: minimal decay.
    if item.status == StatusSignal::Active && days_since < 30.0 {
        return 1.0;
    }

    let lambda = match item.item_type {
        ItemType::Allergy => 0.0001,
        ItemType::Medication => {
            if item.status == StatusSignal::Active {
                0.0005
            } else {
                0.003 // Stopped meds decay faster
            }
        }
        ItemType::Diagnosis => {
            if item.status == StatusSignal::Active {
                0.0003
            } else {
                0.002 // Resolved diagnoses decay faster
            }
        }
        ItemType::LabResult => 0.005,
        ItemType::Symptom => {
            if item.status == StatusSignal::Active {
                0.002 // Active symptoms decay slowly
            } else {
                0.02 // Resolved symptoms fade fast
            }
        }
        ItemType::VitalSign => 0.01,
    };

    let t = (-lambda * days_since).exp();
    t.max(0.1) // Floor at 0.1 — nothing completely disappears
}

/// S factor: Clinical significance.
///
/// Base significance from severity/abnormality + bonus from connection count.
/// Range: [0.5, 2.0]
///
/// - Critical labs: 2.0
/// - Life-threatening allergies: 2.0
/// - Active meds: 1.0 + 0.15 per connection (max +0.8)
/// - Normal labs: 0.8
pub fn compute_s(item: &MedicalItem, connection_count: usize) -> f32 {
    let base = match &item.severity {
        SeveritySignal::LabFlag(flag) => {
            use crate::models::enums::AbnormalFlag;
            match flag {
                AbnormalFlag::CriticalHigh | AbnormalFlag::CriticalLow => 2.0,
                AbnormalFlag::High | AbnormalFlag::Low => 1.5,
                AbnormalFlag::Normal => 0.8,
            }
        }
        SeveritySignal::AllergySeverity(sev) => {
            use crate::models::enums::AllergySeverity;
            match sev {
                AllergySeverity::LifeThreatening => 2.0,
                AllergySeverity::Severe => 1.8,
                AllergySeverity::Moderate => 1.2,
                AllergySeverity::Mild => 0.8,
            }
        }
        SeveritySignal::Numeric(n) => {
            // Symptoms: 0-10 scale mapped to [0.5, 2.0]
            let clamped = (*n as f32).clamp(0.0, 10.0);
            0.5 + clamped * 0.15
        }
        SeveritySignal::None => {
            // Default by item type
            match item.item_type {
                ItemType::Medication => {
                    if item.status == StatusSignal::Active { 1.0 } else { 0.6 }
                }
                ItemType::Diagnosis => {
                    if item.status == StatusSignal::Active { 1.0 } else { 0.6 }
                }
                _ => 0.8,
            }
        }
    };

    // Connection bonus: +0.15 per connection, max +0.8
    let conn_bonus = (connection_count as f32 * 0.15).min(0.8);

    (base + conn_bonus).clamp(0.5, 2.0)
}

/// U factor: Uncertainty from open alerts + missing critical fields.
///
/// Range: [0, 0.8] — items are never fully suppressed.
pub fn compute_u(item: &MedicalItem, uncertainty_ctx: &UncertaintyContext) -> f32 {
    let mut u = 0.0f32;

    // Open alerts: +0.15 per alert, max 0.6
    let alert_count = uncertainty_ctx
        .alert_counts
        .get(&item.id)
        .copied()
        .unwrap_or(0);
    u += (alert_count as f32 * 0.15).min(0.6);

    // Missing critical fields: +0.1 per missing field
    if item.relevant_date.is_none() {
        u += 0.1;
    }
    if item.document_id.is_none() {
        u += 0.1;
    }

    u.min(0.8)
}

// ═══════════════════════════════════════════════════════════════════
// FULL EQUATION
// ═══════════════════════════════════════════════════════════════════

/// Compute the full scoring equation for one item.
///
/// M = D · R · V · T · S · (1 - U)
pub fn compute_score(
    item: &MedicalItem,
    query_domain: QueryDomain,
    bm25: f32,
    graph: f32,
    config: &ScoringConfig,
    doc_verified: bool,
    user_confirmed: bool,
    today: NaiveDate,
    connection_count: usize,
    uncertainty_ctx: &UncertaintyContext,
) -> ScoredItem {
    let d = compute_d(item.item_type, query_domain);
    let r = compute_r(bm25, graph, config);
    let v = compute_v(item, doc_verified, user_confirmed);
    let t = compute_t(item, today);
    let s = compute_s(item, connection_count);
    let u = compute_u(item, uncertainty_ctx);

    let score = d * r * v * t * s * (1.0 - u);

    ScoredItem {
        item: item.clone(),
        d_factor: d,
        r_factor: r,
        v_factor: v,
        t_factor: t,
        s_factor: s,
        u_factor: u,
        score,
    }
}

/// Score all items and return sorted by score (highest first).
///
/// This is the Phase 1 + Phase 2 combined computation.
pub fn score_all(
    items: &[MedicalItem],
    bm25_scores: &[f32],
    graph_scores: &[f32],
    query_domain: QueryDomain,
    config: &ScoringConfig,
    connection_counts: &[usize],
    today: NaiveDate,
    uncertainty_ctx: &UncertaintyContext,
    verification_ctx: &VerificationContext,
) -> Vec<ScoredItem> {
    let mut scored: Vec<ScoredItem> = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let (doc_verified, user_confirmed) = verification_ctx.lookup(item.document_id);
            compute_score(
                item,
                query_domain,
                bm25_scores.get(i).copied().unwrap_or(0.0),
                graph_scores.get(i).copied().unwrap_or(0.0),
                config,
                doc_verified,
                user_confirmed,
                today,
                connection_counts.get(i).copied().unwrap_or(0),
                uncertainty_ctx,
            )
        })
        .collect();

    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scored
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::enums::*;
    use crate::pipeline::rag::medical_item::*;

    fn make_med(name: &str, status: MedicationStatus) -> MedicalItem {
        MedicalItem {
            id: Uuid::new_v4(),
            item_type: ItemType::Medication,
            display_name: name.into(),
            searchable_text: name.into(),
            document_id: Some(Uuid::new_v4()),
            relevant_date: Some(NaiveDate::from_ymd_opt(2026, 2, 1).unwrap()),
            severity: SeveritySignal::None,
            status: match status {
                MedicationStatus::Active => StatusSignal::Active,
                _ => StatusSignal::Inactive,
            },
        }
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 3, 3).unwrap()
    }

    // ── D factor ──────────────────────────────────────────────────

    #[test]
    fn d_factor_medication_for_medication_query() {
        let d = compute_d(ItemType::Medication, QueryDomain::Medication);
        assert_eq!(d, 2.0);
    }

    // ── R factor ──────────────────────────────────────────────────

    #[test]
    fn r_factor_combines_lexical_and_graph() {
        let config = ScoringConfig::default();
        let r = compute_r(0.8, 0.6, &config);
        let expected = 0.70 * 0.8 + 0.30 * 0.6;
        assert!((r - expected).abs() < 0.001);
    }

    #[test]
    fn r_factor_clamped_to_one() {
        let config = ScoringConfig { w_lexical: 0.9, w_graph: 0.9 };
        let r = compute_r(1.0, 1.0, &config);
        assert_eq!(r, 1.0);
    }

    // ── V factor ──────────────────────────────────────────────────

    #[test]
    fn v_factor_extracted() {
        let item = make_med("Aspirin", MedicationStatus::Active);
        assert_eq!(compute_v(&item, false, false), 0.4);
    }

    #[test]
    fn v_factor_confirmed() {
        let item = make_med("Aspirin", MedicationStatus::Active);
        assert_eq!(compute_v(&item, false, true), 0.7);
    }

    #[test]
    fn v_factor_verified() {
        let item = make_med("Aspirin", MedicationStatus::Active);
        assert_eq!(compute_v(&item, true, false), 1.0);
    }

    #[test]
    fn v_factor_severe_allergy_floor() {
        let item = MedicalItem {
            id: Uuid::new_v4(),
            item_type: ItemType::Allergy,
            display_name: "Penicillin".into(),
            searchable_text: "Penicillin".into(),
            document_id: None,
            relevant_date: None,
            severity: SeveritySignal::AllergySeverity(AllergySeverity::LifeThreatening),
            status: StatusSignal::Active,
        };
        let v = compute_v(&item, false, false);
        assert!(v >= 0.5, "Severe allergy V floor is 0.5, got {}", v);
    }

    // ── T factor ──────────────────────────────────────────────────

    #[test]
    fn t_factor_recent_active_is_one() {
        let item = MedicalItem {
            id: Uuid::new_v4(),
            item_type: ItemType::Medication,
            display_name: "Recent".into(),
            searchable_text: "Recent".into(),
            document_id: None,
            relevant_date: Some(NaiveDate::from_ymd_opt(2026, 3, 1).unwrap()),
            severity: SeveritySignal::None,
            status: StatusSignal::Active,
        };
        let t = compute_t(&item, today());
        assert_eq!(t, 1.0, "Active item from 2 days ago should be 1.0");
    }

    #[test]
    fn t_factor_allergy_decays_slowly() {
        let item = MedicalItem {
            id: Uuid::new_v4(),
            item_type: ItemType::Allergy,
            display_name: "Old allergy".into(),
            searchable_text: "Old allergy".into(),
            document_id: None,
            relevant_date: Some(NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()),
            severity: SeveritySignal::None,
            status: StatusSignal::Active,
        };
        let t = compute_t(&item, today());
        // ~2250 days, lambda=0.0001, e^(-0.225) ≈ 0.80
        assert!(t > 0.7, "Allergy from 6 years ago should still be relevant: {}", t);
    }

    #[test]
    fn t_factor_old_lab_decays() {
        let item = MedicalItem {
            id: Uuid::new_v4(),
            item_type: ItemType::LabResult,
            display_name: "Old lab".into(),
            searchable_text: "Old lab".into(),
            document_id: None,
            relevant_date: Some(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()),
            severity: SeveritySignal::None,
            status: StatusSignal::Current,
        };
        let t = compute_t(&item, today());
        // ~427 days, lambda=0.005, e^(-2.135) ≈ 0.118
        assert!(t < 0.3, "Lab from 14 months ago should have decayed significantly: {}", t);
        assert!(t >= 0.1, "Floor should be 0.1: {}", t);
    }

    #[test]
    fn t_factor_floor_at_point_one() {
        let item = MedicalItem {
            id: Uuid::new_v4(),
            item_type: ItemType::Symptom,
            display_name: "Ancient".into(),
            searchable_text: "Ancient".into(),
            document_id: None,
            relevant_date: Some(NaiveDate::from_ymd_opt(2000, 1, 1).unwrap()),
            severity: SeveritySignal::None,
            status: StatusSignal::Inactive,
        };
        let t = compute_t(&item, today());
        assert_eq!(t, 0.1, "Very old resolved symptom should hit floor");
    }

    // ── S factor ──────────────────────────────────────────────────

    #[test]
    fn s_factor_critical_lab() {
        let item = MedicalItem {
            id: Uuid::new_v4(),
            item_type: ItemType::LabResult,
            display_name: "Potassium".into(),
            searchable_text: "Potassium".into(),
            document_id: None,
            relevant_date: None,
            severity: SeveritySignal::LabFlag(AbnormalFlag::CriticalHigh),
            status: StatusSignal::Current,
        };
        assert_eq!(compute_s(&item, 0), 2.0);
    }

    #[test]
    fn s_factor_connection_bonus() {
        let item = make_med("Metformin", MedicationStatus::Active);
        let s_no_conn = compute_s(&item, 0);
        let s_3_conn = compute_s(&item, 3);
        assert!(s_3_conn > s_no_conn);
        assert!((s_3_conn - s_no_conn - 0.45).abs() < 0.01, "3 connections = +0.45");
    }

    #[test]
    fn s_factor_connection_bonus_capped() {
        let item = make_med("Metformin", MedicationStatus::Active);
        let s_10 = compute_s(&item, 10);
        let s_20 = compute_s(&item, 20);
        assert_eq!(s_10, s_20, "Connection bonus caps at +0.8");
    }

    #[test]
    fn s_factor_capped_at_two() {
        let item = MedicalItem {
            id: Uuid::new_v4(),
            item_type: ItemType::LabResult,
            display_name: "Critical".into(),
            searchable_text: "Critical".into(),
            document_id: None,
            relevant_date: None,
            severity: SeveritySignal::LabFlag(AbnormalFlag::CriticalHigh),
            status: StatusSignal::Current,
        };
        let s = compute_s(&item, 10);
        assert_eq!(s, 2.0, "S factor capped at 2.0");
    }

    // ── U factor ──────────────────────────────────────────────────

    #[test]
    fn u_factor_no_alerts_no_missing() {
        let item = make_med("Clean", MedicationStatus::Active);
        let ctx = UncertaintyContext::empty();
        assert_eq!(compute_u(&item, &ctx), 0.0);
    }

    #[test]
    fn u_factor_with_alerts() {
        let item = make_med("Problematic", MedicationStatus::Active);
        let mut ctx = UncertaintyContext::empty();
        ctx.alert_counts.insert(item.id, 3);
        let u = compute_u(&item, &ctx);
        assert!((u - 0.45).abs() < 0.01, "3 alerts * 0.15 = 0.45, got {}", u);
    }

    #[test]
    fn u_factor_capped_at_point_eight() {
        let mut item = make_med("Very uncertain", MedicationStatus::Active);
        item.relevant_date = None;
        item.document_id = None;
        let mut ctx = UncertaintyContext::empty();
        ctx.alert_counts.insert(item.id, 10);
        let u = compute_u(&item, &ctx);
        assert_eq!(u, 0.8, "U factor capped at 0.8");
    }

    // ── Full equation ─────────────────────────────────────────────

    #[test]
    fn full_score_multiplicative() {
        let item = make_med("Metformin", MedicationStatus::Active);
        let config = ScoringConfig::default();
        let ctx = UncertaintyContext::empty();

        let scored = compute_score(
            &item,
            QueryDomain::Medication,
            0.9, // BM25 high
            0.7, // Graph moderate
            &config,
            false,
            true, // user confirmed
            today(),
            2,    // 2 connections
            &ctx,
        );

        // D=2.0, R=0.70*0.9+0.30*0.7=0.84, V=0.7, T=1.0 (recent active), S=1.0+0.30=1.30, U=0.0
        let expected = 2.0 * 0.84 * 0.7 * 1.0 * 1.30 * 1.0;
        assert!(
            (scored.score - expected).abs() < 0.1,
            "Expected ~{:.2}, got {:.2}", expected, scored.score
        );
    }

    #[test]
    fn zero_relevance_kills_score() {
        let item = make_med("Irrelevant", MedicationStatus::Active);
        let config = ScoringConfig::default();
        let ctx = UncertaintyContext::empty();

        let scored = compute_score(
            &item,
            QueryDomain::Medication,
            0.0, // No text match
            0.0, // No graph connection
            &config,
            false, false, today(), 0, &ctx,
        );

        assert_eq!(scored.score, 0.0, "Zero R should kill the score");
    }

    #[test]
    fn uncertainty_penalizes() {
        let item = make_med("Uncertain", MedicationStatus::Active);
        let config = ScoringConfig::default();
        let mut ctx = UncertaintyContext::empty();
        ctx.alert_counts.insert(item.id, 4); // U = 0.6

        let scored_uncertain = compute_score(
            &item, QueryDomain::Medication, 0.8, 0.5, &config,
            false, false, today(), 0, &ctx,
        );

        let scored_clean = compute_score(
            &item, QueryDomain::Medication, 0.8, 0.5, &config,
            false, false, today(), 0, &UncertaintyContext::empty(),
        );

        assert!(
            scored_uncertain.score < scored_clean.score,
            "Uncertainty should penalize: {} < {}", scored_uncertain.score, scored_clean.score
        );
    }

    // ── score_all ─────────────────────────────────────────────────

    #[test]
    fn score_all_sorts_descending() {
        let items = vec![
            make_med("Low", MedicationStatus::Stopped),
            make_med("High", MedicationStatus::Active),
        ];
        let bm25 = vec![0.2, 0.9];
        let graph = vec![0.1, 0.8];
        let config = ScoringConfig::default();
        let conn_counts = vec![0, 3];
        let ctx = UncertaintyContext::empty();

        let sorted = score_all(
            &items, &bm25, &graph,
            QueryDomain::Medication, &config, &conn_counts,
            today(), &ctx, &VerificationContext::empty(),
        );

        assert_eq!(sorted[0].item.display_name, "High");
        assert_eq!(sorted[1].item.display_name, "Low");
        assert!(sorted[0].score >= sorted[1].score);
    }

    #[test]
    fn score_all_uses_verification_context() {
        let items = vec![
            make_med("Unverified", MedicationStatus::Active),
            make_med("Verified", MedicationStatus::Active),
        ];
        let bm25 = vec![0.8, 0.8];
        let graph = vec![0.5, 0.5];
        let config = ScoringConfig::default();
        let conn_counts = vec![0, 0];
        let ctx = UncertaintyContext::empty();

        // Build verification context: second item's doc is verified
        let mut vctx = VerificationContext::empty();
        vctx.doc_status.insert(items[1].document_id.unwrap(), (true, true));

        let sorted = score_all(
            &items, &bm25, &graph,
            QueryDomain::Medication, &config, &conn_counts,
            today(), &ctx, &vctx,
        );

        // Verified item should score higher (V=1.0 vs V=0.4)
        assert_eq!(sorted[0].item.display_name, "Verified");
        assert_eq!(sorted[0].v_factor, 1.0);
        assert_eq!(sorted[1].v_factor, 0.4);
        assert!(sorted[0].score > sorted[1].score);
    }

    #[test]
    fn score_all_uses_confirmation_context() {
        let items = vec![make_med("Confirmed", MedicationStatus::Active)];
        let bm25 = vec![0.8];
        let graph = vec![0.5];
        let config = ScoringConfig::default();
        let conn_counts = vec![0];
        let ctx = UncertaintyContext::empty();

        // Doc confirmed but not verified
        let mut vctx = VerificationContext::empty();
        vctx.doc_status.insert(items[0].document_id.unwrap(), (false, true));

        let scored = score_all(
            &items, &bm25, &graph,
            QueryDomain::Medication, &config, &conn_counts,
            today(), &ctx, &vctx,
        );

        assert_eq!(scored[0].v_factor, 0.7);
    }
}
