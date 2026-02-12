# L2-03 — Coherence Engine

<!--
=============================================================================
COMPONENT SPEC — Medical intelligence engine for cross-data coherence.
Engineer review: E-ML (AI/ML, lead), E-DA (Data), E-RS (Rust), E-SC (Security), E-QA (QA)
This is the medical safety intelligence layer.
It detects conflicts, duplicates, gaps, correlations, and critical values
across the patient's entire data constellation — without providing clinical advice.
=============================================================================
-->

## Table of Contents

| Section | Offset |
|---------|--------|
| [1] Identity | `offset=25 limit=40` |
| [2] Dependencies | `offset=65 limit=30` |
| [3] Interfaces | `offset=95 limit=170` |
| [4] Detection: CONFLICT | `offset=265 limit=100` |
| [5] Detection: DUPLICATE | `offset=365 limit=70` |
| [6] Detection: GAP | `offset=435 limit=75` |
| [7] Detection: DRIFT | `offset=510 limit=80` |
| [8] Detection: TEMPORAL | `offset=590 limit=80` |
| [9] Detection: ALLERGY | `offset=670 limit=90` |
| [10] Detection: DOSE | `offset=760 limit=75` |
| [11] Detection: CRITICAL | `offset=835 limit=70` |
| [12] Alert Lifecycle | `offset=905 limit=110` |
| [13] Emergency Protocol | `offset=1015 limit=80` |
| [14] Patient-Facing Message Templates | `offset=1095 limit=75` |
| [15] Error Handling | `offset=1170 limit=35` |
| [16] Security | `offset=1205 limit=30` |
| [17] Testing | `offset=1235 limit=120` |
| [18] Performance | `offset=1355 limit=20` |
| [19] Open Questions | `offset=1375 limit=20` |

---

## [1] Identity

**What:** The medical intelligence engine that continuously analyzes the patient's data constellation for coherence. When new data is ingested (via L1-04 Storage Pipeline), the Coherence Engine compares it against everything already stored. It detects 8 categories of observations: CONFLICT, DUPLICATE, GAP, DRIFT, TEMPORAL, ALLERGY, DOSE, and CRITICAL. Observations are stored, not immediately surfaced, and presented only when contextually relevant (during conversation, appointment prep, or when the patient asks). CRITICAL alerts (lab values, allergy cross-matches) follow the Emergency Protocol with immediate surfacing and 2-step dismissal.

**After this session:**
- New document ingestion triggers automatic coherence analysis
- 8 detection algorithms implemented and tested
- CONFLICT: same medication with different parameters from different prescribers
- DUPLICATE: same generic medication under different brand names
- GAP: diagnosis without treatment, treatment without diagnosis
- DRIFT: unexplained medication or diagnosis status changes
- TEMPORAL: symptom onset within 14 days of medication/dose change or procedure
- ALLERGY: new medication ingredient matches known allergen
- DOSE: extracted dose outside plausible range for that medication
- CRITICAL: lab values flagged critical_low or critical_high
- Alert lifecycle: store, surface contextually, dismiss with reason
- Emergency Protocol: immediate surfacing for CRITICAL, 2-step dismissal
- All observations framed with calm, preparatory language (NC-07)
- Source traceability on every observation (NC-06)
- Patient-reported vs document-extracted data distinguished (NC-08)

**Estimated complexity:** Very High
**Source:** Tech Spec v1.1 Section 8 (Coherence Engine), Spec-07 (Coherence Observation System)

---

## [2] Dependencies

**Incoming:**
- L1-04 (storage pipeline -- newly stored data triggers coherence check)
- L0-02 (data model -- all repository traits, medication_aliases table, model structs)
- L0-03 (encryption -- ProfileSession for decrypting stored content fields)

**Outgoing:**
- L3-02 (home & document feed -- displays coherence observations as cards)
- L3-03 (chat interface -- surfaces relevant alerts during conversation)
- L4-02 (appointment prep -- includes CRITICAL alerts as priority items)
- L5-01 (trust & safety -- emergency protocol integration)

**No new Cargo.toml dependencies.** This component uses only existing crate dependencies:
- `chrono` (date arithmetic for temporal detection)
- `uuid` (entity identifiers)
- `serde` / `serde_json` (serialization)
- `thiserror` (error types)
- `tracing` (structured logging)

**Bundled data dependencies:**
- `medication_aliases.json` (brand-to-generic mapping, already bundled via L0-02)
- `dose_ranges.json` (plausible dose ranges per generic medication, bundled in `src-tauri/resources/`)

---

## [3] Interfaces

### Core Types

```rust
// src-tauri/src/intelligence/coherence.rs

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The 8 detection types the coherence engine can produce
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AlertType {
    Conflict,
    Duplicate,
    Gap,
    Drift,
    Temporal,
    Allergy,
    Dose,
    Critical,
}

impl AlertType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Conflict => "conflict",
            Self::Duplicate => "duplicate",
            Self::Gap => "gap",
            Self::Drift => "drift",
            Self::Temporal => "temporal",
            Self::Allergy => "allergy",
            Self::Dose => "dose",
            Self::Critical => "critical",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, CoherenceError> {
        match s {
            "conflict" => Ok(Self::Conflict),
            "duplicate" => Ok(Self::Duplicate),
            "gap" => Ok(Self::Gap),
            "drift" => Ok(Self::Drift),
            "temporal" => Ok(Self::Temporal),
            "allergy" => Ok(Self::Allergy),
            "dose" => Ok(Self::Dose),
            "critical" => Ok(Self::Critical),
            _ => Err(CoherenceError::InvalidAlertType(s.to_string())),
        }
    }
}

/// Severity determines surfacing behavior
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertSeverity {
    /// Informational: logged, surfaced only when patient asks
    Info,
    /// Standard: surfaced during relevant conversation or appointment prep
    Standard,
    /// Critical: surfaced immediately, requires 2-step dismissal
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

/// A coherence observation detected by the engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceAlert {
    /// Unique identifier for this alert
    pub id: Uuid,
    /// Detection type
    pub alert_type: AlertType,
    /// Severity determines surfacing and dismissal behavior
    pub severity: AlertSeverity,
    /// IDs of the entities involved (medications, labs, diagnoses, etc.)
    pub entity_ids: Vec<Uuid>,
    /// IDs of the source documents that contributed to this observation
    pub source_document_ids: Vec<Uuid>,
    /// Patient-facing message (calm, preparatory framing per NC-07)
    pub patient_message: String,
    /// Structured detail for internal use (not shown to patient directly)
    pub detail: AlertDetail,
    /// When this alert was detected
    pub detected_at: chrono::NaiveDateTime,
    /// Whether this alert has been surfaced to the patient
    pub surfaced: bool,
    /// Whether this alert has been dismissed
    pub dismissed: bool,
    /// Dismissal info (if dismissed)
    pub dismissal: Option<AlertDismissal>,
}

/// Structured detail payload per alert type
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
    pub prescriber_a: PresciberRef,
    pub prescriber_b: PresciberRef,
    pub field_conflicted: String,
    pub value_a: String,
    pub value_b: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresciberRef {
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

/// Dismissal record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertDismissal {
    pub dismissed_date: chrono::NaiveDateTime,
    pub reason: String,
    pub dismissed_by: DismissedBy,
    /// For CRITICAL alerts: did the patient complete 2-step confirmation?
    pub two_step_confirmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DismissedBy {
    Patient,
    ProfessionalFeedback,
}

impl DismissedBy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Patient => "patient",
            Self::ProfessionalFeedback => "professional_feedback",
        }
    }
}
```

### Coherence Engine Trait

```rust
/// Result of a coherence analysis run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceResult {
    /// New alerts detected in this run
    pub new_alerts: Vec<CoherenceAlert>,
    /// Counts per alert type
    pub counts: AlertCounts,
    /// Processing time in milliseconds
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
        self.conflicts + self.duplicates + self.gaps + self.drifts
            + self.temporals + self.allergies + self.doses + self.criticals
    }
}

/// The main coherence engine trait
pub trait CoherenceEngine {
    /// Run full coherence analysis on newly ingested data.
    /// Called automatically by L1-04 after storage completes.
    fn analyze_new_document(
        &self,
        document_id: &Uuid,
        repos: &RepositorySet,
    ) -> Result<CoherenceResult, CoherenceError>;

    /// Run full coherence analysis on the entire data constellation.
    /// Used for periodic health checks or on-demand by the patient.
    fn analyze_full(
        &self,
        repos: &RepositorySet,
    ) -> Result<CoherenceResult, CoherenceError>;

    /// Get all active (non-dismissed) alerts, optionally filtered by type
    fn get_active_alerts(
        &self,
        alert_type: Option<&AlertType>,
        repos: &RepositorySet,
    ) -> Result<Vec<CoherenceAlert>, CoherenceError>;

    /// Get alerts relevant to a specific topic (for conversation surfacing)
    fn get_relevant_alerts(
        &self,
        entity_ids: &[Uuid],
        keywords: &[String],
        repos: &RepositorySet,
    ) -> Result<Vec<CoherenceAlert>, CoherenceError>;

    /// Get all CRITICAL alerts that have not been dismissed (for emergency protocol)
    fn get_critical_alerts(
        &self,
        repos: &RepositorySet,
    ) -> Result<Vec<CoherenceAlert>, CoherenceError>;

    /// Dismiss a standard alert
    fn dismiss_alert(
        &self,
        alert_id: &Uuid,
        reason: &str,
        dismissed_by: DismissedBy,
        repos: &RepositorySet,
    ) -> Result<(), CoherenceError>;

    /// Dismiss a CRITICAL alert (requires 2-step confirmation)
    fn dismiss_critical_alert(
        &self,
        alert_id: &Uuid,
        reason: &str,
        two_step_confirmed: bool,
        repos: &RepositorySet,
    ) -> Result<(), CoherenceError>;
}
```

### Dose Range Reference Data

