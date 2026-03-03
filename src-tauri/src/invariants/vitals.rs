//! ME-03 I-VIT: Vital sign clinical thresholds.
//!
//! All thresholds from international guidelines (ISH, ESC, BTS, WHO, GLIM).
//! Const data — compiled into the binary, zero I/O at runtime.
//!
//! ## Significance scaling (S factor in ME-01 meaning equation)
//!
//! - **0.2**: Normal range, low clinical concern
//! - **0.4–0.6**: Mild abnormality, monitoring recommended
//! - **0.8–1.2**: Moderate concern, intervention may be needed
//! - **1.3–1.5**: High concern, requires clinical attention
//! - **1.8–2.0**: Critical/extreme, immediate action needed

use crate::invariants::types::InvariantLabel;

// ═══════════════════════════════════════════════════════════
// Blood Pressure — ISH 2020 Global Hypertension Practice Guidelines
// ═══════════════════════════════════════════════════════════

/// A blood pressure classification tier.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BpClassification {
    pub label: InvariantLabel,
    /// Systolic lower bound (inclusive). 0 means no lower bound.
    pub systolic_min: u16,
    /// Systolic upper bound (exclusive). u16::MAX means no upper bound.
    pub systolic_max: u16,
    /// Diastolic lower bound (inclusive).
    pub diastolic_min: u16,
    /// Diastolic upper bound (exclusive).
    pub diastolic_max: u16,
    /// Clinical significance factor (S in meaning equation).
    pub significance: f64,
    pub source: &'static str,
}

pub const BP_CLASSIFICATIONS: &[BpClassification] = &[
    BpClassification {
        label: InvariantLabel {
            key: "bp_normal",
            en: "Normal blood pressure",
            fr: "Pression artérielle normale",
            de: "Normaler Blutdruck",
        },
        systolic_min: 0,
        systolic_max: 130,
        diastolic_min: 0,
        diastolic_max: 85,
        significance: 0.2,
        source: "ISH 2020",
    },
    BpClassification {
        label: InvariantLabel {
            key: "bp_high_normal",
            en: "High-normal blood pressure",
            fr: "Pression artérielle normale haute",
            de: "Hochnormaler Blutdruck",
        },
        systolic_min: 130,
        systolic_max: 140,
        diastolic_min: 85,
        diastolic_max: 90,
        significance: 0.4,
        source: "ISH 2020",
    },
    BpClassification {
        label: InvariantLabel {
            key: "bp_grade_1_htn",
            en: "Grade 1 Hypertension",
            fr: "Hypertension de grade 1",
            de: "Hypertonie Grad 1",
        },
        systolic_min: 140,
        systolic_max: 160,
        diastolic_min: 90,
        diastolic_max: 100,
        significance: 0.6,
        source: "ISH 2020",
    },
    BpClassification {
        label: InvariantLabel {
            key: "bp_grade_2_htn",
            en: "Grade 2 Hypertension",
            fr: "Hypertension de grade 2",
            de: "Hypertonie Grad 2",
        },
        systolic_min: 160,
        systolic_max: u16::MAX,
        diastolic_min: 100,
        diastolic_max: u16::MAX,
        significance: 0.9,
        source: "ISH 2020",
    },
];

/// Classify a blood pressure reading.
///
/// ISH 2020 rule: classification uses the HIGHER category
/// when systolic and diastolic fall in different tiers.
pub fn classify_bp(systolic: u16, diastolic: u16) -> &'static BpClassification {
    // Walk tiers from highest to lowest — return first match on EITHER axis
    for tier in BP_CLASSIFICATIONS.iter().rev() {
        if systolic >= tier.systolic_min || diastolic >= tier.diastolic_min {
            // Check if at least one axis is in this tier
            let sys_match = systolic >= tier.systolic_min && systolic < tier.systolic_max;
            let dia_match = diastolic >= tier.diastolic_min && diastolic < tier.diastolic_max;
            let sys_above = systolic >= tier.systolic_min;
            let dia_above = diastolic >= tier.diastolic_min;

            // ISH: use the higher category
            if sys_match || dia_match || (sys_above && dia_above) {
                return tier;
            }
        }
    }
    // Fallback to normal (should not happen with valid readings)
    &BP_CLASSIFICATIONS[0]
}

