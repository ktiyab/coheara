//! ME-03: Enrichment engine — deterministic clinical insight generation.
//!
//! Pure function: no DB, no async, no LLM.
//! Pairs user medical data with curated invariant thresholds to produce
//! grounded clinical insights.
//!
//! ## Sub-algorithms
//!
//! 1. `classify_vitals` — VitalSign → const tier thresholds → Classification
//! 2. `classify_labs` — LabResult → alias lookup → Classification
//! 3. `detect_interactions` — Medication pairs → interaction pairs → Interaction
//! 4. `detect_cross_reactivity` — Allergy × Medication → chains → CrossReactivity
//! 5. `detect_same_family_allergy` — Allergy × Medication → same drug family → Contraindication
//! 6. `detect_missing_monitoring` — Active med → schedule → missing lab → MissingMonitoring
//! 7. `detect_screening_due` — Demographics → age+sex-gated screening schedules → ScreeningDue

use chrono::NaiveDate;

use crate::crypto::profile::{BiologicalSex, PatientDemographics};
use crate::invariants::labs;
use crate::invariants::types::{
    ClinicalInsight, InsightKind, InsightSeverity, InvariantLabel, MeaningFactors,
};
use crate::invariants::vitals;
use crate::invariants::InvariantRegistry;
use crate::models::enums::MedicationStatus;
use crate::models::{Allergy, LabResult, Medication, VitalSign, VitalType};

// ═══════════════════════════════════════════════════════════
// Static labels for dynamic insight types
// ═══════════════════════════════════════════════════════════

const INTERACTION_LABEL: InvariantLabel = InvariantLabel {
    key: "drug_interaction",
    en: "Drug-drug interaction",
    fr: "Interaction médicamenteuse",
    de: "Arzneimittelinteraktion",
};

const CROSS_REACTIVITY_LABEL: InvariantLabel = InvariantLabel {
    key: "cross_reactivity",
    en: "Allergen cross-reactivity risk",
    fr: "Risque de réactivité croisée",
    de: "Allergen-Kreuzreaktivitätsrisiko",
};

const SAME_FAMILY_ALLERGY_LABEL: InvariantLabel = InvariantLabel {
    key: "same_family_allergy",
    en: "Same-family allergen contraindication",
    fr: "Contre-indication allergène même famille",
    de: "Gleiche-Familie Allergenkontraindikation",
};

const MISSING_MONITORING_LABEL: InvariantLabel = InvariantLabel {
    key: "missing_monitoring",
    en: "Overdue monitoring lab",
    fr: "Examen de surveillance en retard",
    de: "Überfällige Kontrolluntersuchung",
};

// ═══════════════════════════════════════════════════════════
// Main entry point
// ═══════════════════════════════════════════════════════════

/// Enrich patient data with deterministic clinical insights.
///
/// Pure function — no side effects, no I/O. Suitable for testing.
/// Results are sorted by severity (Critical first).
///
/// ME-04: `demographics` enables sex-aware hemoglobin classification,
/// ethnicity-aware BMI thresholds, and age+sex-gated screening schedules.
/// When `None`, conservative universal defaults are used (backward compatible).
pub fn enrich(
    medications: &[Medication],
    lab_results: &[LabResult],
    allergies: &[Allergy],
    vital_signs: &[VitalSign],
    registry: &InvariantRegistry,
    reference_date: NaiveDate,
    demographics: Option<&PatientDemographics>,
) -> Vec<ClinicalInsight> {
    let mut insights = Vec::new();

    insights.extend(classify_vitals(vital_signs, demographics));
    insights.extend(classify_labs(lab_results, registry, demographics));
    insights.extend(detect_interactions(medications, registry));
    insights.extend(detect_cross_reactivity(allergies, medications, registry));
    insights.extend(detect_same_family_allergy(allergies, medications, registry));
    insights.extend(detect_missing_monitoring(
        medications,
        lab_results,
        registry,
        reference_date,
    ));
    insights.extend(crate::invariants::screening::detect_screening_due(demographics));

    // Sort by severity descending (Critical first)
    insights.sort_by(|a, b| b.severity.cmp(&a.severity));

    insights
}

// ═══════════════════════════════════════════════════════════
// Sub-algorithm 1: Classify vital signs
// ═══════════════════════════════════════════════════════════

fn classify_vitals(
    vital_signs: &[VitalSign],
    demographics: Option<&PatientDemographics>,
) -> Vec<ClinicalInsight> {
    let mut insights = Vec::new();

    for vital in vital_signs {
        let insight = match vital.vital_type {
            VitalType::BloodPressure => classify_bp(vital),
            VitalType::HeartRate => classify_hr(vital),
            VitalType::OxygenSaturation => classify_spo2(vital),
            VitalType::Temperature => classify_temp(vital),
            VitalType::BloodGlucose => classify_glucose(vital),
            VitalType::Weight | VitalType::Height => None,
        };
        if let Some(i) = insight {
            insights.push(i);
        }
    }

    // BMI from latest weight + height — ethnicity-aware (ME-04)
    if let Some(insight) = classify_bmi(vital_signs, demographics) {
        insights.push(insight);
    }

    insights
}

fn classify_bp(vital: &VitalSign) -> Option<ClinicalInsight> {
    let systolic = vital.value_primary as u16;
    let diastolic = vital.value_secondary.unwrap_or(0.0) as u16;
    let tier = vitals::classify_bp(systolic, diastolic);
    let severity = significance_to_severity(tier.significance)?;

    Some(ClinicalInsight {
        kind: InsightKind::Classification,
        severity,
        summary_key: format!("BP {}/{} mmHg", systolic, diastolic),
        description: tier.label,
        source: tier.source.to_string(),
        related_entities: vec![vital.id],
        meaning_factors: MeaningFactors {
            significance: tier.significance,
            ..MeaningFactors::default()
        },
    })
}

fn classify_hr(vital: &VitalSign) -> Option<ClinicalInsight> {
    let bpm = vital.value_primary as u16;
    let tier = vitals::classify_hr(bpm);
    let severity = significance_to_severity(tier.significance)?;

    Some(ClinicalInsight {
        kind: InsightKind::Classification,
        severity,
        summary_key: format!("HR {} bpm", bpm),
        description: tier.label,
        source: tier.source.to_string(),
        related_entities: vec![vital.id],
        meaning_factors: MeaningFactors {
            significance: tier.significance,
            ..MeaningFactors::default()
        },
    })
}

fn classify_spo2(vital: &VitalSign) -> Option<ClinicalInsight> {
    let pct = vital.value_primary as u8;
    let tier = vitals::classify_spo2(pct);
    let severity = significance_to_severity(tier.significance)?;

    Some(ClinicalInsight {
        kind: InsightKind::Classification,
        severity,
        summary_key: format!("SpO2 {}%", pct),
        description: tier.label,
        source: tier.source.to_string(),
        related_entities: vec![vital.id],
        meaning_factors: MeaningFactors {
            significance: tier.significance,
            ..MeaningFactors::default()
        },
    })
}

