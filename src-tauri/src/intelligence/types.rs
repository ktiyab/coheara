use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::db::DatabaseError;
use crate::models::enums::{AlertType, DismissedBy};
use crate::models::{
    Allergy, CompoundIngredient, Diagnosis, DoseChange, LabResult, Medication, Procedure,
    Professional, Symptom,
};

// ---------------------------------------------------------------------------
// AlertSeverity
// ---------------------------------------------------------------------------

/// Severity determines surfacing behavior.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertSeverity {
    /// Informational: logged, surfaced only when patient asks.
    Info,
    /// Standard: surfaced during relevant conversation or appointment prep.
    Standard,
    /// Critical: surfaced immediately, requires 2-step dismissal.
    Critical,
}

impl AlertSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Standard => "standard",
            Self::Critical => "critical",
        }
    }
}

// ---------------------------------------------------------------------------
// CoherenceAlert
// ---------------------------------------------------------------------------

/// A coherence observation detected by the engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceAlert {
    pub id: Uuid,
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub entity_ids: Vec<Uuid>,
    pub source_document_ids: Vec<Uuid>,
    /// Patient-facing message (calm, preparatory framing per NC-07).
    pub patient_message: String,
    pub detail: AlertDetail,
    pub detected_at: NaiveDateTime,
    pub surfaced: bool,
    pub dismissed: bool,
    pub dismissal: Option<AlertDismissal>,
}