// ═══════════════════════════════════════════════════════════
// Heart Rate — ESC 2021 Pacing + ESC 2019 SVT Guidelines
// ═══════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HrClassification {
    pub label: InvariantLabel,
    pub bpm_min: u16,
    pub bpm_max: u16,
    pub significance: f64,
    pub source: &'static str,
}

pub const HR_CLASSIFICATIONS: &[HrClassification] = &[
    HrClassification {
        label: InvariantLabel {
            key: "hr_severe_bradycardia",
            en: "Severe bradycardia",
            fr: "Bradycardie sévère",
            de: "Schwere Bradykardie",
        },
        bpm_min: 0,
        bpm_max: 40,
        significance: 1.8,
        source: "ESC 2021 Pacing",
    },
    HrClassification {
        label: InvariantLabel {
            key: "hr_bradycardia",
            en: "Bradycardia",
            fr: "Bradycardie",
            de: "Bradykardie",
        },
        bpm_min: 40,
        bpm_max: 50,
        significance: 1.2,
        source: "ESC 2021 Pacing",
    },
    HrClassification {
        label: InvariantLabel {
            key: "hr_low_normal",
            en: "Low-normal heart rate",
            fr: "Fréquence cardiaque normale basse",
            de: "Niedrig-normale Herzfrequenz",
        },
        bpm_min: 50,
        bpm_max: 60,
        significance: 0.3,
        source: "ESC 2021 Pacing",
    },
    HrClassification {
        label: InvariantLabel {
            key: "hr_normal",
            en: "Normal heart rate",
            fr: "Fréquence cardiaque normale",
            de: "Normale Herzfrequenz",
        },
        bpm_min: 60,
        bpm_max: 101,
        significance: 0.2,
        source: "ESC 2021 Pacing",
    },
    HrClassification {
        label: InvariantLabel {
            key: "hr_tachycardia",
            en: "Tachycardia",
            fr: "Tachycardie",
            de: "Tachykardie",
        },
        bpm_min: 101,
        bpm_max: 151,
        significance: 1.0,
        source: "ESC 2019 SVT",
    },
    HrClassification {
        label: InvariantLabel {
            key: "hr_severe_tachycardia",
            en: "Severe tachycardia",
            fr: "Tachycardie sévère",
            de: "Schwere Tachykardie",
        },
        bpm_min: 151,
        bpm_max: u16::MAX,
        significance: 1.8,
        source: "ESC 2019 SVT",
    },
];

/// Classify a resting heart rate.
pub fn classify_hr(bpm: u16) -> &'static HrClassification {
    HR_CLASSIFICATIONS
        .iter()
        .find(|c| bpm >= c.bpm_min && bpm < c.bpm_max)
        .unwrap_or(&HR_CLASSIFICATIONS[3]) // fallback to normal
}

// ═══════════════════════════════════════════════════════════
// SpO2 — BTS 2017 Guideline for Oxygen Use
// ═══════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spo2Classification {
    pub label: InvariantLabel,
    pub min_pct: u8,
    pub max_pct: u8,
    pub significance: f64,
    pub source: &'static str,
}

