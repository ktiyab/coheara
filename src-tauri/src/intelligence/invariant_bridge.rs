//! B2: Bridge ClinicalInsight → CoherenceAlert.
//!
//! Converts invariant engine outputs into the coherence alert pipeline.
//! Only bridges insight kinds NOT already covered by coherence detection:
//!   - Interaction (drug-drug) → AlertType::Interaction
//!   - MissingMonitoring → AlertType::Monitoring
//!   - ScreeningDue → AlertType::Screening
//!   - AbnormalTrend → AlertType::Trend
//!
//! Skipped (already covered by coherence detection algorithms):
//!   - Classification → overlaps with detect_critical_labs
//!   - CrossReactivity → overlaps with detect_allergy_conflicts

use chrono::NaiveDate;
use uuid::Uuid;

use crate::invariants::enrich::enrich;
use crate::invariants::types::{ClinicalInsight, InsightKind, InsightSeverity};
use crate::invariants::InvariantRegistry;
use crate::models::enums::AlertType;

use super::messages::MessageTemplates;
use super::types::{
    AlertDetail, AlertSeverity, CoherenceAlert, InteractionBridgeDetail,
    MonitoringBridgeDetail, RepositorySnapshot, ScreeningBridgeDetail, TrendBridgeDetail,
};

/// Run the invariant engine on snapshot data and bridge relevant insights to alerts.
///
/// Pure function: takes snapshot data + registry, returns alerts.
/// The caller (engine) handles dedup and storage.
pub fn detect_from_invariants(
    data: &RepositorySnapshot,
    registry: &InvariantRegistry,
) -> Vec<CoherenceAlert> {
    let today = chrono::Local::now().date_naive();

    let insights = enrich(
        &data.medications,
        &data.lab_results,
        &data.allergies,
        &data.vital_signs,
        registry,
        today,
        data.demographics.as_ref(),
    );

    bridge_insights_to_alerts(&insights, today)
}

/// Convert a list of ClinicalInsights into CoherenceAlerts.
///
/// Filters to only the 4 bridgeable insight kinds.
fn bridge_insights_to_alerts(
    insights: &[ClinicalInsight],
    _today: NaiveDate,
) -> Vec<CoherenceAlert> {
    insights
        .iter()
        .filter_map(|insight| bridge_one(insight))
        .collect()
}

/// Bridge a single ClinicalInsight to a CoherenceAlert, if applicable.
fn bridge_one(insight: &ClinicalInsight) -> Option<CoherenceAlert> {
    let (alert_type, detail, message) = match &insight.kind {
        InsightKind::Interaction => {
            let desc = insight.description.get("en");
            (
                AlertType::Interaction,
                AlertDetail::Interaction(InteractionBridgeDetail {
                    insight_key: insight.summary_key.clone(),
                    source: insight.source.clone(),
                    description: desc.to_string(),
                }),
                MessageTemplates::interaction(desc),
            )
        }
        InsightKind::MissingMonitoring => {
            let desc = insight.description.get("en");
            (
                AlertType::Monitoring,
                AlertDetail::Monitoring(MonitoringBridgeDetail {
                    insight_key: insight.summary_key.clone(),
                    source: insight.source.clone(),
                    description: desc.to_string(),
                }),
                MessageTemplates::monitoring(desc),
            )
        }
        InsightKind::ScreeningDue => {
            let desc = insight.description.get("en");
            (
                AlertType::Screening,
                AlertDetail::Screening(ScreeningBridgeDetail {
                    insight_key: insight.summary_key.clone(),
                    source: insight.source.clone(),
                    description: desc.to_string(),
                }),
                MessageTemplates::screening(desc),
            )
        }
        InsightKind::AbnormalTrend => {
            let desc = insight.description.get("en");
            (
                AlertType::Trend,
                AlertDetail::Trend(TrendBridgeDetail {
                    insight_key: insight.summary_key.clone(),
                    source: insight.source.clone(),
                    description: desc.to_string(),
                }),
                MessageTemplates::trend(desc),
            )
        }
        // Classification and CrossReactivity are handled by existing coherence detections
        _ => return None,
    };

    Some(CoherenceAlert {
        id: Uuid::new_v4(),
        alert_type,
        severity: map_severity(&insight.severity),
        entity_ids: insight.related_entities.clone(),
        source_document_ids: Vec::new(), // Invariant insights are data-driven, not document-scoped
        patient_message: message,
        detail,
        detected_at: chrono::Local::now().naive_local(),
        surfaced: false,
        dismissed: false,
        dismissal: None,
    })
}