// ---------------------------------------------------------------------------
// AlertDetail variants
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertDetail {
    Conflict(ConflictDetail),
    Duplicate(DuplicateDetail),
    Gap(GapDetail),
    Drift(DriftDetail),
    Temporal(TemporalDetail),
    Allergy(AllergyDetail),
    Dose(DoseDetail),
    Critical(CriticalDetail),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictDetail {
    pub medication_name: String,
    pub prescriber_a: PrescriberRef,
    pub prescriber_b: PrescriberRef,
    pub field_conflicted: String,
    pub value_a: String,
    pub value_b: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrescriberRef {
    pub professional_id: Uuid,
    pub name: String,
    pub document_id: Uuid,
    pub document_date: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateDetail {
    pub generic_name: String,
    pub brand_a: String,
    pub brand_b: String,
    pub medication_id_a: Uuid,
    pub medication_id_b: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapDetail {
    pub gap_type: GapType,
    pub entity_name: String,
    pub entity_id: Uuid,
    pub expected: String,
    pub document_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GapType {
    DiagnosisWithoutTreatment,
    MedicationWithoutDiagnosis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftDetail {
    pub entity_type: String,
    pub entity_name: String,
    pub old_value: String,
    pub new_value: String,
    pub change_date: Option<NaiveDate>,
    pub reason_documented: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalDetail {
    pub symptom_id: Uuid,
    pub symptom_name: String,
    pub symptom_onset: NaiveDate,
    pub correlated_event: TemporalEvent,
    pub days_between: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TemporalEvent {
    MedicationStarted {
        medication_id: Uuid,
        medication_name: String,
        start_date: NaiveDate,
    },
    DoseChanged {
        medication_id: Uuid,
        medication_name: String,
        old_dose: String,
        new_dose: String,
        change_date: NaiveDate,
    },
    ProcedurePerformed {
        procedure_id: Uuid,
        procedure_name: String,
        procedure_date: NaiveDate,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllergyDetail {
    pub allergen: String,
    pub allergy_severity: String,
    pub allergy_id: Uuid,
    pub medication_name: String,
    pub medication_id: Uuid,
    pub matching_ingredient: String,
    pub ingredient_maps_to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoseDetail {
    pub medication_name: String,
    pub medication_id: Uuid,
    pub extracted_dose: String,
    pub extracted_dose_mg: f64,
    pub typical_range_low_mg: f64,
    pub typical_range_high_mg: f64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalDetail {
    pub test_name: String,
    pub lab_result_id: Uuid,
    pub value: f64,
    pub unit: String,
    pub abnormal_flag: String,
    pub reference_range_low: Option<f64>,
    pub reference_range_high: Option<f64>,
    pub collection_date: NaiveDate,
    pub document_id: Uuid,
}

// ---------------------------------------------------------------------------
// Dismissal
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertDismissal {
    pub dismissed_date: NaiveDateTime,
    pub reason: String,
    pub dismissed_by: DismissedBy,
    pub two_step_confirmed: bool,
}

// ---------------------------------------------------------------------------
// CoherenceResult & AlertCounts
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceResult {
    pub new_alerts: Vec<CoherenceAlert>,
    pub counts: AlertCounts,
    pub processing_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlertCounts {
    pub conflicts: usize,
    pub duplicates: usize,
    pub gaps: usize,
    pub drifts: usize,
    pub temporals: usize,
    pub allergies: usize,
    pub doses: usize,
    pub criticals: usize,
}

impl AlertCounts {
    pub fn total(&self) -> usize {
        self.conflicts
            + self.duplicates
            + self.gaps
            + self.drifts
            + self.temporals
            + self.allergies
            + self.doses
            + self.criticals
    }
}

// ---------------------------------------------------------------------------
// RepositorySnapshot — pre-fetched data for coherence analysis
// ---------------------------------------------------------------------------

/// Pre-fetched data snapshot for coherence analysis.
/// The engine fetches all relevant data from the database, builds this
/// snapshot, and passes it to the detection functions. This keeps
/// detection logic pure and testable.
pub struct RepositorySnapshot {
    pub medications: Vec<Medication>,
    pub diagnoses: Vec<Diagnosis>,
    pub lab_results: Vec<LabResult>,
    pub allergies: Vec<Allergy>,
    pub symptoms: Vec<Symptom>,
    pub procedures: Vec<Procedure>,
    pub professionals: Vec<Professional>,
    pub dose_changes: Vec<DoseChange>,
    pub compound_ingredients: Vec<CompoundIngredient>,
    pub dismissed_alert_keys: std::collections::HashSet<(String, String)>,
}

impl RepositorySnapshot {
    /// Check whether an alert was already dismissed for this entity pair.
    pub fn is_dismissed(&self, alert_type: &str, entity_ids: &[Uuid]) -> bool {
        let mut sorted: Vec<Uuid> = entity_ids.to_vec();
        sorted.sort();
        let key_json = serde_json::to_string(&sorted).unwrap_or_default();
        self.dismissed_alert_keys
            .contains(&(alert_type.to_string(), key_json))
    }

    /// Look up a professional by ID.
    pub fn get_professional(&self, id: &Uuid) -> Option<&Professional> {
        self.professionals.iter().find(|p| p.id == *id)
    }

    /// Get dose changes for a specific medication.
    pub fn get_dose_history(&self, medication_id: &Uuid) -> Vec<&DoseChange> {
        self.dose_changes
            .iter()
            .filter(|dc| dc.medication_id == *medication_id)
            .collect()
    }

    /// Get compound ingredients for a specific medication.
    pub fn get_compound_ingredients(&self, medication_id: &Uuid) -> Vec<&CompoundIngredient> {
        self.compound_ingredients
            .iter()
            .filter(|ci| ci.medication_id == *medication_id)
            .collect()
    }

    /// Resolve prescriber name from professional_id.
    pub fn resolve_prescriber_name(&self, professional_id: Option<Uuid>) -> String {
        if let Some(id) = professional_id {
            if let Some(prof) = self.get_professional(&id) {
                return prof.name.clone();
            }
        }
        "a prescriber".to_string()
    }
}

// ---------------------------------------------------------------------------
// CoherenceError
// ---------------------------------------------------------------------------

#[derive(Error, Debug)]
pub enum CoherenceError {
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("Invalid alert type: {0}")]
    InvalidAlertType(String),

    #[error("Alert not found: {0}")]
    AlertNotFound(Uuid),

    #[error("Critical alert requires 2-step confirmation: {0}")]
    CriticalRequiresTwoStep(Uuid),

    #[error("Two-step confirmation not completed for alert: {0}")]
    TwoStepNotConfirmed(Uuid),

    #[error("Alert {0} is not a CRITICAL alert")]
    NotCriticalAlert(Uuid),

    #[error("Reference data load failed ({0}): {1}")]
    ReferenceDataLoad(String, String),

    #[error("Reference data parse failed ({0}): {1}")]
    ReferenceDataParse(String, String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Internal lock failed")]
    LockFailed,
}

// ---------------------------------------------------------------------------
// CoherenceEngine trait
// ---------------------------------------------------------------------------

/// The main coherence engine trait.
pub trait CoherenceEngine {
    /// Run coherence analysis on newly ingested data.
    fn analyze_new_document(
        &self,
        document_id: &Uuid,
        data: &RepositorySnapshot,
    ) -> Result<CoherenceResult, CoherenceError>;

    /// Run full coherence analysis on the entire data constellation.
    fn analyze_full(
        &self,
        data: &RepositorySnapshot,
    ) -> Result<CoherenceResult, CoherenceError>;

    /// Get all active (non-dismissed) alerts, optionally filtered by type.
    fn get_active_alerts(
        &self,
        alert_type: Option<&AlertType>,
    ) -> Result<Vec<CoherenceAlert>, CoherenceError>;

    /// Get alerts relevant to specific entities or keywords.
    fn get_relevant_alerts(
        &self,
        entity_ids: &[Uuid],
        keywords: &[String],
    ) -> Result<Vec<CoherenceAlert>, CoherenceError>;

    /// Get all CRITICAL non-dismissed alerts.
    fn get_critical_alerts(&self) -> Result<Vec<CoherenceAlert>, CoherenceError>;

    /// Dismiss a standard alert.
    fn dismiss_alert(
        &self,
        alert_id: &Uuid,
        reason: &str,
        dismissed_by: DismissedBy,
    ) -> Result<(), CoherenceError>;

    /// Dismiss a CRITICAL alert (requires 2-step confirmation).
    fn dismiss_critical_alert(
        &self,
        alert_id: &Uuid,
        reason: &str,
        two_step_confirmed: bool,
    ) -> Result<(), CoherenceError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alert_counts_total() {
        let counts = AlertCounts {
            conflicts: 1,
            duplicates: 2,
            gaps: 0,
            drifts: 1,
            temporals: 3,
            allergies: 0,
            doses: 1,
            criticals: 1,
        };
        assert_eq!(counts.total(), 9);
    }

    #[test]
    fn alert_severity_ordering() {
        assert!(AlertSeverity::Info < AlertSeverity::Standard);
        assert!(AlertSeverity::Standard < AlertSeverity::Critical);
    }
}
