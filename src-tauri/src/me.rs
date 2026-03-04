//! L3-06 + ME-REDESIGN: Me Screen — Rich health information center.
//!
//! Orchestrates data fetch + invariant enrichment + reference range assembly
//! into a frontend-ready `MeOverview` response.
//!
//! Key change from L3-06: Reference ranges from const invariant data are
//! ALWAYS populated (16 entries: 6 vitals + 10 labs), even without user data.
//! This solves the "empty on first load" problem.

use std::sync::Arc;

use chrono::{Local, Months, NaiveDate, NaiveDateTime};
use rusqlite::Connection;
use serde::Serialize;

use crate::core_state::{CoreError, CoreState};
use crate::crypto::profile::{BiologicalSex, PatientDemographics};
use crate::invariants::enrich::enrich;
use crate::invariants::labs::{self, ALL_LAB_THRESHOLDS};
use crate::db::ScreeningRecord;
use crate::invariants::screening::{ScreeningSchedule, SCREENING_SCHEDULES};
use crate::invariants::types::{ClinicalInsight, InsightKind, InsightSeverity, InvariantLabel};
use crate::invariants::vitals;
use crate::models::{LabResult, VitalSign, VitalType};

// ═══════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════

/// Complete Me screen payload — single IPC response.
#[derive(Debug, Clone, Serialize)]
pub struct MeOverview {
    pub identity: MeIdentity,
    pub alerts: Vec<MeInsight>,
    pub allergies: Vec<AllergyInfo>,
    pub reference_ranges: Vec<ReferenceRange>,
    pub screenings: Vec<ScreeningInfo>,
}

/// ALLERGY-01 B6: Allergy info for the Me screen allergy section.
#[derive(Debug, Clone, Serialize)]
pub struct AllergyInfo {
    pub id: String,
    pub allergen: String,
    pub reaction: Option<String>,
    pub severity: String,
    pub category: Option<String>,
    pub date_identified: Option<String>,
    pub source: String,
    pub verified: bool,
    pub cross_reactivities: Vec<String>,
}

/// Identity zone: who the user is.
///
/// ME-04: Extended with demographics for ProfileCard — single source of truth
/// for the Me screen header (profile completeness, edit capability).
#[derive(Debug, Clone, Serialize)]
pub struct MeIdentity {
    pub profile_id: String,
    pub name: String,
    pub age: Option<u32>,
    pub sex: Option<String>,
    pub ethnicities: Vec<String>,
    /// BT-01: Blood type key (e.g. "o_positive").
    pub blood_type: Option<String>,
    /// BT-01: Human-readable blood type display (e.g. "O+").
    pub blood_type_display: Option<String>,
    pub weight_kg: Option<f64>,
    pub height_cm: Option<f64>,
    pub bmi: Option<f64>,
    pub medication_count: usize,
    pub allergy_count: usize,
}

/// Flattened clinical insight — language-resolved, enums as strings.
#[derive(Debug, Clone, Serialize)]
pub struct MeInsight {
    pub kind: String,
    pub severity: String,
    pub summary_key: String,
    pub description: String,
    pub source: String,
}

/// A single tier in a reference range (for range bar visualization).
#[derive(Debug, Clone, Serialize)]
pub struct RangeTier {
    pub key: String,
    pub label: String,
    pub min_value: f64,
    pub max_value: f64,
    pub color: String,
}

/// A complete reference range for one vital/lab metric.
#[derive(Debug, Clone, Serialize)]
pub struct ReferenceRange {
    pub key: String,
    pub label: String,
    pub domain: String,
    pub unit: String,
    pub source: String,
    pub tiers: Vec<RangeTier>,
    pub normal_min: f64,
    pub normal_max: f64,
    pub current_value: Option<f64>,
    pub current_display: Option<String>,
    pub current_tier_label: Option<String>,
}

/// Screening schedule info (richer than MeInsight).
#[derive(Debug, Clone, Serialize)]
pub struct ScreeningInfo {
    pub key: String,
    pub label: String,
    pub source: String,
    pub interval_months: u16,
    pub eligible: bool,
    pub min_age: u16,
    pub max_age: Option<u16>,
    pub sex_required: Option<String>,
    // ME-06: Vaccine/screening record fields
    pub category: String,
    pub total_doses: u16,
    pub validity_months: Option<u16>,
    pub completed_doses: Vec<CompletedDose>,
    pub next_due: Option<String>,
    pub is_complete: bool,
}

/// A single completed dose/screening record.
#[derive(Debug, Clone, Serialize)]
pub struct CompletedDose {
    pub record_id: String,
    pub dose_number: i32,
    pub completed_at: String,
    pub provider: Option<String>,
}

// ═══════════════════════════════════════════════════════════
// Metric name labels (trilingual)
// ═══════════════════════════════════════════════════════════

const METRIC_BP: InvariantLabel = InvariantLabel {
    key: "bp",
    en: "Blood Pressure",
    fr: "Pression artérielle",
    de: "Blutdruck",
};
const METRIC_HR: InvariantLabel = InvariantLabel {
    key: "hr",
    en: "Heart Rate",
    fr: "Fréquence cardiaque",
    de: "Herzfrequenz",
};
const METRIC_SPO2: InvariantLabel = InvariantLabel {
    key: "spo2",
    en: "Oxygen Saturation",
    fr: "Saturation en oxygène",
    de: "Sauerstoffsättigung",
};
const METRIC_BMI: InvariantLabel = InvariantLabel {
    key: "bmi",
    en: "Body Mass Index",
    fr: "Indice de masse corporelle",
    de: "Body-Mass-Index",
};
const METRIC_GLUCOSE: InvariantLabel = InvariantLabel {
    key: "glucose",
    en: "Fasting Glucose",
    fr: "Glycémie à jeun",
    de: "Nüchternglukose",
};
const METRIC_TEMP: InvariantLabel = InvariantLabel {
    key: "temp",
    en: "Body Temperature",
    fr: "Température corporelle",
    de: "Körpertemperatur",
};
const METRIC_EGFR: InvariantLabel = InvariantLabel {
    key: "egfr",
    en: "Kidney Function (eGFR)",
    fr: "Fonction rénale (DFG)",
    de: "Nierenfunktion (eGFR)",
};
const METRIC_HBA1C: InvariantLabel = InvariantLabel {
    key: "hba1c",
    en: "HbA1c (Blood Sugar)",
    fr: "HbA1c (Glycémie)",
    de: "HbA1c (Blutzucker)",
};
const METRIC_LDL: InvariantLabel = InvariantLabel {
    key: "ldl",
    en: "LDL Cholesterol",
    fr: "Cholestérol LDL",
    de: "LDL-Cholesterin",
};
const METRIC_POTASSIUM: InvariantLabel = InvariantLabel {
    key: "k",
    en: "Potassium",
    fr: "Potassium",
    de: "Kalium",
};
const METRIC_SODIUM: InvariantLabel = InvariantLabel {
    key: "na",
    en: "Sodium",
    fr: "Sodium",
    de: "Natrium",
};
const METRIC_ALT: InvariantLabel = InvariantLabel {
    key: "alt",
    en: "Liver Function (ALT)",
    fr: "Fonction hépatique (ALAT)",
    de: "Leberfunktion (ALT)",
};
const METRIC_HB: InvariantLabel = InvariantLabel {
    key: "hb",
    en: "Hemoglobin",
    fr: "Hémoglobine",
    de: "Hämoglobin",
};
const METRIC_TSH: InvariantLabel = InvariantLabel {
    key: "tsh",
    en: "Thyroid (TSH)",
    fr: "Thyroïde (TSH)",
    de: "Schilddrüse (TSH)",
};
const METRIC_UACR: InvariantLabel = InvariantLabel {
    key: "uacr",
    en: "Urine Albumin (uACR)",
    fr: "Albumine urinaire (RAC)",
    de: "Urin-Albumin (uACR)",
};
const METRIC_VITD: InvariantLabel = InvariantLabel {
    key: "vitd",
    en: "Vitamin D",
    fr: "Vitamine D",
    de: "Vitamin D",
};