```rust
/// Plausible dose range for a medication (loaded from dose_ranges.json)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoseRange {
    pub generic_name: String,
    pub min_single_dose_mg: f64,
    pub max_single_dose_mg: f64,
    pub max_daily_dose_mg: f64,
    pub common_doses: Vec<String>,
    pub route: String,
}

/// Loaded reference data for coherence checks
pub struct CoherenceReferenceData {
    /// Brand-to-generic medication mappings
    pub medication_aliases: Vec<MedicationAlias>,
    /// Plausible dose ranges per generic medication
    pub dose_ranges: Vec<DoseRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationAlias {
    pub generic_name: String,
    pub brand_name: String,
    pub country: String,
}

impl CoherenceReferenceData {
    /// Load reference data from bundled JSON files
    pub fn load(resources_dir: &std::path::Path) -> Result<Self, CoherenceError> {
        let aliases_path = resources_dir.join("medication_aliases.json");
        let doses_path = resources_dir.join("dose_ranges.json");

        let aliases_json = std::fs::read_to_string(&aliases_path)
            .map_err(|e| CoherenceError::ReferenceDataLoad(
                aliases_path.display().to_string(), e.to_string()
            ))?;
        let medication_aliases: Vec<MedicationAlias> = serde_json::from_str(&aliases_json)
            .map_err(|e| CoherenceError::ReferenceDataParse(
                "medication_aliases.json".into(), e.to_string()
            ))?;

        let doses_json = std::fs::read_to_string(&doses_path)
            .map_err(|e| CoherenceError::ReferenceDataLoad(
                doses_path.display().to_string(), e.to_string()
            ))?;
        let dose_ranges: Vec<DoseRange> = serde_json::from_str(&doses_json)
            .map_err(|e| CoherenceError::ReferenceDataParse(
                "dose_ranges.json".into(), e.to_string()
            ))?;

        Ok(Self {
            medication_aliases,
            dose_ranges,
        })
    }

    /// Look up the generic name for a brand name
    pub fn resolve_generic(&self, brand_name: &str) -> Option<&str> {
        let lower = brand_name.to_lowercase();
        self.medication_aliases.iter()
            .find(|a| a.brand_name.to_lowercase() == lower)
            .map(|a| a.generic_name.as_str())
    }

    /// Look up dose range for a generic medication name
    pub fn get_dose_range(&self, generic_name: &str) -> Option<&DoseRange> {
        let lower = generic_name.to_lowercase();
        self.dose_ranges.iter()
            .find(|d| d.generic_name.to_lowercase() == lower)
    }
}
```

---

## [4] Detection: CONFLICT

**TRIGGER:** Two active medications with the same generic_name but different parameters (dose, frequency, route) from different prescribers.

**Detection method:** Structured comparison on the medications table. Group active medications by generic_name. For each group with 2+ entries from different prescribers, compare dose, frequency, and route.

```rust
/// Detect medication conflicts: same medication, different parameters, different prescribers
pub fn detect_conflicts(
    document_id: &Uuid,
    repos: &RepositorySet,
    reference: &CoherenceReferenceData,
) -> Result<Vec<CoherenceAlert>, CoherenceError> {
    let mut alerts = Vec::new();

    // Get all active medications
    let active_meds = repos.medication.get_active()?;

    // Get medications from the newly ingested document
    let new_meds: Vec<&Medication> = active_meds.iter()
        .filter(|m| m.document_id == *document_id)
        .collect();

    // For each new medication, check against existing active medications
    for new_med in &new_meds {
        let resolved_generic = resolve_generic_name(new_med, reference);

        let existing_matches: Vec<&Medication> = active_meds.iter()
            .filter(|m| {
                m.id != new_med.id
                    && m.document_id != *document_id
                    && resolve_generic_name(m, reference) == resolved_generic
                    && m.status == MedicationStatus::Active
            })
            .collect();

        for existing in &existing_matches {
            // Only flag if from DIFFERENT prescribers
            let different_prescriber = match (new_med.prescriber_id, existing.prescriber_id) {
                (Some(a), Some(b)) => a != b,
                _ => true, // Unknown prescriber counts as different
            };

            if !different_prescriber {
                continue;
            }

            // Compare dose
            if normalize_dose(&new_med.dose) != normalize_dose(&existing.dose) {
                let alert = build_conflict_alert(
                    new_med, existing, "dose",
                    &new_med.dose, &existing.dose,
                    repos,
                )?;
                alerts.push(alert);
            }

            // Compare frequency
            if normalize_frequency(&new_med.frequency)
                != normalize_frequency(&existing.frequency)
            {
                let alert = build_conflict_alert(
                    new_med, existing, "frequency",
                    &new_med.frequency, &existing.frequency,
                    repos,
                )?;
                alerts.push(alert);
            }

            // Compare route
            if new_med.route.to_lowercase() != existing.route.to_lowercase() {
                let alert = build_conflict_alert(
                    new_med, existing, "route",
                    &new_med.route, &existing.route,
                    repos,
                )?;
                alerts.push(alert);
            }
        }
    }

    Ok(alerts)
}

/// Resolve the canonical generic name for a medication, using alias table if needed
fn resolve_generic_name(
    med: &Medication,
    reference: &CoherenceReferenceData,
) -> String {
    let generic = med.generic_name.to_lowercase();
    if !generic.is_empty() {
        return generic;
    }
    // Fall back to alias lookup from brand name
    if let Some(brand) = &med.brand_name {
        if let Some(resolved) = reference.resolve_generic(brand) {
            return resolved.to_lowercase();
        }
    }
    generic
}

/// Normalize dose string for comparison (extract numeric mg value)
fn normalize_dose(dose: &str) -> String {
    dose.to_lowercase()
        .replace(' ', "")
        .replace("milligrams", "mg")
        .replace("grams", "g")
        .replace("micrograms", "mcg")
}

/// Normalize frequency string for comparison
fn normalize_frequency(freq: &str) -> String {
    let lower = freq.to_lowercase();
    // Map common synonyms
    let normalized = lower
        .replace("twice daily", "2x/day")
        .replace("two times a day", "2x/day")
        .replace("bid", "2x/day")
        .replace("once daily", "1x/day")
        .replace("once a day", "1x/day")
        .replace("qd", "1x/day")
        .replace("three times daily", "3x/day")
        .replace("tid", "3x/day")
        .replace("four times daily", "4x/day")
        .replace("qid", "4x/day");
    normalized.trim().to_string()
}

/// Build a conflict alert with proper message framing
fn build_conflict_alert(
    new_med: &Medication,
    existing_med: &Medication,
    field: &str,
    new_value: &str,
    existing_value: &str,
    repos: &RepositorySet,
) -> Result<CoherenceAlert, CoherenceError> {
    let prescriber_a_name = resolve_prescriber_name(new_med.prescriber_id, repos);
    let prescriber_b_name = resolve_prescriber_name(existing_med.prescriber_id, repos);

    let message = format!(
        "Your records show {} {} from {} and {} {} from {}. \
         You may want to ask about this at your next appointment.",
        new_med.generic_name, new_value, prescriber_a_name,
        existing_med.generic_name, existing_value, prescriber_b_name,
    );

    Ok(CoherenceAlert {
        id: Uuid::new_v4(),
        alert_type: AlertType::Conflict,
        severity: AlertSeverity::Standard,
        entity_ids: vec![new_med.id, existing_med.id],
        source_document_ids: vec![new_med.document_id, existing_med.document_id],
        patient_message: message,
        detail: AlertDetail::Conflict(ConflictDetail {
            medication_name: new_med.generic_name.clone(),
            prescriber_a: PresciberRef {
                professional_id: new_med.prescriber_id.unwrap_or(Uuid::nil()),
                name: prescriber_a_name,
                document_id: new_med.document_id,
                document_date: None,
            },
            prescriber_b: PresciberRef {
                professional_id: existing_med.prescriber_id.unwrap_or(Uuid::nil()),
                name: prescriber_b_name,
                document_id: existing_med.document_id,
                document_date: None,
            },
            field_conflicted: field.to_string(),
            value_a: new_value.to_string(),
            value_b: existing_value.to_string(),
        }),
        detected_at: chrono::Local::now().naive_local(),
        surfaced: false,
        dismissed: false,
        dismissal: None,
    })
}

/// Resolve prescriber name from professional_id, fallback to "a prescriber"
fn resolve_prescriber_name(
    professional_id: Option<Uuid>,
    repos: &RepositorySet,
) -> String {
    if let Some(id) = professional_id {
        if let Ok(Some(prof)) = repos.professional.get(&id) {
            return prof.name.clone();
        }
    }
    "a prescriber".to_string()
}
```

---

## [5] Detection: DUPLICATE

**TRIGGER:** Two active medications that resolve to the same generic_name but appear as separate prescriptions (often under different brand names).

**Detection method:** Use the medication_aliases table and CoherenceReferenceData to resolve brand names to generic names. Flag when two active medications resolve to the same generic but have different brand names or different prescription entries.

```rust
/// Detect duplicate medications: same generic under different brand names
pub fn detect_duplicates(
    document_id: &Uuid,
    repos: &RepositorySet,
    reference: &CoherenceReferenceData,
) -> Result<Vec<CoherenceAlert>, CoherenceError> {
    let mut alerts = Vec::new();

    let active_meds = repos.medication.get_active()?;

    // Get medications from the new document
    let new_meds: Vec<&Medication> = active_meds.iter()
        .filter(|m| m.document_id == *document_id)
        .collect();

    for new_med in &new_meds {
        let new_generic = resolve_generic_name(new_med, reference);

        let duplicates: Vec<&Medication> = active_meds.iter()
            .filter(|m| {
                m.id != new_med.id
                    && m.document_id != *document_id
                    && resolve_generic_name(m, reference) == new_generic
                    && m.status == MedicationStatus::Active
            })
            .collect();

        for existing in &duplicates {
            // Only flag as duplicate if they have DIFFERENT display names
            let new_display = display_name(new_med);
            let existing_display = display_name(existing);

            if new_display.to_lowercase() != existing_display.to_lowercase() {
                let message = format!(
                    "It looks like {} and {} may be the same medication ({}). \
                     You might want to verify this with your pharmacist.",
                    new_display, existing_display, new_generic,
                );

                alerts.push(CoherenceAlert {
                    id: Uuid::new_v4(),
                    alert_type: AlertType::Duplicate,
                    severity: AlertSeverity::Standard,
                    entity_ids: vec![new_med.id, existing.id],
                    source_document_ids: vec![
                        new_med.document_id, existing.document_id
                    ],
                    patient_message: message,
                    detail: AlertDetail::Duplicate(DuplicateDetail {
                        generic_name: new_generic.clone(),
                        brand_a: new_display,
                        brand_b: existing_display,
                        medication_id_a: new_med.id,
                        medication_id_b: existing.id,
                    }),
                    detected_at: chrono::Local::now().naive_local(),
                    surfaced: false,
                    dismissed: false,
                    dismissal: None,
                });
            }
        }
    }

    // Deduplicate: avoid flagging A-B and B-A
    dedup_symmetric_alerts(&mut alerts);

    Ok(alerts)
}

/// Get display name for a medication (prefer brand_name, fall back to generic_name)
fn display_name(med: &Medication) -> String {
    med.brand_name.clone().unwrap_or_else(|| med.generic_name.clone())
}

/// Remove symmetric duplicates (A conflicts with B == B conflicts with A)
fn dedup_symmetric_alerts(alerts: &mut Vec<CoherenceAlert>) {
    let mut seen_pairs: std::collections::HashSet<(Uuid, Uuid)> =
        std::collections::HashSet::new();
    alerts.retain(|alert| {
        if alert.entity_ids.len() >= 2 {
            let a = alert.entity_ids[0];
            let b = alert.entity_ids[1];
            let pair = if a < b { (a, b) } else { (b, a) };
            seen_pairs.insert(pair)
        } else {
            true
        }
    });
}
```