/// Map InsightSeverity to AlertSeverity.
fn map_severity(severity: &InsightSeverity) -> AlertSeverity {
    match severity {
        InsightSeverity::Critical => AlertSeverity::Critical,
        InsightSeverity::Warning => AlertSeverity::Standard,
        InsightSeverity::Info => AlertSeverity::Info,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invariants::types::{InvariantLabel, MeaningFactors};

    const TEST_LABEL: InvariantLabel = InvariantLabel {
        key: "test_key",
        en: "Test description",
        fr: "Description de test",
        de: "Testbeschreibung",
    };

    fn make_insight(kind: InsightKind, severity: InsightSeverity, key: &str) -> ClinicalInsight {
        ClinicalInsight {
            kind,
            severity,
            summary_key: key.to_string(),
            description: TEST_LABEL,
            source: "Test Source 2024".to_string(),
            related_entities: vec![Uuid::new_v4()],
            meaning_factors: MeaningFactors::default(),
        }
    }

    #[test]
    fn interaction_insight_bridges_to_alert() {
        let insight = make_insight(
            InsightKind::Interaction,
            InsightSeverity::Warning,
            "warfarin_aspirin",
        );
        let alerts = bridge_insights_to_alerts(&[insight], chrono::Local::now().date_naive());
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].alert_type, AlertType::Interaction);
        assert_eq!(alerts[0].severity, AlertSeverity::Standard);
        assert!(alerts[0].patient_message.contains("Test description"));
    }

    #[test]
    fn monitoring_insight_bridges_to_alert() {
        let insight = make_insight(
            InsightKind::MissingMonitoring,
            InsightSeverity::Info,
            "missing_inr",
        );
        let alerts = bridge_insights_to_alerts(&[insight], chrono::Local::now().date_naive());
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].alert_type, AlertType::Monitoring);
        assert_eq!(alerts[0].severity, AlertSeverity::Info);
    }

    #[test]
    fn screening_insight_bridges_to_alert() {
        let insight = make_insight(
            InsightKind::ScreeningDue,
            InsightSeverity::Info,
            "screening_colorectal",
        );
        let alerts = bridge_insights_to_alerts(&[insight], chrono::Local::now().date_naive());
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].alert_type, AlertType::Screening);
        assert_eq!(alerts[0].severity, AlertSeverity::Info);
    }

    #[test]
    fn trend_insight_bridges_to_alert() {
        let insight = make_insight(
            InsightKind::AbnormalTrend,
            InsightSeverity::Critical,
            "bp_rising_trend",
        );
        let alerts = bridge_insights_to_alerts(&[insight], chrono::Local::now().date_naive());
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].alert_type, AlertType::Trend);
        assert_eq!(alerts[0].severity, AlertSeverity::Critical);
    }

    #[test]
    fn classification_insight_skipped() {
        let insight = make_insight(
            InsightKind::Classification,
            InsightSeverity::Warning,
            "bp_grade_1_htn",
        );
        let alerts = bridge_insights_to_alerts(&[insight], chrono::Local::now().date_naive());
        assert!(alerts.is_empty());
    }

    #[test]
    fn cross_reactivity_insight_skipped() {
        let insight = make_insight(
            InsightKind::CrossReactivity,
            InsightSeverity::Warning,
            "penicillin_cephalosporin",
        );
        let alerts = bridge_insights_to_alerts(&[insight], chrono::Local::now().date_naive());
        assert!(alerts.is_empty());
    }

    #[test]
    fn mixed_insights_filters_correctly() {
        let insights = vec![
            make_insight(InsightKind::Classification, InsightSeverity::Info, "normal_bp"),
            make_insight(InsightKind::Interaction, InsightSeverity::Warning, "drug_a_b"),
            make_insight(InsightKind::CrossReactivity, InsightSeverity::Warning, "allergy_x"),
            make_insight(InsightKind::MissingMonitoring, InsightSeverity::Info, "missing_lab"),
            make_insight(InsightKind::ScreeningDue, InsightSeverity::Info, "screening_x"),
            make_insight(InsightKind::AbnormalTrend, InsightSeverity::Warning, "trend_bp"),
        ];
        let alerts = bridge_insights_to_alerts(&insights, chrono::Local::now().date_naive());
        // Only 4 should bridge: Interaction, MissingMonitoring, ScreeningDue, AbnormalTrend
        assert_eq!(alerts.len(), 4);
    }

    #[test]
    fn severity_mapping() {
        assert_eq!(map_severity(&InsightSeverity::Critical), AlertSeverity::Critical);
        assert_eq!(map_severity(&InsightSeverity::Warning), AlertSeverity::Standard);
        assert_eq!(map_severity(&InsightSeverity::Info), AlertSeverity::Info);
    }

    #[test]
    fn bridged_alert_has_entity_ids() {
        let entity_id = Uuid::new_v4();
        let mut insight = make_insight(
            InsightKind::Interaction,
            InsightSeverity::Warning,
            "test_interaction",
        );
        insight.related_entities = vec![entity_id];
        let alerts = bridge_insights_to_alerts(&[insight], chrono::Local::now().date_naive());
        assert_eq!(alerts[0].entity_ids, vec![entity_id]);
    }

    #[test]
    fn bridged_alert_preserves_source() {
        let insight = make_insight(
            InsightKind::MissingMonitoring,
            InsightSeverity::Info,
            "test_monitoring",
        );
        let alerts = bridge_insights_to_alerts(&[insight], chrono::Local::now().date_naive());
        match &alerts[0].detail {
            AlertDetail::Monitoring(d) => assert_eq!(d.source, "Test Source 2024"),
            _ => panic!("Expected Monitoring detail"),
        }
    }
}
