//! ME-03: JSON loader for the bundled tier of the InvariantRegistry.
//!
//! Large/updatable reference data (drug families, interaction pairs,
//! cross-reactivity chains, monitoring schedules) is stored as JSON
//! in `resources/invariants/` and loaded at startup.

use std::path::Path;

use serde::Deserialize;

// ═══════════════════════════════════════════════════════════
// Drug Family — bundled JSON
// ═══════════════════════════════════════════════════════════

/// A drug family with its member medications.
///
/// Used for: interaction detection, cross-reactivity matching,
/// coherence engine drug family grouping.
#[derive(Debug, Clone, Deserialize)]
pub struct DrugFamily {
    /// Family identifier (e.g., "penicillin", "statin", "nsaid").
    pub key: String,
    /// Display name (English — i18n labels loaded separately if needed).
    pub name: String,
    /// Member drug generic names (lowercase, normalized).
    pub members: Vec<String>,
    /// Source guideline.
    pub source: String,
}

// ═══════════════════════════════════════════════════════════
// Interaction Pair — bundled JSON
// ═══════════════════════════════════════════════════════════

/// A known drug-drug interaction pair.
#[derive(Debug, Clone, Deserialize)]
pub struct InteractionPair {
    /// First drug or drug family key.
    pub drug_a: String,
    /// Second drug or drug family key.
    pub drug_b: String,
    /// Severity: "high", "moderate", "low".
    pub severity: String,
    /// Clinical description.
    pub description: String,
    /// Source guideline.
    pub source: String,
}

// ═══════════════════════════════════════════════════════════
// Cross-Reactivity Chain — bundled JSON
// ═══════════════════════════════════════════════════════════

/// A cross-reactivity chain between allergen families.
#[derive(Debug, Clone, Deserialize)]
pub struct CrossReactivityChain {
    /// Primary allergen or family (e.g., "aminopenicillin").
    pub primary: String,
    /// Cross-reactive class (e.g., "aminocephalosporin").
    pub cross_reactive: String,
    /// Cross-reactivity rate description (e.g., "~16.5% skin test positive").
    pub rate: String,
    /// Clinical action recommendation.
    pub action: String,
    /// Source guideline.
    pub source: String,
}

// ═══════════════════════════════════════════════════════════
// Monitoring Schedule — bundled JSON
// ═══════════════════════════════════════════════════════════

/// A drug-to-lab monitoring requirement.
#[derive(Debug, Clone, Deserialize)]
pub struct MonitoringSchedule {
    /// Drug generic name or family key.
    pub drug: String,
    /// Required lab test key (matches LabThreshold.test_key).
    pub lab_test: String,
    /// Monitoring interval in days.
    pub interval_days: u32,
    /// Context for when monitoring applies.
    pub context: String,
    /// Source guideline.
    pub source: String,
}

// ═══════════════════════════════════════════════════════════
// Loader
// ═══════════════════════════════════════════════════════════

/// Error type for invariant loading.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("IO error reading {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("JSON parse error in {path}: {source}")]
    Json {
        path: String,
        source: serde_json::Error,
    },
}