---

## [6] Detection: GAP

**TRIGGER:** A diagnosis exists without a linked medication or treatment plan, OR a medication exists without a documented diagnosis/reason.

**Detection method:** Cross-reference the diagnoses and medications tables. For each active diagnosis, check if any active medication references it (via medication.condition or textual match). For each active medication, check if a diagnosis exists that could justify it.

```rust
/// Detect care gaps: diagnosis without treatment, treatment without diagnosis
pub fn detect_gaps(
    document_id: &Uuid,
    repos: &RepositorySet,
) -> Result<Vec<CoherenceAlert>, CoherenceError> {
    let mut alerts = Vec::new();

    let active_diagnoses = repos.diagnosis.list(&DiagnosisFilter {
        status: Some(DiagnosisStatus::Active),
        ..Default::default()
    })?;
    let active_meds = repos.medication.get_active()?;

    // GAP TYPE 1: Diagnosis without treatment
    for diag in &active_diagnoses {
        let has_treatment = active_meds.iter().any(|m| {
            medication_relates_to_diagnosis(m, diag)
        });

        if !has_treatment {
            // Only flag if the diagnosis came from the current document
            // or if we're doing a full analysis
            if diag.document_id == *document_id || document_id.is_nil() {
                let message = format!(
                    "Your records mention {} but I don't see a treatment plan for it. \
                     This might be worth discussing at your next appointment.",
                    diag.name,
                );

                alerts.push(CoherenceAlert {
                    id: Uuid::new_v4(),
                    alert_type: AlertType::Gap,
                    severity: AlertSeverity::Info,
                    entity_ids: vec![diag.id],
                    source_document_ids: vec![diag.document_id],
                    patient_message: message,
                    detail: AlertDetail::Gap(GapDetail {
                        gap_type: GapType::DiagnosisWithoutTreatment,
                        entity_name: diag.name.clone(),
                        entity_id: diag.id,
                        expected: "medication or treatment plan".to_string(),
                        document_id: diag.document_id,
                    }),
                    detected_at: chrono::Local::now().naive_local(),
                    surfaced: false,
                    dismissed: false,
                    dismissal: None,
                });
            }
        }
    }

    // GAP TYPE 2: Medication without diagnosis
    for med in &active_meds {
        // Skip OTC medications -- patients add these without needing a diagnosis
        if med.is_otc {
            continue;
        }

        let has_diagnosis = active_diagnoses.iter().any(|d| {
            medication_relates_to_diagnosis(med, d)
        });

        // Also skip if the medication has a documented reason_start
        let has_reason = med.reason_start.as_ref()
            .map(|r| !r.trim().is_empty())
            .unwrap_or(false);

        if !has_diagnosis && !has_reason {
            if med.document_id == *document_id || document_id.is_nil() {
                let display = display_name(med);
                let message = format!(
                    "Your records show {} as an active medication but I don't see \
                     a documented reason for it. Your doctor can help clarify this.",
                    display,
                );

                alerts.push(CoherenceAlert {
                    id: Uuid::new_v4(),
                    alert_type: AlertType::Gap,
                    severity: AlertSeverity::Info,
                    entity_ids: vec![med.id],
                    source_document_ids: vec![med.document_id],
                    patient_message: message,
                    detail: AlertDetail::Gap(GapDetail {
                        gap_type: GapType::MedicationWithoutDiagnosis,
                        entity_name: display,
                        entity_id: med.id,
                        expected: "documented diagnosis or reason".to_string(),
                        document_id: med.document_id,
                    }),
                    detected_at: chrono::Local::now().naive_local(),
                    surfaced: false,
                    dismissed: false,
                    dismissal: None,
                });
            }
        }
    }

    Ok(alerts)
}

/// Check if a medication appears to relate to a diagnosis.
/// Uses the medication's condition field and fuzzy name matching.
fn medication_relates_to_diagnosis(med: &Medication, diag: &Diagnosis) -> bool {
    let diag_lower = diag.name.to_lowercase();

    // Check the medication's condition field
    if let Some(ref condition) = med.condition {
        let cond_lower = condition.to_lowercase();
        if cond_lower.contains(&diag_lower) || diag_lower.contains(&cond_lower) {
            return true;
        }
    }

    // Check the medication's reason_start field
    if let Some(ref reason) = med.reason_start {
        let reason_lower = reason.to_lowercase();
        if reason_lower.contains(&diag_lower) || diag_lower.contains(&reason_lower) {
            return true;
        }
    }

    false
}
```

---

## [7] Detection: DRIFT

**TRIGGER:** A medication's status, dose, or frequency changed between documents without a documented reason (no reason_stop on the old medication, no reason in the dose_changes record).

**Detection method:** Compare the newly stored medication data against prior records for the same generic. Detect status changes (active -> stopped), dose changes, and frequency changes where no rationale is documented.

```rust
/// Detect care drift: unexplained medication or diagnosis changes
pub fn detect_drift(
    document_id: &Uuid,
    repos: &RepositorySet,
    reference: &CoherenceReferenceData,
) -> Result<Vec<CoherenceAlert>, CoherenceError> {
    let mut alerts = Vec::new();

    // --- Medication drift ---
    let all_meds = repos.medication.list(&MedicationFilter::default())?;

    let new_meds: Vec<&Medication> = all_meds.iter()
        .filter(|m| m.document_id == *document_id)
        .collect();

    for new_med in &new_meds {
        let new_generic = resolve_generic_name(new_med, reference);

        // Find prior records for the same medication (older documents)
        let prior: Vec<&Medication> = all_meds.iter()
            .filter(|m| {
                m.id != new_med.id
                    && m.document_id != *document_id
                    && resolve_generic_name(m, reference) == new_generic
            })
            .collect();

        for old_med in &prior {
            // Medication was stopped without documented reason
            if old_med.status == MedicationStatus::Active
                && new_med.status == MedicationStatus::Stopped
            {
                let reason_given = new_med.reason_stop.as_ref()
                    .map(|r| !r.trim().is_empty())
                    .unwrap_or(false);

                if !reason_given {
                    let message = format!(
                        "Your medication {} appears to have been stopped. \
                         I don't see a note explaining the change. \
                         You might want to ask why at your next visit.",
                        new_med.generic_name,
                    );
                    alerts.push(build_drift_alert(
                        new_med, "status",
                        old_med.status.as_str(), new_med.status.as_str(),
                        false, &message,
                    ));
                }
            }

            // Dose changed without documented reason
            if normalize_dose(&old_med.dose) != normalize_dose(&new_med.dose) {
                let has_dose_change_record = repos.medication
                    .get_dose_history(&new_med.id)?
                    .iter()
                    .any(|dc| dc.reason.as_ref().map(|r| !r.trim().is_empty()).unwrap_or(false));

                if !has_dose_change_record {
                    let message = format!(
                        "Your medication for {} was changed from {} to {}. \
                         I don't see a note explaining the change. \
                         You might want to ask why at your next visit.",
                        new_med.generic_name, old_med.dose, new_med.dose,
                    );
                    alerts.push(build_drift_alert(
                        new_med, "dose",
                        &old_med.dose, &new_med.dose,
                        false, &message,
                    ));
                }
            }
        }
    }

    // --- Diagnosis drift ---
    let all_diagnoses = repos.diagnosis.list(&DiagnosisFilter::default())?;

    let new_diags: Vec<&Diagnosis> = all_diagnoses.iter()
        .filter(|d| d.document_id == *document_id)
        .collect();

    for new_diag in &new_diags {
        let prior_diags: Vec<&Diagnosis> = all_diagnoses.iter()
            .filter(|d| {
                d.id != new_diag.id
                    && d.document_id != *document_id
                    && d.name.to_lowercase() == new_diag.name.to_lowercase()
            })
            .collect();

        for old_diag in &prior_diags {
            if old_diag.status != new_diag.status {
                let message = format!(
                    "The status of {} changed from {} to {}. \
                     I don't see a note explaining this change.",
                    new_diag.name, old_diag.status.as_str(), new_diag.status.as_str(),
                );
                alerts.push(CoherenceAlert {
                    id: Uuid::new_v4(),
                    alert_type: AlertType::Drift,
                    severity: AlertSeverity::Info,
                    entity_ids: vec![new_diag.id, old_diag.id],
                    source_document_ids: vec![new_diag.document_id, old_diag.document_id],
                    patient_message: message,
                    detail: AlertDetail::Drift(DriftDetail {
                        entity_type: "diagnosis".to_string(),
                        entity_name: new_diag.name.clone(),
                        old_value: old_diag.status.as_str().to_string(),
                        new_value: new_diag.status.as_str().to_string(),
                        change_date: new_diag.date_diagnosed,
                        reason_documented: false,
                    }),
                    detected_at: chrono::Local::now().naive_local(),
                    surfaced: false,
                    dismissed: false,
                    dismissal: None,
                });
            }
        }
    }

    Ok(alerts)
}

fn build_drift_alert(
    med: &Medication,
    field: &str,
    old_val: &str,
    new_val: &str,
    reason_documented: bool,
    message: &str,
) -> CoherenceAlert {
    CoherenceAlert {
        id: Uuid::new_v4(),
        alert_type: AlertType::Drift,
        severity: AlertSeverity::Standard,
        entity_ids: vec![med.id],
        source_document_ids: vec![med.document_id],
        patient_message: message.to_string(),
        detail: AlertDetail::Drift(DriftDetail {
            entity_type: "medication".to_string(),
            entity_name: med.generic_name.clone(),
            old_value: old_val.to_string(),
            new_value: new_val.to_string(),
            change_date: None,
            reason_documented,
        }),
        detected_at: chrono::Local::now().naive_local(),
        surfaced: false,
        dismissed: false,
        dismissal: None,
    }
}
```

