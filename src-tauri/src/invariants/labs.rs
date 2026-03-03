//! ME-03 I-LAB: Laboratory clinical thresholds.
//!
//! All thresholds from international guidelines (KDIGO, ESC/EAS, IDF, EASL, WHO).
//! Const data — compiled into the binary, zero I/O at runtime.
//!
//! Each threshold has an `aliases` array for name normalization across EN/FR/DE.
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
// Lab Threshold — generic structure for all lab classifications
// ═══════════════════════════════════════════════════════════

/// A single classification tier within a lab test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabTier {
    pub label: InvariantLabel,
    /// Lower bound (inclusive).
    pub min_value: f64,
    /// Upper bound (exclusive). f64::MAX = no upper bound.
    pub max_value: f64,
    /// Clinical significance factor (S in meaning equation).
    pub significance: f64,
}

/// A lab test with its clinical classification tiers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabThreshold {
    /// Canonical key for this test (machine-readable).
    pub test_key: &'static str,
    /// Name aliases for matching extracted test names (EN/FR/DE).
    pub aliases: &'static [&'static str],
    /// Classification tiers (ordered from lowest to highest value).
    pub tiers: &'static [LabTier],
    /// Standard unit for this test.
    pub unit: &'static str,
    /// Guideline source.
    pub source: &'static str,
}

/// Classify a lab value against a threshold's tiers.
/// Returns None if the value doesn't match any tier.
pub fn classify_lab(value: f64, threshold: &LabThreshold) -> Option<&LabTier> {
    threshold
        .tiers
        .iter()
        .find(|t| value >= t.min_value && value < t.max_value)
}

/// Find a matching lab threshold by test name (case-insensitive alias match).
pub fn find_threshold(test_name: &str) -> Option<&'static LabThreshold> {
    let normalized = test_name.trim().to_lowercase();
    ALL_LAB_THRESHOLDS.iter().find(|t| {
        t.test_key == normalized
            || t.aliases
                .iter()
                .any(|a| a.to_lowercase() == normalized)
    })
}

// ═══════════════════════════════════════════════════════════
// eGFR — KDIGO 2024 CKD Classification
// ═══════════════════════════════════════════════════════════

pub static EGFR_TIERS: &[LabTier] = &[
    LabTier {
        label: InvariantLabel {
            key: "ckd_g5",
            en: "CKD G5 — Kidney failure",
            fr: "MRC G5 — Insuffisance rénale",
            de: "CKD G5 — Nierenversagen",
        },
        min_value: 0.0,
        max_value: 15.0,
        significance: 2.0,
    },
    LabTier {
        label: InvariantLabel {
            key: "ckd_g4",
            en: "CKD G4 — Severely decreased",
            fr: "MRC G4 — Sévèrement diminué",
            de: "CKD G4 — Stark vermindert",
        },
        min_value: 15.0,
        max_value: 30.0,
        significance: 1.8,
    },
    LabTier {
        label: InvariantLabel {
            key: "ckd_g3b",
            en: "CKD G3b — Moderately to severely decreased",
            fr: "MRC G3b — Modérément à sévèrement diminué",
            de: "CKD G3b — Mäßig bis stark vermindert",
        },
        min_value: 30.0,
        max_value: 45.0,
        significance: 1.4,
    },
    LabTier {
        label: InvariantLabel {
            key: "ckd_g3a",
            en: "CKD G3a — Mildly to moderately decreased",
            fr: "MRC G3a — Légèrement à modérément diminué",
            de: "CKD G3a — Leicht bis mäßig vermindert",
        },
        min_value: 45.0,
        max_value: 60.0,
        significance: 1.0,
    },
    LabTier {
        label: InvariantLabel {
            key: "ckd_g2",
            en: "CKD G2 — Mildly decreased",
            fr: "MRC G2 — Légèrement diminué",
            de: "CKD G2 — Leicht vermindert",
        },
        min_value: 60.0,
        max_value: 90.0,
        significance: 0.4,
    },
    LabTier {
        label: InvariantLabel {
            key: "ckd_g1",
            en: "CKD G1 — Normal or high",
            fr: "MRC G1 — Normal ou élevé",
            de: "CKD G1 — Normal oder hoch",
        },
        min_value: 90.0,
        max_value: f64::MAX,
        significance: 0.2,
    },
];

