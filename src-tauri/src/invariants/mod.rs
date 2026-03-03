//! ME-03: Invariant Reference Engine.
//!
//! Static medical reference knowledge that pairs with user data to produce
//! grounded clinical insights — deterministic, computable, no SLM required.
//!
//! Two-tier storage:
//! - **Const tier**: Vital sign and lab thresholds (compiled into binary)
//! - **Bundled tier**: Drug families, interactions, cross-reactivity (JSON at startup)
//!
//! All data sourced from international clinical guidelines
//! (ISH, ESC, KDIGO, IDF, WHO, BTS, GLIM, WAO, EAACI, EASL, ETA, IOF).

pub mod types;
pub mod vitals;
pub mod labs;
pub mod loader;
pub mod enrich;
pub mod demographics;
pub mod screening;

use std::path::Path;

use loader::{BundledInvariants, LoadError};

// ═══════════════════════════════════════════════════════════
// InvariantRegistry — single access point for all reference data
// ═══════════════════════════════════════════════════════════

/// Central registry for all medical invariant reference data.
///
/// Loaded once at startup, stored on CoreState.
/// Combines const tier (vital/lab thresholds) with bundled tier (JSON).
#[derive(Debug, Clone)]
pub struct InvariantRegistry {
    /// Bundled tier: drug families, interactions, cross-reactivity, monitoring.
    pub bundled: BundledInvariants,
}

impl InvariantRegistry {
    /// Load the registry from the resources directory.
    ///
    /// Const tier is always available (compiled in).
    /// Bundled tier loads from `resources/invariants/*.json`.
    /// Missing JSON files are treated as empty (graceful degradation).
    pub fn load(resources_dir: &Path) -> Result<Self, LoadError> {
        let bundled = loader::load_bundled(resources_dir)?;
        Ok(Self { bundled })
    }

    /// Create an empty registry (for testing or when resources unavailable).
    pub fn empty() -> Self {
        Self {
            bundled: BundledInvariants::default(),
        }
    }

    // ── Const tier access (always available) ──────────────

