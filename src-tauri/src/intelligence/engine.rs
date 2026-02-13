use std::path::{Path, PathBuf};
use std::time::Instant;

use rusqlite::Connection;
use uuid::Uuid;

use crate::db::sqlite;
use crate::models::enums::{AlertType, DismissedBy};

use super::detection::{
    detect_allergy_conflicts, detect_conflicts, detect_critical_labs,
    detect_daily_dose_accumulation, detect_dose_issues, detect_drift, detect_duplicates,
    detect_gaps, detect_temporal,
};
use super::emergency::EmergencyProtocol;
use super::reference::CoherenceReferenceData;
use super::store::AlertStore;
use super::types::{
    AlertCounts, CoherenceAlert, CoherenceEngine, CoherenceError, CoherenceResult,
    RepositorySnapshot,
};

/// Default implementation of the coherence engine.
/// Orchestrates all 8 detection algorithms, stores results, and manages the alert lifecycle.
///
/// When `db_path` is set, alerts are persisted to SQLite so they survive app restarts.
/// When `db_path` is None (tests), the engine operates purely in-memory.
pub struct DefaultCoherenceEngine {
    pub(crate) store: AlertStore,
    pub(crate) reference: CoherenceReferenceData,
    db_path: Option<PathBuf>,
}

impl DefaultCoherenceEngine {
    /// Create an in-memory-only engine (for tests).
    pub fn new(reference: CoherenceReferenceData) -> Self {
        Self {
            store: AlertStore::new(),
            reference,
            db_path: None,
        }
    }

    /// Create an engine backed by SQLite persistence.
    ///
    /// Loads active alerts from the database on construction, and persists
    /// all alert store/dismiss operations to the database going forward.
    pub fn with_db(
        reference: CoherenceReferenceData,
        conn: &Connection,
        db_path: &Path,
    ) -> Result<Self, CoherenceError> {
        let store = AlertStore::load_from_db(conn)?;
        Ok(Self {
            store,
            reference,
            db_path: Some(db_path.to_path_buf()),
        })
    }

    /// Open a DB connection if db_path is configured.
    fn open_conn(&self) -> Option<Connection> {
        self.db_path.as_ref().and_then(|path| {
            sqlite::open_database(path)
                .map_err(|e| {
                    tracing::warn!(error = %e, "Failed to open DB for alert persistence");
                    e
                })
                .ok()
        })
    }

    /// Run all 8 detection algorithms and collect alerts.
    fn run_detections(
        &self,
        document_id: &Uuid,
        data: &RepositorySnapshot,
    ) -> (Vec<CoherenceAlert>, AlertCounts) {
        let conflicts = detect_conflicts(document_id, data, &self.reference);
        let duplicates = detect_duplicates(document_id, data, &self.reference);
        let gaps = detect_gaps(document_id, data);
        let drifts = detect_drift(document_id, data, &self.reference);
        let temporals = detect_temporal(document_id, data);
        let allergies = detect_allergy_conflicts(document_id, data, &self.reference);
        let mut doses = detect_dose_issues(document_id, data, &self.reference);
        doses.extend(detect_daily_dose_accumulation(document_id, data, &self.reference));
        let criticals = detect_critical_labs(document_id, data);

        let counts = AlertCounts {
            conflicts: conflicts.len(),
            duplicates: duplicates.len(),
            gaps: gaps.len(),
            drifts: drifts.len(),
            temporals: temporals.len(),
            allergies: allergies.len(),
            doses: doses.len(),
            criticals: criticals.len(),
        };

        let all_alerts = conflicts
            .into_iter()
            .chain(duplicates)
            .chain(gaps)
            .chain(drifts)
            .chain(temporals)
            .chain(allergies)
            .chain(doses)
            .chain(criticals)
            .collect();

        (all_alerts, counts)
    }

    /// Store alerts, filtering out dismissed and duplicates.
    /// Returns only the newly stored alerts.
    /// If a DB path is configured, persists each new alert to SQLite.
    fn store_new_alerts(
        &self,
        alerts: Vec<CoherenceAlert>,
        data: &RepositorySnapshot,
    ) -> Result<Vec<CoherenceAlert>, CoherenceError> {
        let mut stored = Vec::new();
        let conn = self.open_conn();

        for alert in alerts {
            let is_dismissed =
                data.is_dismissed(alert.alert_type.as_str(), &alert.entity_ids);
            let was_stored =
                self.store
                    .store_alert_with_db(alert.clone(), is_dismissed, conn.as_ref())?;
            if was_stored {
                stored.push(alert);
            }
        }

        Ok(stored)
    }

