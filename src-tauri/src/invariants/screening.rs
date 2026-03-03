//! ME-04 + ME-06: Evidence-based preventive screening and vaccine schedules.
//!
//! Age+sex-gated, deterministic. No LLM involved.
//! Produces `InsightKind::ScreeningDue` insights when a patient meets
//! the eligibility criteria for a preventive screening or vaccination.
//!
//! Data sources: WHO 2024, IARC 2024, EAU 2024, ESC 2024, IOF 2024, WHO SAGE 2024.

use crate::crypto::profile::{BiologicalSex, PatientDemographics};
use crate::invariants::types::{
    ClinicalInsight, InsightKind, InsightSeverity, InvariantLabel, MeaningFactors,
};

// ═══════════════════════════════════════════════════════════
// Screening schedule definition
// ═══════════════════════════════════════════════════════════

/// Category for UI grouping and filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreeningCategory {
    Cancer,
    Metabolic,
    Vaccine,
}

impl ScreeningCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cancer => "cancer",
            Self::Metabolic => "metabolic",
            Self::Vaccine => "vaccine",
        }
    }
}

/// A preventive screening or vaccine schedule with eligibility criteria.
#[derive(Debug, Clone, Copy)]
pub struct ScreeningSchedule {
    /// Machine key for the screening.
    pub key: &'static str,
    /// Trilingual display label.
    pub label: InvariantLabel,
    /// Clinical guideline source.
    pub source: &'static str,
    /// Required biological sex. `None` = both sexes eligible.
    pub sex: Option<BiologicalSex>,
    /// Minimum age in years (inclusive).
    pub min_age: u16,
    /// Maximum age in years (inclusive). `None` = no upper limit.
    pub max_age: Option<u16>,
    /// Recommended screening interval in months.
    pub interval_months: u16,
    /// Number of doses in a series. 0 = recurring (use interval_months).
    pub total_doses: u16,
    /// Validity window in months after completion. `None` = no expiry.
    pub validity_months: Option<u16>,
    /// Category for UI grouping.
    pub category: ScreeningCategory,
}

/// Look up a screening schedule by key.
pub fn find_schedule(key: &str) -> Option<&'static ScreeningSchedule> {
    SCREENING_SCHEDULES.iter().find(|s| s.key == key)
}

// ═══════════════════════════════════════════════════════════
// 14 evidence-based schedules: 6 screenings + 8 vaccines
// ═══════════════════════════════════════════════════════════

