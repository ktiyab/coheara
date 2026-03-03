//! ME-04: Evidence-based preventive screening schedules.
//!
//! Age+sex-gated, deterministic. No LLM involved.
//! Produces `InsightKind::ScreeningDue` insights when a patient meets
//! the eligibility criteria for a preventive screening.
//!
//! Phase 1: Generate reminders for eligible patients based on demographics.
//! Phase 2 (future): Check against screening records to suppress if already done.

use crate::crypto::profile::{BiologicalSex, PatientDemographics};
use crate::invariants::types::{
    ClinicalInsight, InsightKind, InsightSeverity, InvariantLabel, MeaningFactors,
};

// ═══════════════════════════════════════════════════════════
// Screening schedule definition
// ═══════════════════════════════════════════════════════════

/// A preventive screening schedule with eligibility criteria.
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
}

// ═══════════════════════════════════════════════════════════
// 6 evidence-based screening schedules
// ═══════════════════════════════════════════════════════════

pub static SCREENING_SCHEDULES: &[ScreeningSchedule] = &[
    // 1. Mammography — IARC/WHO 2024
    ScreeningSchedule {
        key: "screening_mammography",
        label: InvariantLabel {
            key: "screening_mammography",
            en: "Mammography screening recommended",
            fr: "Dépistage par mammographie recommandé",
            de: "Mammographie-Screening empfohlen",
        },
        source: "IARC/WHO 2024",
        sex: Some(BiologicalSex::Female),
        min_age: 50,
        max_age: Some(74),
        interval_months: 24,
    },
    // 2. Cervical (Pap/HPV) — WHO 2021
    ScreeningSchedule {
        key: "screening_cervical",
        label: InvariantLabel {
            key: "screening_cervical",
            en: "Cervical cancer screening recommended",
            fr: "Dépistage du cancer du col de l'utérus recommandé",
            de: "Gebärmutterhalskrebs-Screening empfohlen",
        },
        source: "WHO 2021",
        sex: Some(BiologicalSex::Female),
        min_age: 25,
        max_age: Some(65),
        interval_months: 36,
    },
    // 3. Prostate (PSA) — EAU 2024
    ScreeningSchedule {
        key: "screening_prostate",
        label: InvariantLabel {
            key: "screening_prostate",
            en: "Prostate cancer screening (PSA) recommended",
            fr: "Dépistage du cancer de la prostate (PSA) recommandé",
            de: "Prostatakrebs-Screening (PSA) empfohlen",
        },
        source: "EAU 2024",
        sex: Some(BiologicalSex::Male),
        min_age: 50,
        max_age: Some(70),
        interval_months: 12,
    },
    // 4. Colorectal (FIT/colonoscopy) — IARC 2019
    ScreeningSchedule {
        key: "screening_colorectal",
        label: InvariantLabel {
            key: "screening_colorectal",
            en: "Colorectal cancer screening recommended",
            fr: "Dépistage du cancer colorectal recommandé",
            de: "Darmkrebs-Screening empfohlen",
        },
        source: "IARC 2019",
        sex: None, // Both sexes
        min_age: 50,
        max_age: Some(75),
        interval_months: 24,
    },
    // 5. AAA Ultrasound — ESC 2024
    ScreeningSchedule {
        key: "screening_aaa",
        label: InvariantLabel {
            key: "screening_aaa",
            en: "Abdominal aortic aneurysm screening recommended",
            fr: "Dépistage de l'anévrisme de l'aorte abdominale recommandé",
            de: "Bauchaortenaneurysma-Screening empfohlen",
        },
        source: "ESC 2024",
        sex: Some(BiologicalSex::Male),
        min_age: 65,
        max_age: Some(75),
        interval_months: 0, // One-time
    },
    // 6. Osteoporosis (DXA) — IOF 2024
    ScreeningSchedule {
        key: "screening_osteoporosis",
        label: InvariantLabel {
            key: "screening_osteoporosis",
            en: "Osteoporosis screening (DXA) recommended",
            fr: "Dépistage de l'ostéoporose (DXA) recommandé",
            de: "Osteoporose-Screening (DXA) empfohlen",
        },
        source: "IOF 2024",
        sex: Some(BiologicalSex::Female),
        min_age: 65,
        max_age: None, // No upper limit
        interval_months: 24,
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
            None => return false,    // Unknown sex — can't match sex-gated screening
        }
    }

    // Age gate: patient must have known age within range
    let age = match demographics.age_years {
        Some(age) => age,
        None => return false, // Unknown age — can't match age-gated screening
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

/// Detect preventive screenings that are due based on patient demographics.
///
/// Phase 1: Generates reminders for all eligible screenings (no check
/// against prior screening records — those don't exist in DB yet).
pub fn detect_screening_due(
    demographics: Option<&PatientDemographics>,
) -> Vec<ClinicalInsight> {
    let demographics = match demographics {
        Some(d) => d,
        None => return vec![], // No demographics → no screening insights
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
                significance: 0.3, // Low — reminder, not alarm
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

    #[test]
    fn screening_schedules_count() {
        assert_eq!(SCREENING_SCHEDULES.len(), 6);
    }

    #[test]
    fn female_55_gets_mammography_cervical_colorectal() {
        let demo = make_demographics(Some(BiologicalSex::Female), Some(55));
        let insights = detect_screening_due(Some(&demo));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        assert!(keys.contains(&"screening_mammography"));
        assert!(keys.contains(&"screening_cervical"));
        assert!(keys.contains(&"screening_colorectal"));
        assert!(!keys.contains(&"screening_prostate"));
        assert!(!keys.contains(&"screening_aaa"));
        assert!(!keys.contains(&"screening_osteoporosis"));
    }

    #[test]
    fn male_55_gets_prostate_colorectal() {
        let demo = make_demographics(Some(BiologicalSex::Male), Some(55));
        let insights = detect_screening_due(Some(&demo));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        assert!(keys.contains(&"screening_prostate"));
        assert!(keys.contains(&"screening_colorectal"));
        assert!(!keys.contains(&"screening_mammography"));
        assert!(!keys.contains(&"screening_cervical"));
    }

    #[test]
    fn female_30_gets_cervical_only() {
        let demo = make_demographics(Some(BiologicalSex::Female), Some(30));
        let insights = detect_screening_due(Some(&demo));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        assert_eq!(keys, vec!["screening_cervical"]);
    }

    #[test]
    fn male_40_gets_no_screenings() {
        let demo = make_demographics(Some(BiologicalSex::Male), Some(40));
        let insights = detect_screening_due(Some(&demo));
        assert!(insights.is_empty());
    }

    #[test]
    fn female_70_gets_mammography_colorectal_osteoporosis() {
        let demo = make_demographics(Some(BiologicalSex::Female), Some(70));
        let insights = detect_screening_due(Some(&demo));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        assert!(keys.contains(&"screening_mammography"));
        assert!(keys.contains(&"screening_colorectal"));
        assert!(keys.contains(&"screening_osteoporosis"));
        // Cervical stops at 65
        assert!(!keys.contains(&"screening_cervical"));
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
    fn unknown_sex_gets_sex_neutral_screenings_only() {
        let demo = make_demographics(None, Some(55));
        let insights = detect_screening_due(Some(&demo));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        // Only colorectal is sex-neutral
        assert_eq!(keys, vec!["screening_colorectal"]);
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
        // Age 50 is min for mammography and colorectal
        let demo = make_demographics(Some(BiologicalSex::Female), Some(50));
        let insights = detect_screening_due(Some(&demo));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        assert!(keys.contains(&"screening_mammography"));
        assert!(keys.contains(&"screening_cervical"));
        assert!(keys.contains(&"screening_colorectal"));
    }

    #[test]
    fn boundary_age_75_male() {
        // Age 75 is max for colorectal and AAA
        let demo = make_demographics(Some(BiologicalSex::Male), Some(75));
        let insights = detect_screening_due(Some(&demo));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        assert!(keys.contains(&"screening_colorectal"));
        assert!(keys.contains(&"screening_aaa"));
        // Prostate stops at 70
        assert!(!keys.contains(&"screening_prostate"));
    }

    #[test]
    fn age_above_all_ranges() {
        // Age 80 female — only osteoporosis (no upper limit)
        let demo = make_demographics(Some(BiologicalSex::Female), Some(80));
        let insights = detect_screening_due(Some(&demo));
        let keys: Vec<&str> = insights.iter().map(|i| i.summary_key.as_str()).collect();
        assert_eq!(keys, vec!["screening_osteoporosis"]);
    }
}
