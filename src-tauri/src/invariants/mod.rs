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
pub mod allergens;
pub mod blood_types;

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

    /// Allergen-specific cross-reactivity chains (loaded from JSON).
    /// Includes OAS, food-food, insect venom, and extended latex-fruit chains.
    pub fn allergen_cross_reactivity(&self) -> &[loader::CrossReactivityChain] {
        &self.bundled.allergen_cross_reactivity
    }

    /// Allergen aliases mapping common names to canonical keys (loaded from JSON).
    pub fn allergen_aliases(&self) -> &[loader::AllergenAlias] {
        &self.bundled.allergen_aliases
    }

    // ── Const tier access (allergens) ───────────────────────

    // ── Const tier access (blood types) ─────────────────────

    /// All 8 ABO/Rh blood type references (compiled in).
    pub fn blood_type_references(&self) -> &'static [blood_types::BloodTypeInfo] {
        blood_types::BLOOD_TYPES
    }

    /// Find a blood type by exact key.
    pub fn find_blood_type(&self, key: &str) -> Option<&'static blood_types::BloodTypeInfo> {
        blood_types::find_blood_type(key)
    }

    /// Check RBC transfusion compatibility between donor and recipient.
    pub fn check_rbc_compatibility(
        &self,
        donor: &str,
        recipient: &str,
    ) -> blood_types::Compatibility {
        blood_types::check_rbc_compatibility(donor, recipient)
    }

    // ── Const tier access (allergens) ───────────────────────

    /// All 46 canonical allergen references (compiled in).
    pub fn allergen_references(&self) -> &'static [allergens::CanonicalAllergen] {
        allergens::CANONICAL_ALLERGENS
    }

    /// Find a canonical allergen by exact key.
    pub fn find_canonical_allergen(&self, key: &str) -> Option<&'static allergens::CanonicalAllergen> {
        allergens::find_allergen(key)
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

    /// Find cross-reactivity chains for a given allergen (drug chains only).
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

    /// Find ALL cross-reactivity chains for a given allergen.
    ///
    /// Searches BOTH drug cross-reactivity AND allergen cross-reactivity
    /// (OAS, food-food, insect venom, extended latex-fruit).
    /// Uses canonical key resolution: tries alias lookup first for better matching.
    pub fn find_all_cross_reactivity(
        &self,
        allergen: &str,
    ) -> Vec<&loader::CrossReactivityChain> {
        let normalized = allergen.trim().to_lowercase();

        // Try to resolve to canonical key via alias for precise matching
        let canonical_key = self
            .bundled
            .allergen_aliases
            .iter()
            .find(|a| a.alias.to_lowercase() == normalized)
            .map(|a| a.canonical_key.as_str());

        let search_terms: Vec<&str> = match canonical_key {
            Some(key) => vec![key, &normalized],
            None => vec![&normalized],
        };

        let matches_term = |field: &str| -> bool {
            let lower = field.to_lowercase();
            search_terms
                .iter()
                .any(|term| lower == *term || lower.contains(term))
        };

        // Search drug cross-reactivity chains
        let drug_chains = self
            .bundled
            .cross_reactivity
            .iter()
            .filter(|c| matches_term(&c.primary) || matches_term(&c.cross_reactive));

        // Search allergen cross-reactivity chains
        let allergen_chains = self
            .bundled
            .allergen_cross_reactivity
            .iter()
            .filter(|c| matches_term(&c.primary) || matches_term(&c.cross_reactive));

        drug_chains.chain(allergen_chains).collect()
    }

    /// Classify a free-text allergen string to a canonical allergen.
    ///
    /// Resolution order:
    /// 1. Exact canonical key match (e.g., "drug_beta_lactam")
    /// 2. Alias match (e.g., "penicillin" -> drug_beta_lactam)
    /// 3. Fuzzy label match (substring on EN/FR/DE labels)
    ///
    /// Returns the first match. For autocomplete use `allergens::match_allergens()`.
    pub fn classify_allergen(
        &self,
        free_text: &str,
    ) -> Option<&'static allergens::CanonicalAllergen> {
        let normalized = free_text.trim().to_lowercase();
        if normalized.is_empty() {
            return None;
        }

        // 1. Exact key match
        if let Some(a) = allergens::find_allergen(&normalized) {
            return Some(a);
        }

        // 2. Alias match
        if let Some(alias) = self
            .bundled
            .allergen_aliases
            .iter()
            .find(|a| a.alias.to_lowercase() == normalized)
        {
            if let Some(a) = allergens::find_allergen(&alias.canonical_key) {
                return Some(a);
            }
        }

        // 3. Fuzzy label match (first result)
        let matches = allergens::match_allergens(&normalized);
        matches.into_iter().next()
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
        assert!(reg.allergen_cross_reactivity().is_empty());
        assert!(reg.allergen_aliases().is_empty());
    }

    #[test]
    fn empty_registry_has_allergen_references() {
        let reg = InvariantRegistry::empty();
        // Const tier allergens always available
        assert_eq!(reg.allergen_references().len(), 46);
        assert!(reg.find_canonical_allergen("food_peanut").is_some());
        assert!(reg.find_canonical_allergen("nonexistent").is_none());
    }

    #[test]
    fn registry_has_blood_type_references() {
        let reg = InvariantRegistry::empty();
        assert_eq!(reg.blood_type_references().len(), 8);
    }

    #[test]
    fn registry_find_blood_type() {
        let reg = InvariantRegistry::empty();
        assert!(reg.find_blood_type("o_positive").is_some());
        assert!(reg.find_blood_type("ab_negative").is_some());
        assert!(reg.find_blood_type("unknown").is_none());
    }

    #[test]
    fn registry_check_rbc_compatibility() {
        let reg = InvariantRegistry::empty();
        assert_eq!(
            reg.check_rbc_compatibility("o_negative", "ab_positive"),
            blood_types::Compatibility::Compatible,
        );
        assert_eq!(
            reg.check_rbc_compatibility("b_positive", "a_positive"),
            blood_types::Compatibility::Incompatible,
        );
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

    // ── B3: classify_allergen + find_all_cross_reactivity ──

    #[test]
    fn classify_allergen_exact_key() {
        let reg = InvariantRegistry::empty();
        let result = reg.classify_allergen("food_peanut");
        assert!(result.is_some());
        assert_eq!(result.unwrap().key, "food_peanut");
    }

    #[test]
    fn classify_allergen_fuzzy_label() {
        // Empty registry has no aliases, so falls through to fuzzy label match
        let reg = InvariantRegistry::empty();
        let result = reg.classify_allergen("peanut");
        assert!(result.is_some());
        assert_eq!(result.unwrap().key, "food_peanut");
    }

    #[test]
    fn classify_allergen_empty_returns_none() {
        let reg = InvariantRegistry::empty();
        assert!(reg.classify_allergen("").is_none());
        assert!(reg.classify_allergen("  ").is_none());
    }

    #[test]
    fn classify_allergen_unknown_returns_none() {
        let reg = InvariantRegistry::empty();
        assert!(reg.classify_allergen("xyznonexistent123").is_none());
    }

    #[test]
    fn find_all_cross_reactivity_empty_registry() {
        let reg = InvariantRegistry::empty();
        assert!(reg.find_all_cross_reactivity("penicillin").is_empty());
    }

    #[test]
    fn classify_allergen_with_real_aliases() {
        let candidates = [
            std::path::PathBuf::from("resources"),
            std::path::PathBuf::from("src-tauri/resources"),
        ];
        let Some(dir) = candidates.into_iter().find(|p| p.join("invariants").exists()) else {
            return;
        };
        let reg = InvariantRegistry::load(&dir).unwrap();

        // Alias: "penicillin" -> drug_beta_lactam
        let result = reg.classify_allergen("penicillin");
        assert!(result.is_some(), "penicillin should classify");
        assert_eq!(result.unwrap().key, "drug_beta_lactam");

        // Alias: "amoxicillin" -> drug_beta_lactam
        let result = reg.classify_allergen("amoxicillin");
        assert!(result.is_some(), "amoxicillin should classify");
        assert_eq!(result.unwrap().key, "drug_beta_lactam");

        // Alias: "ibuprofen" -> drug_nsaid
        let result = reg.classify_allergen("ibuprofen");
        assert!(result.is_some(), "ibuprofen should classify");
        assert_eq!(result.unwrap().key, "drug_nsaid");

        // Alias: French "arachide" -> food_peanut
        let result = reg.classify_allergen("arachide");
        assert!(result.is_some(), "arachide should classify");
        assert_eq!(result.unwrap().key, "food_peanut");

        // Alias: "dust mite" -> env_dust_mite
        let result = reg.classify_allergen("dust mite");
        assert!(result.is_some(), "dust mite should classify");
        assert_eq!(result.unwrap().key, "env_dust_mite");
    }

    #[test]
    fn find_all_cross_reactivity_with_real_data() {
        let candidates = [
            std::path::PathBuf::from("resources"),
            std::path::PathBuf::from("src-tauri/resources"),
        ];
        let Some(dir) = candidates.into_iter().find(|p| p.join("invariants").exists()) else {
            return;
        };
        let reg = InvariantRegistry::load(&dir).unwrap();

        // Penicillin: should find drug cross-reactivity chains
        let penicillin_chains = reg.find_all_cross_reactivity("penicillin");
        assert!(
            !penicillin_chains.is_empty(),
            "Penicillin should have cross-reactivity chains"
        );

        // Birch: should find allergen (OAS) cross-reactivity chains
        let birch_chains = reg.find_all_cross_reactivity("env_pollen_tree_birch");
        assert!(
            !birch_chains.is_empty(),
            "Birch pollen should have OAS cross-reactivity chains"
        );

        // Peanut: should find food-food cross-reactivity
        let peanut_chains = reg.find_all_cross_reactivity("food_peanut");
        assert!(
            !peanut_chains.is_empty(),
            "Peanut should have food cross-reactivity chains"
        );
    }
}
