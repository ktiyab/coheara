use uuid::Uuid;

use crate::models::enums::DismissedBy;

use super::helpers::entities_match;
use super::types::{
    AlertDismissal, AlertSeverity, CoherenceAlert, CoherenceError,
};

/// In-memory alert store backed by RwLock.
/// Persistence to SQLite is handled by the engine when dismissing.
pub struct AlertStore {
    pub(crate) active: std::sync::RwLock<Vec<CoherenceAlert>>,
}

impl AlertStore {
    pub fn new() -> Self {
        Self {
            active: std::sync::RwLock::new(Vec::new()),
        }
    }

    /// Store a new alert if not already active for this entity pair + type.
    /// Returns true if the alert was stored, false if duplicate or dismissed.
    pub fn store_alert(
        &self,
        alert: CoherenceAlert,
        is_dismissed: bool,
    ) -> Result<bool, CoherenceError> {
        if is_dismissed {
            tracing::debug!(
                alert_type = alert.alert_type.as_str(),
                "Alert already dismissed for this entity pair, skipping"
            );
            return Ok(false);
        }

        let mut active = self.active.write().map_err(|_| CoherenceError::LockFailed)?;

        let already_active = active.iter().any(|existing| {
            existing.alert_type == alert.alert_type
                && entities_match(&existing.entity_ids, &alert.entity_ids)
        });

        if already_active {
            return Ok(false);
        }

        active.push(alert);
        Ok(true)
    }

    /// Get all active alerts, optionally filtered.
    pub fn get_active(
        &self,
        alert_type: Option<&crate::models::enums::AlertType>,
    ) -> Result<Vec<CoherenceAlert>, CoherenceError> {
        let active = self.active.read().map_err(|_| CoherenceError::LockFailed)?;

        let result = match alert_type {
            Some(t) => active
                .iter()
                .filter(|a| &a.alert_type == t && !a.dismissed)
                .cloned()
                .collect(),
            None => active
                .iter()
                .filter(|a| !a.dismissed)
                .cloned()
                .collect(),
        };

        Ok(result)
    }

    /// Get alerts relevant to specific entities or keywords.
    pub fn get_relevant(
        &self,
        entity_ids: &[Uuid],
        keywords: &[String],
    ) -> Result<Vec<CoherenceAlert>, CoherenceError> {
        let active = self.active.read().map_err(|_| CoherenceError::LockFailed)?;

        let results = active
            .iter()
            .filter(|alert| {
                if alert.dismissed {
                    return false;
                }

                let entity_match = alert.entity_ids.iter().any(|id| entity_ids.contains(id));

                let keyword_match = keywords.iter().any(|kw| {
                    alert
                        .patient_message
                        .to_lowercase()
                        .contains(&kw.to_lowercase())
                });

                entity_match || keyword_match
            })
            .cloned()
            .collect();

        Ok(results)
    }

    /// Get all CRITICAL non-dismissed alerts.
    pub fn get_critical(&self) -> Result<Vec<CoherenceAlert>, CoherenceError> {
        let active = self.active.read().map_err(|_| CoherenceError::LockFailed)?;

        Ok(active
            .iter()
            .filter(|a| a.severity == AlertSeverity::Critical && !a.dismissed)
            .cloned()
            .collect())
    }

    /// Dismiss a standard alert. Returns error if alert is CRITICAL.
    pub fn dismiss(
        &self,
        alert_id: &Uuid,
        reason: &str,
        dismissed_by: DismissedBy,
    ) -> Result<(), CoherenceError> {
        let mut active = self
            .active
            .write()
            .map_err(|_| CoherenceError::LockFailed)?;

        let alert = active
            .iter_mut()
            .find(|a| a.id == *alert_id)
            .ok_or(CoherenceError::AlertNotFound(*alert_id))?;

        if alert.severity == AlertSeverity::Critical {
            return Err(CoherenceError::CriticalRequiresTwoStep(*alert_id));
        }

        alert.dismissed = true;
        alert.dismissal = Some(AlertDismissal {
            dismissed_date: chrono::Local::now().naive_local(),
            reason: reason.to_string(),
            dismissed_by,
            two_step_confirmed: false,
        });

        Ok(())
    }

    /// Dismiss a CRITICAL alert (requires 2-step confirmation).
    pub fn dismiss_critical(
        &self,
        alert_id: &Uuid,
        reason: &str,
        two_step_confirmed: bool,
    ) -> Result<(), CoherenceError> {
        if !two_step_confirmed {
            return Err(CoherenceError::TwoStepNotConfirmed(*alert_id));
        }

        let mut active = self
            .active
            .write()
            .map_err(|_| CoherenceError::LockFailed)?;

        let alert = active
            .iter_mut()
            .find(|a| a.id == *alert_id)
            .ok_or(CoherenceError::AlertNotFound(*alert_id))?;

        if alert.severity != AlertSeverity::Critical {
            return Err(CoherenceError::NotCriticalAlert(*alert_id));
        }

        alert.dismissed = true;
        alert.dismissal = Some(AlertDismissal {
            dismissed_date: chrono::Local::now().naive_local(),
            reason: reason.to_string(),
            dismissed_by: DismissedBy::Patient,
            two_step_confirmed: true,
        });

        Ok(())
    }
}