fn classify_temp(vital: &VitalSign) -> Option<ClinicalInsight> {
    let celsius = vital.value_primary;
    let tier = vitals::classify_temperature(celsius);
    let severity = significance_to_severity(tier.significance)?;

    Some(ClinicalInsight {
        kind: InsightKind::Classification,
        severity,
        summary_key: format!("Temp {:.1}°C", celsius),
        description: tier.label,
        source: tier.source.to_string(),
        related_entities: vec![vital.id],
        meaning_factors: MeaningFactors {
            significance: tier.significance,
            ..MeaningFactors::default()
        },
    })
}

fn classify_glucose(vital: &VitalSign) -> Option<ClinicalInsight> {
    let value = vital.value_primary;
    let tier = if vital.unit.contains("mg") {
        vitals::classify_glucose_mgdl(value)
    } else {
        vitals::classify_glucose_mmol(value)
    };
    let severity = significance_to_severity(tier.significance)?;

    Some(ClinicalInsight {
        kind: InsightKind::Classification,
        severity,
        summary_key: format!("Glucose {} {}", value, vital.unit),
        description: tier.label,
        source: tier.source.to_string(),
        related_entities: vec![vital.id],
        meaning_factors: MeaningFactors {
            significance: tier.significance,
            ..MeaningFactors::default()
        },
    })
}

/// ME-04: BMI classification with ethnicity-aware thresholds.
/// Uses WHO 2004 Asian thresholds (23.0/27.5) when demographics indicate
/// South Asian, East Asian, or Pacific Islander heritage.
fn classify_bmi(
    vital_signs: &[VitalSign],
    demographics: Option<&PatientDemographics>,
) -> Option<ClinicalInsight> {
    let latest_weight = vital_signs
        .iter()
        .filter(|v| v.vital_type == VitalType::Weight)
        .max_by_key(|v| v.recorded_at)?;
    let latest_height = vital_signs
        .iter()
        .filter(|v| v.vital_type == VitalType::Height)
        .max_by_key(|v| v.recorded_at)?;

    let bmi = vitals::compute_bmi(latest_weight.value_primary, latest_height.value_primary)?;

    // ME-04: Use Asian thresholds when patient has Asian heritage
    let use_asian = demographics
        .map(|d| d.has_asian_bmi_thresholds())
        .unwrap_or(false);
    let tier = if use_asian {
        crate::invariants::demographics::classify_bmi_asian(bmi)
    } else {
        vitals::classify_bmi(bmi)
    };
    let severity = significance_to_severity(tier.significance)?;

    Some(ClinicalInsight {
        kind: InsightKind::Classification,
        severity,
        summary_key: format!("BMI {:.1} kg/m²", bmi),
        description: tier.label,
        source: tier.source.to_string(),
        related_entities: vec![latest_weight.id, latest_height.id],
        meaning_factors: MeaningFactors {
            significance: tier.significance,
            ..MeaningFactors::default()
        },
    })
}

// ═══════════════════════════════════════════════════════════
// Sub-algorithm 2: Classify lab results
// ═══════════════════════════════════════════════════════════

/// ME-04: Lab classification with sex-aware hemoglobin thresholds.
/// Male: normal ≥ 13.0 g/dL. Female/unknown: normal ≥ 12.0 g/dL (WHO 2024).
fn classify_labs(
    lab_results: &[LabResult],
    registry: &InvariantRegistry,
    demographics: Option<&PatientDemographics>,
) -> Vec<ClinicalInsight> {
    let mut insights = Vec::new();

    for lab in lab_results {
        let Some(value) = lab.value else { continue };
        let Some(threshold) = registry.find_lab_threshold(&lab.test_name) else {
            continue;
        };

        // ME-04: Use male hemoglobin tiers when sex is Male
        let tier = if threshold.test_key == "hemoglobin"
            && demographics.and_then(|d| d.sex) == Some(BiologicalSex::Male)
        {
            crate::invariants::demographics::HEMOGLOBIN_TIERS_MALE
                .iter()
                .find(|t| value >= t.min_value && value < t.max_value)
        } else {
            labs::classify_lab(value, threshold)
        };

        let Some(tier) = tier else { continue };
        let Some(severity) = significance_to_severity(tier.significance) else {
            continue;
        };

        let unit_str = lab.unit.as_deref().unwrap_or(threshold.unit);
        insights.push(ClinicalInsight {
            kind: InsightKind::Classification,
            severity,
            summary_key: format!("{} {} {}", lab.test_name, value, unit_str),
            description: tier.label,
            source: threshold.source.to_string(),
            related_entities: vec![lab.id],
            meaning_factors: MeaningFactors {
                significance: tier.significance,
                ..MeaningFactors::default()
            },
        });
    }

    insights
}

// ═══════════════════════════════════════════════════════════
// Sub-algorithm 3: Detect drug-drug interactions
// ═══════════════════════════════════════════════════════════

fn detect_interactions(
    medications: &[Medication],
    registry: &InvariantRegistry,
) -> Vec<ClinicalInsight> {
    let active_meds: Vec<&Medication> = medications
        .iter()
        .filter(|m| m.status == MedicationStatus::Active)
        .collect();

    let mut insights = Vec::new();

    for i in 0..active_meds.len() {
        for j in (i + 1)..active_meds.len() {
            let med_a = active_meds[i];
            let med_b = active_meds[j];

            for pair in registry.interaction_pairs() {
                let forward = drug_matches_field(&med_a.generic_name, &pair.drug_a, registry)
                    && drug_matches_field(&med_b.generic_name, &pair.drug_b, registry);
                let reverse = drug_matches_field(&med_a.generic_name, &pair.drug_b, registry)
                    && drug_matches_field(&med_b.generic_name, &pair.drug_a, registry);

                if forward || reverse {
                    let severity = interaction_severity(&pair.severity);
                    insights.push(ClinicalInsight {
                        kind: InsightKind::Interaction,
                        severity,
                        summary_key: format!(
                            "{} + {}: {}",
                            med_a.generic_name, med_b.generic_name, pair.description
                        ),
                        description: INTERACTION_LABEL,
                        source: pair.source.clone(),
                        related_entities: vec![med_a.id, med_b.id],
                        meaning_factors: MeaningFactors {
                            domain_boost: 1.2,
                            significance: severity_to_significance(severity),
                            ..MeaningFactors::default()
                        },
                    });
                    break; // One match per medication pair
                }
            }
        }
    }

    insights
}