    /// Get emergency actions for any CRITICAL alerts in the result set.
    pub fn get_emergency_actions(
        alerts: &[CoherenceAlert],
    ) -> Vec<super::emergency::EmergencyAction> {
        EmergencyProtocol::process_critical_alerts(alerts)
    }
}

impl CoherenceEngine for DefaultCoherenceEngine {
    fn analyze_new_document(
        &self,
        document_id: &Uuid,
        data: &RepositorySnapshot,
    ) -> Result<CoherenceResult, CoherenceError> {
        let start = Instant::now();

        let (all_alerts, counts) = self.run_detections(document_id, data);
        let new_alerts = self.store_new_alerts(all_alerts, data)?;

        let processing_time_ms = start.elapsed().as_millis() as u64;

        tracing::info!(
            document_id = %document_id,
            total = counts.total(),
            stored = new_alerts.len(),
            processing_ms = processing_time_ms,
            "Coherence analysis complete for document"
        );

        Ok(CoherenceResult {
            new_alerts,
            counts,
            processing_time_ms,
        })
    }

    fn analyze_full(
        &self,
        data: &RepositorySnapshot,
    ) -> Result<CoherenceResult, CoherenceError> {
        let start = Instant::now();
        let nil = Uuid::nil();

        let (all_alerts, counts) = self.run_detections(&nil, data);
        let new_alerts = self.store_new_alerts(all_alerts, data)?;

        let processing_time_ms = start.elapsed().as_millis() as u64;

        tracing::info!(
            total = counts.total(),
            stored = new_alerts.len(),
            processing_ms = processing_time_ms,
            "Full coherence analysis complete"
        );

        Ok(CoherenceResult {
            new_alerts,
            counts,
            processing_time_ms,
        })
    }

    fn get_active_alerts(
        &self,
        alert_type: Option<&AlertType>,
    ) -> Result<Vec<CoherenceAlert>, CoherenceError> {
        self.store.get_active(alert_type)
    }

    fn get_relevant_alerts(
        &self,
        entity_ids: &[Uuid],
        keywords: &[String],
    ) -> Result<Vec<CoherenceAlert>, CoherenceError> {
        self.store.get_relevant(entity_ids, keywords)
    }

    fn get_critical_alerts(&self) -> Result<Vec<CoherenceAlert>, CoherenceError> {
        self.store.get_critical()
    }

    fn dismiss_alert(
        &self,
        alert_id: &Uuid,
        reason: &str,
        dismissed_by: DismissedBy,
    ) -> Result<(), CoherenceError> {
        let conn = self.open_conn();
        self.store
            .dismiss_with_db(alert_id, reason, dismissed_by, conn.as_ref())
    }