// ═══════════════════════════════════════════════════════════
// Conversion helpers
// ═══════════════════════════════════════════════════════════

fn insight_kind_str(kind: &InsightKind) -> &'static str {
    match kind {
        InsightKind::Classification => "classification",
        InsightKind::Interaction => "interaction",
        InsightKind::CrossReactivity => "cross_reactivity",
        InsightKind::MissingMonitoring => "missing_monitoring",
        InsightKind::ScreeningDue => "screening_due",
        InsightKind::AbnormalTrend => "abnormal_trend",
    }
}

fn severity_str(severity: &InsightSeverity) -> &'static str {
    match severity {
        InsightSeverity::Info => "info",
        InsightSeverity::Warning => "warning",
        InsightSeverity::Critical => "critical",
    }
}

fn flatten_insight(insight: &ClinicalInsight, lang: &str) -> MeInsight {
    MeInsight {
        kind: insight_kind_str(&insight.kind).to_string(),
        severity: severity_str(&insight.severity).to_string(),
        summary_key: insight.summary_key.clone(),
        description: insight.description.get(lang).to_string(),
        source: insight.source.clone(),
    }
}

/// Map invariant significance score (0.2–2.0) to a display color.
fn significance_to_color(significance: f64) -> &'static str {
    if significance <= 0.3 {
        "green"
    } else if significance <= 0.7 {
        "yellow"
    } else if significance <= 1.2 {
        "orange"
    } else {
        "red"
    }
}

/// Worst severity from a list (Critical > Warning > Info).
#[cfg(test)]
fn worst_severity(severities: &[&InsightSeverity]) -> &'static str {
    if severities.iter().any(|s| **s == InsightSeverity::Critical) {
        "critical"
    } else if severities.iter().any(|s| **s == InsightSeverity::Warning) {
        "warning"
    } else if !severities.is_empty() {
        "info"
    } else {
        "none"
    }
}

/// Cap a tier bound to a sensible display range.
fn cap_bound(value: f64, display_min: f64, display_max: f64) -> f64 {
    if value <= display_min || value == 0.0 {
        display_min
    } else if value >= display_max || value == f64::MAX {
        display_max
    } else {
        value
    }
}

// ═══════════════════════════════════════════════════════════
// Reference range builders
// ═══════════════════════════════════════════════════════════

/// Find the latest vital sign of a given type.
fn latest_vital_by_type<'a>(vitals: &'a [VitalSign], vtype: VitalType) -> Option<&'a VitalSign> {
    vitals
        .iter()
        .filter(|v| v.vital_type == vtype)
        .max_by_key(|v| v.recorded_at)
}

/// Find the latest lab result matching a threshold's aliases.
fn latest_lab_for_threshold<'a>(
    labs: &'a [LabResult],
    threshold: &labs::LabThreshold,
) -> Option<&'a LabResult> {
    labs.iter()
        .filter(|l| {
            let test_lower = l.test_name.trim().to_lowercase();
            threshold.test_key == test_lower
                || threshold
                    .aliases
                    .iter()
                    .any(|a| a.to_lowercase() == test_lower)
        })
        .filter(|l| l.value.is_some())
        .max_by_key(|l| l.collection_date)
}

/// Build a ReferenceRange from blood pressure tiers.
fn build_bp_range(lang: &str, vitals_data: &[VitalSign]) -> ReferenceRange {
    let display_min = 80.0;
    let display_max = 200.0;
    let tiers: Vec<RangeTier> = vitals::BP_CLASSIFICATIONS
        .iter()
        .map(|t| RangeTier {
            key: t.label.key.to_string(),
            label: t.label.get(lang).to_string(),
            min_value: cap_bound(t.systolic_min as f64, display_min, display_max),
            max_value: cap_bound(t.systolic_max as f64, display_min, display_max),
            color: significance_to_color(t.significance).to_string(),
        })
        .collect();

    let latest = latest_vital_by_type(vitals_data, VitalType::BloodPressure);
    let (current_value, current_display, current_tier_label) = match latest {
        Some(v) => {
            let sys = v.value_primary as u16;
            let dia = v.value_secondary.unwrap_or(0.0) as u16;
            let tier = vitals::classify_bp(sys, dia);
            (
                Some(v.value_primary),
                Some(format!("{}/{} mmHg", sys, dia)),
                Some(tier.label.get(lang).to_string()),
            )
        }
        None => (None, None, None),
    };

    ReferenceRange {
        key: "blood_pressure".to_string(),
        label: METRIC_BP.get(lang).to_string(),
        domain: "vitals".to_string(),
        unit: "mmHg".to_string(),
        source: "ISH 2020".to_string(),
        tiers,
        normal_min: 80.0,
        normal_max: 130.0,
        current_value,
        current_display,
        current_tier_label,
    }
}

/// Build a ReferenceRange from heart rate tiers.
fn build_hr_range(lang: &str, vitals_data: &[VitalSign]) -> ReferenceRange {
    let display_min = 30.0;
    let display_max = 200.0;
    let tiers: Vec<RangeTier> = vitals::HR_CLASSIFICATIONS
        .iter()
        .map(|t| RangeTier {
            key: t.label.key.to_string(),
            label: t.label.get(lang).to_string(),
            min_value: cap_bound(t.bpm_min as f64, display_min, display_max),
            max_value: cap_bound(t.bpm_max as f64, display_min, display_max),
            color: significance_to_color(t.significance).to_string(),
        })
        .collect();

    let latest = latest_vital_by_type(vitals_data, VitalType::HeartRate);
    let (current_value, current_display, current_tier_label) = match latest {
        Some(v) => {
            let bpm = v.value_primary as u16;
            let tier = vitals::classify_hr(bpm);
            (
                Some(v.value_primary),
                Some(format!("{} bpm", bpm)),
                Some(tier.label.get(lang).to_string()),
            )
        }
        None => (None, None, None),
    };

    ReferenceRange {
        key: "heart_rate".to_string(),
        label: METRIC_HR.get(lang).to_string(),
        domain: "vitals".to_string(),
        unit: "bpm".to_string(),
        source: "ESC 2021".to_string(),
        tiers,
        normal_min: 60.0,
        normal_max: 101.0,
        current_value,
        current_display,
        current_tier_label,
    }
}