pub static EGFR_THRESHOLD: LabThreshold = LabThreshold {
    test_key: "egfr",
    aliases: &[
        "eGFR", "GFR", "estimated GFR", "glomerular filtration rate",
        "DFG", "DFGe", "débit de filtration glomérulaire",
        "GFR geschätzt", "glomeruläre Filtrationsrate",
    ],
    tiers: EGFR_TIERS,
    unit: "mL/min/1.73m²",
    source: "KDIGO 2024",
};

/// KDIGO: eGFR change >20% between consecutive tests warrants evaluation.
pub const EGFR_CHANGE_THRESHOLD_PCT: f64 = 20.0;

// ═══════════════════════════════════════════════════════════
// HbA1c — IDF 2025 + WHO
// ═══════════════════════════════════════════════════════════

pub static HBA1C_TIERS: &[LabTier] = &[
    LabTier {
        label: InvariantLabel {
            key: "hba1c_normal",
            en: "Normal HbA1c",
            fr: "HbA1c normale",
            de: "Normales HbA1c",
        },
        min_value: 0.0,
        max_value: 5.7,
        significance: 0.2,
    },
    LabTier {
        label: InvariantLabel {
            key: "hba1c_prediabetes",
            en: "Pre-diabetes range",
            fr: "Plage pré-diabétique",
            de: "Prädiabetischer Bereich",
        },
        min_value: 5.7,
        max_value: 6.5,
        significance: 0.8,
    },
    LabTier {
        label: InvariantLabel {
            key: "hba1c_diabetes",
            en: "Diabetes range",
            fr: "Plage diabétique",
            de: "Diabetesbereich",
        },
        min_value: 6.5,
        max_value: f64::MAX,
        significance: 1.5,
    },
];

pub static HBA1C_THRESHOLD: LabThreshold = LabThreshold {
    test_key: "hba1c",
    aliases: &[
        "HbA1c", "A1c", "glycated hemoglobin", "glycosylated hemoglobin",
        "hémoglobine glyquée", "hémoglobine A1c",
        "HbA1c", "glykiertes Hämoglobin", "Glykohämoglobin",
    ],
    tiers: HBA1C_TIERS,
    unit: "%",
    source: "IDF 2025, WHO",
};

/// HbA1c increase >0.5% over 6 months: diabetes worsening.
pub const HBA1C_TREND_THRESHOLD: f64 = 0.5;

// ═══════════════════════════════════════════════════════════
// LDL Cholesterol — ESC/EAS 2019/2025 Risk-Stratified Targets
// ═══════════════════════════════════════════════════════════