/// Load a JSON file from the invariants resource directory.
fn load_json<T: serde::de::DeserializeOwned>(
    resources_dir: &Path,
    filename: &str,
) -> Result<Vec<T>, LoadError> {
    let path = resources_dir.join("invariants").join(filename);
    if !path.exists() {
        // File not yet created — return empty (graceful degradation)
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| LoadError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    serde_json::from_str(&content).map_err(|e| LoadError::Json {
        path: path.display().to_string(),
        source: e,
    })
}

/// Load all bundled invariant data from the resources directory.
pub fn load_bundled(
    resources_dir: &Path,
) -> Result<BundledInvariants, LoadError> {
    Ok(BundledInvariants {
        drug_families: load_json(resources_dir, "drug_families.json")?,
        interaction_pairs: load_json(resources_dir, "interaction_pairs.json")?,
        cross_reactivity: load_json(resources_dir, "cross_reactivity.json")?,
        monitoring_schedules: load_json(resources_dir, "monitoring_schedules.json")?,
    })
}

/// All bundled invariant data loaded from JSON.
#[derive(Debug, Clone)]
pub struct BundledInvariants {
    pub drug_families: Vec<DrugFamily>,
    pub interaction_pairs: Vec<InteractionPair>,
    pub cross_reactivity: Vec<CrossReactivityChain>,
    pub monitoring_schedules: Vec<MonitoringSchedule>,
}

impl Default for BundledInvariants {
    fn default() -> Self {
        Self {
            drug_families: Vec::new(),
            interaction_pairs: Vec::new(),
            cross_reactivity: Vec::new(),
            monitoring_schedules: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn load_missing_dir_returns_empty() {
        let result = load_bundled(&PathBuf::from("/nonexistent/path"));
        let bundled = result.unwrap();
        assert!(bundled.drug_families.is_empty());
        assert!(bundled.interaction_pairs.is_empty());
        assert!(bundled.cross_reactivity.is_empty());
        assert!(bundled.monitoring_schedules.is_empty());
    }

    #[test]
    fn bundled_default_is_empty() {
        let b = BundledInvariants::default();
        assert!(b.drug_families.is_empty());
        assert!(b.interaction_pairs.is_empty());
        assert!(b.cross_reactivity.is_empty());
        assert!(b.monitoring_schedules.is_empty());
    }

    #[test]
    fn drug_family_deserialize() {
        let json = r#"[{
            "key": "statin",
            "name": "Statins",
            "members": ["atorvastatin", "rosuvastatin", "simvastatin"],
            "source": "WHO EML"
        }]"#;
        let families: Vec<DrugFamily> = serde_json::from_str(json).unwrap();
        assert_eq!(families.len(), 1);
        assert_eq!(families[0].key, "statin");
        assert_eq!(families[0].members.len(), 3);
    }

    #[test]
    fn interaction_pair_deserialize() {
        let json = r#"[{
            "drug_a": "warfarin",
            "drug_b": "aspirin",
            "severity": "high",
            "description": "Increased bleeding risk",
            "source": "WHO EML"
        }]"#;
        let pairs: Vec<InteractionPair> = serde_json::from_str(json).unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].severity, "high");
    }

    #[test]
    fn cross_reactivity_deserialize() {
        let json = r#"[{
            "primary": "aminopenicillin",
            "cross_reactive": "aminocephalosporin",
            "rate": "~16.5% skin test positive",
            "action": "Skin test before use",
            "source": "EAACI 2020"
        }]"#;
        let chains: Vec<CrossReactivityChain> = serde_json::from_str(json).unwrap();
        assert_eq!(chains.len(), 1);
        assert_eq!(chains[0].primary, "aminopenicillin");
    }

    #[test]
    fn monitoring_schedule_deserialize() {
        let json = r#"[{
            "drug": "metformin",
            "lab_test": "hba1c",
            "interval_days": 90,
            "context": "Quarterly when not at target",
            "source": "IDF 2025"
        }]"#;
        let schedules: Vec<MonitoringSchedule> = serde_json::from_str(json).unwrap();
        assert_eq!(schedules.len(), 1);
        assert_eq!(schedules[0].interval_days, 90);
    }

    // ── Integration tests: load from actual resource files ─────────

    /// Get the path to the real resources directory.
    /// Works both from workspace root and from src-tauri/.
    fn real_resources_dir() -> Option<PathBuf> {
        let candidates = [
            PathBuf::from("resources"),          // run from src-tauri/
            PathBuf::from("src-tauri/resources"), // run from workspace root
        ];
        candidates.into_iter().find(|p| p.join("invariants").exists())
    }

    #[test]
    fn load_real_drug_families() {
        let Some(dir) = real_resources_dir() else {
            // Skip if resources not found (CI without resources)
            return;
        };
        let bundled = load_bundled(&dir).unwrap();
        assert!(
            bundled.drug_families.len() >= 15,
            "Expected 15+ drug families, got {}",
            bundled.drug_families.len()
        );
        // Verify key families exist
        let keys: Vec<&str> = bundled.drug_families.iter().map(|f| f.key.as_str()).collect();
        assert!(keys.contains(&"penicillin"), "Missing penicillin family");
        assert!(keys.contains(&"statin"), "Missing statin family");
        assert!(keys.contains(&"nsaid"), "Missing nsaid family");
        assert!(keys.contains(&"ace_inhibitor"), "Missing ace_inhibitor family");
        assert!(keys.contains(&"opioid"), "Missing opioid family");
        assert!(keys.contains(&"doac"), "Missing doac family");
        assert!(keys.contains(&"ssri"), "Missing ssri family");
    }

    #[test]
    fn load_real_interaction_pairs() {
        let Some(dir) = real_resources_dir() else { return };
        let bundled = load_bundled(&dir).unwrap();
        assert!(
            bundled.interaction_pairs.len() >= 15,
            "Expected 15+ interactions, got {}",
            bundled.interaction_pairs.len()
        );
        // Verify critical interaction exists
        let has_warfarin_nsaid = bundled.interaction_pairs.iter().any(|p| {
            (p.drug_a == "warfarin" && p.drug_b == "nsaid")
                || (p.drug_a == "nsaid" && p.drug_b == "warfarin")
        });
        assert!(has_warfarin_nsaid, "Missing warfarin+NSAID interaction");
    }

    #[test]
    fn load_real_cross_reactivity() {
        let Some(dir) = real_resources_dir() else { return };
        let bundled = load_bundled(&dir).unwrap();
        assert!(
            bundled.cross_reactivity.len() >= 5,
            "Expected 5+ cross-reactivity chains, got {}",
            bundled.cross_reactivity.len()
        );
        let has_penicillin = bundled
            .cross_reactivity
            .iter()
            .any(|c| c.primary.contains("penicillin"));
        assert!(has_penicillin, "Missing penicillin cross-reactivity");
    }

    #[test]
    fn load_real_monitoring_schedules() {
        let Some(dir) = real_resources_dir() else { return };
        let bundled = load_bundled(&dir).unwrap();
        assert!(
            bundled.monitoring_schedules.len() >= 15,
            "Expected 15+ monitoring schedules, got {}",
            bundled.monitoring_schedules.len()
        );
        // Verify metformin → HbA1c exists
        let has_metformin_hba1c = bundled
            .monitoring_schedules
            .iter()
            .any(|s| s.drug == "metformin" && s.lab_test == "hba1c");
        assert!(has_metformin_hba1c, "Missing metformin→HbA1c monitoring");
    }

    #[test]
    fn registry_find_drug_family_from_real_data() {
        let Some(dir) = real_resources_dir() else { return };
        let registry = crate::invariants::InvariantRegistry::load(&dir).unwrap();
        // Test with loaded data
        assert!(registry.find_drug_family("atorvastatin").is_some());
        assert_eq!(registry.find_drug_family("atorvastatin").unwrap().key, "statin");
        assert!(registry.find_drug_family("amoxicillin").is_some());
        assert_eq!(registry.find_drug_family("amoxicillin").unwrap().key, "penicillin");
        assert!(registry.find_drug_family("unknown_drug_xyz").is_none());
    }

    #[test]
    fn registry_find_interactions_from_real_data() {
        let Some(dir) = real_resources_dir() else { return };
        let registry = crate::invariants::InvariantRegistry::load(&dir).unwrap();
        let warfarin_interactions = registry.find_interactions("warfarin");
        assert!(
            !warfarin_interactions.is_empty(),
            "Warfarin should have interactions"
        );
    }

    #[test]
    fn registry_find_monitoring_from_real_data() {
        let Some(dir) = real_resources_dir() else { return };
        let registry = crate::invariants::InvariantRegistry::load(&dir).unwrap();
        let metformin_monitoring = registry.find_monitoring("metformin");
        assert!(
            metformin_monitoring.len() >= 2,
            "Metformin should have HbA1c + eGFR monitoring, got {}",
            metformin_monitoring.len()
        );
    }
}