    /// Blood pressure classifications (ISH 2020).
    pub fn bp_classifications(&self) -> &'static [vitals::BpClassification] {
        vitals::BP_CLASSIFICATIONS
    }

    /// Heart rate classifications (ESC 2021/2019).
    pub fn hr_classifications(&self) -> &'static [vitals::HrClassification] {
        vitals::HR_CLASSIFICATIONS
    }

    /// SpO2 classifications (BTS 2017).
    pub fn spo2_classifications(&self) -> &'static [vitals::Spo2Classification] {
        vitals::SPO2_CLASSIFICATIONS
    }

    /// BMI classifications (WHO TRS 894).
    pub fn bmi_classifications(&self) -> &'static [vitals::BmiClassification] {
        vitals::BMI_CLASSIFICATIONS
    }

    /// Fasting glucose classifications (WHO 2006).
    pub fn glucose_classifications(&self) -> &'static [vitals::GlucoseClassification] {
        vitals::GLUCOSE_CLASSIFICATIONS
    }

    /// Temperature classifications.
    pub fn temperature_classifications(&self) -> &'static [vitals::TemperatureClassification] {
        vitals::TEMP_CLASSIFICATIONS
    }

    /// All lab thresholds (KDIGO, ESC/EAS, IDF, WHO, EASL, ETA, IOF).
    pub fn lab_thresholds(&self) -> &'static [labs::LabThreshold] {
        labs::ALL_LAB_THRESHOLDS
    }

    // ── Bundled tier access ───────────────────────────────

    /// Drug families (loaded from JSON).
    pub fn drug_families(&self) -> &[loader::DrugFamily] {
        &self.bundled.drug_families
    }

    /// Drug interaction pairs (loaded from JSON).
    pub fn interaction_pairs(&self) -> &[loader::InteractionPair] {
        &self.bundled.interaction_pairs
    }

    /// Cross-reactivity chains (loaded from JSON).
    pub fn cross_reactivity(&self) -> &[loader::CrossReactivityChain] {
        &self.bundled.cross_reactivity
    }

    /// Drug monitoring schedules (loaded from JSON).
    pub fn monitoring_schedules(&self) -> &[loader::MonitoringSchedule] {
        &self.bundled.monitoring_schedules
    }

    // ── Lookup helpers ────────────────────────────────────

    /// Find which drug family a medication belongs to (by generic name).
    pub fn find_drug_family(&self, generic_name: &str) -> Option<&loader::DrugFamily> {
        let normalized = generic_name.trim().to_lowercase();
        self.bundled
            .drug_families
            .iter()
            .find(|f| f.members.iter().any(|m| m == &normalized))
    }

    /// Find a lab threshold by test name (case-insensitive alias match).
    pub fn find_lab_threshold(&self, test_name: &str) -> Option<&'static labs::LabThreshold> {
        labs::find_threshold(test_name)
    }

    /// Find monitoring schedules for a given drug.
    ///
    /// Matches by direct drug name first, then by drug family key.
    /// This allows "lisinopril" to match schedules keyed by "ace_inhibitor".
    pub fn find_monitoring(&self, drug_name: &str) -> Vec<&loader::MonitoringSchedule> {
        let normalized = drug_name.trim().to_lowercase();

        // Direct match by drug name
        let mut results: Vec<&loader::MonitoringSchedule> = self
            .bundled
            .monitoring_schedules
            .iter()
            .filter(|s| s.drug.to_lowercase() == normalized)
            .collect();

        // Family-based match: if drug belongs to a family, also include family-keyed schedules
        if let Some(family) = self.find_drug_family(&normalized) {
            let family_key = family.key.to_lowercase();
            if family_key != normalized {
                let family_schedules = self
                    .bundled
                    .monitoring_schedules
                    .iter()
                    .filter(|s| s.drug.to_lowercase() == family_key);
                results.extend(family_schedules);
            }
        }

        results
    }

    /// Find interactions involving a given drug.
    pub fn find_interactions(&self, drug_name: &str) -> Vec<&loader::InteractionPair> {
        let normalized = drug_name.trim().to_lowercase();
        self.bundled
            .interaction_pairs
            .iter()
            .filter(|p| {
                p.drug_a.to_lowercase() == normalized
                    || p.drug_b.to_lowercase() == normalized
            })
            .collect()
    }

    /// Find cross-reactivity chains for a given allergen.
    pub fn find_cross_reactivity(
        &self,
        allergen: &str,
    ) -> Vec<&loader::CrossReactivityChain> {
        let normalized = allergen.trim().to_lowercase();
        self.bundled
            .cross_reactivity
            .iter()
            .filter(|c| {
                c.primary.to_lowercase() == normalized
                    || c.cross_reactive.to_lowercase() == normalized
            })
            .collect()
    }
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_registry_has_const_data() {
        let reg = InvariantRegistry::empty();
        // Const tier is always available
        assert!(!reg.bp_classifications().is_empty());
        assert!(!reg.hr_classifications().is_empty());
        assert!(!reg.spo2_classifications().is_empty());
        assert!(!reg.bmi_classifications().is_empty());
        assert!(!reg.glucose_classifications().is_empty());
        assert!(!reg.temperature_classifications().is_empty());
        assert!(!reg.lab_thresholds().is_empty());
    }

    #[test]
    fn empty_registry_has_empty_bundled() {
        let reg = InvariantRegistry::empty();
        assert!(reg.drug_families().is_empty());
        assert!(reg.interaction_pairs().is_empty());
        assert!(reg.cross_reactivity().is_empty());
        assert!(reg.monitoring_schedules().is_empty());
    }

    #[test]
    fn load_from_nonexistent_dir_succeeds_empty() {
        let reg = InvariantRegistry::load(Path::new("/nonexistent")).unwrap();
        assert!(reg.drug_families().is_empty());
        // But const tier still works
        assert!(!reg.bp_classifications().is_empty());
    }

    #[test]
    fn find_lab_threshold_works() {
        let reg = InvariantRegistry::empty();
        assert!(reg.find_lab_threshold("HbA1c").is_some());
        assert!(reg.find_lab_threshold("eGFR").is_some());
        assert!(reg.find_lab_threshold("unknown").is_none());
    }

    #[test]
    fn find_drug_family_with_empty_registry() {
        let reg = InvariantRegistry::empty();
        assert!(reg.find_drug_family("atorvastatin").is_none());
    }

    #[test]
    fn find_monitoring_with_empty_registry() {
        let reg = InvariantRegistry::empty();
        assert!(reg.find_monitoring("metformin").is_empty());
    }

    #[test]
    fn find_interactions_with_empty_registry() {
        let reg = InvariantRegistry::empty();
        assert!(reg.find_interactions("warfarin").is_empty());
    }

    #[test]
    fn find_cross_reactivity_with_empty_registry() {
        let reg = InvariantRegistry::empty();
        assert!(reg.find_cross_reactivity("penicillin").is_empty());
    }

    #[test]
    fn registry_is_clone() {
        let reg = InvariantRegistry::empty();
        let reg2 = reg.clone();
        assert_eq!(reg.bp_classifications().len(), reg2.bp_classifications().len());
    }

    #[test]
    fn const_tier_counts() {
        let reg = InvariantRegistry::empty();
        assert_eq!(reg.bp_classifications().len(), 4);
        assert_eq!(reg.hr_classifications().len(), 6);
        assert_eq!(reg.spo2_classifications().len(), 5);
        assert_eq!(reg.bmi_classifications().len(), 6);
        assert_eq!(reg.glucose_classifications().len(), 3);
        assert_eq!(reg.temperature_classifications().len(), 7);
        assert_eq!(reg.lab_thresholds().len(), 10);
    }
}