pub static LDL_TIERS: &[LabTier] = &[
    LabTier {
        label: InvariantLabel {
            key: "ldl_extreme_risk",
            en: "Above extreme-risk target (>1.0 mmol/L)",
            fr: "Au-dessus de la cible à risque extrême (>1,0 mmol/L)",
            de: "Über dem Extremrisiko-Ziel (>1,0 mmol/L)",
        },
        // Note: this tier captures any LDL in the extreme range context.
        // Actual classification depends on patient risk category.
        min_value: 0.0,
        max_value: 1.0,
        significance: 0.2,
    },
    LabTier {
        label: InvariantLabel {
            key: "ldl_very_high_risk",
            en: "Above very-high-risk target (1.0-1.4 mmol/L)",
            fr: "Au-dessus de la cible à très haut risque (1,0-1,4 mmol/L)",
            de: "Über dem Sehr-Hoch-Risiko-Ziel (1,0-1,4 mmol/L)",
        },
        min_value: 1.0,
        max_value: 1.4,
        significance: 0.3,
    },
    LabTier {
        label: InvariantLabel {
            key: "ldl_high_risk",
            en: "Above high-risk target (1.4-1.8 mmol/L)",
            fr: "Au-dessus de la cible à haut risque (1,4-1,8 mmol/L)",
            de: "Über dem Hochrisiko-Ziel (1,4-1,8 mmol/L)",
        },
        min_value: 1.4,
        max_value: 1.8,
        significance: 0.5,
    },
    LabTier {
        label: InvariantLabel {
            key: "ldl_moderate_risk",
            en: "Above moderate-risk target (1.8-2.6 mmol/L)",
            fr: "Au-dessus de la cible à risque modéré (1,8-2,6 mmol/L)",
            de: "Über dem mäßigen Risikoziel (1,8-2,6 mmol/L)",
        },
        min_value: 1.8,
        max_value: 2.6,
        significance: 0.6,
    },
    LabTier {
        label: InvariantLabel {
            key: "ldl_low_risk",
            en: "Above low-risk target (2.6-3.0 mmol/L)",
            fr: "Au-dessus de la cible à faible risque (2,6-3,0 mmol/L)",
            de: "Über dem Niedrigrisiko-Ziel (2,6-3,0 mmol/L)",
        },
        min_value: 2.6,
        max_value: 3.0,
        significance: 0.4,
    },
    LabTier {
        label: InvariantLabel {
            key: "ldl_elevated",
            en: "Elevated LDL (≥3.0 mmol/L)",
            fr: "LDL élevé (≥3,0 mmol/L)",
            de: "Erhöhtes LDL (≥3,0 mmol/L)",
        },
        min_value: 3.0,
        max_value: f64::MAX,
        significance: 0.8,
    },
];

pub static LDL_THRESHOLD: LabThreshold = LabThreshold {
    test_key: "ldl_cholesterol",
    aliases: &[
        "LDL", "LDL-C", "LDL cholesterol", "low-density lipoprotein",
        "cholestérol LDL", "LDL-cholestérol",
        "LDL-Cholesterin", "LDL Cholesterin",
    ],
    tiers: LDL_TIERS,
    unit: "mmol/L",
    source: "ESC/EAS 2019/2025",
};

// ═══════════════════════════════════════════════════════════
// Potassium — KDIGO 2024
// ═══════════════════════════════════════════════════════════

pub static POTASSIUM_TIERS: &[LabTier] = &[
    LabTier {
        label: InvariantLabel {
            key: "k_severe_hypokalemia",
            en: "Severe hypokalemia",
            fr: "Hypokaliémie sévère",
            de: "Schwere Hypokaliämie",
        },
        min_value: 0.0,
        max_value: 3.0,
        significance: 2.0,
    },
    LabTier {
        label: InvariantLabel {
            key: "k_hypokalemia",
            en: "Hypokalemia",
            fr: "Hypokaliémie",
            de: "Hypokaliämie",
        },
        min_value: 3.0,
        max_value: 3.5,
        significance: 1.3,
    },
    LabTier {
        label: InvariantLabel {
            key: "k_normal",
            en: "Normal potassium",
            fr: "Potassium normal",
            de: "Normales Kalium",
        },
        min_value: 3.5,
        max_value: 5.0,
        significance: 0.2,
    },
    LabTier {
        label: InvariantLabel {
            key: "k_mild_hyperkalemia",
            en: "Mild hyperkalemia",
            fr: "Hyperkaliémie légère",
            de: "Leichte Hyperkaliämie",
        },
        min_value: 5.0,
        max_value: 5.5,
        significance: 0.8,
    },
    LabTier {
        label: InvariantLabel {
            key: "k_hyperkalemia",
            en: "Hyperkalemia — review medications",
            fr: "Hyperkaliémie — réviser les médicaments",
            de: "Hyperkaliämie — Medikamente überprüfen",
        },
        min_value: 5.5,
        max_value: 6.0,
        significance: 1.5,
    },
    LabTier {
        label: InvariantLabel {
            key: "k_severe_hyperkalemia",
            en: "Severe hyperkalemia — emergency",
            fr: "Hyperkaliémie sévère — urgence",
            de: "Schwere Hyperkaliämie — Notfall",
        },
        min_value: 6.0,
        max_value: f64::MAX,
        significance: 2.0,
    },
];