/// Build a ReferenceRange from SpO2 tiers (skip COPD tier).
fn build_spo2_range(lang: &str, vitals_data: &[VitalSign]) -> ReferenceRange {
    let display_min = 80.0;
    let display_max = 100.0;
    let tiers: Vec<RangeTier> = vitals::SPO2_CLASSIFICATIONS
        .iter()
        .filter(|t| t.label.key != "spo2_copd_target")
        .map(|t| RangeTier {
            key: t.label.key.to_string(),
            label: t.label.get(lang).to_string(),
            min_value: cap_bound(t.min_pct as f64, display_min, display_max),
            max_value: cap_bound(t.max_pct as f64, display_min, display_max),
            color: significance_to_color(t.significance).to_string(),
        })
        .collect();

    let latest = latest_vital_by_type(vitals_data, VitalType::OxygenSaturation);
    let (current_value, current_display, current_tier_label) = match latest {
        Some(v) => {
            let pct = v.value_primary as u8;
            let tier = vitals::classify_spo2(pct);
            (
                Some(v.value_primary),
                Some(format!("{}%", pct)),
                Some(tier.label.get(lang).to_string()),
            )
        }
        None => (None, None, None),
    };

    ReferenceRange {
        key: "spo2".to_string(),
        label: METRIC_SPO2.get(lang).to_string(),
        domain: "vitals".to_string(),
        unit: "%".to_string(),
        source: "BTS 2017".to_string(),
        tiers,
        normal_min: 95.0,
        normal_max: 100.0,
        current_value,
        current_display,
        current_tier_label,
    }
}

/// Build a ReferenceRange from BMI tiers (ethnicity-aware).
fn build_bmi_range(
    lang: &str,
    vitals_data: &[VitalSign],
    demographics: Option<&PatientDemographics>,
) -> ReferenceRange {
    let display_min = 15.0;
    let display_max = 45.0;

    let use_asian = demographics
        .map(|d| d.has_asian_bmi_thresholds())
        .unwrap_or(false);

    let (bmi_tiers, source) = if use_asian {
        (
            crate::invariants::demographics::BMI_ASIAN_CLASSIFICATIONS,
            "WHO Expert Consultation 2004",
        )
    } else {
        (vitals::BMI_CLASSIFICATIONS, "WHO TRS 894")
    };

    let tiers: Vec<RangeTier> = bmi_tiers
        .iter()
        .map(|t| RangeTier {
            key: t.label.key.to_string(),
            label: t.label.get(lang).to_string(),
            min_value: cap_bound(t.min_bmi, display_min, display_max),
            max_value: cap_bound(t.max_bmi, display_min, display_max),
            color: significance_to_color(t.significance).to_string(),
        })
        .collect();

    // Compute BMI from latest weight + height
    let latest_weight = latest_vital_by_type(vitals_data, VitalType::Weight);
    let latest_height = latest_vital_by_type(vitals_data, VitalType::Height);
    let computed_bmi = latest_weight
        .zip(latest_height)
        .and_then(|(w, h)| vitals::compute_bmi(w.value_primary, h.value_primary));

    let (current_value, current_display, current_tier_label) = match computed_bmi {
        Some(bmi) => {
            let tier = if use_asian {
                crate::invariants::demographics::classify_bmi_asian(bmi)
            } else {
                vitals::classify_bmi(bmi)
            };
            (
                Some(bmi),
                Some(format!("{:.1} kg/m²", bmi)),
                Some(tier.label.get(lang).to_string()),
            )
        }
        None => (None, None, None),
    };

    let normal_max = if use_asian { 23.0 } else { 25.0 };
    ReferenceRange {
        key: "bmi".to_string(),
        label: METRIC_BMI.get(lang).to_string(),
        domain: "vitals".to_string(),
        unit: "kg/m²".to_string(),
        source: source.to_string(),
        tiers,
        normal_min: 18.5,
        normal_max,
        current_value,
        current_display,
        current_tier_label,
    }
}

/// Build a ReferenceRange from fasting glucose tiers.
fn build_glucose_range(lang: &str, vitals_data: &[VitalSign]) -> ReferenceRange {
    let display_min = 3.0;
    let display_max = 12.0;
    let tiers: Vec<RangeTier> = vitals::GLUCOSE_CLASSIFICATIONS
        .iter()
        .map(|t| RangeTier {
            key: t.label.key.to_string(),
            label: t.label.get(lang).to_string(),
            min_value: cap_bound(t.min_mmol, display_min, display_max),
            max_value: cap_bound(t.max_mmol, display_min, display_max),
            color: significance_to_color(t.significance).to_string(),
        })
        .collect();

    let latest = latest_vital_by_type(vitals_data, VitalType::BloodGlucose);
    let (current_value, current_display, current_tier_label) = match latest {
        Some(v) => {
            let tier = vitals::classify_glucose_mmol(v.value_primary);
            (
                Some(v.value_primary),
                Some(format!("{:.1} mmol/L", v.value_primary)),
                Some(tier.label.get(lang).to_string()),
            )
        }
        None => (None, None, None),
    };

    ReferenceRange {
        key: "glucose".to_string(),
        label: METRIC_GLUCOSE.get(lang).to_string(),
        domain: "vitals".to_string(),
        unit: "mmol/L".to_string(),
        source: "WHO 2006".to_string(),
        tiers,
        normal_min: 3.0,
        normal_max: 6.1,
        current_value,
        current_display,
        current_tier_label,
    }
}

/// Build a ReferenceRange from temperature tiers.
fn build_temp_range(lang: &str, vitals_data: &[VitalSign]) -> ReferenceRange {
    let display_min = 34.0;
    let display_max = 42.0;
    let tiers: Vec<RangeTier> = vitals::TEMP_CLASSIFICATIONS
        .iter()
        .map(|t| RangeTier {
            key: t.label.key.to_string(),
            label: t.label.get(lang).to_string(),
            min_value: cap_bound(t.min_celsius, display_min, display_max),
            max_value: cap_bound(t.max_celsius, display_min, display_max),
            color: significance_to_color(t.significance).to_string(),
        })
        .collect();

    let latest = latest_vital_by_type(vitals_data, VitalType::Temperature);
    let (current_value, current_display, current_tier_label) = match latest {
        Some(v) => {
            let tier = vitals::classify_temperature(v.value_primary);
            (
                Some(v.value_primary),
                Some(format!("{:.1} °C", v.value_primary)),
                Some(tier.label.get(lang).to_string()),
            )
        }
        None => (None, None, None),
    };

    ReferenceRange {
        key: "temperature".to_string(),
        label: METRIC_TEMP.get(lang).to_string(),
        domain: "vitals".to_string(),
        unit: "°C".to_string(),
        source: "Clinical standard".to_string(),
        tiers,
        normal_min: 36.1,
        normal_max: 37.2,
        current_value,
        current_display,
        current_tier_label,
    }
}

/// Display-cap ranges for each lab test (reasonable clinical visualization bounds).
fn lab_display_caps(test_key: &str) -> (f64, f64) {
    match test_key {
        "egfr" => (0.0, 120.0),
        "hba1c" => (4.0, 12.0),
        "ldl_cholesterol" => (0.0, 5.0),
        "potassium" => (2.0, 7.0),
        "sodium" => (120.0, 155.0),
        "alt" => (0.0, 300.0),
        "hemoglobin" => (5.0, 20.0),
        "tsh" => (0.0, 15.0),
        "uacr" => (0.0, 500.0),
        "vitamin_d" => (0.0, 120.0),
        _ => (0.0, 100.0),
    }
}

/// Metric label for a lab test key.
fn lab_metric_label(test_key: &str) -> &'static InvariantLabel {
    match test_key {
        "egfr" => &METRIC_EGFR,
        "hba1c" => &METRIC_HBA1C,
        "ldl_cholesterol" => &METRIC_LDL,
        "potassium" => &METRIC_POTASSIUM,
        "sodium" => &METRIC_SODIUM,
        "alt" => &METRIC_ALT,
        "hemoglobin" => &METRIC_HB,
        "tsh" => &METRIC_TSH,
        "uacr" => &METRIC_UACR,
        "vitamin_d" => &METRIC_VITD,
        _ => &METRIC_EGFR, // fallback
    }
}