impl Default for AlertStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;
    use crate::intelligence::types::{AlertDetail, CriticalDetail};
    use crate::models::enums::AlertType;

    fn make_standard_alert() -> CoherenceAlert {
        CoherenceAlert {
            id: Uuid::new_v4(),
            alert_type: AlertType::Conflict,
            severity: AlertSeverity::Standard,
            entity_ids: vec![Uuid::new_v4(), Uuid::new_v4()],
            source_document_ids: vec![Uuid::new_v4()],
            patient_message: "Test standard alert".into(),
            detail: AlertDetail::Conflict(crate::intelligence::types::ConflictDetail {
                medication_name: "Metformin".into(),
                prescriber_a: crate::intelligence::types::PrescriberRef {
                    professional_id: Uuid::nil(),
                    name: "Dr. A".into(),
                    document_id: Uuid::new_v4(),
                    document_date: None,
                },
                prescriber_b: crate::intelligence::types::PrescriberRef {
                    professional_id: Uuid::nil(),
                    name: "Dr. B".into(),
                    document_id: Uuid::new_v4(),
                    document_date: None,
                },
                field_conflicted: "dose".into(),
                value_a: "500mg".into(),
                value_b: "1000mg".into(),
            }),
            detected_at: chrono::Local::now().naive_local(),
            surfaced: false,
            dismissed: false,
            dismissal: None,
        }
    }

    fn make_critical_alert() -> CoherenceAlert {
        CoherenceAlert {
            id: Uuid::new_v4(),
            alert_type: AlertType::Critical,
            severity: AlertSeverity::Critical,
            entity_ids: vec![Uuid::new_v4()],
            source_document_ids: vec![Uuid::new_v4()],
            patient_message: "Test critical alert".into(),
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

    /// T-26: Standard alert dismiss (single step).
    #[test]
    fn dismiss_standard_alert() {
        let store = AlertStore::new();
        let alert = make_standard_alert();
        let alert_id = alert.id;

        store.store_alert(alert, false).unwrap();
        store
            .dismiss(&alert_id, "Doctor addressed it", DismissedBy::Patient)
            .unwrap();

        let active = store.get_active(None).unwrap();
        assert!(active.is_empty());
    }

    /// T-24: CRITICAL alert dismiss without 2-step should fail.
    #[test]
    fn critical_alert_dismiss_without_two_step_fails() {
        let store = AlertStore::new();
        let alert = make_critical_alert();
        let alert_id = alert.id;

        store.store_alert(alert, false).unwrap();

        let result = store.dismiss_critical(&alert_id, "reason", false);
        assert!(result.is_err());
        match result.unwrap_err() {
            CoherenceError::TwoStepNotConfirmed(id) => assert_eq!(id, alert_id),
            other => panic!("Expected TwoStepNotConfirmed, got: {:?}", other),
        }
    }

    /// T-25: CRITICAL alert dismiss with 2-step confirmed.
    #[test]
    fn critical_alert_dismiss_with_two_step() {
        let store = AlertStore::new();
        let alert = make_critical_alert();
        let alert_id = alert.id;

        store.store_alert(alert, false).unwrap();
        store
            .dismiss_critical(&alert_id, "Doctor reviewed", true)
            .unwrap();

        let critical = store.get_critical().unwrap();
        assert!(critical.is_empty());
    }

    /// Standard dismiss on CRITICAL should fail.
    #[test]
    fn standard_dismiss_on_critical_fails() {
        let store = AlertStore::new();
        let alert = make_critical_alert();
        let alert_id = alert.id;

        store.store_alert(alert, false).unwrap();

        let result = store.dismiss(&alert_id, "reason", DismissedBy::Patient);
        assert!(result.is_err());
        match result.unwrap_err() {
            CoherenceError::CriticalRequiresTwoStep(_) => {}
            other => panic!("Expected CriticalRequiresTwoStep, got: {:?}", other),
        }
    }

    /// T-23: Dismissed alert not re-stored (duplicate detection).
    #[test]
    fn dismissed_alert_not_re_stored() {
        let store = AlertStore::new();
        let alert = make_standard_alert();
        let stored = store.store_alert(alert, true).unwrap();
        assert!(!stored, "Should not store already-dismissed alert");
    }

    /// Duplicate active alert not re-stored.
    #[test]
    fn duplicate_active_alert_not_stored() {
        let store = AlertStore::new();
        let alert1 = make_standard_alert();
        let mut alert2 = make_standard_alert();
        alert2.entity_ids = alert1.entity_ids.clone();
        alert2.alert_type = alert1.alert_type.clone();

        assert!(store.store_alert(alert1, false).unwrap());
        assert!(!store.store_alert(alert2, false).unwrap());
    }

    #[test]
    fn get_relevant_by_keyword() {
        let store = AlertStore::new();
        let mut alert = make_standard_alert();
        alert.patient_message = "Your Metformin dose differs between prescribers.".into();
        store.store_alert(alert, false).unwrap();

        let results = store
            .get_relevant(&[], &["Metformin".to_string()])
            .unwrap();
        assert_eq!(results.len(), 1);

        let results = store
            .get_relevant(&[], &["unknown_keyword".to_string()])
            .unwrap();
        assert!(results.is_empty());
    }
}