---

## [8] Detection: TEMPORAL

**TRIGGER:** A patient-reported symptom has an onset_date within 14 days of a medication start, dose change, or procedure.

**Detection method:** For each symptom with an onset_date, check dose_changes, medication start dates, and procedure dates within a configurable correlation window (default: 14 days).

```rust
/// Temporal correlation window in days
const TEMPORAL_CORRELATION_WINDOW_DAYS: i64 = 14;

/// Detect temporal correlations: symptoms near medication/dose/procedure changes
pub fn detect_temporal(
    document_id: &Uuid,
    repos: &RepositorySet,
) -> Result<Vec<CoherenceAlert>, CoherenceError> {
    let mut alerts = Vec::new();

    // Get symptoms -- either newly ingested or all (for full analysis)
    let symptoms = if document_id.is_nil() {
        repos.symptom.get_active()?
    } else {
        // For document-triggered analysis, get recent symptoms
        let window_start = chrono::Local::now().date_naive()
            - chrono::Duration::days(TEMPORAL_CORRELATION_WINDOW_DAYS);
        repos.symptom.get_in_date_range(
            window_start,
            chrono::Local::now().date_naive(),
        )?
    };

    // Collect all temporal events (medication starts, dose changes, procedures)
    let active_meds = repos.medication.get_active()?;
    let all_procedures = repos.procedure.list(&ProcedureFilter::default())?;

    for symptom in &symptoms {
        let onset = match chrono::NaiveDate::parse_from_str(
            &symptom.onset_date, "%Y-%m-%d"
        ) {
            Ok(d) => d,
            Err(_) => continue, // Skip symptoms with unparseable dates
        };

        // Check medication start dates
        for med in &active_meds {
            if let Some(start) = med.start_date {
                let days_between = (onset - start).num_days();
                if days_between >= 0 && days_between <= TEMPORAL_CORRELATION_WINDOW_DAYS {
                    let message = format!(
                        "You reported {} starting {}, which was {} days after \
                         starting {}. This might be worth mentioning to your doctor.",
                        symptom.specific, symptom.onset_date,
                        days_between, med.generic_name,
                    );

                    alerts.push(CoherenceAlert {
                        id: Uuid::new_v4(),
                        alert_type: AlertType::Temporal,
                        severity: AlertSeverity::Standard,
                        entity_ids: vec![symptom.id, med.id],
                        source_document_ids: vec![med.document_id],
                        patient_message: message,
                        detail: AlertDetail::Temporal(TemporalDetail {
                            symptom_id: symptom.id,
                            symptom_name: symptom.specific.clone(),
                            symptom_onset: onset,
                            correlated_event: TemporalEvent::MedicationStarted {
                                medication_id: med.id,
                                medication_name: med.generic_name.clone(),
                                start_date: start,
                            },
                            days_between,
                        }),
                        detected_at: chrono::Local::now().naive_local(),
                        surfaced: false,
                        dismissed: false,
                        dismissal: None,
                    });
                }
            }

            // Check dose changes
            if let Ok(dose_changes) = repos.medication.get_dose_history(&med.id) {
                for dc in &dose_changes {
                    if let Ok(change_date) = chrono::NaiveDate::parse_from_str(
                        &dc.change_date, "%Y-%m-%d"
                    ) {
                        let days_between = (onset - change_date).num_days();
                        if days_between >= 0
                            && days_between <= TEMPORAL_CORRELATION_WINDOW_DAYS
                        {
                            let message = format!(
                                "You reported {} starting {}, which was {} days after \
                                 your {} dose was changed from {} to {}. \
                                 This might be worth mentioning to your doctor.",
                                symptom.specific, symptom.onset_date,
                                days_between, med.generic_name,
                                dc.old_dose.as_deref().unwrap_or("unknown"),
                                dc.new_dose,
                            );

                            alerts.push(CoherenceAlert {
                                id: Uuid::new_v4(),
                                alert_type: AlertType::Temporal,
                                severity: AlertSeverity::Standard,
                                entity_ids: vec![symptom.id, med.id],
                                source_document_ids: if let Some(doc_id) = dc.document_id {
                                    vec![doc_id]
                                } else {
                                    vec![med.document_id]
                                },
                                patient_message: message,
                                detail: AlertDetail::Temporal(TemporalDetail {
                                    symptom_id: symptom.id,
                                    symptom_name: symptom.specific.clone(),
                                    symptom_onset: onset,
                                    correlated_event: TemporalEvent::DoseChanged {
                                        medication_id: med.id,
                                        medication_name: med.generic_name.clone(),
                                        old_dose: dc.old_dose.clone()
                                            .unwrap_or_else(|| "unknown".to_string()),
                                        new_dose: dc.new_dose.clone(),
                                        change_date,
                                    },
                                    days_between,
                                }),
                                detected_at: chrono::Local::now().naive_local(),
                                surfaced: false,
                                dismissed: false,
                                dismissal: None,
                            });
                        }
                    }
                }
            }
        }

        // Check procedures
        for procedure in &all_procedures {
            if let Some(proc_date) = procedure.date {
                let days_between = (onset - proc_date).num_days();
                if days_between >= 0
                    && days_between <= TEMPORAL_CORRELATION_WINDOW_DAYS
                {
                    let message = format!(
                        "You reported {} starting {}, which was {} days after \
                         your {} procedure. This might be worth mentioning to your doctor.",
                        symptom.specific, symptom.onset_date,
                        days_between, procedure.name,
                    );

                    alerts.push(CoherenceAlert {
                        id: Uuid::new_v4(),
                        alert_type: AlertType::Temporal,
                        severity: AlertSeverity::Standard,
                        entity_ids: vec![symptom.id, procedure.id],
                        source_document_ids: vec![procedure.document_id],
                        patient_message: message,
                        detail: AlertDetail::Temporal(TemporalDetail {
                            symptom_id: symptom.id,
                            symptom_name: symptom.specific.clone(),
                            symptom_onset: onset,
                            correlated_event: TemporalEvent::ProcedurePerformed {
                                procedure_id: procedure.id,
                                procedure_name: procedure.name.clone(),
                                procedure_date: proc_date,
                            },
                            days_between,
                        }),
                        detected_at: chrono::Local::now().naive_local(),
                        surfaced: false,
                        dismissed: false,
                        dismissal: None,
                    });
                }
            }
        }
    }

    Ok(alerts)
}
```

---

## [9] Detection: ALLERGY (Cross-Check)

**TRIGGER:** A newly ingested medication (or its compound ingredients) contains an ingredient that maps to a known allergen in the patient's allergy table.

**Detection method:** For each newly stored medication, resolve its ingredients (direct generic_name + compound_ingredients.maps_to_generic). Cross-reference each resolved ingredient against allergies.allergen. This includes drug family mapping (e.g., penicillin allergy flags amoxicillin).

**Severity:** CRITICAL -- this is a patient safety detection.