pub const SPO2_CLASSIFICATIONS: &[Spo2Classification] = &[
    Spo2Classification {
        label: InvariantLabel {
            key: "spo2_hypoxemia",
            en: "Hypoxemia - supplemental oxygen indicated",
            fr: "Hypoxémie - oxygène supplémentaire indiqué",
            de: "Hypoxämie - zusätzlicher Sauerstoff indiziert",
        },
        min_pct: 0,
        max_pct: 90,
        significance: 1.8,
        source: "BTS 2017, WHO",
    },
    Spo2Classification {
        label: InvariantLabel {
            key: "spo2_copd_target",
            en: "COPD target range (88-92%)",
            fr: "Plage cible BPCO (88-92%)",
            de: "COPD-Zielbereich (88-92%)",
        },
        min_pct: 88,
        max_pct: 93,
        significance: 0.8,
        source: "BTS 2017",
    },
    Spo2Classification {
        label: InvariantLabel {
            key: "spo2_low",
            en: "Below action threshold - investigate",
            fr: "Sous le seuil d'action - investiguer",
            de: "Unter der Handlungsschwelle - untersuchen",
        },
        min_pct: 90,
        max_pct: 94,
        significance: 1.2,
        source: "BTS 2017",
    },
    Spo2Classification {
        label: InvariantLabel {
            key: "spo2_borderline",
            en: "Lower limit of normal",
            fr: "Limite inférieure de la normale",
            de: "Untere Normalgrenze",
        },
        min_pct: 94,
        max_pct: 95,
        significance: 0.5,
        source: "BTS 2017",
    },
    Spo2Classification {
        label: InvariantLabel {
            key: "spo2_normal",
            en: "Normal oxygen saturation",
            fr: "Saturation en oxygène normale",
            de: "Normale Sauerstoffsättigung",
        },
        min_pct: 95,
        max_pct: 101,
        significance: 0.2,
        source: "BTS 2017",
    },
];

/// Classify SpO2 for a non-COPD patient.
///
/// COPD patients use a different target range (88-92%), but
/// that requires knowing the patient's COPD status (diagnosis context).
/// This function classifies for general population.
pub fn classify_spo2(pct: u8) -> &'static Spo2Classification {
    if pct < 90 {
        return &SPO2_CLASSIFICATIONS[0]; // Hypoxemia
    }
    if pct < 94 {
        return &SPO2_CLASSIFICATIONS[2]; // Low
    }
    if pct < 95 {
        return &SPO2_CLASSIFICATIONS[3]; // Borderline
    }
    &SPO2_CLASSIFICATIONS[4] // Normal
}

// ═══════════════════════════════════════════════════════════
// BMI — WHO Technical Report Series 894 (2000)
//        WHO Expert Consultation, Lancet 2004 (Asian cut-points)
// ═══════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BmiClassification {
    pub label: InvariantLabel,
    /// Lower bound (inclusive).
    pub min_bmi: f64,
    /// Upper bound (exclusive). f64::MAX means no upper bound.
    pub max_bmi: f64,
    pub significance: f64,
    pub source: &'static str,
}

pub const BMI_CLASSIFICATIONS: &[BmiClassification] = &[
    BmiClassification {
        label: InvariantLabel {
            key: "bmi_underweight",
            en: "Underweight",
            fr: "Insuffisance pondérale",
            de: "Untergewicht",
        },
        min_bmi: 0.0,
        max_bmi: 18.5,
        significance: 1.2,
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
        max_bmi: 25.0,
        significance: 0.2,
        source: "WHO TRS 894",
    },
    BmiClassification {
        label: InvariantLabel {
            key: "bmi_overweight",
            en: "Overweight (pre-obese)",
            fr: "Surpoids (pré-obésité)",
            de: "Übergewicht (Präadipositas)",
        },
        min_bmi: 25.0,
        max_bmi: 30.0,
        significance: 0.5,
        source: "WHO TRS 894",
    },
    BmiClassification {
        label: InvariantLabel {
            key: "bmi_obese_i",
            en: "Obese Class I",
            fr: "Obésité classe I",
            de: "Adipositas Grad I",
        },
        min_bmi: 30.0,
        max_bmi: 35.0,
        significance: 0.8,
        source: "WHO TRS 894",
    },
    BmiClassification {
        label: InvariantLabel {
            key: "bmi_obese_ii",
            en: "Obese Class II",
            fr: "Obésité classe II",
            de: "Adipositas Grad II",
        },
        min_bmi: 35.0,
        max_bmi: 40.0,
        significance: 1.2,
        source: "WHO TRS 894",
    },
    BmiClassification {
        label: InvariantLabel {
            key: "bmi_obese_iii",
            en: "Obese Class III",
            fr: "Obésité classe III",
            de: "Adipositas Grad III",
        },
        min_bmi: 40.0,
        max_bmi: f64::MAX,
        significance: 1.5,
        source: "WHO TRS 894",
    },
];