pub static SCREENING_SCHEDULES: &[ScreeningSchedule] = &[
    // ─── CANCER SCREENINGS ───────────────────────────────

    // 1. Mammography - IARC/WHO 2024
    ScreeningSchedule {
        key: "screening_mammography",
        label: InvariantLabel {
            key: "screening_mammography",
            en: "Mammography screening recommended",
            fr: "D\u{00e9}pistage par mammographie recommand\u{00e9}",
            de: "Mammographie-Screening empfohlen",
        },
        source: "IARC/WHO 2024",
        sex: Some(BiologicalSex::Female),
        min_age: 50,
        max_age: Some(74),
        interval_months: 24,
        total_doses: 0,
        validity_months: None,
        category: ScreeningCategory::Cancer,
    },
    // 2. Cervical (Pap/HPV) - WHO 2021
    ScreeningSchedule {
        key: "screening_cervical",
        label: InvariantLabel {
            key: "screening_cervical",
            en: "Cervical cancer screening recommended",
            fr: "D\u{00e9}pistage du cancer du col de l'ut\u{00e9}rus recommand\u{00e9}",
            de: "Geb\u{00e4}rmutterhalskrebs-Screening empfohlen",
        },
        source: "WHO 2021",
        sex: Some(BiologicalSex::Female),
        min_age: 25,
        max_age: Some(65),
        interval_months: 36,
        total_doses: 0,
        validity_months: None,
        category: ScreeningCategory::Cancer,
    },
    // 3. Prostate (PSA) - EAU 2024
    ScreeningSchedule {
        key: "screening_prostate",
        label: InvariantLabel {
            key: "screening_prostate",
            en: "Prostate cancer screening (PSA) recommended",
            fr: "D\u{00e9}pistage du cancer de la prostate (PSA) recommand\u{00e9}",
            de: "Prostatakrebs-Screening (PSA) empfohlen",
        },
        source: "EAU 2024",
        sex: Some(BiologicalSex::Male),
        min_age: 50,
        max_age: Some(70),
        interval_months: 12,
        total_doses: 0,
        validity_months: None,
        category: ScreeningCategory::Cancer,
    },
    // 4. Colorectal (FIT/colonoscopy) - IARC 2019
    ScreeningSchedule {
        key: "screening_colorectal",
        label: InvariantLabel {
            key: "screening_colorectal",
            en: "Colorectal cancer screening recommended",
            fr: "D\u{00e9}pistage du cancer colorectal recommand\u{00e9}",
            de: "Darmkrebs-Screening empfohlen",
        },
        source: "IARC 2019",
        sex: None, // Both sexes
        min_age: 50,
        max_age: Some(75),
        interval_months: 24,
        total_doses: 0,
        validity_months: None,
        category: ScreeningCategory::Cancer,
    },
    // 5. AAA Ultrasound - ESC 2024
    ScreeningSchedule {
        key: "screening_aaa",
        label: InvariantLabel {
            key: "screening_aaa",
            en: "Abdominal aortic aneurysm screening recommended",
            fr: "D\u{00e9}pistage de l'an\u{00e9}vrisme de l'aorte abdominale recommand\u{00e9}",
            de: "Bauchaortenaneurysma-Screening empfohlen",
        },
        source: "ESC 2024",
        sex: Some(BiologicalSex::Male),
        min_age: 65,
        max_age: Some(75),
        interval_months: 0, // One-time
        total_doses: 0,
        validity_months: None,
        category: ScreeningCategory::Cancer,
    },

    // ─── METABOLIC SCREENINGS ────────────────────────────

    // 6. Osteoporosis (DXA) - IOF 2024
    ScreeningSchedule {
        key: "screening_osteoporosis",
        label: InvariantLabel {
            key: "screening_osteoporosis",
            en: "Osteoporosis screening (DXA) recommended",
            fr: "D\u{00e9}pistage de l'ost\u{00e9}oporose (DXA) recommand\u{00e9}",
            de: "Osteoporose-Screening (DXA) empfohlen",
        },
        source: "IOF 2024",
        sex: Some(BiologicalSex::Female),
        min_age: 65,
        max_age: None, // No upper limit
        interval_months: 24,
        total_doses: 0,
        validity_months: None,
        category: ScreeningCategory::Metabolic,
    },

    // ─── VACCINES ────────────────────────────────────────

    // 7. Influenza (seasonal) - WHO 2024
    ScreeningSchedule {
        key: "vaccine_influenza",
        label: InvariantLabel {
            key: "vaccine_influenza",
            en: "Seasonal influenza vaccination",
            fr: "Vaccination antigrippale saisonni\u{00e8}re",
            de: "Saisonale Grippeimpfung",
        },
        source: "WHO 2024",
        sex: None,
        min_age: 18,
        max_age: None,
        interval_months: 12,
        total_doses: 0,            // Recurring annually
        validity_months: Some(12), // Valid 1 year
        category: ScreeningCategory::Vaccine,
    },
    // 8. Tetanus-Diphtheria-Pertussis (Tdap/Td) - WHO 2024
    ScreeningSchedule {
        key: "vaccine_tdap",
        label: InvariantLabel {
            key: "vaccine_tdap",
            en: "Tetanus-Diphtheria-Pertussis (Tdap) booster",
            fr: "Rappel T\u{00e9}tanos-Dipht\u{00e9}rie-Coqueluche (dTca)",
            de: "Tetanus-Diphtherie-Pertussis (Tdap) Auffrischung",
        },
        source: "WHO 2024",
        sex: None,
        min_age: 18,
        max_age: None,
        interval_months: 120,       // Every 10 years
        total_doses: 0,             // Recurring
        validity_months: Some(120), // Valid 10 years
        category: ScreeningCategory::Vaccine,
    },
    // 9. Pneumococcal (PCV/PPSV) - WHO 2024
    ScreeningSchedule {
        key: "vaccine_pneumococcal",
        label: InvariantLabel {
            key: "vaccine_pneumococcal",
            en: "Pneumococcal vaccination",
            fr: "Vaccination antipneumococcique",
            de: "Pneumokokken-Impfung",
        },
        source: "WHO 2024",
        sex: None,
        min_age: 65,
        max_age: None,
        interval_months: 0,         // One-time
        total_doses: 1,
        validity_months: None,      // Lifetime
        category: ScreeningCategory::Vaccine,
    },
    // 10. Shingles (Herpes Zoster) - WHO 2024
    ScreeningSchedule {
        key: "vaccine_shingles",
        label: InvariantLabel {
            key: "vaccine_shingles",
            en: "Shingles (Herpes Zoster) vaccination",
            fr: "Vaccination contre le zona",
            de: "G\u{00fc}rtelrose (Herpes Zoster) Impfung",
        },
        source: "WHO 2024",
        sex: None,
        min_age: 50,
        max_age: None,
        interval_months: 0,
        total_doses: 2,             // 2-dose series
        validity_months: None,      // Lifetime after series
        category: ScreeningCategory::Vaccine,
    },
    // 11. Hepatitis B - WHO 2024
    ScreeningSchedule {
        key: "vaccine_hepatitis_b",
        label: InvariantLabel {
            key: "vaccine_hepatitis_b",
            en: "Hepatitis B vaccination",
            fr: "Vaccination contre l'h\u{00e9}patite B",
            de: "Hepatitis-B-Impfung",
        },
        source: "WHO 2024",
        sex: None,
        min_age: 18,
        max_age: None,
        interval_months: 0,
        total_doses: 3,             // 3-dose series (0, 1, 6 months)
        validity_months: None,      // Lifetime after series
        category: ScreeningCategory::Vaccine,
    },
    // 12. HPV - WHO 2024
    ScreeningSchedule {
        key: "vaccine_hpv",
        label: InvariantLabel {
            key: "vaccine_hpv",
            en: "HPV vaccination",
            fr: "Vaccination contre le HPV",
            de: "HPV-Impfung",
        },
        source: "WHO 2024",
        sex: None,       // Both sexes per WHO 2024
        min_age: 18,
        max_age: Some(26),
        interval_months: 0,
        total_doses: 2,             // 2-dose for age 15-26
        validity_months: None,      // Lifetime
        category: ScreeningCategory::Vaccine,
    },
    // 13. MMR (Measles-Mumps-Rubella) - WHO 2024
    ScreeningSchedule {
        key: "vaccine_mmr",
        label: InvariantLabel {
            key: "vaccine_mmr",
            en: "Measles-Mumps-Rubella (MMR) vaccination",
            fr: "Vaccination Rougeole-Oreillons-Rub\u{00e9}ole (ROR)",
            de: "Masern-Mumps-R\u{00f6}teln (MMR) Impfung",
        },
        source: "WHO 2024",
        sex: None,
        min_age: 18,
        max_age: None,
        interval_months: 0,
        total_doses: 2,
        validity_months: None,      // Lifetime after 2 doses
        category: ScreeningCategory::Vaccine,
    },
    // 14. COVID-19 - WHO SAGE 2024
    ScreeningSchedule {
        key: "vaccine_covid19",
        label: InvariantLabel {
            key: "vaccine_covid19",
            en: "COVID-19 vaccination (updated booster)",
            fr: "Vaccination COVID-19 (rappel actualis\u{00e9})",
            de: "COVID-19-Impfung (aktualisierte Auffrischung)",
        },
        source: "WHO SAGE 2024",
        sex: None,
        min_age: 18,
        max_age: None,
        interval_months: 12,
        total_doses: 0,             // Recurring (annual booster)
        validity_months: Some(12),
        category: ScreeningCategory::Vaccine,
    },
];