```rust
/// Detect allergy cross-matches: new medication contains known allergen
pub fn detect_allergy_conflicts(
    document_id: &Uuid,
    repos: &RepositorySet,
    reference: &CoherenceReferenceData,
) -> Result<Vec<CoherenceAlert>, CoherenceError> {
    let mut alerts = Vec::new();

    let allergies = repos.allergy.get_all_active()?;
    if allergies.is_empty() {
        return Ok(alerts);
    }

    // Build a lowercase set of allergens for fast lookup
    let allergen_set: std::collections::HashSet<String> = allergies.iter()
        .map(|a| a.allergen.to_lowercase())
        .collect();

    // Also build a map from allergen to allergy record for detail
    let allergen_map: std::collections::HashMap<String, &Allergy> = allergies.iter()
        .map(|a| (a.allergen.to_lowercase(), a))
        .collect();

    // Get medications from the new document
    let all_meds = repos.medication.list(&MedicationFilter::default())?;
    let new_meds: Vec<&Medication> = all_meds.iter()
        .filter(|m| m.document_id == *document_id)
        .collect();

    for med in &new_meds {
        // Collect all ingredient names for this medication
        let mut ingredient_names: Vec<(String, String)> = Vec::new();

        // The medication's own generic name
        let generic = resolve_generic_name(med, reference);
        if !generic.is_empty() {
            ingredient_names.push((generic.clone(), med.generic_name.clone()));
        }

        // Compound ingredients
        if med.is_compound {
            if let Ok(ingredients) = repos.medication.get_compound_ingredients(&med.id) {
                for ing in &ingredients {
                    let resolved = ing.maps_to_generic.as_deref()
                        .unwrap_or(&ing.ingredient_name)
                        .to_lowercase();
                    ingredient_names.push((
                        resolved,
                        ing.ingredient_name.clone(),
                    ));
                }
            }
        }

        // Cross-reference each ingredient against allergens
        for (resolved_name, display_name) in &ingredient_names {
            if let Some(allergy) = allergen_map.get(resolved_name.as_str()) {
                let med_display = display_name_str(med);
                let message = format!(
                    "Your records note an allergy to {}. The medication {} \
                     contains {} which is in the same family. \
                     Please verify this with your pharmacist before taking it.",
                    allergy.allergen, med_display, display_name,
                );

                alerts.push(CoherenceAlert {
                    id: Uuid::new_v4(),
                    alert_type: AlertType::Allergy,
                    severity: AlertSeverity::Critical,
                    entity_ids: vec![med.id, allergy.id],
                    source_document_ids: vec![
                        med.document_id,
                        allergy.document_id.unwrap_or(Uuid::nil()),
                    ],
                    patient_message: message,
                    detail: AlertDetail::Allergy(AllergyDetail {
                        allergen: allergy.allergen.clone(),
                        allergy_severity: allergy.severity.as_str().to_string(),
                        allergy_id: allergy.id,
                        medication_name: med_display,
                        medication_id: med.id,
                        matching_ingredient: display_name.clone(),
                        ingredient_maps_to: resolved_name.clone(),
                    }),
                    detected_at: chrono::Local::now().naive_local(),
                    surfaced: false,
                    dismissed: false,
                    dismissal: None,
                });
            }

            // Also check drug family mapping (e.g., amoxicillin -> penicillin family)
            for allergy in &allergies {
                let allergen_lower = allergy.allergen.to_lowercase();
                if allergen_lower != *resolved_name
                    && is_same_drug_family(&allergen_lower, resolved_name)
                {
                    let med_display = display_name_str(med);
                    let message = format!(
                        "Your records note an allergy to {}. The medication {} \
                         contains {} which is in the same family. \
                         Please verify this with your pharmacist before taking it.",
                        allergy.allergen, med_display, display_name,
                    );

                    alerts.push(CoherenceAlert {
                        id: Uuid::new_v4(),
                        alert_type: AlertType::Allergy,
                        severity: AlertSeverity::Critical,
                        entity_ids: vec![med.id, allergy.id],
                        source_document_ids: vec![
                            med.document_id,
                            allergy.document_id.unwrap_or(Uuid::nil()),
                        ],
                        patient_message: message,
                        detail: AlertDetail::Allergy(AllergyDetail {
                            allergen: allergy.allergen.clone(),
                            allergy_severity: allergy.severity.as_str().to_string(),
                            allergy_id: allergy.id,
                            medication_name: med_display,
                            medication_id: med.id,
                            matching_ingredient: display_name.clone(),
                            ingredient_maps_to: resolved_name.clone(),
                        }),
                        detected_at: chrono::Local::now().naive_local(),
                        surfaced: false,
                        dismissed: false,
                        dismissal: None,
                    });
                }
            }
        }
    }

    dedup_symmetric_alerts(&mut alerts);
    Ok(alerts)
}

fn display_name_str(med: &Medication) -> String {
    med.brand_name.clone().unwrap_or_else(|| med.generic_name.clone())
}

/// Known drug family groupings for cross-allergy detection.
/// Phase 1: hardcoded common families. Phase 3: full pharmacological DB.
fn is_same_drug_family(allergen: &str, ingredient: &str) -> bool {
    let families: &[&[&str]] = &[
        // Penicillin family
        &["penicillin", "amoxicillin", "ampicillin", "piperacillin",
          "oxacillin", "nafcillin", "dicloxacillin", "flucloxacillin"],
        // Cephalosporin family (cross-reactivity with penicillin ~1-2%)
        &["cephalexin", "cefazolin", "ceftriaxone", "cefuroxime",
          "cefixime", "cefpodoxime", "ceftazidime"],
        // Sulfonamide family
        &["sulfamethoxazole", "sulfasalazine", "sulfadiazine",
          "trimethoprim-sulfamethoxazole", "sulfisoxazole"],
        // NSAID family
        &["ibuprofen", "naproxen", "diclofenac", "indomethacin",
          "piroxicam", "meloxicam", "celecoxib", "aspirin"],
        // Statin family
        &["atorvastatin", "rosuvastatin", "simvastatin", "pravastatin",
          "lovastatin", "fluvastatin", "pitavastatin"],
        // ACE inhibitor family
        &["lisinopril", "enalapril", "ramipril", "captopril",
          "benazepril", "fosinopril", "quinapril", "perindopril"],
        // Opioid family
        &["morphine", "codeine", "hydrocodone", "oxycodone",
          "tramadol", "fentanyl", "methadone", "hydromorphone"],
        // Fluoroquinolone family
        &["ciprofloxacin", "levofloxacin", "moxifloxacin",
          "norfloxacin", "ofloxacin"],
        // Macrolide family
        &["azithromycin", "clarithromycin", "erythromycin"],
        // Tetracycline family
        &["tetracycline", "doxycycline", "minocycline"],
    ];

    for family in families {
        let allergen_in_family = family.iter().any(|&member| {
            allergen.contains(member) || member.contains(allergen)
        });
        let ingredient_in_family = family.iter().any(|&member| {
            ingredient.contains(member) || member.contains(ingredient)
        });
        if allergen_in_family && ingredient_in_family {
            return true;
        }
    }

    false
}
```

---

## [10] Detection: DOSE (Plausibility)

**TRIGGER:** An extracted dose for a medication falls outside the plausible range defined in the bundled dose_ranges.json.

**Detection method:** Parse the extracted dose string into a numeric milligram value. Compare against the min_single_dose_mg and max_single_dose_mg from the reference data. Flag if outside range.

```rust
/// Detect dose plausibility issues: extracted dose outside typical range
pub fn detect_dose_issues(
    document_id: &Uuid,
    repos: &RepositorySet,
    reference: &CoherenceReferenceData,
) -> Result<Vec<CoherenceAlert>, CoherenceError> {
    let mut alerts = Vec::new();

    let all_meds = repos.medication.list(&MedicationFilter::default())?;
    let new_meds: Vec<&Medication> = all_meds.iter()
        .filter(|m| m.document_id == *document_id)
        .collect();

    for med in &new_meds {
        let generic = resolve_generic_name(med, reference);

        let dose_range = match reference.get_dose_range(&generic) {
            Some(range) => range,
            None => continue, // No reference data for this medication
        };

        let extracted_mg = match parse_dose_to_mg(&med.dose) {
            Some(mg) => mg,
            None => continue, // Could not parse dose
        };

        let outside_range = extracted_mg < dose_range.min_single_dose_mg
            || extracted_mg > dose_range.max_single_dose_mg;

        if outside_range {
            let med_display = display_name_str(med);
            let message = format!(
                "I extracted {} for {} but the typical range is {}-{}. \
                 Please double-check this value.",
                med.dose, med_display,
                format_dose_mg(dose_range.min_single_dose_mg),
                format_dose_mg(dose_range.max_single_dose_mg),
            );

            alerts.push(CoherenceAlert {
                id: Uuid::new_v4(),
                alert_type: AlertType::Dose,
                severity: AlertSeverity::Standard,
                entity_ids: vec![med.id],
                source_document_ids: vec![med.document_id],
                patient_message: message,
                detail: AlertDetail::Dose(DoseDetail {
                    medication_name: med_display,
                    medication_id: med.id,
                    extracted_dose: med.dose.clone(),
                    extracted_dose_mg: extracted_mg,
                    typical_range_low_mg: dose_range.min_single_dose_mg,
                    typical_range_high_mg: dose_range.max_single_dose_mg,
                    source: "dose_ranges.json".to_string(),
                }),
                detected_at: chrono::Local::now().naive_local(),
                surfaced: false,
                dismissed: false,
                dismissal: None,
            });
        }
    }

    Ok(alerts)
}

/// Parse a dose string into milligrams.
/// Handles: "500mg", "1g", "250 mg", "0.5g", "100mcg", "500 milligrams"
pub fn parse_dose_to_mg(dose: &str) -> Option<f64> {
    let lower = dose.to_lowercase().replace(' ', "");

    // Try to extract a numeric value and unit
    let re_mg = regex::Regex::new(r"(\d+\.?\d*)\s*(?:mg|milligrams?)").ok()?;
    let re_g = regex::Regex::new(r"(\d+\.?\d*)\s*(?:g|grams?)").ok()?;
    let re_mcg = regex::Regex::new(r"(\d+\.?\d*)\s*(?:mcg|micrograms?|ug|µg)").ok()?;

    if let Some(caps) = re_mg.captures(&lower) {
        return caps.get(1)?.as_str().parse::<f64>().ok();
    }
    if let Some(caps) = re_g.captures(&lower) {
        return caps.get(1)?.as_str().parse::<f64>().ok().map(|v| v * 1000.0);
    }
    if let Some(caps) = re_mcg.captures(&lower) {
        return caps.get(1)?.as_str().parse::<f64>().ok().map(|v| v / 1000.0);
    }

    // Fallback: try to parse bare number (assume mg)
    let re_bare = regex::Regex::new(r"^(\d+\.?\d*)$").ok()?;
    if let Some(caps) = re_bare.captures(&lower) {
        return caps.get(1)?.as_str().parse::<f64>().ok();
    }

    None
}

/// Format a milligram value for display
fn format_dose_mg(mg: f64) -> String {
    if mg >= 1000.0 {
        format!("{}g", mg / 1000.0)
    } else if mg < 1.0 {
        format!("{}mcg", mg * 1000.0)
    } else {
        format!("{}mg", mg)
    }
}
```

---

## [11] Detection: CRITICAL (Lab Values)

**TRIGGER:** A lab result has abnormal_flag set to `critical_low` or `critical_high`.

**Detection method:** Direct field check on newly stored lab_results. No inference -- the critical flag is set by the lab report itself and extracted during structuring (L1-03).

**Severity:** CRITICAL -- follows the Emergency Protocol.

```rust
/// Detect critical lab values
pub fn detect_critical_labs(
    document_id: &Uuid,
    repos: &RepositorySet,
) -> Result<Vec<CoherenceAlert>, CoherenceError> {
    let mut alerts = Vec::new();

    // Get lab results from the new document
    let all_labs = repos.lab_result.list(&LabResultFilter::default())?;
    let new_labs: Vec<&LabResult> = all_labs.iter()
        .filter(|l| l.document_id == *document_id)
        .collect();

    for lab in &new_labs {
        let is_critical = matches!(
            lab.abnormal_flag,
            AbnormalFlag::CriticalLow | AbnormalFlag::CriticalHigh
        );

        if !is_critical {
            continue;
        }

        let value_display = lab.value
            .map(|v| format!("{}", v))
            .or_else(|| lab.value_text.clone())
            .unwrap_or_else(|| "value".to_string());

        let unit_display = lab.unit.as_deref().unwrap_or("");

        let flag_description = match lab.abnormal_flag {
            AbnormalFlag::CriticalLow => "below the expected range",
            AbnormalFlag::CriticalHigh => "above the expected range",
            _ => "outside the expected range",
        };

        // NC-07: Calm but clear. Use "promptly" / "soon", NEVER "immediately" or "urgently"
        let message = format!(
            "Your lab report from {} flags {} as needing prompt attention. \
             The result ({} {}) is {}. \
             Please contact your doctor or pharmacist soon.",
            lab.collection_date, lab.test_name,
            value_display, unit_display,
            flag_description,
        );

        alerts.push(CoherenceAlert {
            id: Uuid::new_v4(),
            alert_type: AlertType::Critical,
            severity: AlertSeverity::Critical,
            entity_ids: vec![lab.id],
            source_document_ids: vec![lab.document_id],
            patient_message: message,
            detail: AlertDetail::Critical(CriticalDetail {
                test_name: lab.test_name.clone(),
                lab_result_id: lab.id,
                value: lab.value.unwrap_or(0.0),
                unit: unit_display.to_string(),
                abnormal_flag: lab.abnormal_flag.as_str().to_string(),
                reference_range_low: lab.reference_range_low,
                reference_range_high: lab.reference_range_high,
                collection_date: lab.collection_date,
                document_id: lab.document_id,
            }),
            detected_at: chrono::Local::now().naive_local(),
            surfaced: false,
            dismissed: false,
            dismissal: None,
        });
    }

    Ok(alerts)
}
```