pub static POTASSIUM_THRESHOLD: LabThreshold = LabThreshold {
    test_key: "potassium",
    aliases: &[
        "K", "K+", "potassium", "serum potassium",
        "kaliémie", "potassium sérique",
        "Kalium", "Serum-Kalium",
    ],
    tiers: POTASSIUM_TIERS,
    unit: "mmol/L",
    source: "KDIGO 2024",
};

// ═══════════════════════════════════════════════════════════
// Sodium — Clinical standard
// ═══════════════════════════════════════════════════════════

pub static SODIUM_TIERS: &[LabTier] = &[
    LabTier {
        label: InvariantLabel {
            key: "na_severe_hyponatremia",
            en: "Severe hyponatremia",
            fr: "Hyponatrémie sévère",
            de: "Schwere Hyponatriämie",
        },
        min_value: 0.0,
        max_value: 125.0,
        significance: 2.0,
    },
    LabTier {
        label: InvariantLabel {
            key: "na_hyponatremia",
            en: "Hyponatremia",
            fr: "Hyponatrémie",
            de: "Hyponatriämie",
        },
        min_value: 125.0,
        max_value: 135.0,
        significance: 1.3,
    },
    LabTier {
        label: InvariantLabel {
            key: "na_normal",
            en: "Normal sodium",
            fr: "Sodium normal",
            de: "Normales Natrium",
        },
        min_value: 135.0,
        max_value: 145.0,
        significance: 0.2,
    },
    LabTier {
        label: InvariantLabel {
            key: "na_hypernatremia",
            en: "Hypernatremia",
            fr: "Hypernatrémie",
            de: "Hypernatriämie",
        },
        min_value: 145.0,
        max_value: f64::MAX,
        significance: 1.3,
    },
];

pub static SODIUM_THRESHOLD: LabThreshold = LabThreshold {
    test_key: "sodium",
    aliases: &[
        "Na", "Na+", "sodium", "serum sodium",
        "natrémie", "sodium sérique",
        "Natrium", "Serum-Natrium",
    ],
    tiers: SODIUM_TIERS,
    unit: "mmol/L",
    source: "Clinical standard",
};

// ═══════════════════════════════════════════════════════════
// ALT (SGPT) — EASL DILI Guidelines
// ULN = Upper Limit of Normal, typically 40 U/L for ALT (EASL reference).
// DILI thresholds: 3× ULN (120 U/L) = hepatocellular injury, 5× ULN (200 U/L) = severe.
// ═══════════════════════════════════════════════════════════

pub static ALT_TIERS: &[LabTier] = &[
    LabTier {
        label: InvariantLabel {
            key: "alt_normal",
            en: "Normal ALT",
            fr: "ALAT normale",
            de: "Normales ALT",
        },
        min_value: 0.0,
        max_value: 40.0, // Standard ULN
        significance: 0.2,
    },
    LabTier {
        label: InvariantLabel {
            key: "alt_mild_elevation",
            en: "Mildly elevated ALT (1-3× ULN)",
            fr: "ALAT légèrement élevée (1-3× LSN)",
            de: "Leicht erhöhtes ALT (1-3× OGW)",
        },
        min_value: 40.0,
        max_value: 120.0, // 3× ULN
        significance: 0.6,
    },
    LabTier {
        label: InvariantLabel {
            key: "alt_hepatocellular_injury",
            en: "Hepatocellular injury (ALT >3× ULN)",
            fr: "Lésion hépatocellulaire (ALAT >3× LSN)",
            de: "Hepatozelluläre Schädigung (ALT >3× OGW)",
        },
        min_value: 120.0,
        max_value: 200.0, // 5× ULN
        significance: 1.5,
    },
    LabTier {
        label: InvariantLabel {
            key: "alt_severe_liver_injury",
            en: "Severe liver injury (ALT >5× ULN)",
            fr: "Lésion hépatique sévère (ALAT >5× LSN)",
            de: "Schwere Leberschädigung (ALT >5× OGW)",
        },
        min_value: 200.0,
        max_value: f64::MAX,
        significance: 2.0,
    },
];