/// Check if a medication matches an interaction field (drug name or family key).
fn drug_matches_field(
    generic_name: &str,
    interaction_field: &str,
    registry: &InvariantRegistry,
) -> bool {
    let med_lower = generic_name.trim().to_lowercase();
    let field_lower = interaction_field.trim().to_lowercase();

    // Direct name match
    if med_lower == field_lower {
        return true;
    }

    // Family match: medication belongs to the family identified by field
    if let Some(family) = registry.find_drug_family(&med_lower) {
        if family.key.to_lowercase() == field_lower {
            return true;
        }
    }

    false
}

// ═══════════════════════════════════════════════════════════
// Sub-algorithm 4: Detect allergen-drug cross-reactivity
// ═══════════════════════════════════════════════════════════

fn detect_cross_reactivity(
    allergies: &[Allergy],
    medications: &[Medication],
    registry: &InvariantRegistry,
) -> Vec<ClinicalInsight> {
    let active_meds: Vec<&Medication> = medications
        .iter()
        .filter(|m| m.status == MedicationStatus::Active)
        .collect();

    let mut insights = Vec::new();

    for allergy in allergies {
        let allergen_lower = allergy.allergen.trim().to_lowercase();
        let allergen_family = registry.find_drug_family(&allergen_lower);

        for chain in registry.cross_reactivity() {
            let primary_lower = chain.primary.to_lowercase();
            let cross_lower = chain.cross_reactive.to_lowercase();

            // Does allergen match the primary side of this chain?
            // Uses contains() because chain entries can be qualified
            // (e.g., "aminopenicillin" contains "penicillin").
            let allergen_matches_primary = allergen_lower == primary_lower
                || primary_lower.contains(&allergen_lower)
                || allergen_family
                    .map(|f| {
                        let fkey = f.key.to_lowercase();
                        primary_lower.contains(&fkey) || fkey == primary_lower
                    })
                    .unwrap_or(false);

            if !allergen_matches_primary {
                continue;
            }

            // Does any active medication match the cross-reactive side?
            // Uses contains() because chain entries can be qualified
            // (e.g., "cephalosporin (dissimilar R1)" contains "cephalosporin").
            for med in &active_meds {
                let med_lower = med.generic_name.trim().to_lowercase();
                let med_family = registry.find_drug_family(&med_lower);

                let med_matches_cross = med_lower == cross_lower
                    || cross_lower.contains(&med_lower)
                    || med_family
                        .map(|f| cross_lower.contains(&f.key.to_lowercase()))
                        .unwrap_or(false);

                if med_matches_cross {
                    insights.push(ClinicalInsight {
                        kind: InsightKind::CrossReactivity,
                        severity: InsightSeverity::Critical,
                        summary_key: format!(
                            "{} allergy → {} ({} cross-reactivity)",
                            allergy.allergen, med.generic_name, chain.rate
                        ),
                        description: CROSS_REACTIVITY_LABEL,
                        source: chain.source.clone(),
                        related_entities: vec![allergy.id, med.id],
                        meaning_factors: MeaningFactors {
                            domain_boost: 1.3,
                            significance: 2.0,
                            ..MeaningFactors::default()
                        },
                    });
                }
            }
        }
    }

    insights
}

// ═══════════════════════════════════════════════════════════
// Sub-algorithm 5: Detect same-family allergen contraindication
// ═══════════════════════════════════════════════════════════

/// Detect when an active medication belongs to the same drug family as a known allergen.
///
/// Catches cases like: penicillin allergy + amoxicillin (both penicillin family).
/// Complements `detect_cross_reactivity` which handles cross-family chains
/// (e.g., penicillin → cephalosporin).
fn detect_same_family_allergy(
    allergies: &[Allergy],
    medications: &[Medication],
    registry: &InvariantRegistry,
) -> Vec<ClinicalInsight> {
    let active_meds: Vec<&Medication> = medications
        .iter()
        .filter(|m| m.status == MedicationStatus::Active)
        .collect();

    let mut insights = Vec::new();

    for allergy in allergies {
        let allergen_lower = allergy.allergen.trim().to_lowercase();
        let Some(allergen_family) = registry.find_drug_family(&allergen_lower) else {
            continue;
        };

        for med in &active_meds {
            let med_lower = med.generic_name.trim().to_lowercase();

            // Skip exact match — if allergen == medication, this is a direct contraindication
            // already obvious in the EHR; we focus on non-obvious same-family cases.
            if med_lower == allergen_lower {
                continue;
            }

            let Some(med_family) = registry.find_drug_family(&med_lower) else {
                continue;
            };

            if allergen_family.key == med_family.key {
                insights.push(ClinicalInsight {
                    kind: InsightKind::CrossReactivity,
                    severity: InsightSeverity::Critical,
                    summary_key: format!(
                        "{} allergy → {} (same {} family)",
                        allergy.allergen, med.generic_name, allergen_family.name
                    ),
                    description: SAME_FAMILY_ALLERGY_LABEL,
                    source: allergen_family.source.clone(),
                    related_entities: vec![allergy.id, med.id],
                    meaning_factors: MeaningFactors {
                        domain_boost: 1.5,
                        significance: 2.0,
                        ..MeaningFactors::default()
                    },
                });
            }
        }
    }

    insights
}

// ═══════════════════════════════════════════════════════════
// Sub-algorithm 6: Detect missing monitoring labs
// ═══════════════════════════════════════════════════════════

fn detect_missing_monitoring(
    medications: &[Medication],
    lab_results: &[LabResult],
    registry: &InvariantRegistry,
    reference_date: NaiveDate,
) -> Vec<ClinicalInsight> {
    let active_meds: Vec<&Medication> = medications
        .iter()
        .filter(|m| m.status == MedicationStatus::Active)
        .collect();

    let mut insights = Vec::new();

    for med in &active_meds {
        let schedules = registry.find_monitoring(&med.generic_name);

        for schedule in schedules {
            // Find the most recent lab matching this schedule's test
            let latest_lab = find_latest_matching_lab(lab_results, &schedule.lab_test, registry);

            let is_overdue = match latest_lab {
                Some(lab) => {
                    let days_since = (reference_date - lab.collection_date).num_days();
                    days_since > schedule.interval_days as i64
                }
                None => true, // No matching lab found at all
            };

            if is_overdue {
                let detail = match latest_lab {
                    Some(lab) => format!(
                        "{}: {} overdue (last: {}, interval: {} days)",
                        med.generic_name, schedule.lab_test, lab.collection_date, schedule.interval_days
                    ),
                    None => format!(
                        "{}: {} not found (monitoring interval: {} days)",
                        med.generic_name, schedule.lab_test, schedule.interval_days
                    ),
                };

                insights.push(ClinicalInsight {
                    kind: InsightKind::MissingMonitoring,
                    severity: InsightSeverity::Warning,
                    summary_key: detail,
                    description: MISSING_MONITORING_LABEL,
                    source: schedule.source.clone(),
                    related_entities: vec![med.id],
                    meaning_factors: MeaningFactors {
                        uncertainty_delta: 0.3,
                        significance: 0.8,
                        ..MeaningFactors::default()
                    },
                });
            }
        }
    }

    insights
}