/// WHO 2004 Asian overweight threshold (kg/m²).
pub const BMI_ASIAN_OVERWEIGHT: f64 = 23.0;
/// WHO 2004 Asian obese threshold (kg/m²).
pub const BMI_ASIAN_OBESE: f64 = 27.5;

/// Classify BMI using WHO standard (18.5, 25.0, 30.0, 35.0, 40.0) cut-points.
///
/// WHO 2004 Asian thresholds (23.0 overweight, 27.5 obese) are available as
/// constants `BMI_ASIAN_OVERWEIGHT` and `BMI_ASIAN_OBESE` but require patient
/// ethnicity context. Applied in the enrichment layer, not here.
pub fn classify_bmi(bmi: f64) -> &'static BmiClassification {
    BMI_CLASSIFICATIONS
        .iter()
        .find(|c| bmi >= c.min_bmi && bmi < c.max_bmi)
        .unwrap_or(&BMI_CLASSIFICATIONS[0])
}

/// Compute BMI from weight (kg) and height (cm).
/// Returns None if height is zero or negative.
pub fn compute_bmi(weight_kg: f64, height_cm: f64) -> Option<f64> {
    if height_cm <= 0.0 {
        return None;
    }
    let height_m = height_cm / 100.0;
    Some(weight_kg / (height_m * height_m))
}

// ═══════════════════════════════════════════════════════════
// Fasting Glucose — WHO 2006 Diagnostic Criteria
// ═══════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlucoseClassification {
    pub label: InvariantLabel,
    /// Lower bound in mmol/L (inclusive).
    pub min_mmol: f64,
    /// Upper bound in mmol/L (exclusive).
    pub max_mmol: f64,
    pub significance: f64,
    pub source: &'static str,
}

/// Conversion factor: mmol/L × 18.0 = mg/dL
pub const GLUCOSE_MMOL_TO_MGDL: f64 = 18.0;

pub const GLUCOSE_CLASSIFICATIONS: &[GlucoseClassification] = &[
    GlucoseClassification {
        label: InvariantLabel {
            key: "glucose_normal",
            en: "Normal fasting glucose",
            fr: "Glycémie à jeun normale",
            de: "Normale Nüchternglukose",
        },
        min_mmol: 0.0,
        max_mmol: 6.1,
        significance: 0.2,
        source: "WHO 2006",
    },
    GlucoseClassification {
        label: InvariantLabel {
            key: "glucose_ifg",
            en: "Impaired fasting glucose (pre-diabetes)",
            fr: "Hyperglycémie modérée à jeun (pré-diabète)",
            de: "Gestörte Nüchternglukose (Prädiabetes)",
        },
        min_mmol: 6.1,
        max_mmol: 7.0,
        significance: 0.8,
        source: "WHO 2006",
    },
    GlucoseClassification {
        label: InvariantLabel {
            key: "glucose_diabetes",
            en: "Diabetes range (fasting)",
            fr: "Plage diabétique (à jeun)",
            de: "Diabetesbereich (nüchtern)",
        },
        min_mmol: 7.0,
        max_mmol: f64::MAX,
        significance: 1.5,
        source: "WHO 2006",
    },
];

/// Classify fasting glucose in mmol/L (WHO thresholds).
pub fn classify_glucose_mmol(value: f64) -> &'static GlucoseClassification {
    GLUCOSE_CLASSIFICATIONS
        .iter()
        .find(|c| value >= c.min_mmol && value < c.max_mmol)
        .unwrap_or(&GLUCOSE_CLASSIFICATIONS[0])
}

/// Classify fasting glucose in mg/dL (converts to mmol/L, then classifies).
pub fn classify_glucose_mgdl(value: f64) -> &'static GlucoseClassification {
    classify_glucose_mmol(value / GLUCOSE_MMOL_TO_MGDL)
}

// ═══════════════════════════════════════════════════════════
// Temperature — Clinical standard
// ═══════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemperatureClassification {
    pub label: InvariantLabel,
    pub min_celsius: f64,
    pub max_celsius: f64,
    pub significance: f64,
    pub source: &'static str,
}