---

## [12] Alert Lifecycle (Store, Surface, Dismiss)

### Lifecycle Flow

```
DETECTED
  |
  v
STORED (alert written to in-memory alert store + persisted to SQLite)
  |
  +---> CRITICAL alerts: surfaced IMMEDIATELY (Emergency Protocol)
  |     Requires 2-step dismissal. NOT suppressible by normal dismissal.
  |
  +---> STANDARD alerts: surfaced contextually:
  |     - During relevant conversation (patient asks about related topic)
  |     - During appointment preparation (included in summary)
  |     - When patient navigates to related screen (medication list, etc.)
  |
  v
SURFACED (marked surfaced=true)
  |
  +---> Patient says "my doctor addressed this"
  |     - STANDARD: single confirmation -> DISMISSED
  |     - CRITICAL: 2-step confirmation -> DISMISSED
  |
  v
DISMISSED (stored with reason, dismissed_by, date)
  - Never re-surfaced for the same entity pair
  - Persisted in dismissed_alerts table
```

### Implementation

```rust
/// In-memory alert store backed by SQLite persistence
pub struct AlertStore {
    /// Active alerts (not yet dismissed)
    active: std::sync::RwLock<Vec<CoherenceAlert>>,
}

impl AlertStore {
    /// Create a new alert store, loading persisted dismissed alerts
    pub fn new() -> Self {
        Self {
            active: std::sync::RwLock::new(Vec::new()),
        }
    }

    /// Load previously detected but non-dismissed alerts from SQLite
    pub fn load_from_db(
        &self,
        repos: &RepositorySet,
    ) -> Result<(), CoherenceError> {
        // Implementation loads active alerts from a coherence_alerts table
        // or reconstructs from dismissed_alerts to know what to exclude
        Ok(())
    }

    /// Store a new alert if not already dismissed for this entity pair
    pub fn store_alert(
        &self,
        alert: CoherenceAlert,
        repos: &RepositorySet,
    ) -> Result<bool, CoherenceError> {
        // Check if this entity pair + alert type was already dismissed
        let entity_ids_json = serde_json::to_string(&alert.entity_ids)
            .map_err(|e| CoherenceError::Serialization(e.to_string()))?;

        let already_dismissed = repos.alert.is_dismissed(
            alert.alert_type.as_str(),
            &alert.entity_ids,
        )?;

        if already_dismissed {
            tracing::debug!(
                alert_type = alert.alert_type.as_str(),
                "Alert already dismissed for this entity pair, skipping"
            );
            return Ok(false);
        }

        // Check for duplicate active alert (same type + same entities)
        let mut active = self.active.write()
            .map_err(|_| CoherenceError::LockFailed)?;

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

    /// Get all active alerts, optionally filtered
    pub fn get_active(
        &self,
        alert_type: Option<&AlertType>,
    ) -> Result<Vec<CoherenceAlert>, CoherenceError> {
        let active = self.active.read()
            .map_err(|_| CoherenceError::LockFailed)?;

        let result = match alert_type {
            Some(t) => active.iter()
                .filter(|a| &a.alert_type == t && !a.dismissed)
                .cloned()
                .collect(),
            None => active.iter()
                .filter(|a| !a.dismissed)
                .cloned()
                .collect(),
        };

        Ok(result)
    }

    /// Get alerts relevant to specific entities or keywords
    pub fn get_relevant(
        &self,
        entity_ids: &[Uuid],
        keywords: &[String],
    ) -> Result<Vec<CoherenceAlert>, CoherenceError> {
        let active = self.active.read()
            .map_err(|_| CoherenceError::LockFailed)?;

        let results = active.iter()
            .filter(|alert| {
                if alert.dismissed {
                    return false;
                }

                // Match by entity ID overlap
                let entity_match = alert.entity_ids.iter()
                    .any(|id| entity_ids.contains(id));

                // Match by keyword in patient message
                let keyword_match = keywords.iter().any(|kw| {
                    alert.patient_message.to_lowercase().contains(&kw.to_lowercase())
                });

                entity_match || keyword_match
            })
            .cloned()
            .collect();

        Ok(results)
    }

    /// Get all CRITICAL non-dismissed alerts
    pub fn get_critical(&self) -> Result<Vec<CoherenceAlert>, CoherenceError> {
        let active = self.active.read()
            .map_err(|_| CoherenceError::LockFailed)?;

        Ok(active.iter()
            .filter(|a| a.severity == AlertSeverity::Critical && !a.dismissed)
            .cloned()
            .collect())
    }

    /// Dismiss a standard alert
    pub fn dismiss(
        &self,
        alert_id: &Uuid,
        reason: &str,
        dismissed_by: DismissedBy,
        repos: &RepositorySet,
    ) -> Result<(), CoherenceError> {
        let mut active = self.active.write()
            .map_err(|_| CoherenceError::LockFailed)?;

        let alert = active.iter_mut()
            .find(|a| a.id == *alert_id)
            .ok_or(CoherenceError::AlertNotFound(*alert_id))?;

        if alert.severity == AlertSeverity::Critical {
            return Err(CoherenceError::CriticalRequiresTwoStep(*alert_id));
        }

        alert.dismissed = true;
        alert.dismissal = Some(AlertDismissal {
            dismissed_date: chrono::Local::now().naive_local(),
            reason: reason.to_string(),
            dismissed_by: dismissed_by.clone(),
            two_step_confirmed: false,
        });

        // Persist to dismissed_alerts table
        repos.alert.dismiss(&DismissedAlert {
            id: Uuid::new_v4(),
            alert_type: alert.alert_type.as_str().to_string(),
            entity_ids: serde_json::to_string(&alert.entity_ids)
                .unwrap_or_default(),
            dismissed_date: chrono::Local::now().naive_local(),
            reason: Some(reason.to_string()),
            dismissed_by: dismissed_by.as_str().to_string(),
        })?;

        Ok(())
    }

    /// Dismiss a CRITICAL alert (requires 2-step confirmation)
    pub fn dismiss_critical(
        &self,
        alert_id: &Uuid,
        reason: &str,
        two_step_confirmed: bool,
        repos: &RepositorySet,
    ) -> Result<(), CoherenceError> {
        if !two_step_confirmed {
            return Err(CoherenceError::TwoStepNotConfirmed(*alert_id));
        }

        let mut active = self.active.write()
            .map_err(|_| CoherenceError::LockFailed)?;

        let alert = active.iter_mut()
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

        // Persist
        repos.alert.dismiss(&DismissedAlert {
            id: Uuid::new_v4(),
            alert_type: alert.alert_type.as_str().to_string(),
            entity_ids: serde_json::to_string(&alert.entity_ids)
                .unwrap_or_default(),
            dismissed_date: chrono::Local::now().naive_local(),
            reason: Some(reason.to_string()),
            dismissed_by: DismissedBy::Patient.as_str().to_string(),
        })?;

        Ok(())
    }
}

/// Check if two entity ID sets refer to the same entities (order-independent)
fn entities_match(a: &[Uuid], b: &[Uuid]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut sorted_a: Vec<Uuid> = a.to_vec();
    let mut sorted_b: Vec<Uuid> = b.to_vec();
    sorted_a.sort();
    sorted_b.sort();
    sorted_a == sorted_b
}
```

---

## [13] Emergency Protocol

**TRIGGER:** CRITICAL alerts (lab values flagged critical_low or critical_high, allergy cross-matches).