    fn dismiss_critical_alert(
        &self,
        alert_id: &Uuid,
        reason: &str,
        two_step_confirmed: bool,
    ) -> Result<(), CoherenceError> {
        let conn = self.open_conn();
        self.store
            .dismiss_critical_with_db(alert_id, reason, two_step_confirmed, conn.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use chrono::NaiveDate;

    use super::*;
    use crate::intelligence::types::AlertSeverity;
    use crate::models::enums::*;
    use crate::models::*;

    fn empty_snapshot() -> RepositorySnapshot {
        RepositorySnapshot {
            medications: vec![],
            diagnoses: vec![],
            lab_results: vec![],
            allergies: vec![],
            symptoms: vec![],
            procedures: vec![],
            professionals: vec![],
            dose_changes: vec![],
            compound_ingredients: vec![],
            dismissed_alert_keys: HashSet::new(),
        }
    }

    fn make_medication(
        id: Uuid,
        generic: &str,
        brand: Option<&str>,
        dose: &str,
        freq: &str,
        prescriber: Option<Uuid>,
        doc_id: Uuid,
    ) -> Medication {
        Medication {
            id,
            generic_name: generic.into(),
            brand_name: brand.map(|s| s.into()),
            dose: dose.into(),
            frequency: freq.into(),
            frequency_type: FrequencyType::Scheduled,
            route: "oral".into(),
            prescriber_id: prescriber,
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
            document_id: doc_id,
        }
    }

    /// Engine detects conflict + critical + allergy on the same document.
    #[test]
    fn engine_full_analysis_multi_detection() {
        let ref_data = CoherenceReferenceData::load_test();
        let engine = DefaultCoherenceEngine::new(ref_data);

        let doc1 = Uuid::new_v4();
        let doc2 = Uuid::new_v4();
        let dr_a = Uuid::new_v4();
        let dr_b = Uuid::new_v4();

        let mut data = empty_snapshot();
        data.medications = vec![
            make_medication(
                Uuid::new_v4(),
                "Metformin",
                Some("Glucophage"),
                "500mg",
                "twice daily",
                Some(dr_a),
                doc1,
            ),
            make_medication(
                Uuid::new_v4(),
                "Metformin",
                Some("Metformin"),
                "1000mg",
                "twice daily",
                Some(dr_b),
                doc2,
            ),
            make_medication(
                Uuid::new_v4(),
                "amoxicillin",
                None,
                "500mg",
                "three times daily",
                Some(dr_b),
                doc2,
            ),
        ];
        data.allergies = vec![Allergy {
            id: Uuid::new_v4(),
            allergen: "penicillin".into(),
            reaction: Some("anaphylaxis".into()),
            severity: AllergySeverity::Severe,
            date_identified: None,
            source: AllergySource::DocumentExtracted,
            document_id: Some(Uuid::new_v4()),
            verified: true,
        }];
        data.lab_results = vec![LabResult {
            id: Uuid::new_v4(),
            test_name: "Potassium".into(),
            test_code: None,
            value: Some(6.5),
            value_text: None,
            unit: Some("mEq/L".into()),
            reference_range_low: Some(3.5),
            reference_range_high: Some(5.0),
            abnormal_flag: AbnormalFlag::CriticalHigh,
            collection_date: NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
            lab_facility: None,
            ordering_physician_id: None,
            document_id: doc2,
        }];

        let result = engine.analyze_new_document(&doc2, &data).unwrap();

        assert!(
            result.counts.conflicts >= 1,
            "Expected conflict: got {}",
            result.counts.conflicts
        );
        assert!(
            result.counts.allergies >= 1,
            "Expected allergy: got {}",
            result.counts.allergies
        );
        assert!(
            result.counts.criticals >= 1,
            "Expected critical: got {}",
            result.counts.criticals
        );

        // All critical/allergy alerts have Critical severity
        for alert in &result.new_alerts {
            if alert.alert_type == AlertType::Critical || alert.alert_type == AlertType::Allergy {
                assert_eq!(alert.severity, AlertSeverity::Critical);
            }
        }
    }

    /// Engine does not store already-dismissed alerts.
    #[test]
    fn engine_skips_dismissed_alerts() {
        let ref_data = CoherenceReferenceData::load_test();
        let engine = DefaultCoherenceEngine::new(ref_data);

        let doc1 = Uuid::new_v4();
        let doc2 = Uuid::new_v4();
        let dr_a = Uuid::new_v4();
        let dr_b = Uuid::new_v4();

        let med1 = Uuid::new_v4();
        let med2 = Uuid::new_v4();

        let mut data = empty_snapshot();
        data.medications = vec![
            make_medication(med1, "Metformin", None, "500mg", "twice daily", Some(dr_a), doc1),
            make_medication(med2, "Metformin", None, "1000mg", "twice daily", Some(dr_b), doc2),
        ];

        // Mark this conflict pair as dismissed
        let mut sorted_ids = vec![med1, med2];
        sorted_ids.sort();
        let key_json = serde_json::to_string(&sorted_ids).unwrap();
        data.dismissed_alert_keys
            .insert(("conflict".to_string(), key_json));

        let result = engine.analyze_new_document(&doc2, &data).unwrap();

        // The conflict was detected but not stored because it was dismissed
        let stored_conflicts: Vec<_> = result
            .new_alerts
            .iter()
            .filter(|a| a.alert_type == AlertType::Conflict)
            .collect();
        assert!(
            stored_conflicts.is_empty(),
            "Dismissed conflicts should not be stored"
        );
    }

    /// Engine full analysis uses Uuid::nil as document_id.
    #[test]
    fn engine_full_analysis_detects_gaps() {
        let ref_data = CoherenceReferenceData::load_test();
        let engine = DefaultCoherenceEngine::new(ref_data);

        let doc = Uuid::new_v4();
        let mut data = empty_snapshot();
        data.diagnoses = vec![Diagnosis {
            id: Uuid::new_v4(),
            name: "Type 2 Diabetes".into(),
            icd_code: None,
            date_diagnosed: None,
            diagnosing_professional_id: None,
            status: DiagnosisStatus::Active,
            document_id: doc,
        }];

        let result = engine.analyze_full(&data).unwrap();
        assert!(
            result.counts.gaps >= 1,
            "Full analysis should detect gaps: got {}",
            result.counts.gaps
        );
    }

    /// Emergency actions generated for critical alerts.
    #[test]
    fn engine_emergency_actions_for_critical() {
        let ref_data = CoherenceReferenceData::load_test();
        let engine = DefaultCoherenceEngine::new(ref_data);

        let doc = Uuid::new_v4();
        let mut data = empty_snapshot();
        data.lab_results = vec![LabResult {
            id: Uuid::new_v4(),
            test_name: "Potassium".into(),
            test_code: None,
            value: Some(6.5),
            value_text: None,
            unit: Some("mEq/L".into()),
            reference_range_low: Some(3.5),
            reference_range_high: Some(5.0),
            abnormal_flag: AbnormalFlag::CriticalHigh,
            collection_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            lab_facility: None,
            ordering_physician_id: None,
            document_id: doc,
        }];

        let result = engine.analyze_new_document(&doc, &data).unwrap();
        let actions = DefaultCoherenceEngine::get_emergency_actions(&result.new_alerts);

        assert!(!actions.is_empty(), "Critical labs should generate emergency actions");
        assert_eq!(actions[0].dismissal_steps, 2);
        assert!(actions[0].appointment_priority);
    }

    /// Dismiss standard alert through engine.
    #[test]
    fn engine_dismiss_standard_alert() {
        let ref_data = CoherenceReferenceData::load_test();
        let engine = DefaultCoherenceEngine::new(ref_data);

        let doc1 = Uuid::new_v4();
        let doc2 = Uuid::new_v4();
        let dr_a = Uuid::new_v4();
        let dr_b = Uuid::new_v4();

        let mut data = empty_snapshot();
        data.medications = vec![
            make_medication(
                Uuid::new_v4(), "Metformin", None, "500mg", "twice daily", Some(dr_a), doc1,
            ),
            make_medication(
                Uuid::new_v4(), "Metformin", None, "1000mg", "twice daily", Some(dr_b), doc2,
            ),
        ];

        let result = engine.analyze_new_document(&doc2, &data).unwrap();
        assert!(!result.new_alerts.is_empty());

        let alert_id = result.new_alerts[0].id;
        engine
            .dismiss_alert(&alert_id, "Doctor confirmed", DismissedBy::Patient)
            .unwrap();

        let active = engine.get_active_alerts(None).unwrap();
        assert!(
            !active.iter().any(|a| a.id == alert_id),
            "Dismissed alert should not appear in active list"
        );
    }

    /// Processing time is recorded.
    #[test]
    fn engine_records_processing_time() {
        let ref_data = CoherenceReferenceData::load_test();
        let engine = DefaultCoherenceEngine::new(ref_data);

        let data = empty_snapshot();
        let result = engine.analyze_full(&data).unwrap();

        // Processing time should be non-negative (and very fast for empty data)
        assert!(result.processing_time_ms < 1000);
    }

    // === DB persistence tests (RS-L2-03-001) ===

    fn make_db_engine(
        db_path: &std::path::Path,
    ) -> (DefaultCoherenceEngine, rusqlite::Connection) {
        let conn = crate::db::sqlite::open_database(db_path).unwrap();
        let ref_data = CoherenceReferenceData::load_test();
        let engine = DefaultCoherenceEngine::with_db(ref_data, &conn, db_path).unwrap();
        (engine, conn)
    }

    fn make_conflict_snapshot() -> (RepositorySnapshot, Uuid) {
        let doc1 = Uuid::new_v4();
        let doc2 = Uuid::new_v4();
        let dr_a = Uuid::new_v4();
        let dr_b = Uuid::new_v4();

        let mut data = empty_snapshot();
        data.medications = vec![
            make_medication(
                Uuid::new_v4(),
                "Metformin",
                None,
                "500mg",
                "twice daily",
                Some(dr_a),
                doc1,
            ),
            make_medication(
                Uuid::new_v4(),
                "Metformin",
                None,
                "1000mg",
                "twice daily",
                Some(dr_b),
                doc2,
            ),
        ];
        (data, doc2)
    }

    /// Engine with DB persists alerts detected during analysis.
    #[test]
    fn engine_db_persists_analyzed_alerts() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let (engine, _conn) = make_db_engine(&db_path);
        let (data, doc2) = make_conflict_snapshot();

        let result = engine.analyze_new_document(&doc2, &data).unwrap();
        assert!(!result.new_alerts.is_empty(), "Should detect conflicts");

        // Drop engine and reload from DB — alerts should survive
        drop(engine);
        let ref_data2 = CoherenceReferenceData::load_test();
        let conn2 = crate::db::sqlite::open_database(&db_path).unwrap();
        let engine2 = DefaultCoherenceEngine::with_db(ref_data2, &conn2, &db_path).unwrap();

        let active = engine2.get_active_alerts(None).unwrap();
        assert_eq!(
            active.len(),
            result.new_alerts.len(),
            "Reloaded engine should have the same alerts"
        );
    }

    /// Engine with DB persists dismissal so reloaded engine does not see the alert.
    #[test]
    fn engine_db_persists_dismissal() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let (engine, _conn) = make_db_engine(&db_path);
        let (data, doc2) = make_conflict_snapshot();

        let result = engine.analyze_new_document(&doc2, &data).unwrap();
        let alert_id = result.new_alerts[0].id;

        engine
            .dismiss_alert(&alert_id, "Doctor confirmed", DismissedBy::Patient)
            .unwrap();

        // Reload — dismissed alert should not appear
        drop(engine);
        let ref_data2 = CoherenceReferenceData::load_test();
        let conn2 = crate::db::sqlite::open_database(&db_path).unwrap();
        let engine2 = DefaultCoherenceEngine::with_db(ref_data2, &conn2, &db_path).unwrap();

        let active = engine2.get_active_alerts(None).unwrap();
        assert!(
            !active.iter().any(|a| a.id == alert_id),
            "Dismissed alert should not survive reload"
        );
    }