/// Build all lab reference ranges.
fn build_lab_ranges(
    lang: &str,
    labs_data: &[LabResult],
    demographics: Option<&PatientDemographics>,
) -> Vec<ReferenceRange> {
    ALL_LAB_THRESHOLDS
        .iter()
        .map(|threshold| {
            let (display_min, display_max) = lab_display_caps(threshold.test_key);

            // Use sex-specific hemoglobin tiers for males
            let use_male_hb = threshold.test_key == "hemoglobin"
                && demographics
                    .and_then(|d| d.sex)
                    .map(|s| s == BiologicalSex::Male)
                    .unwrap_or(false);

            let lab_tiers = if use_male_hb {
                crate::invariants::demographics::HEMOGLOBIN_TIERS_MALE
            } else {
                threshold.tiers
            };

            let tiers: Vec<RangeTier> = lab_tiers
                .iter()
                .map(|t| RangeTier {
                    key: t.label.key.to_string(),
                    label: t.label.get(lang).to_string(),
                    min_value: cap_bound(t.min_value, display_min, display_max),
                    max_value: cap_bound(t.max_value, display_min, display_max),
                    color: significance_to_color(t.significance).to_string(),
                })
                .collect();

            // Find normal tier (lowest significance)
            let normal_tier = lab_tiers
                .iter()
                .min_by(|a, b| a.significance.partial_cmp(&b.significance).unwrap());
            let (normal_min, normal_max) = normal_tier
                .map(|t| {
                    (
                        cap_bound(t.min_value, display_min, display_max),
                        cap_bound(t.max_value, display_min, display_max),
                    )
                })
                .unwrap_or((display_min, display_max));

            // Latest lab value
            let latest = latest_lab_for_threshold(labs_data, threshold);
            let (current_value, current_display, current_tier_label) = match latest {
                Some(l) => {
                    let val = l.value.unwrap_or(0.0);
                    let classified =
                        labs::classify_lab(val, threshold).map(|t| t.label.get(lang).to_string());
                    (
                        Some(val),
                        Some(format!("{:.1} {}", val, threshold.unit)),
                        classified,
                    )
                }
                None => (None, None, None),
            };

            let metric_label = lab_metric_label(threshold.test_key);

            ReferenceRange {
                key: threshold.test_key.to_string(),
                label: metric_label.get(lang).to_string(),
                domain: "labs".to_string(),
                unit: threshold.unit.to_string(),
                source: threshold.source.to_string(),
                tiers,
                normal_min,
                normal_max,
                current_value,
                current_display,
                current_tier_label,
            }
        })
        .collect()
}

/// Build all reference ranges (6 vitals + 10 labs = 16 total).
fn build_reference_ranges(
    lang: &str,
    demographics: Option<&PatientDemographics>,
    vitals_data: &[VitalSign],
    labs_data: &[LabResult],
) -> Vec<ReferenceRange> {
    let mut ranges = vec![
        build_bp_range(lang, vitals_data),
        build_hr_range(lang, vitals_data),
        build_spo2_range(lang, vitals_data),
        build_bmi_range(lang, vitals_data, demographics),
        build_glucose_range(lang, vitals_data),
        build_temp_range(lang, vitals_data),
    ];
    ranges.extend(build_lab_ranges(lang, labs_data, demographics));
    ranges
}

/// Build screening schedule info — sex-filtered when demographics available.
/// ME-04 B1: When user sex is known, exclude schedules requiring the opposite sex.
/// Compute validity status from schedule + records.
fn compute_screening_status(
    schedule: &ScreeningSchedule,
    records: &[ScreeningRecord],
    today: NaiveDate,
) -> (Vec<CompletedDose>, Option<String>, bool) {
    let completed: Vec<CompletedDose> = records
        .iter()
        .map(|r| CompletedDose {
            record_id: r.id.clone(),
            dose_number: r.dose_number,
            completed_at: r.completed_at.format("%Y-%m-%d").to_string(),
            provider: r.provider.clone(),
        })
        .collect();

    if schedule.total_doses == 0 {
        // Recurring: check most recent record + validity
        let latest = records.iter().max_by_key(|r| r.completed_at);
        match (latest, schedule.validity_months) {
            (Some(r), Some(validity)) => {
                let valid_until = r
                    .completed_at
                    .checked_add_months(Months::new(validity as u32))
                    .unwrap_or(r.completed_at);
                let is_valid = today <= valid_until;
                // Always send valid_until — frontend uses is_complete to show
                // "Valid until {date}" (green) vs "Expired" (amber)
                let next_due = Some(valid_until.format("%Y-%m-%d").to_string());
                (completed, next_due, is_valid)
            }
            (Some(_), None) => (completed, None, true), // No expiry
            (None, _) => (completed, None, false),       // Never done
        }
    } else {
        // Fixed series: check if all doses completed
        let done_count = records.len() as u16;
        let all_done = done_count >= schedule.total_doses;
        (completed, None, all_done)
    }
}

fn build_screening_info(
    lang: &str,
    demographics: Option<&PatientDemographics>,
    records: &[ScreeningRecord],
) -> Vec<ScreeningInfo> {
    let eligible_keys: Vec<String> =
        crate::invariants::screening::detect_screening_due(demographics)
            .into_iter()
            .map(|i| i.summary_key)
            .collect();

    let user_sex = demographics.and_then(|d| d.sex);
    let today = Local::now().date_naive();

    SCREENING_SCHEDULES
        .iter()
        // ME-04 B1: Exclude sex-incompatible schedules when user sex is known
        .filter(|s| match (user_sex, s.sex) {
            (Some(user), Some(required)) => user == required,
            (Some(_), None) => true,
            (None, _) => true,
        })
        .map(|s| {
            let matching_records: Vec<&ScreeningRecord> = records
                .iter()
                .filter(|r| r.screening_key == s.key)
                .collect();
            let owned: Vec<ScreeningRecord> = matching_records.iter().map(|r| (*r).clone()).collect();
            let (completed_doses, next_due, is_complete) =
                compute_screening_status(s, &owned, today);

            ScreeningInfo {
                key: s.key.to_string(),
                label: s.label.get(lang).to_string(),
                source: s.source.to_string(),
                interval_months: s.interval_months,
                eligible: eligible_keys.contains(&s.key.to_string()),
                min_age: s.min_age,
                max_age: s.max_age,
                sex_required: s.sex.map(|sex| match sex {
                    BiologicalSex::Male => "male".to_string(),
                    BiologicalSex::Female => "female".to_string(),
                }),
                category: s.category.as_str().to_string(),
                total_doses: s.total_doses,
                validity_months: s.validity_months,
                completed_doses,
                next_due,
                is_complete,
            }
        })
        .collect()
}