pub const TEMP_CLASSIFICATIONS: &[TemperatureClassification] = &[
    TemperatureClassification {
        label: InvariantLabel {
            key: "temp_hypothermia",
            en: "Hypothermia",
            fr: "Hypothermie",
            de: "Hypothermie",
        },
        min_celsius: 0.0,
        max_celsius: 35.0,
        significance: 1.5,
        source: "WHO",
    },
    TemperatureClassification {
        label: InvariantLabel {
            key: "temp_low",
            en: "Below normal temperature",
            fr: "Température inférieure à la normale",
            de: "Untertemperatur",
        },
        min_celsius: 35.0,
        max_celsius: 36.1,
        significance: 0.5,
        source: "Clinical standard",
    },
    TemperatureClassification {
        label: InvariantLabel {
            key: "temp_normal",
            en: "Normal temperature",
            fr: "Température normale",
            de: "Normaltemperatur",
        },
        min_celsius: 36.1,
        max_celsius: 37.2,
        significance: 0.2,
        source: "Clinical standard",
    },
    TemperatureClassification {
        label: InvariantLabel {
            key: "temp_low_grade_fever",
            en: "Low-grade fever",
            fr: "Fièvre légère",
            de: "Leichtes Fieber",
        },
        min_celsius: 37.2,
        max_celsius: 38.0,
        significance: 0.5,
        source: "Clinical standard",
    },
    TemperatureClassification {
        label: InvariantLabel {
            key: "temp_fever",
            en: "Fever",
            fr: "Fièvre",
            de: "Fieber",
        },
        min_celsius: 38.0,
        max_celsius: 39.0,
        significance: 1.0,
        source: "WHO",
    },
    TemperatureClassification {
        label: InvariantLabel {
            key: "temp_high_fever",
            en: "High fever",
            fr: "Fièvre élevée",
            de: "Hohes Fieber",
        },
        min_celsius: 39.0,
        max_celsius: 41.0,
        significance: 1.5,
        source: "WHO",
    },
    TemperatureClassification {
        label: InvariantLabel {
            key: "temp_hyperthermia",
            en: "Hyperthermia - emergency",
            fr: "Hyperthermie - urgence",
            de: "Hyperthermie - Notfall",
        },
        min_celsius: 41.0,
        max_celsius: f64::MAX,
        significance: 2.0,
        source: "WHO",
    },
];

/// Classify body temperature in °C.
pub fn classify_temperature(celsius: f64) -> &'static TemperatureClassification {
    TEMP_CLASSIFICATIONS
        .iter()
        .find(|c| celsius >= c.min_celsius && celsius < c.max_celsius)
        .unwrap_or(&TEMP_CLASSIFICATIONS[0])
}

// ═══════════════════════════════════════════════════════════
// Trend thresholds — ISH 2020, ESC 2018, GLIM 2019
// ═══════════════════════════════════════════════════════════

/// BP trend: systolic increase ≥20 mmHg over 6-12 months.
pub const BP_SYSTOLIC_TREND_THRESHOLD: f64 = 20.0;
/// BP trend: diastolic increase ≥10 mmHg over 6-12 months.
pub const BP_DIASTOLIC_TREND_THRESHOLD: f64 = 10.0;

/// Orthostatic hypotension (ESC 2018 Syncope):
/// SBP drop ≥20 OR DBP drop ≥10 OR absolute SBP <90, within 3 min of standing.
pub const ORTHOSTATIC_SBP_DROP: f64 = 20.0;
pub const ORTHOSTATIC_DBP_DROP: f64 = 10.0;
pub const ORTHOSTATIC_SBP_FLOOR: f64 = 90.0;