/// Find the most recent lab result matching a monitoring schedule's test key.
fn find_latest_matching_lab<'a>(
    lab_results: &'a [LabResult],
    lab_test_key: &str,
    registry: &InvariantRegistry,
) -> Option<&'a LabResult> {
    lab_results
        .iter()
        .filter(|lab| {
            // Match via threshold alias normalization
            if let Some(threshold) = registry.find_lab_threshold(&lab.test_name) {
                threshold.test_key == lab_test_key
            } else {
                // Fallback: direct key comparison
                lab.test_name.to_lowercase() == lab_test_key.to_lowercase()
            }
        })
        .max_by_key(|lab| lab.collection_date)
}

// ═══════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════

/// Map clinical significance score to insight severity.
/// Returns None for normal values (significance ≤ 0.3) — no insight generated.
fn significance_to_severity(significance: f64) -> Option<InsightSeverity> {
    if significance <= 0.3 {
        None // Normal range, skip
    } else if significance <= 0.5 {
        Some(InsightSeverity::Info)
    } else if significance < 1.3 {
        Some(InsightSeverity::Warning)
    } else {
        Some(InsightSeverity::Critical)
    }
}

/// Map interaction severity string to InsightSeverity.
fn interaction_severity(severity_str: &str) -> InsightSeverity {
    match severity_str.to_lowercase().as_str() {
        "critical" | "high" => InsightSeverity::Critical,
        "moderate" => InsightSeverity::Warning,
        _ => InsightSeverity::Info,
    }
}