pub static ALT_THRESHOLD: LabThreshold = LabThreshold {
    test_key: "alt",
    aliases: &[
        "ALT", "SGPT", "ALAT", "alanine aminotransferase", "alanine transaminase",
        "ALAT", "alanine aminotransférase", "transaminase ALAT",
        "ALT", "Alanin-Aminotransferase", "GPT",
    ],
    tiers: ALT_TIERS,
    unit: "U/L",
    source: "EASL DILI Guidelines, CIOMS",
};

// ═══════════════════════════════════════════════════════════
// Hemoglobin — WHO 2024
// Uses female threshold (12.0 g/dL) as conservative default for all patients.
// WHO gender-specific lower limits: Female 12.0 g/dL, Male 13.0 g/dL.
// The enrichment layer can apply sex-specific correction when patient sex is known.
// ═══════════════════════════════════════════════════════════

pub static HEMOGLOBIN_TIERS: &[LabTier] = &[
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
        max_value: 12.0, // Female threshold (WHO); male = 13.0
        significance: 0.6,
    },
    LabTier {
        label: InvariantLabel {
            key: "hb_normal",
            en: "Normal hemoglobin",
            fr: "Hémoglobine normale",
            de: "Normales Hämoglobin",
        },
        min_value: 12.0,
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

pub static HEMOGLOBIN_THRESHOLD: LabThreshold = LabThreshold {
    test_key: "hemoglobin",
    aliases: &[
        "Hb", "Hgb", "hemoglobin", "haemoglobin",
        "hémoglobine", "Hb",
        "Hämoglobin", "Hb",
    ],
    tiers: HEMOGLOBIN_TIERS,
    unit: "g/dL",
    source: "WHO",
};

/// Hemoglobin drop >2 g/dL over 3 months: urgent investigation.
pub const HB_DROP_THRESHOLD: f64 = 2.0;

// ═══════════════════════════════════════════════════════════
// TSH — ETA (European Thyroid Association)
// ═══════════════════════════════════════════════════════════

pub static TSH_TIERS: &[LabTier] = &[
    LabTier {
        label: InvariantLabel {
            key: "tsh_suppressed",
            en: "Suppressed TSH (hyperthyroidism)",
            fr: "TSH supprimée (hyperthyroïdie)",
            de: "Supprimiertes TSH (Hyperthyreose)",
        },
        min_value: 0.0,
        max_value: 0.4,
        significance: 1.3,
    },
    LabTier {
        label: InvariantLabel {
            key: "tsh_normal",
            en: "Normal TSH",
            fr: "TSH normale",
            de: "Normales TSH",
        },
        min_value: 0.4,
        max_value: 4.0,
        significance: 0.2,
    },
    LabTier {
        label: InvariantLabel {
            key: "tsh_subclinical_hypo",
            en: "Subclinical hypothyroidism",
            fr: "Hypothyroïdie infraclinique",
            de: "Subklinische Hypothyreose",
        },
        min_value: 4.0,
        max_value: 10.0,
        significance: 0.8,
    },
    LabTier {
        label: InvariantLabel {
            key: "tsh_overt_hypo",
            en: "Overt hypothyroidism",
            fr: "Hypothyroïdie manifeste",
            de: "Manifeste Hypothyreose",
        },
        min_value: 10.0,
        max_value: f64::MAX,
        significance: 1.5,
    },
];

pub static TSH_THRESHOLD: LabThreshold = LabThreshold {
    test_key: "tsh",
    aliases: &[
        "TSH", "thyroid stimulating hormone", "thyrotropin",
        "TSH", "thyréostimuline",
        "TSH", "Thyreotropin",
    ],
    tiers: TSH_TIERS,
    unit: "mU/L",
    source: "ETA",
};

// ═══════════════════════════════════════════════════════════
// uACR — KDIGO 2024 Albuminuria Categories
// ═══════════════════════════════════════════════════════════

pub static UACR_TIERS: &[LabTier] = &[
    LabTier {
        label: InvariantLabel {
            key: "uacr_a1",
            en: "A1 — Normal to mildly increased",
            fr: "A1 — Normal à légèrement augmenté",
            de: "A1 — Normal bis leicht erhöht",
        },
        min_value: 0.0,
        max_value: 30.0,
        significance: 0.2,
    },
    LabTier {
        label: InvariantLabel {
            key: "uacr_a2",
            en: "A2 — Moderately increased (microalbuminuria)",
            fr: "A2 — Modérément augmenté (microalbuminurie)",
            de: "A2 — Mäßig erhöht (Mikroalbuminurie)",
        },
        min_value: 30.0,
        max_value: 300.0,
        significance: 1.0,
    },
    LabTier {
        label: InvariantLabel {
            key: "uacr_a3",
            en: "A3 — Severely increased (macroalbuminuria)",
            fr: "A3 — Sévèrement augmenté (macroalbuminurie)",
            de: "A3 — Stark erhöht (Makroalbuminurie)",
        },
        min_value: 300.0,
        max_value: f64::MAX,
        significance: 1.8,
    },
];

pub static UACR_THRESHOLD: LabThreshold = LabThreshold {
    test_key: "uacr",
    aliases: &[
        "uACR", "ACR", "albumin-creatinine ratio", "urine albumin creatinine ratio",
        "RAC", "rapport albumine/créatinine urinaire",
        "Albumin-Kreatinin-Verhältnis", "uACR",
    ],
    tiers: UACR_TIERS,
    unit: "mg/g",
    source: "KDIGO 2024",
};

// ═══════════════════════════════════════════════════════════
// Vitamin D — Endocrine Society / IOF
// ═══════════════════════════════════════════════════════════

pub static VITAMIN_D_TIERS: &[LabTier] = &[
    LabTier {
        label: InvariantLabel {
            key: "vitd_deficient",
            en: "Vitamin D deficiency",
            fr: "Carence en vitamine D",
            de: "Vitamin-D-Mangel",
        },
        min_value: 0.0,
        max_value: 20.0,
        significance: 1.0,
    },
    LabTier {
        label: InvariantLabel {
            key: "vitd_insufficient",
            en: "Vitamin D insufficiency",
            fr: "Insuffisance en vitamine D",
            de: "Vitamin-D-Insuffizienz",
        },
        min_value: 20.0,
        max_value: 30.0,
        significance: 0.5,
    },
    LabTier {
        label: InvariantLabel {
            key: "vitd_sufficient",
            en: "Sufficient vitamin D",
            fr: "Vitamine D suffisante",
            de: "Ausreichendes Vitamin D",
        },
        min_value: 30.0,
        max_value: 100.0,
        significance: 0.2,
    },
    LabTier {
        label: InvariantLabel {
            key: "vitd_excess",
            en: "Excess vitamin D (risk of toxicity)",
            fr: "Excès de vitamine D (risque de toxicité)",
            de: "Vitamin-D-Überschuss (Toxizitätsrisiko)",
        },
        min_value: 100.0,
        max_value: f64::MAX,
        significance: 1.3,
    },
];

pub static VITAMIN_D_THRESHOLD: LabThreshold = LabThreshold {
    test_key: "vitamin_d",
    aliases: &[
        "vitamin D", "25-OH vitamin D", "25-hydroxyvitamin D", "calcidiol",
        "vitamine D", "25-OH vitamine D", "calcidiol",
        "Vitamin D", "25-OH-Vitamin-D", "Calcidiol",
    ],
    tiers: VITAMIN_D_TIERS,
    unit: "ng/mL",
    source: "Endocrine Society, IOF",
};

// ═══════════════════════════════════════════════════════════
// Master registry of all lab thresholds
// ═══════════════════════════════════════════════════════════

pub static ALL_LAB_THRESHOLDS: &[LabThreshold] = &[
    EGFR_THRESHOLD,
    HBA1C_THRESHOLD,
    LDL_THRESHOLD,
    POTASSIUM_THRESHOLD,
    SODIUM_THRESHOLD,
    ALT_THRESHOLD,
    HEMOGLOBIN_THRESHOLD,
    TSH_THRESHOLD,
    UACR_THRESHOLD,
    VITAMIN_D_THRESHOLD,
];

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // --- eGFR / CKD stages ---

    #[test]
    fn egfr_g1() {
        let tier = classify_lab(95.0, &EGFR_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "ckd_g1");
    }

    #[test]
    fn egfr_g2() {
        let tier = classify_lab(75.0, &EGFR_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "ckd_g2");
    }

    #[test]
    fn egfr_g3a() {
        let tier = classify_lab(50.0, &EGFR_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "ckd_g3a");
    }

    #[test]
    fn egfr_g3b() {
        let tier = classify_lab(35.0, &EGFR_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "ckd_g3b");
    }

    #[test]
    fn egfr_g4() {
        let tier = classify_lab(20.0, &EGFR_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "ckd_g4");
    }

    #[test]
    fn egfr_g5() {
        let tier = classify_lab(10.0, &EGFR_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "ckd_g5");
    }

    #[test]
    fn egfr_boundary_60() {
        let tier = classify_lab(60.0, &EGFR_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "ckd_g2");
    }

    #[test]
    fn egfr_boundary_90() {
        let tier = classify_lab(90.0, &EGFR_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "ckd_g1");
    }

    // --- HbA1c ---

    #[test]
    fn hba1c_normal() {
        let tier = classify_lab(5.2, &HBA1C_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "hba1c_normal");
    }

    #[test]
    fn hba1c_prediabetes() {
        let tier = classify_lab(6.0, &HBA1C_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "hba1c_prediabetes");
    }

    #[test]
    fn hba1c_diabetes() {
        let tier = classify_lab(7.5, &HBA1C_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "hba1c_diabetes");
    }

    // --- LDL ---

    #[test]
    fn ldl_low_risk() {
        let tier = classify_lab(2.8, &LDL_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "ldl_low_risk");
    }

    #[test]
    fn ldl_elevated() {
        let tier = classify_lab(3.5, &LDL_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "ldl_elevated");
    }

    // --- Potassium ---

    #[test]
    fn k_normal() {
        let tier = classify_lab(4.2, &POTASSIUM_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "k_normal");
    }

    #[test]
    fn k_hyperkalemia() {
        let tier = classify_lab(5.7, &POTASSIUM_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "k_hyperkalemia");
    }

    #[test]
    fn k_severe_hyperkalemia() {
        let tier = classify_lab(6.5, &POTASSIUM_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "k_severe_hyperkalemia");
    }

    // --- ALT / DILI ---

    #[test]
    fn alt_normal() {
        let tier = classify_lab(25.0, &ALT_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "alt_normal");
    }

    #[test]
    fn alt_3x_uln() {
        let tier = classify_lab(130.0, &ALT_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "alt_hepatocellular_injury");
    }

    #[test]
    fn alt_5x_uln() {
        let tier = classify_lab(250.0, &ALT_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "alt_severe_liver_injury");
    }

    // --- Hemoglobin ---

    #[test]
    fn hb_severe_anemia() {
        let tier = classify_lab(6.5, &HEMOGLOBIN_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "hb_severe_anemia");
    }

    #[test]
    fn hb_normal() {
        let tier = classify_lab(14.0, &HEMOGLOBIN_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "hb_normal");
    }

    // --- TSH ---

    #[test]
    fn tsh_normal() {
        let tier = classify_lab(2.0, &TSH_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "tsh_normal");
    }

    #[test]
    fn tsh_overt_hypo() {
        let tier = classify_lab(15.0, &TSH_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "tsh_overt_hypo");
    }

    // --- uACR ---

    #[test]
    fn uacr_a1() {
        let tier = classify_lab(15.0, &UACR_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "uacr_a1");
    }

    #[test]
    fn uacr_a2() {
        let tier = classify_lab(150.0, &UACR_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "uacr_a2");
    }

    #[test]
    fn uacr_a3() {
        let tier = classify_lab(500.0, &UACR_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "uacr_a3");
    }

    // --- Vitamin D ---

    #[test]
    fn vitd_deficient() {
        let tier = classify_lab(12.0, &VITAMIN_D_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "vitd_deficient");
    }

    #[test]
    fn vitd_sufficient() {
        let tier = classify_lab(45.0, &VITAMIN_D_THRESHOLD).unwrap();
        assert_eq!(tier.label.key, "vitd_sufficient");
    }

    // --- Name normalization ---

    #[test]
    fn find_threshold_by_key() {
        assert!(find_threshold("egfr").is_some());
        assert!(find_threshold("hba1c").is_some());
        assert!(find_threshold("potassium").is_some());
    }

    #[test]
    fn find_threshold_by_alias_en() {
        assert_eq!(find_threshold("eGFR").unwrap().test_key, "egfr");
        assert_eq!(find_threshold("HbA1c").unwrap().test_key, "hba1c");
        assert_eq!(find_threshold("LDL").unwrap().test_key, "ldl_cholesterol");
        assert_eq!(find_threshold("K+").unwrap().test_key, "potassium");
        assert_eq!(find_threshold("ALT").unwrap().test_key, "alt");
        assert_eq!(find_threshold("TSH").unwrap().test_key, "tsh");
    }

    #[test]
    fn find_threshold_by_alias_fr() {
        assert_eq!(
            find_threshold("hémoglobine glyquée").unwrap().test_key,
            "hba1c"
        );
        assert_eq!(
            find_threshold("débit de filtration glomérulaire")
                .unwrap()
                .test_key,
            "egfr"
        );
        assert_eq!(find_threshold("kaliémie").unwrap().test_key, "potassium");
        assert_eq!(find_threshold("ALAT").unwrap().test_key, "alt");
    }

    #[test]
    fn find_threshold_by_alias_de() {
        assert_eq!(
            find_threshold("glomeruläre Filtrationsrate")
                .unwrap()
                .test_key,
            "egfr"
        );
        assert_eq!(
            find_threshold("glykiertes Hämoglobin").unwrap().test_key,
            "hba1c"
        );
        assert_eq!(find_threshold("Kalium").unwrap().test_key, "potassium");
    }

    #[test]
    fn find_threshold_case_insensitive() {
        assert!(find_threshold("EGFR").is_some());
        assert!(find_threshold("hba1c").is_some());
        assert!(find_threshold("LDL-C").is_some());
    }

    #[test]
    fn find_threshold_unknown_returns_none() {
        assert!(find_threshold("unknown_test").is_none());
        assert!(find_threshold("random_lab").is_none());
    }

    // --- i18n coverage ---

    #[test]
    fn all_lab_tiers_have_i18n() {
        for threshold in ALL_LAB_THRESHOLDS {
            for tier in threshold.tiers {
                assert!(
                    !tier.label.en.is_empty(),
                    "{}/{} missing EN",
                    threshold.test_key,
                    tier.label.key
                );
                assert!(
                    !tier.label.fr.is_empty(),
                    "{}/{} missing FR",
                    threshold.test_key,
                    tier.label.key
                );
                assert!(
                    !tier.label.de.is_empty(),
                    "{}/{} missing DE",
                    threshold.test_key,
                    tier.label.key
                );
            }
        }
    }

    // --- Source attribution ---

    #[test]
    fn all_thresholds_have_sources() {
        for threshold in ALL_LAB_THRESHOLDS {
            assert!(
                !threshold.source.is_empty(),
                "{} missing source",
                threshold.test_key
            );
        }
    }

    // --- Master registry ---

    #[test]
    fn all_lab_thresholds_count() {
        assert_eq!(ALL_LAB_THRESHOLDS.len(), 10);
    }
}