/// Weight loss thresholds (GLIM 2019):
/// >5% in ≤6 months = moderate; >10% in ≤6 months = severe.
pub const WEIGHT_LOSS_MODERATE_PCT: f64 = 5.0;
pub const WEIGHT_LOSS_SEVERE_PCT: f64 = 10.0;
/// Weight gain threshold: ≥5% in 6 months.
pub const WEIGHT_GAIN_THRESHOLD_PCT: f64 = 5.0;

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // --- Blood Pressure ---

    #[test]
    fn bp_normal() {
        let c = classify_bp(120, 75);
        assert_eq!(c.label.key, "bp_normal");
    }

    #[test]
    fn bp_high_normal_systolic() {
        let c = classify_bp(135, 80);
        assert_eq!(c.label.key, "bp_high_normal");
    }

    #[test]
    fn bp_high_normal_diastolic() {
        let c = classify_bp(125, 87);
        assert_eq!(c.label.key, "bp_high_normal");
    }

    #[test]
    fn bp_grade_1_systolic() {
        let c = classify_bp(145, 80);
        assert_eq!(c.label.key, "bp_grade_1_htn");
    }

    #[test]
    fn bp_grade_1_diastolic() {
        let c = classify_bp(125, 95);
        assert_eq!(c.label.key, "bp_grade_1_htn");
    }

    #[test]
    fn bp_grade_2_systolic() {
        let c = classify_bp(170, 85);
        assert_eq!(c.label.key, "bp_grade_2_htn");
    }

    #[test]
    fn bp_grade_2_diastolic() {
        let c = classify_bp(135, 105);
        assert_eq!(c.label.key, "bp_grade_2_htn");
    }

    #[test]
    fn bp_higher_category_wins() {
        // ISH 2020: when SBP and DBP fall in different tiers, use the higher tier
        let c = classify_bp(145, 105); // SBP=Grade1, DBP=Grade2 → Grade 2
        assert_eq!(c.label.key, "bp_grade_2_htn");
    }

    #[test]
    fn bp_boundary_140_90() {
        let c = classify_bp(140, 90);
        assert_eq!(c.label.key, "bp_grade_1_htn");
    }

    #[test]
    fn bp_boundary_130_85() {
        let c = classify_bp(130, 85);
        assert_eq!(c.label.key, "bp_high_normal");
    }

    #[test]
    fn bp_significance_increases_with_grade() {
        let normal = classify_bp(120, 75);
        let g1 = classify_bp(145, 92);
        let g2 = classify_bp(170, 105);
        assert!(normal.significance < g1.significance);
        assert!(g1.significance < g2.significance);
    }

    // --- Heart Rate ---

    #[test]
    fn hr_severe_bradycardia() {
        assert_eq!(classify_hr(35).label.key, "hr_severe_bradycardia");
    }

    #[test]
    fn hr_bradycardia() {
        assert_eq!(classify_hr(45).label.key, "hr_bradycardia");
    }

    #[test]
    fn hr_low_normal() {
        assert_eq!(classify_hr(55).label.key, "hr_low_normal");
    }

    #[test]
    fn hr_normal() {
        assert_eq!(classify_hr(72).label.key, "hr_normal");
    }

    #[test]
    fn hr_tachycardia() {
        assert_eq!(classify_hr(110).label.key, "hr_tachycardia");
    }

    #[test]
    fn hr_severe_tachycardia() {
        assert_eq!(classify_hr(160).label.key, "hr_severe_tachycardia");
    }

    #[test]
    fn hr_boundary_100() {
        assert_eq!(classify_hr(100).label.key, "hr_normal");
    }

    #[test]
    fn hr_boundary_101() {
        assert_eq!(classify_hr(101).label.key, "hr_tachycardia");
    }

    // --- SpO2 ---

    #[test]
    fn spo2_hypoxemia() {
        assert_eq!(classify_spo2(85).label.key, "spo2_hypoxemia");
        assert_eq!(classify_spo2(89).label.key, "spo2_hypoxemia");
    }

    #[test]
    fn spo2_low() {
        assert_eq!(classify_spo2(90).label.key, "spo2_low");
        assert_eq!(classify_spo2(93).label.key, "spo2_low");
    }

    #[test]
    fn spo2_borderline() {
        assert_eq!(classify_spo2(94).label.key, "spo2_borderline");
    }

    #[test]
    fn spo2_normal() {
        assert_eq!(classify_spo2(95).label.key, "spo2_normal");
        assert_eq!(classify_spo2(98).label.key, "spo2_normal");
        assert_eq!(classify_spo2(100).label.key, "spo2_normal");
    }

    #[test]
    fn spo2_action_threshold_is_94() {
        // BTS 2017: 94% is the action threshold
        let at_94 = classify_spo2(94);
        let below_94 = classify_spo2(93);
        assert_ne!(at_94.label.key, below_94.label.key);
    }

    // --- BMI ---

    #[test]
    fn bmi_underweight() {
        assert_eq!(classify_bmi(16.5).label.key, "bmi_underweight");
    }

    #[test]
    fn bmi_normal() {
        assert_eq!(classify_bmi(22.0).label.key, "bmi_normal");
    }

    #[test]
    fn bmi_overweight() {
        assert_eq!(classify_bmi(27.0).label.key, "bmi_overweight");
    }

    #[test]
    fn bmi_obese_i() {
        assert_eq!(classify_bmi(32.0).label.key, "bmi_obese_i");
    }

    #[test]
    fn bmi_obese_ii() {
        assert_eq!(classify_bmi(37.0).label.key, "bmi_obese_ii");
    }

    #[test]
    fn bmi_obese_iii() {
        assert_eq!(classify_bmi(42.0).label.key, "bmi_obese_iii");
    }

    #[test]
    fn bmi_boundary_25() {
        assert_eq!(classify_bmi(25.0).label.key, "bmi_overweight");
    }

    #[test]
    fn bmi_boundary_18_5() {
        assert_eq!(classify_bmi(18.5).label.key, "bmi_normal");
    }

    #[test]
    fn bmi_asian_thresholds() {
        assert!((BMI_ASIAN_OVERWEIGHT - 23.0).abs() < f64::EPSILON);
        assert!((BMI_ASIAN_OBESE - 27.5).abs() < f64::EPSILON);
    }

    #[test]
    fn compute_bmi_standard() {
        // 70kg, 175cm → BMI ≈ 22.86
        let bmi = compute_bmi(70.0, 175.0).unwrap();
        assert!((bmi - 22.86).abs() < 0.01);
    }

    #[test]
    fn compute_bmi_zero_height() {
        assert!(compute_bmi(70.0, 0.0).is_none());
    }

    #[test]
    fn compute_bmi_negative_height() {
        assert!(compute_bmi(70.0, -5.0).is_none());
    }

    // --- Glucose ---

    #[test]
    fn glucose_normal_mmol() {
        assert_eq!(classify_glucose_mmol(5.0).label.key, "glucose_normal");
    }

    #[test]
    fn glucose_ifg_mmol() {
        assert_eq!(classify_glucose_mmol(6.5).label.key, "glucose_ifg");
    }

    #[test]
    fn glucose_diabetes_mmol() {
        assert_eq!(classify_glucose_mmol(8.0).label.key, "glucose_diabetes");
    }

    #[test]
    fn glucose_boundary_6_1_is_ifg() {
        // WHO IFG threshold: 6.1 mmol/L
        assert_eq!(classify_glucose_mmol(6.1).label.key, "glucose_ifg");
    }

    #[test]
    fn glucose_boundary_7_0_is_diabetes() {
        assert_eq!(classify_glucose_mmol(7.0).label.key, "glucose_diabetes");
    }

    #[test]
    fn glucose_mgdl_conversion() {
        // 110 mg/dL = 6.11 mmol/L → IFG
        assert_eq!(classify_glucose_mgdl(110.0).label.key, "glucose_ifg");
        // 100 mg/dL = 5.56 mmol/L → Normal (WHO, not ADA)
        assert_eq!(classify_glucose_mgdl(100.0).label.key, "glucose_normal");
    }

    // --- Temperature ---

    #[test]
    fn temp_hypothermia() {
        assert_eq!(classify_temperature(34.0).label.key, "temp_hypothermia");
    }

    #[test]
    fn temp_normal() {
        assert_eq!(classify_temperature(36.8).label.key, "temp_normal");
    }

    #[test]
    fn temp_fever() {
        assert_eq!(classify_temperature(38.5).label.key, "temp_fever");
    }

    #[test]
    fn temp_high_fever() {
        assert_eq!(classify_temperature(39.5).label.key, "temp_high_fever");
    }

    #[test]
    fn temp_hyperthermia() {
        assert_eq!(classify_temperature(41.5).label.key, "temp_hyperthermia");
    }

    // --- i18n ---

    #[test]
    fn vitals_i18n_coverage() {
        // Verify all BP labels have all 3 languages populated
        for bp in BP_CLASSIFICATIONS {
            assert!(!bp.label.en.is_empty(), "BP {} missing EN", bp.label.key);
            assert!(!bp.label.fr.is_empty(), "BP {} missing FR", bp.label.key);
            assert!(!bp.label.de.is_empty(), "BP {} missing DE", bp.label.key);
        }
        for hr in HR_CLASSIFICATIONS {
            assert!(!hr.label.en.is_empty(), "HR {} missing EN", hr.label.key);
            assert!(!hr.label.fr.is_empty(), "HR {} missing FR", hr.label.key);
            assert!(!hr.label.de.is_empty(), "HR {} missing DE", hr.label.key);
        }
        for spo2 in SPO2_CLASSIFICATIONS {
            assert!(!spo2.label.en.is_empty(), "SpO2 {} missing EN", spo2.label.key);
            assert!(!spo2.label.fr.is_empty(), "SpO2 {} missing FR", spo2.label.key);
            assert!(!spo2.label.de.is_empty(), "SpO2 {} missing DE", spo2.label.key);
        }
        for bmi in BMI_CLASSIFICATIONS {
            assert!(!bmi.label.en.is_empty(), "BMI {} missing EN", bmi.label.key);
            assert!(!bmi.label.fr.is_empty(), "BMI {} missing FR", bmi.label.key);
            assert!(!bmi.label.de.is_empty(), "BMI {} missing DE", bmi.label.key);
        }
        for g in GLUCOSE_CLASSIFICATIONS {
            assert!(!g.label.en.is_empty(), "Glucose {} missing EN", g.label.key);
            assert!(!g.label.fr.is_empty(), "Glucose {} missing FR", g.label.key);
            assert!(!g.label.de.is_empty(), "Glucose {} missing DE", g.label.key);
        }
        for t in TEMP_CLASSIFICATIONS {
            assert!(!t.label.en.is_empty(), "Temp {} missing EN", t.label.key);
            assert!(!t.label.fr.is_empty(), "Temp {} missing FR", t.label.key);
            assert!(!t.label.de.is_empty(), "Temp {} missing DE", t.label.key);
        }
    }

    // --- Trend constants ---

    #[test]
    fn trend_thresholds_sane() {
        assert!((BP_SYSTOLIC_TREND_THRESHOLD - 20.0).abs() < f64::EPSILON);
        assert!((BP_DIASTOLIC_TREND_THRESHOLD - 10.0).abs() < f64::EPSILON);
        assert!((ORTHOSTATIC_SBP_DROP - 20.0).abs() < f64::EPSILON);
        assert!((ORTHOSTATIC_DBP_DROP - 10.0).abs() < f64::EPSILON);
        assert!((ORTHOSTATIC_SBP_FLOOR - 90.0).abs() < f64::EPSILON);
        assert!((WEIGHT_LOSS_MODERATE_PCT - 5.0).abs() < f64::EPSILON);
        assert!((WEIGHT_LOSS_SEVERE_PCT - 10.0).abs() < f64::EPSILON);
    }

    // --- Source attribution ---

    #[test]
    fn all_classifications_have_sources() {
        for bp in BP_CLASSIFICATIONS {
            assert!(!bp.source.is_empty());
        }
        for hr in HR_CLASSIFICATIONS {
            assert!(!hr.source.is_empty());
        }
        for spo2 in SPO2_CLASSIFICATIONS {
            assert!(!spo2.source.is_empty());
        }
        for bmi in BMI_CLASSIFICATIONS {
            assert!(!bmi.source.is_empty());
        }
        for g in GLUCOSE_CLASSIFICATIONS {
            assert!(!g.source.is_empty());
        }
        for t in TEMP_CLASSIFICATIONS {
            assert!(!t.source.is_empty());
        }
    }
}