/// ALLERGY-01 B6: Build allergy info with cross-reactivity notes.
fn build_allergy_info(
    allergies: &[crate::models::Allergy],
    registry: &crate::invariants::InvariantRegistry,
) -> Vec<AllergyInfo> {
    allergies
        .iter()
        .map(|a| {
            let cross = registry.find_all_cross_reactivity(&a.allergen);
            let cross_notes: Vec<String> = cross
                .iter()
                .take(3)
                .map(|c| format!("{} ({})", c.cross_reactive, c.rate))
                .collect();

            AllergyInfo {
                id: a.id.to_string(),
                allergen: a.allergen.clone(),
                reaction: a.reaction.clone(),
                severity: a.severity.as_str().to_string(),
                category: a.allergen_category.as_ref().map(|c| c.as_str().to_string()),
                date_identified: a.date_identified.map(|d| d.to_string()),
                source: a.source.as_str().to_string(),
                verified: a.verified,
                cross_reactivities: cross_notes,
            }
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════
// Assembly
// ═══════════════════════════════════════════════════════════

/// Assemble the complete Me overview from current profile data.
///
/// `lang` is the UI locale passed from the frontend (e.g. "en", "fr", "de").
/// This ensures invariant labels match the user's active display language,
/// not the DB-stored preference which may default to "en".
pub fn assemble_me_overview(
    conn: &Connection,
    state: &Arc<CoreState>,
    lang: &str,
) -> Result<MeOverview, CoreError> {
    let demographics = state.get_patient_demographics();

    // Profile name + id from session
    let (profile_id, name) = {
        let guard = state.read_session()?;
        match guard.as_ref() {
            Some(s) => (s.profile_id.to_string(), s.profile_name.clone()),
            None => (String::new(), String::new()),
        }
    };

    // Age and sex from demographics
    let age = demographics
        .as_ref()
        .and_then(|d| d.age_years.map(|y| y as u32));
    let sex = demographics.as_ref().and_then(|d| {
        d.sex.map(|s| match s {
            BiologicalSex::Male => "male".to_string(),
            BiologicalSex::Female => "female".to_string(),
        })
    });
    let ethnicities: Vec<String> = demographics
        .as_ref()
        .map(|d| d.ethnicities.iter().map(|e| format!("{e:?}")).collect())
        .unwrap_or_default();

    // BT-01: Blood type from demographics + display from registry
    let blood_type = demographics
        .as_ref()
        .and_then(|d| d.blood_type.as_ref().map(|bt| bt.as_str().to_string()));
    let blood_type_display = blood_type.as_deref().and_then(|key| {
        crate::invariants::blood_types::find_blood_type(key).map(|info| info.display.to_string())
    });

    // Fetch entities (12-month window for labs and vitals)
    let today = Local::now().date_naive();
    let twelve_months_ago = today - chrono::Duration::days(365);

    let medications = crate::db::get_active_medications(conn).unwrap_or_default();
    let allergies = crate::db::get_all_allergies(conn).unwrap_or_default();
    let labs = crate::db::get_lab_results_since(conn, &twelve_months_ago).unwrap_or_default();

    let vitals_from = NaiveDateTime::new(twelve_months_ago, chrono::NaiveTime::MIN);
    let vitals_to = NaiveDateTime::new(
        today,
        chrono::NaiveTime::from_hms_opt(23, 59, 59).unwrap(),
    );
    let vitals =
        crate::db::get_vital_signs_in_range(conn, &vitals_from, &vitals_to).unwrap_or_default();

    // ME-04: Latest weight/height for identity card + BMI
    let weight_kg = crate::db::get_latest_vital_sign(conn, &VitalType::Weight)
        .ok()
        .flatten()
        .map(|v| v.value_primary);
    let height_cm = crate::db::get_latest_vital_sign(conn, &VitalType::Height)
        .ok()
        .flatten()
        .map(|v| v.value_primary);
    let bmi = match (weight_kg, height_cm) {
        (Some(w), Some(h)) => crate::invariants::vitals::compute_bmi(w, h),
        _ => None,
    };

    // Run enrichment
    let registry = state.invariants();
    let raw_insights = enrich(
        &medications,
        &labs,
        &allergies,
        &vitals,
        registry,
        today,
        demographics.as_ref(),
    );

    // Flatten insights — separate ScreeningDue from clinical alerts
    let alerts: Vec<MeInsight> = raw_insights
        .iter()
        .filter(|i| i.kind != InsightKind::ScreeningDue)
        .map(|i| flatten_insight(i, lang))
        .collect();

    // Build reference ranges (always 16 entries)
    let reference_ranges =
        build_reference_ranges(lang, demographics.as_ref(), &vitals, &labs);

    // ME-06: Load screening records and build info with record data
    let screening_records =
        crate::db::get_screening_records(conn, &profile_id).unwrap_or_default();
    let screenings = build_screening_info(lang, demographics.as_ref(), &screening_records);

    // ALLERGY-01 B6: Build allergy info with cross-reactivity notes
    let allergy_infos = build_allergy_info(&allergies, registry);

    Ok(MeOverview {
        identity: MeIdentity {
            profile_id,
            name,
            age,
            sex,
            ethnicities,
            blood_type,
            blood_type_display,
            weight_kg,
            height_cm,
            bmi,
            medication_count: medications.len(),
            allergy_count: allergies.len(),
        },
        alerts,
        allergies: allergy_infos,
        reference_ranges,
        screenings,
    })
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invariants::types::{InvariantLabel, MeaningFactors};
    use crate::models::enums::{AllergySeverity, AllergenCategory, AllergySource};
    use uuid::Uuid;

    // --- Insight helpers (kept from L3-06) ---

    #[test]
    fn insight_kind_to_string() {
        assert_eq!(insight_kind_str(&InsightKind::Classification), "classification");
        assert_eq!(insight_kind_str(&InsightKind::Interaction), "interaction");
        assert_eq!(insight_kind_str(&InsightKind::CrossReactivity), "cross_reactivity");
        assert_eq!(insight_kind_str(&InsightKind::MissingMonitoring), "missing_monitoring");
        assert_eq!(insight_kind_str(&InsightKind::ScreeningDue), "screening_due");
        assert_eq!(insight_kind_str(&InsightKind::AbnormalTrend), "abnormal_trend");
    }

    #[test]
    fn severity_to_string() {
        assert_eq!(severity_str(&InsightSeverity::Info), "info");
        assert_eq!(severity_str(&InsightSeverity::Warning), "warning");
        assert_eq!(severity_str(&InsightSeverity::Critical), "critical");
    }

    #[test]
    fn flatten_insight_resolves_language() {
        let insight = ClinicalInsight {
            kind: InsightKind::Classification,
            severity: InsightSeverity::Warning,
            summary_key: "bp_grade_1_htn".to_string(),
            description: InvariantLabel {
                key: "grade_1_htn",
                en: "Grade 1 Hypertension",
                fr: "Hypertension de grade 1",
                de: "Hypertonie Grad 1",
            },
            source: "ISH 2020".to_string(),
            related_entities: vec![Uuid::nil()],
            meaning_factors: MeaningFactors::default(),
        };

        let en = flatten_insight(&insight, "en");
        assert_eq!(en.description, "Grade 1 Hypertension");
        assert_eq!(en.kind, "classification");
        assert_eq!(en.severity, "warning");
        assert_eq!(en.source, "ISH 2020");

        let fr = flatten_insight(&insight, "fr");
        assert_eq!(fr.description, "Hypertension de grade 1");

        let de = flatten_insight(&insight, "de");
        assert_eq!(de.description, "Hypertonie Grad 1");
    }

    #[test]
    fn worst_severity_picks_critical() {
        let severities = vec![
            &InsightSeverity::Info,
            &InsightSeverity::Critical,
            &InsightSeverity::Warning,
        ];
        assert_eq!(worst_severity(&severities), "critical");
    }

    #[test]
    fn worst_severity_empty_is_none() {
        assert_eq!(worst_severity(&[]), "none");
    }

    #[test]
    fn worst_severity_info_only() {
        let severities = vec![&InsightSeverity::Info, &InsightSeverity::Info];
        assert_eq!(worst_severity(&severities), "info");
    }

    // --- Significance to color ---

    #[test]
    fn significance_to_color_mapping() {
        assert_eq!(significance_to_color(0.2), "green");
        assert_eq!(significance_to_color(0.3), "green");
        assert_eq!(significance_to_color(0.4), "yellow");
        assert_eq!(significance_to_color(0.7), "yellow");
        assert_eq!(significance_to_color(0.8), "orange");
        assert_eq!(significance_to_color(1.2), "orange");
        assert_eq!(significance_to_color(1.3), "red");
        assert_eq!(significance_to_color(2.0), "red");
    }

    // --- Reference ranges ---

    #[test]
    fn reference_ranges_always_16() {
        let ranges = build_reference_ranges("en", None, &[], &[]);
        assert_eq!(ranges.len(), 16);
    }

    #[test]
    fn reference_ranges_vitals_count_6() {
        let ranges = build_reference_ranges("en", None, &[], &[]);
        let vitals_count = ranges.iter().filter(|r| r.domain == "vitals").count();
        assert_eq!(vitals_count, 6);
    }

    #[test]
    fn reference_ranges_labs_count_10() {
        let ranges = build_reference_ranges("en", None, &[], &[]);
        let labs_count = ranges.iter().filter(|r| r.domain == "labs").count();
        assert_eq!(labs_count, 10);
    }

    #[test]
    fn reference_ranges_have_tiers() {
        let ranges = build_reference_ranges("en", None, &[], &[]);
        for range in &ranges {
            assert!(
                range.tiers.len() >= 2,
                "{} has only {} tiers",
                range.key,
                range.tiers.len()
            );
            for tier in &range.tiers {
                assert!(!tier.label.is_empty(), "{}/{} has empty label", range.key, tier.key);
            }
        }
    }

    #[test]
    fn reference_ranges_normal_zone_valid() {
        let ranges = build_reference_ranges("en", None, &[], &[]);
        for range in &ranges {
            assert!(
                range.normal_min < range.normal_max,
                "{}: normal_min {} >= normal_max {}",
                range.key,
                range.normal_min,
                range.normal_max
            );
        }
    }

    #[test]
    fn reference_ranges_tiers_capped() {
        let ranges = build_reference_ranges("en", None, &[], &[]);
        for range in &ranges {
            for tier in &range.tiers {
                assert!(
                    tier.max_value < f64::MAX,
                    "{}/{} has uncapped max_value",
                    range.key,
                    tier.key
                );
            }
        }
    }

    #[test]
    fn reference_ranges_no_current_when_empty() {
        let ranges = build_reference_ranges("en", None, &[], &[]);
        for range in &ranges {
            assert!(range.current_value.is_none(), "{} has current_value with no data", range.key);
        }
    }

    #[test]
    fn reference_ranges_sex_specific_hemoglobin() {
        use crate::crypto::profile::AgeContext;
        let male_demo = PatientDemographics {
            sex: Some(BiologicalSex::Male),
            ethnicities: vec![],
            age_context: Some(AgeContext::Adult),
            age_years: Some(45),
            blood_type: None,
        };
        let ranges = build_reference_ranges("en", Some(&male_demo), &[], &[]);
        let hb = ranges.iter().find(|r| r.key == "hemoglobin").unwrap();
        // Male normal starts at 13.0 (vs female 12.0)
        assert!(
            (hb.normal_min - 13.0).abs() < 0.1,
            "Male hemoglobin normal_min should be ~13.0, got {}",
            hb.normal_min
        );
    }

    #[test]
    fn reference_ranges_asian_bmi() {
        use crate::crypto::profile::{AgeContext, EthnicityGroup};
        let asian_demo = PatientDemographics {
            sex: Some(BiologicalSex::Female),
            ethnicities: vec![EthnicityGroup::EastAsian],
            age_context: Some(AgeContext::Adult),
            age_years: Some(35),
            blood_type: None,
        };
        let ranges = build_reference_ranges("en", Some(&asian_demo), &[], &[]);
        let bmi = ranges.iter().find(|r| r.key == "bmi").unwrap();
        // Asian normal_max is 23.0 (vs standard 25.0)
        assert!(
            (bmi.normal_max - 23.0).abs() < 0.1,
            "Asian BMI normal_max should be 23.0, got {}",
            bmi.normal_max
        );
    }

    #[test]
    fn reference_ranges_trilingual() {
        let en_ranges = build_reference_ranges("en", None, &[], &[]);
        let fr_ranges = build_reference_ranges("fr", None, &[], &[]);
        let de_ranges = build_reference_ranges("de", None, &[], &[]);

        let en_bp = en_ranges.iter().find(|r| r.key == "blood_pressure").unwrap();
        let fr_bp = fr_ranges.iter().find(|r| r.key == "blood_pressure").unwrap();
        let de_bp = de_ranges.iter().find(|r| r.key == "blood_pressure").unwrap();

        assert_eq!(en_bp.label, "Blood Pressure");
        assert_eq!(fr_bp.label, "Pression artérielle");
        assert_eq!(de_bp.label, "Blutdruck");

        // Tier labels should differ too
        assert_ne!(en_bp.tiers[0].label, fr_bp.tiers[0].label);
    }

    // --- Screening info ---

    // ME-04 B1: No demographics → no sex filter → all 14 schedules (6 cancer + 8 vaccine)
    #[test]
    fn screening_info_no_demographics_returns_all() {
        let screenings = build_screening_info("en", None, &[]);
        assert_eq!(screenings.len(), 14);
    }

    #[test]
    fn screening_info_none_eligible_without_demographics() {
        let screenings = build_screening_info("en", None, &[]);
        assert!(screenings.iter().all(|s| !s.eligible));
    }

    // ME-04 B1: Male user sees only male-compatible + sex-neutral screenings
    #[test]
    fn screening_info_male_excludes_female_schedules() {
        use crate::crypto::profile::AgeContext;
        let demo = PatientDemographics {
            sex: Some(BiologicalSex::Male),
            ethnicities: vec![],
            age_context: Some(AgeContext::Adult),
            age_years: Some(39),
            blood_type: None,
        };
        let screenings = build_screening_info("en", Some(&demo), &[]);
        // Male sees: prostate, colorectal, AAA (3 cancer) + 8 vaccines = 11
        assert_eq!(screenings.len(), 11);
        let keys: Vec<&str> = screenings.iter().map(|s| s.key.as_str()).collect();
        assert!(keys.contains(&"screening_prostate"));
        assert!(keys.contains(&"screening_colorectal"));
        assert!(keys.contains(&"screening_aaa"));
        assert!(!keys.contains(&"screening_mammography"));
        assert!(!keys.contains(&"screening_cervical"));
        assert!(!keys.contains(&"screening_osteoporosis"));
    }

    // ME-04 B1: Female user sees only female-compatible + sex-neutral screenings
    #[test]
    fn screening_info_female_excludes_male_schedules() {
        use crate::crypto::profile::AgeContext;
        let demo = PatientDemographics {
            sex: Some(BiologicalSex::Female),
            ethnicities: vec![],
            age_context: Some(AgeContext::Adult),
            age_years: Some(55),
            blood_type: None,
        };
        let screenings = build_screening_info("en", Some(&demo), &[]);
        // Female sees: mammography, cervical, colorectal, osteoporosis (4 cancer) + 8 vaccines = 12
        assert_eq!(screenings.len(), 12);
        let keys: Vec<&str> = screenings.iter().map(|s| s.key.as_str()).collect();
        assert!(keys.contains(&"screening_mammography"));
        assert!(keys.contains(&"screening_cervical"));
        assert!(keys.contains(&"screening_colorectal"));
        assert!(keys.contains(&"screening_osteoporosis"));
        assert!(!keys.contains(&"screening_prostate"));
        assert!(!keys.contains(&"screening_aaa"));
    }

    // ME-04 B1: Unknown sex → all 14 schedules, sex-gated ones have sex_required
    #[test]
    fn screening_info_unknown_sex_returns_all_with_sex_required() {
        use crate::crypto::profile::AgeContext;
        let demo = PatientDemographics {
            sex: None,
            ethnicities: vec![],
            age_context: Some(AgeContext::Adult),
            age_years: Some(55),
            blood_type: None,
        };
        let screenings = build_screening_info("en", Some(&demo), &[]);
        assert_eq!(screenings.len(), 14);
        let mammography = screenings.iter().find(|s| s.key == "screening_mammography").unwrap();
        assert_eq!(mammography.sex_required.as_deref(), Some("female"));
        let prostate = screenings.iter().find(|s| s.key == "screening_prostate").unwrap();
        assert_eq!(prostate.sex_required.as_deref(), Some("male"));
    }

    #[test]
    fn screening_info_eligible_female_55() {
        use crate::crypto::profile::AgeContext;
        let demo = PatientDemographics {
            sex: Some(BiologicalSex::Female),
            ethnicities: vec![],
            age_context: Some(AgeContext::Adult),
            age_years: Some(55),
            blood_type: None,
        };
        let screenings = build_screening_info("en", Some(&demo), &[]);
        // ME-04 B1: Female now sees 12 (4 cancer + 8 vaccines) — male schedules filtered
        assert_eq!(screenings.len(), 12);
        let eligible_keys: Vec<&str> = screenings
            .iter()
            .filter(|s| s.eligible)
            .map(|s| s.key.as_str())
            .collect();
        assert!(eligible_keys.contains(&"screening_mammography"));
        assert!(eligible_keys.contains(&"screening_cervical"));
        assert!(eligible_keys.contains(&"screening_colorectal"));
    }

    #[test]
    fn screening_info_has_metadata() {
        // No demographics → all 14 returned, verify metadata
        let screenings = build_screening_info("en", None, &[]);
        assert_eq!(screenings.len(), 14);
        for s in &screenings {
            assert!(!s.label.is_empty());
            assert!(!s.source.is_empty());
            assert!(s.min_age > 0);
        }
    }

    // --- Age / sex helpers ---

    #[test]
    fn age_from_demographics() {
        let age_years: Option<u16> = Some(45);
        let result = age_years.map(|y| y as u32);
        assert_eq!(result, Some(45));
    }

    #[test]
    fn sex_from_demographics() {
        let sex = Some(BiologicalSex::Female);
        let result = sex.map(|s| match s {
            BiologicalSex::Male => "male".to_string(),
            BiologicalSex::Female => "female".to_string(),
        });
        assert_eq!(result, Some("female".to_string()));
    }

    // ─── ME-06: Vaccine/screening validity tests ──────

    #[test]
    fn compute_recurring_valid() {
        use crate::invariants::screening::{ScreeningCategory, ScreeningSchedule};

        let schedule = ScreeningSchedule {
            key: "vaccine_flu",
            label: InvariantLabel { key: "flu", en: "Flu", fr: "Grippe", de: "Grippe" },
            source: "WHO",
            sex: None,
            min_age: 18,
            max_age: None,
            interval_months: 12,
            total_doses: 0,
            validity_months: Some(12),
            category: ScreeningCategory::Vaccine,
        };
        let record = ScreeningRecord {
            id: "r1".to_string(),
            profile_id: "p1".to_string(),
            screening_key: "vaccine_flu".to_string(),
            dose_number: 1,
            completed_at: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            provider: None,
            notes: None,
        };
        let today = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        let (doses, next_due, is_complete) =
            compute_screening_status(&schedule, &[record], today);
        assert_eq!(doses.len(), 1);
        assert!(is_complete); // Still valid (Jan + 12 months = Jan 2027)
        assert!(next_due.is_some());
    }

    #[test]
    fn compute_recurring_expired() {
        use crate::invariants::screening::{ScreeningCategory, ScreeningSchedule};

        let schedule = ScreeningSchedule {
            key: "vaccine_flu",
            label: InvariantLabel { key: "flu", en: "Flu", fr: "Grippe", de: "Grippe" },
            source: "WHO",
            sex: None,
            min_age: 18,
            max_age: None,
            interval_months: 12,
            total_doses: 0,
            validity_months: Some(12),
            category: ScreeningCategory::Vaccine,
        };
        let record = ScreeningRecord {
            id: "r1".to_string(),
            profile_id: "p1".to_string(),
            screening_key: "vaccine_flu".to_string(),
            dose_number: 1,
            completed_at: NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            provider: None,
            notes: None,
        };
        let today = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        let (_, next_due, is_complete) =
            compute_screening_status(&schedule, &[record], today);
        assert!(!is_complete); // Expired (Jan 2024 + 12m = Jan 2025 < Mar 2026)
        assert!(next_due.is_some()); // Always sends expiry date
        assert_eq!(next_due.unwrap(), "2025-01-15"); // Jan 2024 + 12mo
    }

    #[test]
    fn compute_multi_dose_complete() {
        use crate::invariants::screening::{ScreeningCategory, ScreeningSchedule};

        let schedule = ScreeningSchedule {
            key: "vaccine_hep_b",
            label: InvariantLabel { key: "hb", en: "HepB", fr: "HepB", de: "HepB" },
            source: "WHO",
            sex: None,
            min_age: 18,
            max_age: None,
            interval_months: 0,
            total_doses: 3,
            validity_months: None,
            category: ScreeningCategory::Vaccine,
        };
        let records = vec![
            ScreeningRecord {
                id: "r1".to_string(),
                profile_id: "p1".to_string(),
                screening_key: "vaccine_hep_b".to_string(),
                dose_number: 1,
                completed_at: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                provider: Some("Dr. A".to_string()),
                notes: None,
            },
            ScreeningRecord {
                id: "r2".to_string(),
                profile_id: "p1".to_string(),
                screening_key: "vaccine_hep_b".to_string(),
                dose_number: 2,
                completed_at: NaiveDate::from_ymd_opt(2025, 2, 1).unwrap(),
                provider: None,
                notes: None,
            },
            ScreeningRecord {
                id: "r3".to_string(),
                profile_id: "p1".to_string(),
                screening_key: "vaccine_hep_b".to_string(),
                dose_number: 3,
                completed_at: NaiveDate::from_ymd_opt(2025, 7, 1).unwrap(),
                provider: None,
                notes: None,
            },
        ];
        let today = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        let (doses, _, is_complete) =
            compute_screening_status(&schedule, &records, today);
        assert_eq!(doses.len(), 3);
        assert!(is_complete); // All 3 doses done
    }

    #[test]
    fn compute_multi_dose_partial() {
        use crate::invariants::screening::{ScreeningCategory, ScreeningSchedule};

        let schedule = ScreeningSchedule {
            key: "vaccine_hep_b",
            label: InvariantLabel { key: "hb", en: "HepB", fr: "HepB", de: "HepB" },
            source: "WHO",
            sex: None,
            min_age: 18,
            max_age: None,
            interval_months: 0,
            total_doses: 3,
            validity_months: None,
            category: ScreeningCategory::Vaccine,
        };
        let records = vec![
            ScreeningRecord {
                id: "r1".to_string(),
                profile_id: "p1".to_string(),
                screening_key: "vaccine_hep_b".to_string(),
                dose_number: 1,
                completed_at: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                provider: None,
                notes: None,
            },
        ];
        let today = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        let (doses, _, is_complete) =
            compute_screening_status(&schedule, &records, today);
        assert_eq!(doses.len(), 1);
        assert!(!is_complete); // Only 1 of 3 done
    }

    #[test]
    fn compute_no_records() {
        use crate::invariants::screening::{ScreeningCategory, ScreeningSchedule};

        let schedule = ScreeningSchedule {
            key: "vaccine_flu",
            label: InvariantLabel { key: "flu", en: "Flu", fr: "Grippe", de: "Grippe" },
            source: "WHO",
            sex: None,
            min_age: 18,
            max_age: None,
            interval_months: 12,
            total_doses: 0,
            validity_months: Some(12),
            category: ScreeningCategory::Vaccine,
        };
        let (doses, next_due, is_complete) =
            compute_screening_status(&schedule, &[], NaiveDate::from_ymd_opt(2026, 3, 1).unwrap());
        assert!(doses.is_empty());
        assert!(next_due.is_none());
        assert!(!is_complete);
    }

    #[test]
    fn screening_info_has_category_fields() {
        use crate::crypto::profile::AgeContext;

        let demo = crate::crypto::profile::PatientDemographics {
            sex: Some(BiologicalSex::Female),
            ethnicities: vec![],
            age_context: Some(AgeContext::Adult),
            age_years: Some(55),
            blood_type: None,
        };
        let info = build_screening_info("en", Some(&demo), &[]);
        // Should have both cancer screenings and vaccines
        let cancer_count = info.iter().filter(|s| s.category == "cancer").count();
        let vaccine_count = info.iter().filter(|s| s.category == "vaccine").count();
        assert!(cancer_count > 0, "Should have cancer screenings");
        assert!(vaccine_count > 0, "Should have vaccines");
        // All items should have the new fields
        for s in &info {
            assert!(!s.category.is_empty());
        }
    }

    // --- ALLERGY-01 B9: build_allergy_info tests ---

    fn make_test_allergy(
        allergen: &str,
        severity: AllergySeverity,
        category: Option<AllergenCategory>,
        verified: bool,
    ) -> crate::models::Allergy {
        crate::models::Allergy {
            id: Uuid::new_v4(),
            allergen: allergen.to_string(),
            reaction: Some("Rash".to_string()),
            severity,
            allergen_category: category,
            date_identified: Some(chrono::NaiveDate::from_ymd_opt(2025, 6, 15).unwrap()),
            source: AllergySource::PatientReported,
            document_id: None,
            verified,
        }
    }

    #[test]
    fn allergy_info_basic_fields() {
        let registry = crate::invariants::InvariantRegistry::empty();
        let allergy = make_test_allergy(
            "Penicillin",
            AllergySeverity::Severe,
            Some(AllergenCategory::Drug),
            true,
        );
        let infos = build_allergy_info(&[allergy.clone()], &registry);
        assert_eq!(infos.len(), 1);
        let info = &infos[0];
        assert_eq!(info.allergen, "Penicillin");
        assert_eq!(info.severity, "severe");
        assert_eq!(info.category.as_deref(), Some("drug"));
        assert_eq!(info.source, "patient_reported");
        assert!(info.verified);
        assert_eq!(info.reaction.as_deref(), Some("Rash"));
        assert_eq!(info.date_identified.as_deref(), Some("2025-06-15"));
        assert_eq!(info.id, allergy.id.to_string());
    }

    #[test]
    fn allergy_info_no_category() {
        let registry = crate::invariants::InvariantRegistry::empty();
        let allergy = make_test_allergy("Something", AllergySeverity::Mild, None, false);
        let infos = build_allergy_info(&[allergy], &registry);
        assert_eq!(infos.len(), 1);
        assert!(infos[0].category.is_none());
        assert!(!infos[0].verified);
    }

    #[test]
    fn allergy_info_empty_list() {
        let registry = crate::invariants::InvariantRegistry::empty();
        let infos = build_allergy_info(&[], &registry);
        assert!(infos.is_empty());
    }

    #[test]
    fn allergy_info_multiple() {
        let registry = crate::invariants::InvariantRegistry::empty();
        let allergies = vec![
            make_test_allergy("Penicillin", AllergySeverity::Severe, None, true),
            make_test_allergy("Peanut", AllergySeverity::LifeThreatening, None, false),
            make_test_allergy("Dust mite", AllergySeverity::Mild, None, false),
        ];
        let infos = build_allergy_info(&allergies, &registry);
        assert_eq!(infos.len(), 3);
        assert_eq!(infos[0].allergen, "Penicillin");
        assert_eq!(infos[1].allergen, "Peanut");
        assert_eq!(infos[2].allergen, "Dust mite");
    }

    #[test]
    fn allergy_info_cross_reactivity_with_loaded_registry() {
        let candidates = [
            std::path::PathBuf::from("resources"),
            std::path::PathBuf::from("src-tauri/resources"),
        ];
        let mut registry = crate::invariants::InvariantRegistry::empty();
        for dir in &candidates {
            if dir.join("invariants").exists() {
                registry = crate::invariants::InvariantRegistry::load(dir).unwrap();
                break;
            }
        }
        if registry.allergen_cross_reactivity().is_empty() {
            return; // Skip if invariant data not available
        }

        // Penicillin should have cross-reactivity notes (cephalosporins)
        let allergy = make_test_allergy("Penicillin", AllergySeverity::Severe, None, false);
        let infos = build_allergy_info(&[allergy], &registry);
        assert_eq!(infos.len(), 1);
        // Cross-reactivity notes capped at 3
        assert!(infos[0].cross_reactivities.len() <= 3);
    }

    #[test]
    fn allergy_info_severity_variants() {
        let registry = crate::invariants::InvariantRegistry::empty();
        let severities = [
            (AllergySeverity::Mild, "mild"),
            (AllergySeverity::Moderate, "moderate"),
            (AllergySeverity::Severe, "severe"),
            (AllergySeverity::LifeThreatening, "life_threatening"),
        ];
        for (sev, expected) in &severities {
            let allergy = make_test_allergy("Test", sev.clone(), None, false);
            let infos = build_allergy_info(&[allergy], &registry);
            assert_eq!(infos[0].severity, *expected, "Severity mismatch for {expected}");
        }
    }

    // ── BT-01: Blood type on MeIdentity ────────────────────

    #[test]
    fn format_blood_type_resolves_display() {
        // Verify the blood_type → blood_type_display mapping helper
        let key = "o_positive";
        let info = crate::invariants::blood_types::find_blood_type(key);
        assert!(info.is_some());
        assert_eq!(info.unwrap().display, "O+");
    }

    #[test]
    fn format_blood_type_negative_resolves() {
        let key = "ab_negative";
        let info = crate::invariants::blood_types::find_blood_type(key);
        assert!(info.is_some());
        assert_eq!(info.unwrap().display, "AB-");
    }
}
