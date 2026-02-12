use serde::{Deserialize, Serialize};

use super::types::CoherenceError;

/// Plausible dose range for a medication (loaded from dose_ranges.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoseRange {
    pub generic_name: String,
    pub min_single_dose_mg: f64,
    pub max_single_dose_mg: f64,
    pub max_daily_dose_mg: f64,
    pub common_doses: Vec<String>,
    pub route: String,
}

/// Brand-to-generic medication mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicationAlias {
    pub generic_name: String,
    pub brand_name: String,
    pub country: String,
}

/// Loaded reference data for coherence checks.
pub struct CoherenceReferenceData {
    pub medication_aliases: Vec<MedicationAlias>,
    pub dose_ranges: Vec<DoseRange>,
}

impl CoherenceReferenceData {
    /// Load reference data from bundled JSON files.
    pub fn load(resources_dir: &std::path::Path) -> Result<Self, CoherenceError> {
        let aliases_path = resources_dir.join("medication_aliases.json");
        let doses_path = resources_dir.join("dose_ranges.json");

        let aliases_json = std::fs::read_to_string(&aliases_path).map_err(|e| {
            CoherenceError::ReferenceDataLoad(aliases_path.display().to_string(), e.to_string())
        })?;
        let medication_aliases: Vec<MedicationAlias> =
            serde_json::from_str(&aliases_json).map_err(|e| {
                CoherenceError::ReferenceDataParse(
                    "medication_aliases.json".into(),
                    e.to_string(),
                )
            })?;

        let doses_json = std::fs::read_to_string(&doses_path).map_err(|e| {
            CoherenceError::ReferenceDataLoad(doses_path.display().to_string(), e.to_string())
        })?;
        let dose_ranges: Vec<DoseRange> = serde_json::from_str(&doses_json).map_err(|e| {
            CoherenceError::ReferenceDataParse("dose_ranges.json".into(), e.to_string())
        })?;

        Ok(Self {
            medication_aliases,
            dose_ranges,
        })
    }

    /// Create reference data for tests (no file I/O).
    pub fn load_test() -> Self {
        Self {
            medication_aliases: vec![
                MedicationAlias {
                    generic_name: "metformin".into(),
                    brand_name: "Glucophage".into(),
                    country: "US".into(),
                },
                MedicationAlias {
                    generic_name: "metformin".into(),
                    brand_name: "Fortamet".into(),
                    country: "US".into(),
                },
                MedicationAlias {
                    generic_name: "atorvastatin".into(),
                    brand_name: "Lipitor".into(),
                    country: "US".into(),
                },
                MedicationAlias {
                    generic_name: "lisinopril".into(),
                    brand_name: "Zestril".into(),
                    country: "US".into(),
                },
                MedicationAlias {
                    generic_name: "amoxicillin".into(),
                    brand_name: "Amoxil".into(),
                    country: "US".into(),
                },
            ],
            dose_ranges: vec![
                DoseRange {
                    generic_name: "metformin".into(),
                    min_single_dose_mg: 250.0,
                    max_single_dose_mg: 2000.0,
                    max_daily_dose_mg: 2550.0,
                    common_doses: vec!["500mg".into(), "850mg".into(), "1000mg".into()],
                    route: "oral".into(),
                },
                DoseRange {
                    generic_name: "lisinopril".into(),
                    min_single_dose_mg: 2.5,
                    max_single_dose_mg: 40.0,
                    max_daily_dose_mg: 80.0,
                    common_doses: vec!["5mg".into(), "10mg".into(), "20mg".into(), "40mg".into()],
                    route: "oral".into(),
                },
                DoseRange {
                    generic_name: "atorvastatin".into(),
                    min_single_dose_mg: 10.0,
                    max_single_dose_mg: 80.0,
                    max_daily_dose_mg: 80.0,
                    common_doses: vec!["10mg".into(), "20mg".into(), "40mg".into(), "80mg".into()],
                    route: "oral".into(),
                },
                DoseRange {
                    generic_name: "amoxicillin".into(),
                    min_single_dose_mg: 250.0,
                    max_single_dose_mg: 1000.0,
                    max_daily_dose_mg: 3000.0,
                    common_doses: vec!["250mg".into(), "500mg".into(), "875mg".into()],
                    route: "oral".into(),
                },
            ],
        }
    }

    /// Look up the generic name for a brand name.
    pub fn resolve_generic(&self, brand_name: &str) -> Option<&str> {
        let lower = brand_name.to_lowercase();
        self.medication_aliases
            .iter()
            .find(|a| a.brand_name.to_lowercase() == lower)
            .map(|a| a.generic_name.as_str())
    }

    /// Look up dose range for a generic medication name.
    pub fn get_dose_range(&self, generic_name: &str) -> Option<&DoseRange> {
        let lower = generic_name.to_lowercase();
        self.dose_ranges
            .iter()
            .find(|d| d.generic_name.to_lowercase() == lower)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_generic_glucophage() {
        let ref_data = CoherenceReferenceData::load_test();
        assert_eq!(ref_data.resolve_generic("Glucophage"), Some("metformin"));
    }

    #[test]
    fn resolve_generic_case_insensitive() {
        let ref_data = CoherenceReferenceData::load_test();
        assert_eq!(ref_data.resolve_generic("glucophage"), Some("metformin"));
        assert_eq!(ref_data.resolve_generic("LIPITOR"), Some("atorvastatin"));
    }

    #[test]
    fn resolve_generic_unknown() {
        let ref_data = CoherenceReferenceData::load_test();
        assert_eq!(ref_data.resolve_generic("UnknownBrand"), None);
    }

    #[test]
    fn get_dose_range_metformin() {
        let ref_data = CoherenceReferenceData::load_test();
        let range = ref_data.get_dose_range("metformin").unwrap();
        assert_eq!(range.min_single_dose_mg, 250.0);
        assert_eq!(range.max_single_dose_mg, 2000.0);
    }

    #[test]
    fn get_dose_range_unknown() {
        let ref_data = CoherenceReferenceData::load_test();
        assert!(ref_data.get_dose_range("unknown_drug").is_none());
    }
}