// ═══════════════════════════════════════════════════════════
// Eligibility check
// ═══════════════════════════════════════════════════════════

/// Check if a patient matches a screening schedule's eligibility criteria.
fn is_eligible(schedule: &ScreeningSchedule, demographics: &PatientDemographics) -> bool {
    // Sex gate: if schedule requires specific sex, patient must match
    if let Some(required_sex) = schedule.sex {
        match demographics.sex {
            Some(patient_sex) if patient_sex == required_sex => {}
            Some(_) => return false, // Wrong sex
            None => return false,    // Unknown sex - can't match sex-gated screening
        }
    }

    // Age gate: patient must have known age within range
    let age = match demographics.age_years {
        Some(age) => age,
        None => return false, // Unknown age - can't match age-gated screening
    };

    if age < schedule.min_age {
        return false;
    }
    if let Some(max) = schedule.max_age {
        if age > max {
            return false;
        }
    }

    true
}

// ═══════════════════════════════════════════════════════════
// Sub-algorithm 7: Detect screening due
// ═══════════════════════════════════════════════════════════

/// Detect preventive screenings and vaccines that are due based on demographics.
///
/// Pure eligibility check - no DB access, no record checking.
/// Record-aware logic lives in `build_screening_info()` (me.rs).
pub fn detect_screening_due(
    demographics: Option<&PatientDemographics>,
) -> Vec<ClinicalInsight> {
    let demographics = match demographics {
        Some(d) => d,
        None => return vec![], // No demographics - no screening insights
    };

    SCREENING_SCHEDULES
        .iter()
        .filter(|s| is_eligible(s, demographics))
        .map(|s| ClinicalInsight {
            kind: InsightKind::ScreeningDue,
            severity: InsightSeverity::Info,
            summary_key: s.key.to_string(),
            description: s.label,
            source: s.source.to_string(),
            related_entities: vec![],
            meaning_factors: MeaningFactors {
                significance: 0.3, // Low - reminder, not alarm
                ..MeaningFactors::default()
            },
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::profile::AgeContext;

    fn make_demographics(
        sex: Option<BiologicalSex>,
        age: Option<u16>,
    ) -> PatientDemographics {
        PatientDemographics {
            sex,
            ethnicities: vec![],
            age_context: age.map(|a| {
                if a < 1 {
                    AgeContext::Newborn
                } else if a < 13 {
                    AgeContext::Child
                } else if a < 18 {
                    AgeContext::Adolescent
                } else {
                    AgeContext::Adult
                }
            }),
            age_years: age,
        }
    }

    // ─── Schedule data ────────────────────────────────

    #[test]
    fn screening_schedules_count() {
        assert_eq!(SCREENING_SCHEDULES.len(), 14); // 6 screenings + 8 vaccines
    }

    #[test]
    fn all_schedules_have_category() {
        let cancer = SCREENING_SCHEDULES.iter().filter(|s| s.category == ScreeningCategory::Cancer).count();
        let metabolic = SCREENING_SCHEDULES.iter().filter(|s| s.category == ScreeningCategory::Metabolic).count();
        let vaccine = SCREENING_SCHEDULES.iter().filter(|s| s.category == ScreeningCategory::Vaccine).count();
        assert_eq!(cancer, 5);
        assert_eq!(metabolic, 1);
        assert_eq!(vaccine, 8);
        assert_eq!(cancer + metabolic + vaccine, 14);
    }

    #[test]
    fn existing_screenings_backward_compat() {
        // Existing 6 screenings still have the same keys
        let keys: Vec<&str> = SCREENING_SCHEDULES.iter().map(|s| s.key).collect();
        assert!(keys.contains(&"screening_mammography"));
        assert!(keys.contains(&"screening_cervical"));
        assert!(keys.contains(&"screening_prostate"));
        assert!(keys.contains(&"screening_colorectal"));
        assert!(keys.contains(&"screening_aaa"));
        assert!(keys.contains(&"screening_osteoporosis"));
    }

    #[test]
    fn vaccine_keys_present() {
        let keys: Vec<&str> = SCREENING_SCHEDULES.iter().map(|s| s.key).collect();
        assert!(keys.contains(&"vaccine_influenza"));
        assert!(keys.contains(&"vaccine_tdap"));
        assert!(keys.contains(&"vaccine_pneumococcal"));
        assert!(keys.contains(&"vaccine_shingles"));
        assert!(keys.contains(&"vaccine_hepatitis_b"));
        assert!(keys.contains(&"vaccine_hpv"));
        assert!(keys.contains(&"vaccine_mmr"));
        assert!(keys.contains(&"vaccine_covid19"));
    }

    #[test]
    fn find_schedule_works() {
        assert!(find_schedule("vaccine_tdap").is_some());
        assert!(find_schedule("screening_mammography").is_some());
        assert!(find_schedule("nonexistent").is_none());
    }

    #[test]
    fn multi_dose_vaccines_correct() {
        let hep_b = find_schedule("vaccine_hepatitis_b").unwrap();
        assert_eq!(hep_b.total_doses, 3);
        assert!(hep_b.validity_months.is_none()); // Lifetime

        let shingles = find_schedule("vaccine_shingles").unwrap();
        assert_eq!(shingles.total_doses, 2);

        let hpv = find_schedule("vaccine_hpv").unwrap();
        assert_eq!(hpv.total_doses, 2);
        assert_eq!(hpv.max_age, Some(26));
    }

    #[test]
    fn recurring_vaccines_correct() {
        let flu = find_schedule("vaccine_influenza").unwrap();
        assert_eq!(flu.total_doses, 0); // Recurring
        assert_eq!(flu.validity_months, Some(12));
        assert_eq!(flu.interval_months, 12);

        let tdap = find_schedule("vaccine_tdap").unwrap();
        assert_eq!(tdap.total_doses, 0);
        assert_eq!(tdap.validity_months, Some(120)); // 10 years
    }

    #[test]
    fn category_as_str() {
        assert_eq!(ScreeningCategory::Cancer.as_str(), "cancer");
        assert_eq!(ScreeningCategory::Metabolic.as_str(), "metabolic");
        assert_eq!(ScreeningCategory::Vaccine.as_str(), "vaccine");
    }

    // ─── Screening eligibility (existing tests) ──────

    #[test]
    fn female_55_gets_mammography_cervical_colorectal_and_vaccines() {
        let demo = make_demographics(Some(BiologicalSex::Female), Some(55));
        let insights = detect_screening_due(Some(&demo));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        // Original screenings
        assert!(keys.contains(&"screening_mammography"));
        assert!(keys.contains(&"screening_cervical"));
        assert!(keys.contains(&"screening_colorectal"));
        assert!(!keys.contains(&"screening_prostate"));
        assert!(!keys.contains(&"screening_aaa"));
        assert!(!keys.contains(&"screening_osteoporosis"));
        // Vaccines (adult 18+, no sex gate, some age-gated)
        assert!(keys.contains(&"vaccine_influenza"));
        assert!(keys.contains(&"vaccine_tdap"));
        assert!(keys.contains(&"vaccine_shingles")); // 50+
        assert!(keys.contains(&"vaccine_hepatitis_b"));
        assert!(keys.contains(&"vaccine_mmr"));
        assert!(keys.contains(&"vaccine_covid19"));
        assert!(!keys.contains(&"vaccine_hpv")); // 18-26 only
        assert!(!keys.contains(&"vaccine_pneumococcal")); // 65+
    }

    #[test]
    fn male_55_gets_prostate_colorectal_and_vaccines() {
        let demo = make_demographics(Some(BiologicalSex::Male), Some(55));
        let insights = detect_screening_due(Some(&demo));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        assert!(keys.contains(&"screening_prostate"));
        assert!(keys.contains(&"screening_colorectal"));
        assert!(!keys.contains(&"screening_mammography"));
        assert!(!keys.contains(&"screening_cervical"));
        // Vaccines
        assert!(keys.contains(&"vaccine_influenza"));
        assert!(keys.contains(&"vaccine_shingles")); // 50+
    }

    #[test]
    fn female_30_gets_cervical_and_adult_vaccines() {
        let demo = make_demographics(Some(BiologicalSex::Female), Some(30));
        let insights = detect_screening_due(Some(&demo));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        assert!(keys.contains(&"screening_cervical"));
        assert!(keys.contains(&"vaccine_influenza"));
        assert!(keys.contains(&"vaccine_tdap"));
        assert!(keys.contains(&"vaccine_hepatitis_b"));
        assert!(keys.contains(&"vaccine_mmr"));
        assert!(keys.contains(&"vaccine_covid19"));
        assert!(!keys.contains(&"vaccine_shingles")); // 50+
        assert!(!keys.contains(&"vaccine_hpv")); // 18-26
    }

    #[test]
    fn male_40_gets_vaccines_only() {
        let demo = make_demographics(Some(BiologicalSex::Male), Some(40));
        let insights = detect_screening_due(Some(&demo));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        // No cancer screenings at 40 male
        assert!(!keys.contains(&"screening_prostate"));
        assert!(!keys.contains(&"screening_colorectal"));
        // But vaccines apply
        assert!(keys.contains(&"vaccine_influenza"));
        assert!(keys.contains(&"vaccine_tdap"));
        assert!(keys.contains(&"vaccine_hepatitis_b"));
        assert!(keys.contains(&"vaccine_mmr"));
        assert!(keys.contains(&"vaccine_covid19"));
    }

    #[test]
    fn vaccine_hpv_age_gate() {
        let young = make_demographics(Some(BiologicalSex::Female), Some(22));
        let insights = detect_screening_due(Some(&young));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        assert!(keys.contains(&"vaccine_hpv"));

        let old = make_demographics(Some(BiologicalSex::Female), Some(30));
        let insights = detect_screening_due(Some(&old));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        assert!(!keys.contains(&"vaccine_hpv"));
    }

    #[test]
    fn vaccine_pneumococcal_65_plus() {
        let young = make_demographics(None, Some(60));
        let insights = detect_screening_due(Some(&young));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        assert!(!keys.contains(&"vaccine_pneumococcal"));

        let senior = make_demographics(None, Some(65));
        let insights = detect_screening_due(Some(&senior));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        assert!(keys.contains(&"vaccine_pneumococcal"));
    }

    #[test]
    fn vaccine_shingles_50_plus() {
        let young = make_demographics(None, Some(45));
        let insights = detect_screening_due(Some(&young));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        assert!(!keys.contains(&"vaccine_shingles"));

        let mid = make_demographics(None, Some(50));
        let insights = detect_screening_due(Some(&mid));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        assert!(keys.contains(&"vaccine_shingles"));
    }

    #[test]
    fn female_70_gets_mammography_colorectal_osteoporosis_and_vaccines() {
        let demo = make_demographics(Some(BiologicalSex::Female), Some(70));
        let insights = detect_screening_due(Some(&demo));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        assert!(keys.contains(&"screening_mammography"));
        assert!(keys.contains(&"screening_colorectal"));
        assert!(keys.contains(&"screening_osteoporosis"));
        assert!(!keys.contains(&"screening_cervical")); // Stops at 65
        assert!(keys.contains(&"vaccine_pneumococcal")); // 65+
        assert!(keys.contains(&"vaccine_shingles")); // 50+
    }

    #[test]
    fn male_70_gets_prostate_colorectal_aaa() {
        let demo = make_demographics(Some(BiologicalSex::Male), Some(70));
        let insights = detect_screening_due(Some(&demo));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        assert!(keys.contains(&"screening_prostate"));
        assert!(keys.contains(&"screening_colorectal"));
        assert!(keys.contains(&"screening_aaa"));
    }

    #[test]
    fn no_demographics_returns_empty() {
        let insights = detect_screening_due(None);
        assert!(insights.is_empty());
    }

    #[test]
    fn unknown_sex_gets_sex_neutral_screenings_and_vaccines() {
        let demo = make_demographics(None, Some(55));
        let insights = detect_screening_due(Some(&demo));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        // Only colorectal is sex-neutral among cancer screenings
        assert!(keys.contains(&"screening_colorectal"));
        assert!(!keys.contains(&"screening_mammography"));
        // Vaccines are sex-neutral
        assert!(keys.contains(&"vaccine_influenza"));
        assert!(keys.contains(&"vaccine_shingles")); // 50+
    }

    #[test]
    fn unknown_age_returns_empty() {
        let demo = make_demographics(Some(BiologicalSex::Female), None);
        let insights = detect_screening_due(Some(&demo));
        assert!(insights.is_empty());
    }

    #[test]
    fn all_screening_insights_are_info_severity() {
        let demo = make_demographics(Some(BiologicalSex::Female), Some(70));
        let insights = detect_screening_due(Some(&demo));
        assert!(!insights.is_empty());
        for insight in &insights {
            assert_eq!(insight.severity, InsightSeverity::Info);
            assert_eq!(insight.kind, InsightKind::ScreeningDue);
        }
    }

    #[test]
    fn boundary_age_50_female() {
        let demo = make_demographics(Some(BiologicalSex::Female), Some(50));
        let insights = detect_screening_due(Some(&demo));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        assert!(keys.contains(&"screening_mammography"));
        assert!(keys.contains(&"screening_cervical"));
        assert!(keys.contains(&"screening_colorectal"));
        assert!(keys.contains(&"vaccine_shingles")); // 50+ boundary
    }

    #[test]
    fn boundary_age_75_male() {
        let demo = make_demographics(Some(BiologicalSex::Male), Some(75));
        let insights = detect_screening_due(Some(&demo));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        assert!(keys.contains(&"screening_colorectal"));
        assert!(keys.contains(&"screening_aaa"));
        assert!(!keys.contains(&"screening_prostate")); // Stops at 70
    }

    #[test]
    fn age_above_all_ranges() {
        let demo = make_demographics(Some(BiologicalSex::Female), Some(80));
        let insights = detect_screening_due(Some(&demo));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        assert!(keys.contains(&"screening_osteoporosis")); // No upper limit
        // Vaccines with no upper limit
        assert!(keys.contains(&"vaccine_influenza"));
        assert!(keys.contains(&"vaccine_tdap"));
        assert!(keys.contains(&"vaccine_pneumococcal")); // 65+
        assert!(keys.contains(&"vaccine_shingles")); // 50+
    }
}
