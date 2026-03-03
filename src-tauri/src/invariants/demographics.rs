//! ME-04: Demographic-aware clinical classification helpers.
//!
//! Sex-specific hemoglobin tiers (WHO 2024) and ethnicity-specific
//! BMI thresholds (WHO 2004) for personalized clinical enrichment.

use crate::invariants::labs::LabTier;
use crate::invariants::types::InvariantLabel;
use crate::invariants::vitals::BmiClassification;

// ═══════════════════════════════════════════════════════════
// Male hemoglobin tiers — WHO 2024
// Normal lower limit: 13.0 g/dL (vs female 12.0 g/dL)
// Mild anemia: 11.0-13.0 (vs female 11.0-12.0)
// Severe/moderate tiers are identical across sexes.
// ═══════════════════════════════════════════════════════════

pub static HEMOGLOBIN_TIERS_MALE: &[LabTier] = &[
    LabTier {
        label: InvariantLabel {
            key: "hb_severe_anemia",
            en: "Severe anemia",
            fr: "Anémie sévère",
            de: "Schwere Anämie",
        },
        min_value: 0.0,
        max_value: 8.0,
        significance: 2.0,
    },
    LabTier {
        label: InvariantLabel {
            key: "hb_moderate_anemia",
            en: "Moderate anemia",
            fr: "Anémie modérée",
            de: "Mäßige Anämie",
        },
        min_value: 8.0,
        max_value: 11.0,
        significance: 1.3,
    },
    LabTier {
        label: InvariantLabel {
            key: "hb_mild_anemia",
            en: "Mild anemia",
            fr: "Anémie légère",
            de: "Leichte Anämie",
        },
        min_value: 11.0,
        max_value: 13.0, // Male threshold (WHO 2024); female = 12.0
        significance: 0.6,
    },
    LabTier {
        label: InvariantLabel {
            key: "hb_normal",
            en: "Normal hemoglobin",
            fr: "Hémoglobine normale",
            de: "Normales Hämoglobin",
        },
        min_value: 13.0,
        max_value: 17.5,
        significance: 0.2,
    },
    LabTier {
        label: InvariantLabel {
            key: "hb_polycythemia",
            en: "Elevated hemoglobin (polycythemia)",
            fr: "Hémoglobine élevée (polyglobulie)",
            de: "Erhöhtes Hämoglobin (Polyzythämie)",
        },
        min_value: 17.5,
        max_value: f64::MAX,
        significance: 1.0,
    },
];

// ═══════════════════════════════════════════════════════════
// Asian BMI classifications — WHO Expert Consultation 2004
// Lower thresholds: overweight 23.0, obese 27.5
// Applied when patient has South Asian, East Asian, or
// Pacific Islander ethnicity.
// ═══════════════════════════════════════════════════════════

pub const BMI_ASIAN_CLASSIFICATIONS: &[BmiClassification] = &[
    BmiClassification {
        label: InvariantLabel {
            key: "bmi_underweight",
            en: "Underweight",
            fr: "Insuffisance pondérale",
            de: "Untergewicht",
        },
        min_bmi: 0.0,
        max_bmi: 18.5,
        significance: 0.8,
        source: "WHO TRS 894",
    },
    BmiClassification {
        label: InvariantLabel {
            key: "bmi_normal",
            en: "Normal weight",
            fr: "Poids normal",
            de: "Normalgewicht",
        },
        min_bmi: 18.5,
        max_bmi: 23.0, // Asian threshold (vs 25.0 standard)
        significance: 0.2,
        source: "WHO Expert Consultation 2004",
    },
    BmiClassification {
        label: InvariantLabel {
            key: "bmi_overweight",
            en: "Overweight (Asian threshold)",
            fr: "Surpoids (seuil asiatique)",
            de: "Übergewicht (asiatischer Schwellenwert)",
        },
        min_bmi: 23.0,
        max_bmi: 27.5, // Asian obesity threshold
        significance: 0.5,
        source: "WHO Expert Consultation 2004",
    },
    BmiClassification {
        label: InvariantLabel {
            key: "bmi_obese_i",
            en: "Obese Class I (Asian threshold)",
            fr: "Obésité classe I (seuil asiatique)",
            de: "Adipositas Grad I (asiatischer Schwellenwert)",
        },
        min_bmi: 27.5,
        max_bmi: 32.5,
        significance: 0.8,
        source: "WHO Expert Consultation 2004",
    },
    BmiClassification {
        label: InvariantLabel {
            key: "bmi_obese_ii",
            en: "Obese Class II",
            fr: "Obésité classe II",
            de: "Adipositas Grad II",
        },
        min_bmi: 32.5,
        max_bmi: 37.5,
        significance: 1.2,
        source: "WHO Expert Consultation 2004",
    },
    BmiClassification {
        label: InvariantLabel {
            key: "bmi_obese_iii",
            en: "Obese Class III",
            fr: "Obésité classe III",
            de: "Adipositas Grad III",
        },
        min_bmi: 37.5,
        max_bmi: f64::MAX,
        significance: 1.5,
        source: "WHO Expert Consultation 2004",
    },
];