/// Map InsightSeverity back to significance score for MeaningFactors.
fn severity_to_significance(severity: InsightSeverity) -> f64 {
    match severity {
        InsightSeverity::Info => 0.4,
        InsightSeverity::Warning => 0.8,
        InsightSeverity::Critical => 1.5,
    }
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::enums::*;
    use crate::models::VitalSource;
    use chrono::NaiveDateTime;
    use uuid::Uuid;

    // ── Test helpers ────────────────────────────────────────

    fn make_vital(vital_type: VitalType, primary: f64, secondary: Option<f64>) -> VitalSign {
        VitalSign {
            id: Uuid::new_v4(),
            vital_type,
            value_primary: primary,
            value_secondary: secondary,
            unit: match vital_type {
                VitalType::BloodPressure => "mmHg".to_string(),
                VitalType::HeartRate => "bpm".to_string(),
                VitalType::Temperature => "°C".to_string(),
                VitalType::OxygenSaturation => "%".to_string(),
                VitalType::BloodGlucose => "mmol/L".to_string(),
                VitalType::Weight => "kg".to_string(),
                VitalType::Height => "cm".to_string(),
            },
            recorded_at: NaiveDateTime::parse_from_str("2026-01-15 10:00:00", "%Y-%m-%d %H:%M:%S")
                .unwrap(),
            notes: None,
            source: VitalSource::Imported,
            created_at: NaiveDateTime::parse_from_str("2026-01-15 10:00:00", "%Y-%m-%d %H:%M:%S")
                .unwrap(),
        }
    }

    fn make_lab(test_name: &str, value: f64, date: &str) -> LabResult {
        LabResult {
            id: Uuid::new_v4(),
            test_name: test_name.to_string(),
            test_code: None,
            value: Some(value),
            value_text: None,
            unit: None,
            reference_range_low: None,
            reference_range_high: None,
            abnormal_flag: AbnormalFlag::Normal,
            collection_date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            lab_facility: None,
            ordering_physician_id: None,
            document_id: Uuid::new_v4(),
        }
    }

    fn make_med(name: &str) -> Medication {
        Medication {
            id: Uuid::new_v4(),
            generic_name: name.to_string(),
            brand_name: None,
            dose: "500mg".to_string(),
            frequency: "once daily".to_string(),
            frequency_type: FrequencyType::Scheduled,
            route: "oral".to_string(),
            prescriber_id: None,
            start_date: None,
            end_date: None,
            reason_start: None,
            reason_stop: None,
            is_otc: false,
            status: MedicationStatus::Active,
            administration_instructions: None,
            max_daily_dose: None,
            condition: None,
            dose_type: DoseType::Fixed,
            is_compound: false,
            document_id: Uuid::new_v4(),
        }
    }

    fn make_allergy(allergen: &str) -> Allergy {
        Allergy {
            id: Uuid::new_v4(),
            allergen: allergen.to_string(),
            reaction: Some("Rash".to_string()),
            severity: AllergySeverity::Moderate,
            date_identified: None,
            source: AllergySource::DocumentExtracted,
            document_id: None,
            verified: true,
        }
    }

    fn loaded_registry() -> InvariantRegistry {
        let candidates = [
            std::path::PathBuf::from("resources"),
            std::path::PathBuf::from("src-tauri/resources"),
        ];
        for dir in &candidates {
            if dir.join("invariants").exists() {
                return InvariantRegistry::load(dir).unwrap();
            }
        }
        InvariantRegistry::empty()
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 3, 2).unwrap()
    }

    // ── classify_vitals tests ───────────────────────────────

    #[test]
    fn bp_grade_1_produces_warning() {
        let vitals = vec![make_vital(VitalType::BloodPressure, 145.0, Some(92.0))];
        let insights = classify_vitals(&vitals, None);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].kind, InsightKind::Classification);
        assert_eq!(insights[0].severity, InsightSeverity::Warning);
        assert!(insights[0].summary_key.contains("BP 145/92"));
        assert_eq!(insights[0].description.key, "bp_grade_1_htn");
    }

    #[test]
    fn bp_grade_2_produces_warning() {
        // Grade 2 HTN significance = 0.9 → Warning (not Critical)
        let vitals = vec![make_vital(VitalType::BloodPressure, 185.0, Some(115.0))];
        let insights = classify_vitals(&vitals, None);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].severity, InsightSeverity::Warning);
        assert_eq!(insights[0].description.key, "bp_grade_2_htn");
    }

    #[test]
    fn bp_normal_produces_no_insight() {
        let vitals = vec![make_vital(VitalType::BloodPressure, 120.0, Some(75.0))];
        let insights = classify_vitals(&vitals, None);
        assert!(insights.is_empty(), "Normal BP should not produce insight");
    }

    #[test]
    fn hr_tachycardia_produces_warning() {
        let vitals = vec![make_vital(VitalType::HeartRate, 110.0, None)];
        let insights = classify_vitals(&vitals, None);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].severity, InsightSeverity::Warning);
        assert_eq!(insights[0].description.key, "hr_tachycardia");
    }

    #[test]
    fn hr_severe_bradycardia_produces_critical() {
        let vitals = vec![make_vital(VitalType::HeartRate, 35.0, None)];
        let insights = classify_vitals(&vitals, None);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].severity, InsightSeverity::Critical);
    }

    #[test]
    fn spo2_hypoxemia_produces_critical() {
        let vitals = vec![make_vital(VitalType::OxygenSaturation, 85.0, None)];
        let insights = classify_vitals(&vitals, None);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].severity, InsightSeverity::Critical);
        assert!(insights[0].summary_key.contains("SpO2 85%"));
    }

    #[test]
    fn spo2_normal_produces_no_insight() {
        let vitals = vec![make_vital(VitalType::OxygenSaturation, 98.0, None)];
        let insights = classify_vitals(&vitals, None);
        assert!(insights.is_empty());
    }

    #[test]
    fn temp_fever_produces_warning() {
        let vitals = vec![make_vital(VitalType::Temperature, 38.5, None)];
        let insights = classify_vitals(&vitals, None);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].severity, InsightSeverity::Warning);
        assert_eq!(insights[0].description.key, "temp_fever");
    }

    #[test]
    fn glucose_diabetes_produces_critical() {
        let vitals = vec![make_vital(VitalType::BloodGlucose, 8.0, None)];
        let insights = classify_vitals(&vitals, None);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].severity, InsightSeverity::Critical);
        assert_eq!(insights[0].description.key, "glucose_diabetes");
    }

    #[test]
    fn bmi_obese_from_weight_and_height() {
        let vitals = vec![
            make_vital(VitalType::Weight, 100.0, None),
            make_vital(VitalType::Height, 170.0, None),
        ];
        let insights = classify_vitals(&vitals, None);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].kind, InsightKind::Classification);
        assert!(insights[0].summary_key.contains("BMI"));
        // 100 / 1.7^2 ≈ 34.6 → Obese Class I
        assert_eq!(insights[0].description.key, "bmi_obese_i");
    }

    #[test]
    fn bmi_not_generated_without_both_measurements() {
        let vitals = vec![make_vital(VitalType::Weight, 70.0, None)];
        let insights = classify_vitals(&vitals, None);
        assert!(insights.is_empty(), "BMI needs both weight and height");
    }

    #[test]
    fn multiple_abnormal_vitals_produce_multiple_insights() {
        let vitals = vec![
            make_vital(VitalType::BloodPressure, 145.0, Some(92.0)),
            make_vital(VitalType::HeartRate, 110.0, None),
            make_vital(VitalType::Temperature, 38.5, None),
        ];
        let insights = classify_vitals(&vitals, None);
        assert_eq!(insights.len(), 3);
    }

    // ── classify_labs tests ─────────────────────────────────

    #[test]
    fn egfr_g4_produces_critical() {
        let registry = InvariantRegistry::empty();
        let labs = vec![make_lab("eGFR", 28.0, "2026-01-15")];
        let insights = classify_labs(&labs, &registry, None);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].severity, InsightSeverity::Critical);
        assert_eq!(insights[0].description.key, "ckd_g4");
    }

    #[test]
    fn hba1c_prediabetes_produces_warning() {
        let registry = InvariantRegistry::empty();
        let labs = vec![make_lab("HbA1c", 6.2, "2026-01-15")];
        let insights = classify_labs(&labs, &registry, None);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].severity, InsightSeverity::Warning);
        assert_eq!(insights[0].description.key, "hba1c_prediabetes");
    }

    #[test]
    fn normal_lab_produces_no_insight() {
        let registry = InvariantRegistry::empty();
        let labs = vec![make_lab("eGFR", 95.0, "2026-01-15")];
        let insights = classify_labs(&labs, &registry, None);
        assert!(insights.is_empty(), "Normal eGFR should not produce insight");
    }

    #[test]
    fn unknown_lab_produces_no_insight() {
        let registry = InvariantRegistry::empty();
        let labs = vec![make_lab("unknown_test", 50.0, "2026-01-15")];
        let insights = classify_labs(&labs, &registry, None);
        assert!(insights.is_empty());
    }

    #[test]
    fn lab_without_numeric_value_skipped() {
        let registry = InvariantRegistry::empty();
        let mut lab = make_lab("eGFR", 0.0, "2026-01-15");
        lab.value = None;
        lab.value_text = Some("pending".to_string());
        let insights = classify_labs(&[lab], &registry, None);
        assert!(insights.is_empty());
    }

    #[test]
    fn potassium_hyperkalemia_produces_critical() {
        let registry = InvariantRegistry::empty();
        let labs = vec![make_lab("K+", 5.7, "2026-01-15")];
        let insights = classify_labs(&labs, &registry, None);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].severity, InsightSeverity::Critical);
        assert_eq!(insights[0].description.key, "k_hyperkalemia");
    }

    // ── detect_interactions tests ───────────────────────────

    #[test]
    fn warfarin_nsaid_interaction_detected() {
        let registry = loaded_registry();
        if registry.interaction_pairs().is_empty() {
            return; // Skip if no JSON loaded
        }
        let meds = vec![make_med("warfarin"), make_med("ibuprofen")];
        let insights = detect_interactions(&meds, &registry);
        assert!(
            !insights.is_empty(),
            "Warfarin + ibuprofen (NSAID) should trigger interaction"
        );
        assert_eq!(insights[0].kind, InsightKind::Interaction);
    }

    #[test]
    fn no_interaction_for_unrelated_drugs() {
        let registry = loaded_registry();
        let meds = vec![make_med("metformin"), make_med("omeprazole")];
        let insights = detect_interactions(&meds, &registry);
        assert!(
            insights.is_empty(),
            "Metformin + omeprazole should not interact"
        );
    }

    #[test]
    fn stopped_medication_not_checked_for_interaction() {
        let registry = loaded_registry();
        if registry.interaction_pairs().is_empty() {
            return;
        }
        let mut warfarin = make_med("warfarin");
        warfarin.status = MedicationStatus::Stopped;
        let meds = vec![warfarin, make_med("ibuprofen")];
        let insights = detect_interactions(&meds, &registry);
        assert!(
            insights.is_empty(),
            "Stopped medications should not trigger interactions"
        );
    }

    #[test]
    fn interaction_via_family_key() {
        let registry = loaded_registry();
        if registry.interaction_pairs().is_empty() {
            return;
        }
        // ACE inhibitor + K-sparing diuretic
        let meds = vec![make_med("lisinopril"), make_med("spironolactone")];
        let insights = detect_interactions(&meds, &registry);
        assert!(
            !insights.is_empty(),
            "ACEi + K-sparing diuretic should trigger interaction"
        );
    }

    #[test]
    fn empty_registry_produces_no_interactions() {
        let registry = InvariantRegistry::empty();
        let meds = vec![make_med("warfarin"), make_med("ibuprofen")];
        let insights = detect_interactions(&meds, &registry);
        assert!(insights.is_empty());
    }

    // ── detect_cross_reactivity tests ───────────────────────

    #[test]
    fn penicillin_allergy_cephalosporin_cross_reactivity() {
        let registry = loaded_registry();
        if registry.cross_reactivity().is_empty() {
            return;
        }
        let allergies = vec![make_allergy("penicillin")];
        let meds = vec![make_med("cephalexin")];
        let insights = detect_cross_reactivity(&allergies, &meds, &registry);
        assert!(
            !insights.is_empty(),
            "Penicillin allergy + cephalosporin should trigger cross-reactivity"
        );
        assert_eq!(insights[0].kind, InsightKind::CrossReactivity);
        assert_eq!(insights[0].severity, InsightSeverity::Critical);
    }

    #[test]
    fn no_cross_reactivity_for_unrelated_allergy() {
        let registry = loaded_registry();
        let allergies = vec![make_allergy("peanut")];
        let meds = vec![make_med("metformin")];
        let insights = detect_cross_reactivity(&allergies, &meds, &registry);
        assert!(insights.is_empty());
    }

    #[test]
    fn empty_registry_produces_no_cross_reactivity() {
        let registry = InvariantRegistry::empty();
        let allergies = vec![make_allergy("penicillin")];
        let meds = vec![make_med("cephalexin")];
        let insights = detect_cross_reactivity(&allergies, &meds, &registry);
        assert!(insights.is_empty());
    }

    // ── detect_missing_monitoring tests ─────────────────────

    #[test]
    fn metformin_missing_hba1c_detected() {
        let registry = loaded_registry();
        if registry.monitoring_schedules().is_empty() {
            return;
        }
        let meds = vec![make_med("metformin")];
        let labs: Vec<LabResult> = vec![]; // No labs at all
        let insights = detect_missing_monitoring(&meds, &labs, &registry, today());
        assert!(
            !insights.is_empty(),
            "Metformin without any HbA1c should trigger missing monitoring"
        );
        assert_eq!(insights[0].kind, InsightKind::MissingMonitoring);
        assert!(insights[0].summary_key.contains("not found"));
    }

    #[test]
    fn metformin_with_recent_hba1c_no_alert() {
        let registry = loaded_registry();
        if registry.monitoring_schedules().is_empty() {
            return;
        }
        let meds = vec![make_med("metformin")];
        // HbA1c from 30 days ago (within 90-day interval)
        let labs = vec![make_lab("HbA1c", 6.5, "2026-02-01")];
        let insights = detect_missing_monitoring(&meds, &labs, &registry, today());
        // Should not have HbA1c alert (recent), but may have eGFR alert
        let hba1c_alerts: Vec<_> = insights
            .iter()
            .filter(|i| i.summary_key.contains("hba1c"))
            .collect();
        assert!(
            hba1c_alerts.is_empty(),
            "Recent HbA1c should not trigger missing monitoring"
        );
    }

    #[test]
    fn metformin_with_old_hba1c_triggers_alert() {
        let registry = loaded_registry();
        if registry.monitoring_schedules().is_empty() {
            return;
        }
        let meds = vec![make_med("metformin")];
        // HbA1c from 120 days ago (beyond 90-day interval)
        let labs = vec![make_lab("HbA1c", 6.5, "2025-11-01")];
        let insights = detect_missing_monitoring(&meds, &labs, &registry, today());
        let hba1c_alerts: Vec<_> = insights
            .iter()
            .filter(|i| i.summary_key.contains("hba1c"))
            .collect();
        assert!(
            !hba1c_alerts.is_empty(),
            "Old HbA1c should trigger missing monitoring"
        );
        assert!(hba1c_alerts[0].summary_key.contains("overdue"));
    }

    #[test]
    fn stopped_medication_no_monitoring() {
        let registry = loaded_registry();
        let mut met = make_med("metformin");
        met.status = MedicationStatus::Stopped;
        let insights = detect_missing_monitoring(&[met], &[], &registry, today());
        assert!(
            insights.is_empty(),
            "Stopped medications should not trigger monitoring alerts"
        );
    }

    #[test]
    fn empty_registry_no_monitoring_alerts() {
        let registry = InvariantRegistry::empty();
        let meds = vec![make_med("metformin")];
        let insights = detect_missing_monitoring(&meds, &[], &registry, today());
        assert!(insights.is_empty());
    }

    // ── enrich() integration tests ──────────────────────────

    #[test]
    fn enrich_empty_data_produces_no_insights() {
        let registry = InvariantRegistry::empty();
        let insights = enrich(&[], &[], &[], &[], &registry, today(), None);
        assert!(insights.is_empty());
    }

    #[test]
    fn enrich_sorted_by_severity_critical_first() {
        let registry = InvariantRegistry::empty();
        let vitals = vec![
            make_vital(VitalType::BloodPressure, 145.0, Some(92.0)), // Warning (Grade 1)
            make_vital(VitalType::OxygenSaturation, 85.0, None),     // Critical (hypoxemia)
            make_vital(VitalType::Temperature, 37.5, None),          // Info (low-grade)
        ];
        let insights = enrich(&[], &[], &[], &vitals, &registry, today(), None);
        assert!(insights.len() >= 2);

        // Verify descending severity
        for window in insights.windows(2) {
            assert!(
                window[0].severity >= window[1].severity,
                "Insights not sorted: {:?} before {:?}",
                window[0].severity,
                window[1].severity
            );
        }
    }

    #[test]
    fn enrich_full_patient_scenario() {
        let registry = loaded_registry();

        let vitals = vec![
            make_vital(VitalType::BloodPressure, 145.0, Some(92.0)),
            make_vital(VitalType::Weight, 85.0, None),
            make_vital(VitalType::Height, 175.0, None),
        ];
        let labs = vec![
            make_lab("HbA1c", 7.2, "2025-10-01"), // Old (>90 days)
            make_lab("eGFR", 28.0, "2026-01-15"),
        ];
        let meds = vec![make_med("metformin")];
        let allergies = vec![make_allergy("peanut")];

        let insights = enrich(&meds, &labs, &allergies, &vitals, &registry, today(), None);

        // Should produce:
        // - BP Grade 1 HTN (Warning)
        // - BMI ~27.8 overweight (Info)
        // - HbA1c 7.2 diabetes (Critical)
        // - eGFR 28 CKD G4 (Critical)
        // - Metformin: HbA1c overdue (Warning)
        assert!(
            insights.len() >= 3,
            "Expected 3+ insights for full patient scenario, got {}",
            insights.len()
        );

        // Verify critical insights exist
        let critical: Vec<_> = insights
            .iter()
            .filter(|i| i.severity == InsightSeverity::Critical)
            .collect();
        assert!(
            !critical.is_empty(),
            "Should have critical insights for eGFR 28 and HbA1c 7.2"
        );
    }

    // ── Helper function tests ───────────────────────────────

    #[test]
    fn significance_mapping() {
        assert!(significance_to_severity(0.2).is_none()); // Normal, skip
        assert!(significance_to_severity(0.3).is_none()); // Low-normal, skip
        assert_eq!(significance_to_severity(0.4), Some(InsightSeverity::Info));
        assert_eq!(significance_to_severity(0.5), Some(InsightSeverity::Info));
        assert_eq!(
            significance_to_severity(0.6),
            Some(InsightSeverity::Warning)
        );
        assert_eq!(
            significance_to_severity(1.0),
            Some(InsightSeverity::Warning)
        );
        assert_eq!(
            significance_to_severity(1.2),
            Some(InsightSeverity::Warning)
        );
        assert_eq!(
            significance_to_severity(1.3),
            Some(InsightSeverity::Critical)
        );
        assert_eq!(
            significance_to_severity(2.0),
            Some(InsightSeverity::Critical)
        );
    }

    #[test]
    fn interaction_severity_mapping() {
        assert_eq!(interaction_severity("critical"), InsightSeverity::Critical);
        assert_eq!(interaction_severity("high"), InsightSeverity::Critical);
        assert_eq!(interaction_severity("moderate"), InsightSeverity::Warning);
        assert_eq!(interaction_severity("low"), InsightSeverity::Info);
        assert_eq!(interaction_severity("unknown"), InsightSeverity::Info);
    }

    #[test]
    fn drug_matches_field_direct_name() {
        let registry = InvariantRegistry::empty();
        assert!(drug_matches_field("warfarin", "warfarin", &registry));
        assert!(drug_matches_field("Warfarin", "warfarin", &registry));
        assert!(!drug_matches_field("metformin", "warfarin", &registry));
    }

    #[test]
    fn drug_matches_field_via_family() {
        let registry = loaded_registry();
        if registry.drug_families().is_empty() {
            return;
        }
        // ibuprofen belongs to NSAID family
        assert!(drug_matches_field("ibuprofen", "nsaid", &registry));
        // atorvastatin belongs to statin family
        assert!(drug_matches_field("atorvastatin", "statin", &registry));
        // metformin is not an NSAID
        assert!(!drug_matches_field("metformin", "nsaid", &registry));
    }

    // ── FIX-M1: Monitoring via drug family key ──────────────

    #[test]
    fn lisinopril_monitoring_via_ace_inhibitor_family() {
        let registry = loaded_registry();
        if registry.monitoring_schedules().is_empty() || registry.drug_families().is_empty() {
            return;
        }
        let meds = vec![make_med("lisinopril")];
        let labs: Vec<LabResult> = vec![];
        let insights = detect_missing_monitoring(&meds, &labs, &registry, today());
        assert!(
            !insights.is_empty(),
            "Lisinopril (ACE inhibitor) should trigger monitoring via family key"
        );
        // Should have potassium + eGFR monitoring from ace_inhibitor schedules
        let has_potassium = insights.iter().any(|i| i.summary_key.contains("potassium"));
        let has_egfr = insights.iter().any(|i| i.summary_key.contains("egfr"));
        assert!(has_potassium, "ACE inhibitor should require potassium monitoring");
        assert!(has_egfr, "ACE inhibitor should require eGFR monitoring");
    }

    #[test]
    fn atorvastatin_monitoring_via_statin_family() {
        let registry = loaded_registry();
        if registry.monitoring_schedules().is_empty() || registry.drug_families().is_empty() {
            return;
        }
        let meds = vec![make_med("atorvastatin")];
        let labs: Vec<LabResult> = vec![];
        let insights = detect_missing_monitoring(&meds, &labs, &registry, today());
        assert!(
            !insights.is_empty(),
            "Atorvastatin (statin) should trigger monitoring via family key"
        );
        // Should have ALT + LDL cholesterol monitoring from statin schedules
        let has_alt = insights.iter().any(|i| i.summary_key.contains("alt"));
        let has_ldl = insights.iter().any(|i| i.summary_key.contains("ldl"));
        assert!(has_alt, "Statin should require ALT monitoring");
        assert!(has_ldl, "Statin should require LDL cholesterol monitoring");
    }

    #[test]
    fn direct_drug_monitoring_still_works() {
        let registry = loaded_registry();
        if registry.monitoring_schedules().is_empty() {
            return;
        }
        // Metformin has direct entries (not via family key)
        let schedules = registry.find_monitoring("metformin");
        assert!(
            schedules.len() >= 2,
            "Metformin direct lookup should still work: got {}",
            schedules.len()
        );
    }

    // ── FIX-M2: Same-family allergy detection ───────────────

    #[test]
    fn penicillin_allergy_amoxicillin_same_family_detected() {
        let registry = loaded_registry();
        if registry.drug_families().is_empty() {
            return;
        }
        let allergies = vec![make_allergy("penicillin")];
        let meds = vec![make_med("amoxicillin")];
        let insights = detect_same_family_allergy(&allergies, &meds, &registry);
        assert!(
            !insights.is_empty(),
            "Penicillin allergy + amoxicillin (same family) should be detected"
        );
        assert_eq!(insights[0].kind, InsightKind::CrossReactivity);
        assert_eq!(insights[0].severity, InsightSeverity::Critical);
        assert!(insights[0].summary_key.contains("same"));
    }

    #[test]
    fn nsaid_allergy_ibuprofen_same_family_detected() {
        let registry = loaded_registry();
        if registry.drug_families().is_empty() {
            return;
        }
        let allergies = vec![make_allergy("aspirin")];
        let meds = vec![make_med("ibuprofen")];
        let insights = detect_same_family_allergy(&allergies, &meds, &registry);
        assert!(
            !insights.is_empty(),
            "Aspirin allergy + ibuprofen (same NSAID family) should be detected"
        );
        assert_eq!(insights[0].severity, InsightSeverity::Critical);
    }

    #[test]
    fn unrelated_allergy_no_same_family_alert() {
        let registry = loaded_registry();
        let allergies = vec![make_allergy("peanut")];
        let meds = vec![make_med("metformin")];
        let insights = detect_same_family_allergy(&allergies, &meds, &registry);
        assert!(
            insights.is_empty(),
            "Peanut allergy + metformin should not trigger same-family alert"
        );
    }

    #[test]
    fn exact_match_allergen_skipped_in_same_family() {
        let registry = loaded_registry();
        if registry.drug_families().is_empty() {
            return;
        }
        // Exact same drug: penicillin allergy + penicillin medication
        // This is a direct contraindication, not a same-family discovery
        let allergies = vec![make_allergy("amoxicillin")];
        let meds = vec![make_med("amoxicillin")];
        let insights = detect_same_family_allergy(&allergies, &meds, &registry);
        assert!(
            insights.is_empty(),
            "Exact allergen=medication match should be skipped (direct contraindication)"
        );
    }

    #[test]
    fn stopped_medication_no_same_family_alert() {
        let registry = loaded_registry();
        if registry.drug_families().is_empty() {
            return;
        }
        let allergies = vec![make_allergy("penicillin")];
        let mut amox = make_med("amoxicillin");
        amox.status = MedicationStatus::Stopped;
        let insights = detect_same_family_allergy(&allergies, &[amox], &registry);
        assert!(
            insights.is_empty(),
            "Stopped medications should not trigger same-family alert"
        );
    }

    #[test]
    fn empty_registry_no_same_family_alert() {
        let registry = InvariantRegistry::empty();
        let allergies = vec![make_allergy("penicillin")];
        let meds = vec![make_med("amoxicillin")];
        let insights = detect_same_family_allergy(&allergies, &meds, &registry);
        assert!(insights.is_empty());
    }

    // ── ME-04: Demographic-aware enrichment tests ───────────

    fn make_demographics(
        sex: Option<BiologicalSex>,
        ethnicities: Vec<crate::crypto::profile::EthnicityGroup>,
    ) -> PatientDemographics {
        PatientDemographics {
            sex,
            ethnicities,
            age_context: None,
            age_years: None,
        }
    }

    #[test]
    fn hemoglobin_12_5_male_mild_anemia() {
        let registry = InvariantRegistry::empty();
        let labs = vec![make_lab("Hb", 12.5, "2026-01-15")];
        let demo = make_demographics(Some(BiologicalSex::Male), Vec::new());
        let insights = classify_labs(&labs, &registry, Some(&demo));
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].description.key, "hb_mild_anemia");
        assert_eq!(insights[0].severity, InsightSeverity::Warning);
    }

    #[test]
    fn hemoglobin_12_5_female_normal() {
        let registry = InvariantRegistry::empty();
        let labs = vec![make_lab("Hb", 12.5, "2026-01-15")];
        let demo = make_demographics(Some(BiologicalSex::Female), Vec::new());
        let insights = classify_labs(&labs, &registry, Some(&demo));
        // 12.5 is in normal range for female (12.0-17.5) — significance 0.2 → no insight
        assert!(insights.is_empty(), "12.5 is normal for female");
    }

    #[test]
    fn hemoglobin_12_5_unknown_sex_normal() {
        let registry = InvariantRegistry::empty();
        let labs = vec![make_lab("Hb", 12.5, "2026-01-15")];
        let insights = classify_labs(&labs, &registry, None);
        // Conservative female default: 12.0 = normal → no insight
        assert!(insights.is_empty(), "12.5 should be normal with unknown sex");
    }

    #[test]
    fn hemoglobin_7_5_severe_regardless_of_sex() {
        let registry = InvariantRegistry::empty();
        let labs = vec![make_lab("Hb", 7.5, "2026-01-15")];
        let demo_male = make_demographics(Some(BiologicalSex::Male), Vec::new());
        let demo_female = make_demographics(Some(BiologicalSex::Female), Vec::new());
        let male_insights = classify_labs(&labs, &registry, Some(&demo_male));
        let female_insights = classify_labs(&labs, &registry, Some(&demo_female));
        assert_eq!(male_insights[0].description.key, "hb_severe_anemia");
        assert_eq!(female_insights[0].description.key, "hb_severe_anemia");
    }

    #[test]
    fn bmi_24_asian_overweight() {
        use crate::crypto::profile::EthnicityGroup;
        let vitals = vec![
            make_vital(VitalType::Weight, 69.0, None),
            make_vital(VitalType::Height, 170.0, None),
        ];
        // BMI ≈ 69/(1.7²) ≈ 23.9 → Asian: overweight (≥23.0), Standard: normal (<25.0)
        let demo = make_demographics(None, vec![EthnicityGroup::EastAsian]);
        let insights = classify_vitals(&vitals, Some(&demo));
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].description.key, "bmi_overweight");
    }

    #[test]
    fn bmi_24_european_normal() {
        use crate::crypto::profile::EthnicityGroup;
        let vitals = vec![
            make_vital(VitalType::Weight, 69.0, None),
            make_vital(VitalType::Height, 170.0, None),
        ];
        // BMI ≈ 23.9 → Standard: normal (<25.0)
        let demo = make_demographics(None, vec![EthnicityGroup::European]);
        let insights = classify_vitals(&vitals, Some(&demo));
        assert!(insights.is_empty(), "23.9 is normal for European thresholds");
    }

    #[test]
    fn bmi_24_blend_asian_european_uses_asian() {
        use crate::crypto::profile::EthnicityGroup;
        let vitals = vec![
            make_vital(VitalType::Weight, 69.0, None),
            make_vital(VitalType::Height, 170.0, None),
        ];
        // Any Asian ethnicity in blend → use Asian thresholds
        let demo = make_demographics(
            None,
            vec![EthnicityGroup::European, EthnicityGroup::EastAsian],
        );
        let insights = classify_vitals(&vitals, Some(&demo));
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].description.key, "bmi_overweight");
    }

    #[test]
    fn bmi_24_no_demographics_uses_standard() {
        let vitals = vec![
            make_vital(VitalType::Weight, 69.0, None),
            make_vital(VitalType::Height, 170.0, None),
        ];
        let insights = classify_vitals(&vitals, None);
        assert!(insights.is_empty(), "23.9 is normal with WHO standard thresholds");
    }

    #[test]
    fn bmi_28_south_asian_obese() {
        use crate::crypto::profile::EthnicityGroup;
        let vitals = vec![
            make_vital(VitalType::Weight, 81.0, None),
            make_vital(VitalType::Height, 170.0, None),
        ];
        // BMI ≈ 81/(1.7²) ≈ 28.0 → Asian: obese I (≥27.5), Standard: overweight (<30)
        let demo = make_demographics(None, vec![EthnicityGroup::SouthAsian]);
        let insights = classify_vitals(&vitals, Some(&demo));
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].description.key, "bmi_obese_i");
    }

    #[test]
    fn bmi_28_european_overweight() {
        use crate::crypto::profile::EthnicityGroup;
        let vitals = vec![
            make_vital(VitalType::Weight, 81.0, None),
            make_vital(VitalType::Height, 170.0, None),
        ];
        let demo = make_demographics(None, vec![EthnicityGroup::European]);
        let insights = classify_vitals(&vitals, Some(&demo));
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].description.key, "bmi_overweight");
    }

    #[test]
    fn enrich_demographics_none_backward_compat() {
        let registry = InvariantRegistry::empty();
        let vitals = vec![make_vital(VitalType::BloodPressure, 145.0, Some(92.0))];
        let insights = enrich(&[], &[], &[], &vitals, &registry, today(), None);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].description.key, "bp_grade_1_htn");
    }
}
