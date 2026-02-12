use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::types::{AlertDetail, AlertSeverity, CoherenceAlert};

/// Emergency protocol handler for CRITICAL alerts.
pub struct EmergencyProtocol;

impl EmergencyProtocol {
    /// Process critical alerts detected during ingestion.
    /// Returns actions that need immediate surfacing.
    pub fn process_critical_alerts(alerts: &[CoherenceAlert]) -> Vec<EmergencyAction> {
        alerts
            .iter()
            .filter(|a| a.severity == AlertSeverity::Critical)
            .map(|alert| match &alert.detail {
                AlertDetail::Critical(_) => EmergencyAction {
                    alert_id: alert.id,
                    action_type: EmergencyActionType::LabCritical,
                    ingestion_message:
                        "This result is marked as requiring attention on your lab report."
                            .to_string(),
                    home_banner: alert.patient_message.clone(),
                    appointment_priority: true,
                    dismissal_steps: 2,
                    dismissal_prompt_1: "Has your doctor addressed this?".to_string(),
                    dismissal_prompt_2: "Yes, my doctor has seen this result".to_string(),
                },
                AlertDetail::Allergy(_) => EmergencyAction {
                    alert_id: alert.id,
                    action_type: EmergencyActionType::AllergyMatch,
                    ingestion_message:
                        "This medication may contain an ingredient related to a known allergy in your records."
                            .to_string(),
                    home_banner: alert.patient_message.clone(),
                    appointment_priority: true,
                    dismissal_steps: 2,
                    dismissal_prompt_1:
                        "Has your doctor or pharmacist addressed this?".to_string(),
                    dismissal_prompt_2:
                        "Yes, this has been reviewed by my healthcare provider".to_string(),
                },
                _ => EmergencyAction {
                    alert_id: alert.id,
                    action_type: EmergencyActionType::Other,
                    ingestion_message: alert.patient_message.clone(),
                    home_banner: alert.patient_message.clone(),
                    appointment_priority: true,
                    dismissal_steps: 2,
                    dismissal_prompt_1: "Has your doctor addressed this?".to_string(),
                    dismissal_prompt_2: "Yes, my doctor has reviewed this".to_string(),
                },
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyAction {
    pub alert_id: Uuid,
    pub action_type: EmergencyActionType,
    /// Message shown during ingestion review (L3-04).
    pub ingestion_message: String,
    /// Banner shown on Home/Chat screens (L3-02).
    pub home_banner: String,
    /// Whether to add as priority item in appointment prep (L4-02).
    pub appointment_priority: bool,
    /// Number of dismissal steps required (always 2 for CRITICAL).
    pub dismissal_steps: u8,
    /// Step 1 prompt.
    pub dismissal_prompt_1: String,
    /// Step 2 prompt: confirmation text.
    pub dismissal_prompt_2: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EmergencyActionType {
    LabCritical,
    AllergyMatch,
    Other,
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;
    use crate::intelligence::types::{AlertDetail, CriticalDetail};
    use crate::models::enums::AlertType;

    fn make_critical_lab_alert() -> CoherenceAlert {
        CoherenceAlert {
            id: Uuid::new_v4(),
            alert_type: AlertType::Critical,
            severity: AlertSeverity::Critical,
            entity_ids: vec![Uuid::new_v4()],
            source_document_ids: vec![Uuid::new_v4()],
            patient_message: "Your lab report flags Potassium as needing prompt attention."
                .into(),
            detail: AlertDetail::Critical(CriticalDetail {
                test_name: "Potassium".into(),
                lab_result_id: Uuid::new_v4(),
                value: 6.5,
                unit: "mEq/L".into(),
                abnormal_flag: "critical_high".into(),
                reference_range_low: Some(3.5),
                reference_range_high: Some(5.0),
                collection_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
                document_id: Uuid::new_v4(),
            }),
            detected_at: chrono::Local::now().naive_local(),
            surfaced: false,
            dismissed: false,
            dismissal: None,
        }
    }

    /// T-27: Emergency protocol generates correct action for critical lab.
    #[test]
    fn emergency_protocol_critical_lab() {
        let alert = make_critical_lab_alert();
        let actions = EmergencyProtocol::process_critical_alerts(&[alert]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_type, EmergencyActionType::LabCritical);
        assert_eq!(actions[0].dismissal_steps, 2);
        assert!(actions[0].appointment_priority);
        assert!(!actions[0].home_banner.is_empty());
    }

    #[test]
    fn emergency_protocol_skips_non_critical() {
        let mut alert = make_critical_lab_alert();
        alert.severity = AlertSeverity::Standard;
        let actions = EmergencyProtocol::process_critical_alerts(&[alert]);
        assert!(actions.is_empty());
    }
}