/// Classify BMI using WHO 2004 Asian thresholds.
/// Overweight at 23.0, obese at 27.5 (vs standard 25.0, 30.0).
pub fn classify_bmi_asian(bmi: f64) -> &'static BmiClassification {
    BMI_ASIAN_CLASSIFICATIONS
        .iter()
        .find(|c| bmi >= c.min_bmi && bmi < c.max_bmi)
        .unwrap_or(&BMI_ASIAN_CLASSIFICATIONS[0])
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invariants::labs;

    #[test]
    fn male_hemoglobin_tiers_count() {
        assert_eq!(HEMOGLOBIN_TIERS_MALE.len(), 5);
    }

    #[test]
    fn male_hemoglobin_mild_anemia_at_12_5() {
        // 12.5 g/dL is below male threshold (13.0) = mild anemia
        let tier = HEMOGLOBIN_TIERS_MALE
            .iter()
            .find(|t| 12.5 >= t.min_value && 12.5 < t.max_value)
            .unwrap();
        assert_eq!(tier.label.key, "hb_mild_anemia");
    }

    #[test]
    fn female_hemoglobin_normal_at_12_5() {
        // 12.5 g/dL is above female threshold (12.0) = normal
        let tier = labs::HEMOGLOBIN_TIERS
            .iter()
            .find(|t| 12.5 >= t.min_value && 12.5 < t.max_value)
            .unwrap();
        assert_eq!(tier.label.key, "hb_normal");
    }

    #[test]
    fn male_hemoglobin_normal_at_13_5() {
        let tier = HEMOGLOBIN_TIERS_MALE
            .iter()
            .find(|t| 13.5 >= t.min_value && 13.5 < t.max_value)
            .unwrap();
        assert_eq!(tier.label.key, "hb_normal");
    }

    #[test]
    fn severe_anemia_same_across_sexes() {
        // Severe anemia at 7.5 g/dL — same for both
        let male_tier = HEMOGLOBIN_TIERS_MALE
            .iter()
            .find(|t| 7.5 >= t.min_value && 7.5 < t.max_value)
            .unwrap();
        let female_tier = labs::HEMOGLOBIN_TIERS
            .iter()
            .find(|t| 7.5 >= t.min_value && 7.5 < t.max_value)
            .unwrap();
        assert_eq!(male_tier.label.key, "hb_severe_anemia");
        assert_eq!(female_tier.label.key, "hb_severe_anemia");
    }

    #[test]
    fn asian_bmi_classifications_count() {
        assert_eq!(BMI_ASIAN_CLASSIFICATIONS.len(), 6);
    }

    #[test]
    fn asian_bmi_24_overweight() {
        let tier = classify_bmi_asian(24.0);
        assert_eq!(tier.label.key, "bmi_overweight");
    }

    #[test]
    fn standard_bmi_24_normal() {
        use crate::invariants::vitals;
        let tier = vitals::classify_bmi(24.0);
        assert_eq!(tier.label.key, "bmi_normal");
    }

    #[test]
    fn asian_bmi_28_obese() {
        let tier = classify_bmi_asian(28.0);
        assert_eq!(tier.label.key, "bmi_obese_i");
    }

    #[test]
    fn standard_bmi_28_overweight() {
        use crate::invariants::vitals;
        let tier = vitals::classify_bmi(28.0);
        assert_eq!(tier.label.key, "bmi_overweight");
    }

    #[test]
    fn asian_bmi_22_normal() {
        let tier = classify_bmi_asian(22.0);
        assert_eq!(tier.label.key, "bmi_normal");
    }

    #[test]
    fn asian_bmi_underweight() {
        let tier = classify_bmi_asian(17.0);
        assert_eq!(tier.label.key, "bmi_underweight");
    }
}