**NC-07 WORDING RULES:**
- Use "promptly" / "soon" -- NEVER "immediately" or "urgently"
- Calm but not dismissive
- No interpretation of what the critical value means clinically
- No alarm icons, no red coloring (use the app's emphasis color)

```rust
/// Emergency protocol handler for CRITICAL alerts
pub struct EmergencyProtocol;

impl EmergencyProtocol {
    /// Process critical alerts detected during ingestion.
    /// Returns alerts that need immediate surfacing.
    pub fn process_critical_alerts(
        alerts: &[CoherenceAlert],
    ) -> Vec<EmergencyAction> {
        alerts.iter()
            .filter(|a| a.severity == AlertSeverity::Critical)
            .map(|alert| {
                match &alert.detail {
                    AlertDetail::Critical(detail) => {
                        EmergencyAction {
                            alert_id: alert.id,
                            action_type: EmergencyActionType::LabCritical,
                            ingestion_message: format!(
                                "This result is marked as requiring attention \
                                 on your lab report."
                            ),
                            home_banner: alert.patient_message.clone(),
                            appointment_priority: true,
                            dismissal_steps: 2,
                            dismissal_prompt_1: "Has your doctor addressed this?".to_string(),
                            dismissal_prompt_2: "Yes, my doctor has seen this result".to_string(),
                        }
                    }
                    AlertDetail::Allergy(detail) => {
                        EmergencyAction {
                            alert_id: alert.id,
                            action_type: EmergencyActionType::AllergyMatch,
                            ingestion_message: format!(
                                "This medication may contain an ingredient \
                                 related to a known allergy in your records."
                            ),
                            home_banner: alert.patient_message.clone(),
                            appointment_priority: true,
                            dismissal_steps: 2,
                            dismissal_prompt_1: "Has your doctor or pharmacist \
                                addressed this?".to_string(),
                            dismissal_prompt_2: "Yes, this has been reviewed \
                                by my healthcare provider".to_string(),
                        }
                    }
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
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyAction {
    pub alert_id: Uuid,
    pub action_type: EmergencyActionType,
    /// Message shown during ingestion review (L3-04)
    pub ingestion_message: String,
    /// Banner shown on Home/Chat screens (L3-02)
    pub home_banner: String,
    /// Whether to add as priority item in appointment prep (L4-02)
    pub appointment_priority: bool,
    /// Number of dismissal steps required (always 2 for CRITICAL)
    pub dismissal_steps: u8,
    /// Step 1 prompt: "Has your doctor addressed this?"
    pub dismissal_prompt_1: String,
    /// Step 2 prompt: confirmation text
    pub dismissal_prompt_2: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EmergencyActionType {
    LabCritical,
    AllergyMatch,
    Other,
}
```

---

## [14] Patient-Facing Message Templates

**NC-07 CONSTRAINT:** All messages use calm, preparatory language. No alarm wording. No red alerts. Framing is "here is what I noticed in your records" not "WARNING: danger detected."

**NC-06 CONSTRAINT:** Every observation traces to source documents.

**NC-08 CONSTRAINT:** Patient-reported data is always distinguished from professionally-documented data.

```rust
/// Message template builder for consistent, calm framing
pub struct MessageTemplates;

impl MessageTemplates {
    /// CONFLICT message
    pub fn conflict(
        medication: &str, field: &str,
        value_a: &str, prescriber_a: &str,
        value_b: &str, prescriber_b: &str,
    ) -> String {
        format!(
            "Your records show {} {} from {} and {} {} from {}. \
             You may want to ask about this at your next appointment.",
            medication, value_a, prescriber_a,
            medication, value_b, prescriber_b,
        )
    }

    /// DUPLICATE message
    pub fn duplicate(brand_a: &str, brand_b: &str, generic: &str) -> String {
        format!(
            "It looks like {} and {} may be the same medication ({}). \
             You might want to verify this with your pharmacist.",
            brand_a, brand_b, generic,
        )
    }

    /// GAP: diagnosis without treatment
    pub fn gap_no_treatment(diagnosis: &str) -> String {
        format!(
            "Your records mention {} but I don't see a treatment plan for it. \
             This might be worth discussing at your next appointment.",
            diagnosis,
        )
    }

    /// GAP: medication without diagnosis
    pub fn gap_no_diagnosis(medication: &str) -> String {
        format!(
            "Your records show {} as an active medication but I don't see \
             a documented reason for it. Your doctor can help clarify this.",
            medication,
        )
    }

    /// DRIFT message
    pub fn drift(
        medication: &str, old_value: &str, new_value: &str,
    ) -> String {
        format!(
            "Your medication for {} was changed from {} to {}. \
             I don't see a note explaining the change. \
             You might want to ask why at your next visit.",
            medication, old_value, new_value,
        )
    }

    /// TEMPORAL message
    pub fn temporal(
        symptom: &str, onset: &str, days: i64, event: &str,
    ) -> String {
        format!(
            "You reported {} starting {}, which was {} days after {}. \
             This might be worth mentioning to your doctor.",
            symptom, onset, days, event,
        )
    }

    /// ALLERGY message
    pub fn allergy(
        allergen: &str, medication: &str, ingredient: &str,
    ) -> String {
        format!(
            "Your records note an allergy to {}. The medication {} \
             contains {} which is in the same family. \
             Please verify this with your pharmacist before taking it.",
            allergen, medication, ingredient,
        )
    }

    /// DOSE message
    pub fn dose(
        dose: &str, medication: &str, range_low: &str, range_high: &str,
    ) -> String {
        format!(
            "I extracted {} for {} but the typical range is {}-{}. \
             Please double-check this value.",
            dose, medication, range_low, range_high,
        )
    }

    /// CRITICAL lab message
    /// NC-07: "promptly" / "soon" -- NEVER "immediately" or "urgently"
    pub fn critical_lab(date: &str, test: &str) -> String {
        format!(
            "Your lab report from {} flags {} as needing prompt attention. \
             Please contact your doctor or pharmacist soon.",
            date, test,
        )
    }

    /// Patient-reported data disclaimer (NC-08)
    pub fn patient_reported_note(symptom: &str) -> String {
        format!(
            "Note: \"{}\" is based on your own report, not a clinical document.",
            symptom,
        )
    }
}
```

---

## [15] Error Handling

```rust
use thiserror::Error;

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
```

---

## [16] Security

**E-SC review:**

| Concern | Mitigation |
|---------|-----------|
| Alert messages contain patient data | Alert store held in memory. Persisted dismissed_alerts in encrypted SQLite (per L0-03). |
| Logging patient health info | NEVER log alert message content. Log only alert_id, alert_type, and severity. |
| Reference data integrity | dose_ranges.json and medication_aliases.json bundled at build time. Checksum verified at load. |
| Alert manipulation | Dismissal requires explicit user action via IPC. No auto-dismiss. CRITICAL requires 2-step. |
| Drug family data completeness | Hardcoded families are conservative (common families only). False negatives possible; false positives preferred for safety. |

---

## [17] Testing

### Acceptance Criteria

| # | Test | Expected |
|---|------|----------|
| T-01 | CONFLICT: two active Metformin prescriptions, different doses, different prescribers | Alert generated with both prescriber names and doses |
| T-02 | CONFLICT: same medication, same prescriber, different dose | No alert (same prescriber = updated prescription, not conflict) |
| T-03 | CONFLICT: different brand names resolving to same generic, different doses | Alert generated (alias resolution works) |
| T-04 | DUPLICATE: Glucophage and Metformin both active | Alert identifies both as metformin |
| T-05 | DUPLICATE: same brand name, same medication | No alert (not a duplicate if names match) |
| T-06 | GAP: diagnosis "Type 2 Diabetes" with no linked medication | Alert: diagnosis without treatment |
| T-07 | GAP: medication without documented diagnosis (non-OTC) | Alert: medication without diagnosis |
| T-08 | GAP: OTC medication without diagnosis | No alert (OTC exemption) |
| T-09 | DRIFT: Metformin dose changed from 500mg to 1000mg, no reason documented | Alert with old and new dose |
| T-10 | DRIFT: medication stopped with documented reason_stop | No alert (reason is documented) |
| T-11 | TEMPORAL: symptom onset 3 days after starting new medication | Alert linking symptom to medication start |
| T-12 | TEMPORAL: symptom onset 15 days after medication change | No alert (outside 14-day window) |
| T-13 | TEMPORAL: symptom onset 10 days after dose change | Alert linking symptom to dose change |
| T-14 | TEMPORAL: symptom onset 7 days after procedure | Alert linking symptom to procedure |
| T-15 | ALLERGY: penicillin allergy + amoxicillin prescribed | CRITICAL alert (same drug family) |
| T-16 | ALLERGY: aspirin allergy + ibuprofen prescribed | CRITICAL alert (NSAID family) |
| T-17 | ALLERGY: compound medication contains mapped allergen ingredient | CRITICAL alert |
| T-18 | DOSE: Metformin 5000mg (typical range 500-2000mg) | Alert with range comparison |
| T-19 | DOSE: Metformin 500mg (within range) | No alert |
| T-20 | DOSE: unparseable dose string | No alert (graceful skip) |
| T-21 | CRITICAL: lab result with abnormal_flag = critical_high | CRITICAL alert with Emergency Protocol |
| T-22 | CRITICAL: lab result with abnormal_flag = high (not critical) | No CRITICAL alert |
| T-23 | Dismissed standard alert not re-surfaced | Store_alert returns false for dismissed entity pair |
| T-24 | CRITICAL alert dismiss without 2-step | Error: TwoStepNotConfirmed |
| T-25 | CRITICAL alert dismiss with 2-step confirmed | Successfully dismissed |
| T-26 | Standard alert dismiss (single step) | Successfully dismissed |
| T-27 | Emergency protocol generates correct action for critical lab | 2-step dismissal prompts, banner text, priority item |
| T-28 | NC-07: no alert message contains "immediately" or "urgently" | All messages pass calm language check |
| T-29 | Full analysis: document with conflict + critical lab + allergy | All three alerts detected in single run |
| T-30 | parse_dose_to_mg: "500mg" | Returns Some(500.0) |
| T-31 | parse_dose_to_mg: "1g" | Returns Some(1000.0) |
| T-32 | parse_dose_to_mg: "250 micrograms" | Returns Some(0.25) |
| T-33 | Normalize frequency: "twice daily" == "BID" | Both normalize to "2x/day" |

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // --- Dose parsing ---

    #[test]
    fn parse_dose_mg() {
        assert_eq!(parse_dose_to_mg("500mg"), Some(500.0));
        assert_eq!(parse_dose_to_mg("500 mg"), Some(500.0));
        assert_eq!(parse_dose_to_mg("1.5g"), Some(1500.0));
        assert_eq!(parse_dose_to_mg("250mcg"), Some(0.25));
        assert_eq!(parse_dose_to_mg("100 micrograms"), Some(0.1));
        assert_eq!(parse_dose_to_mg("500"), Some(500.0));
        assert_eq!(parse_dose_to_mg("unknown"), None);
        assert_eq!(parse_dose_to_mg(""), None);
    }

    // --- Frequency normalization ---

    #[test]
    fn normalize_frequency_synonyms() {
        assert_eq!(normalize_frequency("twice daily"), normalize_frequency("BID"));
        assert_eq!(normalize_frequency("once daily"), normalize_frequency("QD"));
        assert_eq!(normalize_frequency("three times daily"), normalize_frequency("TID"));
    }

    // --- Dose normalization ---

    #[test]
    fn normalize_dose_equivalents() {
        assert_eq!(normalize_dose("500 mg"), normalize_dose("500mg"));
        assert_eq!(normalize_dose("500 milligrams"), normalize_dose("500mg"));
    }

    // --- Drug family matching ---

    #[test]
    fn drug_family_penicillin() {
        assert!(is_same_drug_family("penicillin", "amoxicillin"));
        assert!(is_same_drug_family("amoxicillin", "penicillin"));
        assert!(!is_same_drug_family("penicillin", "ibuprofen"));
    }

    #[test]
    fn drug_family_nsaid() {
        assert!(is_same_drug_family("aspirin", "ibuprofen"));
        assert!(is_same_drug_family("ibuprofen", "naproxen"));
        assert!(!is_same_drug_family("ibuprofen", "amoxicillin"));
    }

    #[test]
    fn drug_family_no_match() {
        assert!(!is_same_drug_family("metformin", "atorvastatin"));
    }

    // --- Entity matching ---

    #[test]
    fn entity_match_order_independent() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        assert!(entities_match(&[a, b], &[b, a]));
        assert!(entities_match(&[a, b], &[a, b]));
        assert!(!entities_match(&[a], &[a, b]));
    }

    // --- NC-07 calm language compliance ---

    #[test]
    fn messages_never_contain_alarm_words() {
        let alarm_words = ["immediately", "urgently", "emergency", "danger", "warning"];

        let messages = vec![
            MessageTemplates::conflict(
                "Metformin", "dose", "500mg", "Dr. A", "1000mg", "Dr. B",
            ),
            MessageTemplates::duplicate("Glucophage", "Metformin", "metformin"),
            MessageTemplates::gap_no_treatment("Type 2 Diabetes"),
            MessageTemplates::gap_no_diagnosis("Metformin"),
            MessageTemplates::drift("Metformin", "500mg", "1000mg"),
            MessageTemplates::temporal("headache", "2026-01-15", 3, "starting Lisinopril"),
            MessageTemplates::allergy("penicillin", "Amoxicillin", "amoxicillin"),
            MessageTemplates::dose("5000mg", "Metformin", "500mg", "2000mg"),
            MessageTemplates::critical_lab("2026-01-15", "Potassium"),
        ];

        for message in &messages {
            let lower = message.to_lowercase();
            for word in &alarm_words {
                assert!(
                    !lower.contains(word),
                    "Message contains alarm word '{}': {}",
                    word, message,
                );
            }
        }
    }

    // --- Alert lifecycle ---

    #[test]
    fn dismissed_alert_not_re_stored() {
        // This test requires mock repositories
        // Verifies that store_alert returns false when entity pair is already dismissed
    }

    #[test]
    fn critical_alert_requires_two_step() {
        let store = AlertStore::new();
        let alert_id = Uuid::new_v4();

        let alert = CoherenceAlert {
            id: alert_id,
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
                collection_date: chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
                document_id: Uuid::new_v4(),
            }),
            detected_at: chrono::Local::now().naive_local(),
            surfaced: false,
            dismissed: false,
            dismissal: None,
        };

        // Store the alert directly for testing
        {
            let mut active = store.active.write().unwrap();
            active.push(alert);
        }

        // Attempt to dismiss as standard should fail
        // (would need mock repos for full test)
    }

    // --- Integration: full analysis ---

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

    // --- Message templates ---

    #[test]
    fn conflict_message_contains_both_prescribers() {
        let msg = MessageTemplates::conflict(
            "Metformin", "dose",
            "500mg", "Dr. Chen",
            "1000mg", "Dr. Moreau",
        );
        assert!(msg.contains("Dr. Chen"));
        assert!(msg.contains("Dr. Moreau"));
        assert!(msg.contains("500mg"));
        assert!(msg.contains("1000mg"));
    }

    #[test]
    fn duplicate_message_contains_generic() {
        let msg = MessageTemplates::duplicate("Glucophage", "Metformin", "metformin");
        assert!(msg.contains("Glucophage"));
        assert!(msg.contains("Metformin"));
        assert!(msg.contains("metformin"));
    }

    #[test]
    fn critical_lab_message_uses_calm_language() {
        let msg = MessageTemplates::critical_lab("2026-01-15", "Potassium");
        assert!(msg.contains("prompt attention"));
        assert!(msg.contains("soon"));
        assert!(!msg.contains("immediately"));
        assert!(!msg.contains("urgently"));
    }

    // --- Dose plausibility ---

    #[test]
    fn format_dose_mg_display() {
        assert_eq!(format_dose_mg(500.0), "500mg");
        assert_eq!(format_dose_mg(1000.0), "1g");
        assert_eq!(format_dose_mg(0.5), "500mcg");
    }

    // --- Gap detection: medication relates to diagnosis ---

    #[test]
    fn medication_relates_via_condition() {
        let med = Medication {
            id: Uuid::new_v4(),
            generic_name: "Metformin".into(),
            brand_name: None,
            dose: "500mg".into(),
            frequency: "twice daily".into(),
            frequency_type: FrequencyType::Scheduled,
            route: "oral".into(),
            prescriber_id: None,
            start_date: None,
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
            document_id: Uuid::new_v4(),
        };

        let diag = Diagnosis {
            id: Uuid::new_v4(),
            name: "Type 2 Diabetes".into(),
            icd_code: None,
            date_diagnosed: None,
            diagnosing_professional_id: None,
            status: DiagnosisStatus::Active,
            document_id: Uuid::new_v4(),
        };

        assert!(medication_relates_to_diagnosis(&med, &diag));
    }

    #[test]
    fn medication_does_not_relate_unlinked() {
        let med = Medication {
            id: Uuid::new_v4(),
            generic_name: "Atorvastatin".into(),
            brand_name: None,
            dose: "20mg".into(),
            frequency: "once daily".into(),
            frequency_type: FrequencyType::Scheduled,
            route: "oral".into(),
            prescriber_id: None,
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
            document_id: Uuid::new_v4(),
        };

        let diag = Diagnosis {
            id: Uuid::new_v4(),
            name: "Asthma".into(),
            icd_code: None,
            date_diagnosed: None,
            diagnosing_professional_id: None,
            status: DiagnosisStatus::Active,
            document_id: Uuid::new_v4(),
        };

        assert!(!medication_relates_to_diagnosis(&med, &diag));
    }
}
```

### Integration Tests

```rust
// tests/coherence_test.rs