    /// Engine with DB: critical dismissal persists.
    #[test]
    fn engine_db_persists_critical_dismissal() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let (engine, _conn) = make_db_engine(&db_path);

        let doc = Uuid::new_v4();
        let mut data = empty_snapshot();
        data.lab_results = vec![LabResult {
            id: Uuid::new_v4(),
            test_name: "Potassium".into(),
            test_code: None,
            value: Some(6.5),
            value_text: None,
            unit: Some("mEq/L".into()),
            reference_range_low: Some(3.5),
            reference_range_high: Some(5.0),
            abnormal_flag: AbnormalFlag::CriticalHigh,
            collection_date: NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
            lab_facility: None,
            ordering_physician_id: None,
            document_id: doc,
        }];

        let result = engine.analyze_new_document(&doc, &data).unwrap();
        let critical_alerts: Vec<_> = result
            .new_alerts
            .iter()
            .filter(|a| a.severity == AlertSeverity::Critical)
            .collect();
        assert!(!critical_alerts.is_empty(), "Should detect critical labs");

        let alert_id = critical_alerts[0].id;
        engine
            .dismiss_critical_alert(&alert_id, "Emergency addressed", true)
            .unwrap();

        // Reload — dismissed critical should not appear
        drop(engine);
        let ref_data2 = CoherenceReferenceData::load_test();
        let conn2 = crate::db::sqlite::open_database(&db_path).unwrap();
        let engine2 = DefaultCoherenceEngine::with_db(ref_data2, &conn2, &db_path).unwrap();

        let critical = engine2.get_critical_alerts().unwrap();
        assert!(critical.is_empty(), "Dismissed critical should not survive reload");
    }

    /// Engine without DB still works (backward compat).
    #[test]
    fn engine_without_db_still_works() {
        let ref_data = CoherenceReferenceData::load_test();
        let engine = DefaultCoherenceEngine::new(ref_data);
        let (data, doc2) = make_conflict_snapshot();

        let result = engine.analyze_new_document(&doc2, &data).unwrap();
        assert!(!result.new_alerts.is_empty());

        let alert_id = result.new_alerts[0].id;
        engine
            .dismiss_alert(&alert_id, "reason", DismissedBy::Patient)
            .unwrap();
        let active = engine.get_active_alerts(None).unwrap();
        assert!(!active.iter().any(|a| a.id == alert_id));
    }
}