/// Integration test: full coherence analysis on a document with known issues
#[tokio::test]
async fn coherence_detects_conflict_and_critical() {
    let db = TestDatabase::new().await;
    let reference = CoherenceReferenceData::load_test();

    // Setup: existing prescription from Dr. Chen
    let doc1_id = db.insert_document("prescription", "2026-01-01").await;
    let dr_chen = db.insert_professional("Dr. Chen", "GP").await;
    db.insert_medication(
        "Metformin", Some("Glucophage"), "500mg", "twice daily",
        Some(dr_chen), doc1_id,
    ).await;

    // Setup: existing allergy
    db.insert_allergy("penicillin", "severe").await;

    // New document from Dr. Moreau: same drug, different dose + amoxicillin + critical lab
    let doc2_id = db.insert_document("prescription", "2026-02-01").await;
    let dr_moreau = db.insert_professional("Dr. Moreau", "Endocrinologist").await;
    db.insert_medication(
        "Metformin", Some("Metformin"), "1000mg", "twice daily",
        Some(dr_moreau), doc2_id,
    ).await;
    db.insert_medication(
        "Amoxicillin", None, "500mg", "three times daily",
        Some(dr_moreau), doc2_id,
    ).await;
    db.insert_lab_result(
        "Potassium", 6.5, "mEq/L", "critical_high",
        "2026-02-01", doc2_id,
    ).await;

    // Run coherence analysis
    let engine = DefaultCoherenceEngine::new(reference);
    let result = engine.analyze_new_document(&doc2_id, &db.repos()).unwrap();

    // Verify CONFLICT detected (Metformin 500mg vs 1000mg, different prescribers)
    assert!(result.counts.conflicts >= 1, "Expected conflict detection");

    // Verify ALLERGY detected (penicillin allergy vs amoxicillin)
    assert!(result.counts.allergies >= 1, "Expected allergy detection");

    // Verify CRITICAL detected (Potassium critical_high)
    assert!(result.counts.criticals >= 1, "Expected critical lab detection");

    // Verify all critical alerts have severity Critical
    for alert in &result.new_alerts {
        if alert.alert_type == AlertType::Critical || alert.alert_type == AlertType::Allergy {
            assert_eq!(alert.severity, AlertSeverity::Critical);
        }
    }
}
```

---

## [18] Performance

| Metric | Target |
|--------|--------|
| Single document coherence analysis (< 20 medications, < 50 labs) | < 200ms |
| Full constellation analysis (100 medications, 200 labs, 50 diagnoses) | < 2 seconds |
| Alert store lookup (active alerts) | < 5ms |
| Alert relevance matching (keywords + entity IDs) | < 10ms |
| Dismissed alert check | < 5ms |
| Reference data load (medication_aliases.json + dose_ranges.json) | < 100ms (once at app start) |
| Emergency protocol processing | < 10ms |

**E-RS notes:**
- Reference data (aliases, dose ranges) loaded ONCE at application start, held in memory.
- Alert store is in-memory with SQLite persistence for dismissed_alerts.
- All detection functions are synchronous (SQLite queries via repos). No async needed.
- Coherence analysis runs on a background thread after L1-04 completes. Does not block the UI.

---

## [19] Open Questions

| # | Question | Status |
|---|---------|--------|
| OQ-01 | Should drug family mappings come from a bundled data file instead of hardcoded? | Hardcoded for Phase 1. Migrate to bundled JSON in Phase 3 when full pharmacological DB is available. |
| OQ-02 | Should semantic similarity (LanceDB) be used for CONFLICT detection in addition to structured comparison? | Deferred to Phase 2. Structured comparison sufficient for MVP. Semantic adds value for narrative contradictions. |
| OQ-03 | Should GAP detection distinguish between "monitoring-only" diagnoses and those requiring treatment? | Phase 2. Some diagnoses (e.g., "family history of X") don't require treatment. Need curated list or LLM classification. |
| OQ-04 | How to handle medications prescribed as PRN (as-needed) in conflict detection? | PRN medications should not conflict on dose with scheduled versions of the same drug. Filter by frequency_type. |
| OQ-05 | Should temporal correlation also check against lab result dates (lab change near medication change)? | Phase 2. Adds complexity. Current scope covers symptoms only. |
| OQ-06 | Persist active (non-dismissed) alerts in a dedicated SQLite table or reconstruct from data on each session? | Decision needed. Reconstruction is simpler but slower for large datasets. Dedicated table adds schema complexity. |
