use uuid::Uuid;

use crate::models::enums::{AbnormalFlag, AlertType, DiagnosisStatus, MedicationStatus};
use crate::models::Medication;

use super::helpers::{
    dedup_symmetric_alerts, display_name, format_dose_mg, frequency_to_daily_multiplier,
    is_same_drug_family, normalize_dose, normalize_frequency, parse_dose_to_mg,
    resolve_generic_name,
};
use super::messages::MessageTemplates;
use super::reference::CoherenceReferenceData;
use super::types::*;

/// Temporal correlation window in days.
const TEMPORAL_CORRELATION_WINDOW_DAYS: i64 = 14;

// ---------------------------------------------------------------------------
// [4] CONFLICT detection
// ---------------------------------------------------------------------------

/// Detect medication conflicts: same medication, different parameters, different prescribers.
pub fn detect_conflicts(
    document_id: &Uuid,
    data: &RepositorySnapshot,
    reference: &CoherenceReferenceData,
) -> Vec<CoherenceAlert> {
    let mut alerts = Vec::new();

    let active_meds: Vec<&Medication> = data
        .medications
        .iter()
        .filter(|m| m.status == MedicationStatus::Active)
        .collect();

    let new_meds: Vec<&&Medication> = active_meds
        .iter()
        .filter(|m| m.document_id == *document_id)
        .collect();

    for new_med in &new_meds {
        let resolved_generic = resolve_generic_name(new_med, reference);

        let existing_matches: Vec<&&Medication> = active_meds
            .iter()
            .filter(|m| {
                m.id != new_med.id
                    && m.document_id != *document_id
                    && resolve_generic_name(m, reference) == resolved_generic
                    && m.status == MedicationStatus::Active
            })
            .collect();

        for existing in &existing_matches {
            let different_prescriber = match (new_med.prescriber_id, existing.prescriber_id) {
                (Some(a), Some(b)) => a != b,
                _ => true,
            };

            if !different_prescriber {
                continue;
            }

            if normalize_dose(&new_med.dose) != normalize_dose(&existing.dose) {
                alerts.push(build_conflict_alert(
                    new_med, existing, "dose", &new_med.dose, &existing.dose, data,
                ));
            }

            if normalize_frequency(&new_med.frequency)
                != normalize_frequency(&existing.frequency)
            {
                alerts.push(build_conflict_alert(
                    new_med,
                    existing,
                    "frequency",
                    &new_med.frequency,
                    &existing.frequency,
                    data,
                ));
            }

            if new_med.route.to_lowercase() != existing.route.to_lowercase() {
                alerts.push(build_conflict_alert(
                    new_med, existing, "route", &new_med.route, &existing.route, data,
                ));
            }
        }
    }

    alerts
}

fn build_conflict_alert(
    new_med: &Medication,
    existing_med: &Medication,
    field: &str,
    new_value: &str,
    existing_value: &str,
    data: &RepositorySnapshot,
) -> CoherenceAlert {
    let prescriber_a_name = data.resolve_prescriber_name(new_med.prescriber_id);
    let prescriber_b_name = data.resolve_prescriber_name(existing_med.prescriber_id);

    let message = MessageTemplates::conflict(
        &new_med.generic_name,
        field,
        new_value,
        &prescriber_a_name,
        existing_value,
        &prescriber_b_name,
    );

    CoherenceAlert {
        id: Uuid::new_v4(),
        alert_type: AlertType::Conflict,
        severity: AlertSeverity::Standard,
        entity_ids: vec![new_med.id, existing_med.id],
        source_document_ids: vec![new_med.document_id, existing_med.document_id],
        patient_message: message,
        detail: AlertDetail::Conflict(ConflictDetail {
            medication_name: new_med.generic_name.clone(),
            prescriber_a: PrescriberRef {
                professional_id: new_med.prescriber_id.unwrap_or(Uuid::nil()),
                name: prescriber_a_name,
                document_id: new_med.document_id,
                document_date: None,
            },
            prescriber_b: PrescriberRef {
                professional_id: existing_med.prescriber_id.unwrap_or(Uuid::nil()),
                name: prescriber_b_name,
                document_id: existing_med.document_id,
                document_date: None,
            },
            field_conflicted: field.to_string(),
            value_a: new_value.to_string(),
            value_b: existing_value.to_string(),
        }),
        detected_at: chrono::Local::now().naive_local(),
        surfaced: false,
        dismissed: false,
        dismissal: None,
    }
}

// ---------------------------------------------------------------------------
// [5] DUPLICATE detection
// ---------------------------------------------------------------------------

/// Detect duplicate medications: same generic under different brand names.
pub fn detect_duplicates(
    document_id: &Uuid,
    data: &RepositorySnapshot,
    reference: &CoherenceReferenceData,
) -> Vec<CoherenceAlert> {
    let mut alerts = Vec::new();

    let active_meds: Vec<&Medication> = data
        .medications
        .iter()
        .filter(|m| m.status == MedicationStatus::Active)
        .collect();

    let new_meds: Vec<&&Medication> = active_meds
        .iter()
        .filter(|m| m.document_id == *document_id)
        .collect();

    for new_med in &new_meds {
        let new_generic = resolve_generic_name(new_med, reference);

        let duplicates: Vec<&&Medication> = active_meds
            .iter()
            .filter(|m| {
                m.id != new_med.id
                    && m.document_id != *document_id
                    && resolve_generic_name(m, reference) == new_generic
                    && m.status == MedicationStatus::Active
            })
            .collect();

        for existing in &duplicates {
            let new_display = display_name(new_med);
            let existing_display = display_name(existing);

            if new_display.to_lowercase() != existing_display.to_lowercase() {
                let message =
                    MessageTemplates::duplicate(&new_display, &existing_display, &new_generic);

                alerts.push(CoherenceAlert {
                    id: Uuid::new_v4(),
                    alert_type: AlertType::Duplicate,
                    severity: AlertSeverity::Standard,
                    entity_ids: vec![new_med.id, existing.id],
                    source_document_ids: vec![new_med.document_id, existing.document_id],
                    patient_message: message,
                    detail: AlertDetail::Duplicate(DuplicateDetail {
                        generic_name: new_generic.clone(),
                        brand_a: new_display,
                        brand_b: existing_display,
                        medication_id_a: new_med.id,
                        medication_id_b: existing.id,
                    }),
                    detected_at: chrono::Local::now().naive_local(),
                    surfaced: false,
                    dismissed: false,
                    dismissal: None,
                });
            }
        }
    }

    dedup_symmetric_alerts(&mut alerts);
    alerts
}

// ---------------------------------------------------------------------------
// [6] GAP detection
// ---------------------------------------------------------------------------

/// Detect care gaps: diagnosis without treatment, treatment without diagnosis.
pub fn detect_gaps(
    document_id: &Uuid,
    data: &RepositorySnapshot,
) -> Vec<CoherenceAlert> {
    let mut alerts = Vec::new();

    let active_diagnoses: Vec<&_> = data
        .diagnoses
        .iter()
        .filter(|d| d.status == DiagnosisStatus::Active)
        .collect();
    let active_meds: Vec<&_> = data
        .medications
        .iter()
        .filter(|m| m.status == MedicationStatus::Active)
        .collect();

    // GAP TYPE 1: Diagnosis without treatment
    for diag in &active_diagnoses {
        let has_treatment = active_meds
            .iter()
            .any(|m| super::helpers::medication_relates_to_diagnosis(m, diag));

        if !has_treatment && (diag.document_id == *document_id || document_id.is_nil()) {
            let message = MessageTemplates::gap_no_treatment(&diag.name);

            alerts.push(CoherenceAlert {
                id: Uuid::new_v4(),
                alert_type: AlertType::Gap,
                severity: AlertSeverity::Info,
                entity_ids: vec![diag.id],
                source_document_ids: vec![diag.document_id],
                patient_message: message,
                detail: AlertDetail::Gap(GapDetail {
                    gap_type: GapType::DiagnosisWithoutTreatment,
                    entity_name: diag.name.clone(),
                    entity_id: diag.id,
                    expected: "medication or treatment plan".to_string(),
                    document_id: diag.document_id,
                }),
                detected_at: chrono::Local::now().naive_local(),
                surfaced: false,
                dismissed: false,
                dismissal: None,
            });
        }
    }

    // GAP TYPE 2: Medication without diagnosis
    for med in &active_meds {
        if med.is_otc {
            continue;
        }

        let has_diagnosis = active_diagnoses
            .iter()
            .any(|d| super::helpers::medication_relates_to_diagnosis(med, d));

        let has_reason = med
            .reason_start
            .as_ref()
            .map(|r| !r.trim().is_empty())
            .unwrap_or(false);

        if !has_diagnosis
            && !has_reason
            && (med.document_id == *document_id || document_id.is_nil())
        {
            let med_display = display_name(med);
            let message = MessageTemplates::gap_no_diagnosis(&med_display);

            alerts.push(CoherenceAlert {
                id: Uuid::new_v4(),
                alert_type: AlertType::Gap,
                severity: AlertSeverity::Info,
                entity_ids: vec![med.id],
                source_document_ids: vec![med.document_id],
                patient_message: message,
                detail: AlertDetail::Gap(GapDetail {
                    gap_type: GapType::MedicationWithoutDiagnosis,
                    entity_name: med_display,
                    entity_id: med.id,
                    expected: "documented diagnosis or reason".to_string(),
                    document_id: med.document_id,
                }),
                detected_at: chrono::Local::now().naive_local(),
                surfaced: false,
                dismissed: false,
                dismissal: None,
            });
        }
    }

    alerts
}

// ---------------------------------------------------------------------------
// [7] DRIFT detection
// ---------------------------------------------------------------------------

/// Detect care drift: unexplained medication or diagnosis changes.
pub fn detect_drift(
    document_id: &Uuid,
    data: &RepositorySnapshot,
    reference: &CoherenceReferenceData,
) -> Vec<CoherenceAlert> {
    let mut alerts = Vec::new();

    // --- Medication drift ---
    let new_meds: Vec<&Medication> = data
        .medications
        .iter()
        .filter(|m| m.document_id == *document_id)
        .collect();

    for new_med in &new_meds {
        let new_generic = resolve_generic_name(new_med, reference);

        let prior: Vec<&Medication> = data
            .medications
            .iter()
            .filter(|m| {
                m.id != new_med.id
                    && m.document_id != *document_id
                    && resolve_generic_name(m, reference) == new_generic
            })
            .collect();

        for old_med in &prior {
            // Status changed to stopped without reason
            if old_med.status == MedicationStatus::Active
                && new_med.status == MedicationStatus::Stopped
            {
                let reason_given = new_med
                    .reason_stop
                    .as_ref()
                    .map(|r| !r.trim().is_empty())
                    .unwrap_or(false);

                if !reason_given {
                    let message = MessageTemplates::drift(
                        &new_med.generic_name,
                        old_med.status.as_str(),
                        new_med.status.as_str(),
                    );
                    alerts.push(build_drift_alert(
                        new_med,
                        "status",
                        old_med.status.as_str(),
                        new_med.status.as_str(),
                        false,
                        &message,
                    ));
                }
            }

            // Dose changed without documented reason
            if normalize_dose(&old_med.dose) != normalize_dose(&new_med.dose) {
                let has_dose_change_record = data
                    .get_dose_history(&new_med.id)
                    .iter()
                    .any(|dc| {
                        dc.reason
                            .as_ref()
                            .map(|r| !r.trim().is_empty())
                            .unwrap_or(false)
                    });

                if !has_dose_change_record {
                    let message = MessageTemplates::drift(
                        &new_med.generic_name,
                        &old_med.dose,
                        &new_med.dose,
                    );
                    alerts.push(build_drift_alert(
                        new_med,
                        "dose",
                        &old_med.dose,
                        &new_med.dose,
                        false,
                        &message,
                    ));
                }
            }
        }
    }

    // --- Diagnosis drift ---
    let new_diags: Vec<&_> = data
        .diagnoses
        .iter()
        .filter(|d| d.document_id == *document_id)
        .collect();

    for new_diag in &new_diags {
        let prior_diags: Vec<&_> = data
            .diagnoses
            .iter()
            .filter(|d| {
                d.id != new_diag.id
                    && d.document_id != *document_id
                    && d.name.to_lowercase() == new_diag.name.to_lowercase()
            })
            .collect();

        for old_diag in &prior_diags {
            if old_diag.status != new_diag.status {
                let message = format!(
                    "The status of {} changed from {} to {}. \
                     I don't see a note explaining this change.",
                    new_diag.name,
                    old_diag.status.as_str(),
                    new_diag.status.as_str(),
                );

                alerts.push(CoherenceAlert {
                    id: Uuid::new_v4(),
                    alert_type: AlertType::Drift,
                    severity: AlertSeverity::Info,
                    entity_ids: vec![new_diag.id, old_diag.id],
                    source_document_ids: vec![new_diag.document_id, old_diag.document_id],
                    patient_message: message,
                    detail: AlertDetail::Drift(DriftDetail {
                        entity_type: "diagnosis".to_string(),
                        entity_name: new_diag.name.clone(),
                        old_value: old_diag.status.as_str().to_string(),
                        new_value: new_diag.status.as_str().to_string(),
                        change_date: new_diag.date_diagnosed,
                        reason_documented: false,
                    }),
                    detected_at: chrono::Local::now().naive_local(),
                    surfaced: false,
                    dismissed: false,
                    dismissal: None,
                });
            }
        }
    }

    alerts
}

fn build_drift_alert(
    med: &Medication,
    _field: &str,
    old_val: &str,
    new_val: &str,
    reason_documented: bool,
    message: &str,
) -> CoherenceAlert {
    CoherenceAlert {
        id: Uuid::new_v4(),
        alert_type: AlertType::Drift,
        severity: AlertSeverity::Standard,
        entity_ids: vec![med.id],
        source_document_ids: vec![med.document_id],
        patient_message: message.to_string(),
        detail: AlertDetail::Drift(DriftDetail {
            entity_type: "medication".to_string(),
            entity_name: med.generic_name.clone(),
            old_value: old_val.to_string(),
            new_value: new_val.to_string(),
            change_date: None,
            reason_documented,
        }),
        detected_at: chrono::Local::now().naive_local(),
        surfaced: false,
        dismissed: false,
        dismissal: None,
    }
}

// ---------------------------------------------------------------------------
// [8] TEMPORAL detection
// ---------------------------------------------------------------------------

/// Detect temporal correlations: symptoms near medication/dose/procedure changes.
pub fn detect_temporal(
    document_id: &Uuid,
    data: &RepositorySnapshot,
) -> Vec<CoherenceAlert> {
    let mut alerts = Vec::new();

    let symptoms = if document_id.is_nil() {
        // Full analysis: all active symptoms
        data.symptoms
            .iter()
            .filter(|s| s.still_active)
            .collect::<Vec<_>>()
    } else {
        // Document-triggered: recent symptoms within window
        let window_start = chrono::Local::now().date_naive()
            - chrono::Duration::days(TEMPORAL_CORRELATION_WINDOW_DAYS);
        data.symptoms
            .iter()
            .filter(|s| s.onset_date >= window_start)
            .collect()
    };

    let active_meds: Vec<&Medication> = data
        .medications
        .iter()
        .filter(|m| m.status == MedicationStatus::Active)
        .collect();

    for symptom in &symptoms {
        let onset = symptom.onset_date;

        // Check medication start dates
        for med in &active_meds {
            if let Some(start) = med.start_date {
                let days_between = (onset - start).num_days();
                if (0..=TEMPORAL_CORRELATION_WINDOW_DAYS).contains(&days_between) {
                    let message = MessageTemplates::temporal(
                        &symptom.specific,
                        &onset.to_string(),
                        days_between,
                        &format!("starting {}", med.generic_name),
                    );

                    alerts.push(CoherenceAlert {
                        id: Uuid::new_v4(),
                        alert_type: AlertType::Temporal,
                        severity: AlertSeverity::Standard,
                        entity_ids: vec![symptom.id, med.id],
                        source_document_ids: vec![med.document_id],
                        patient_message: message,
                        detail: AlertDetail::Temporal(TemporalDetail {
                            symptom_id: symptom.id,
                            symptom_name: symptom.specific.clone(),
                            symptom_onset: onset,
                            correlated_event: TemporalEvent::MedicationStarted {
                                medication_id: med.id,
                                medication_name: med.generic_name.clone(),
                                start_date: start,
                            },
                            days_between,
                        }),
                        detected_at: chrono::Local::now().naive_local(),
                        surfaced: false,
                        dismissed: false,
                        dismissal: None,
                    });
                }
            }

            // Check dose changes
            for dc in data.get_dose_history(&med.id) {
                let change_date = dc.change_date;
                let days_between = (onset - change_date).num_days();
                if (0..=TEMPORAL_CORRELATION_WINDOW_DAYS).contains(&days_between) {
                    let message = MessageTemplates::temporal(
                        &symptom.specific,
                        &onset.to_string(),
                        days_between,
                        &format!(
                            "your {} dose was changed from {} to {}",
                            med.generic_name,
                            dc.old_dose.as_deref().unwrap_or("unknown"),
                            dc.new_dose,
                        ),
                    );

                    alerts.push(CoherenceAlert {
                        id: Uuid::new_v4(),
                        alert_type: AlertType::Temporal,
                        severity: AlertSeverity::Standard,
                        entity_ids: vec![symptom.id, med.id],
                        source_document_ids: if let Some(doc_id) = dc.document_id {
                            vec![doc_id]
                        } else {
                            vec![med.document_id]
                        },
                        patient_message: message,
                        detail: AlertDetail::Temporal(TemporalDetail {
                            symptom_id: symptom.id,
                            symptom_name: symptom.specific.clone(),
                            symptom_onset: onset,
                            correlated_event: TemporalEvent::DoseChanged {
                                medication_id: med.id,
                                medication_name: med.generic_name.clone(),
                                old_dose: dc
                                    .old_dose
                                    .clone()
                                    .unwrap_or_else(|| "unknown".to_string()),
                                new_dose: dc.new_dose.clone(),
                                change_date,
                            },
                            days_between,
                        }),
                        detected_at: chrono::Local::now().naive_local(),
                        surfaced: false,
                        dismissed: false,
                        dismissal: None,
                    });
                }
            }
        }

        // Check procedures
        for procedure in &data.procedures {
            if let Some(proc_date) = procedure.date {
                let days_between = (onset - proc_date).num_days();
                if (0..=TEMPORAL_CORRELATION_WINDOW_DAYS).contains(&days_between) {
                    let message = MessageTemplates::temporal(
                        &symptom.specific,
                        &onset.to_string(),
                        days_between,
                        &format!("your {} procedure", procedure.name),
                    );

                    alerts.push(CoherenceAlert {
                        id: Uuid::new_v4(),
                        alert_type: AlertType::Temporal,
                        severity: AlertSeverity::Standard,
                        entity_ids: vec![symptom.id, procedure.id],
                        source_document_ids: vec![procedure.document_id],
                        patient_message: message,
                        detail: AlertDetail::Temporal(TemporalDetail {
                            symptom_id: symptom.id,
                            symptom_name: symptom.specific.clone(),
                            symptom_onset: onset,
                            correlated_event: TemporalEvent::ProcedurePerformed {
                                procedure_id: procedure.id,
                                procedure_name: procedure.name.clone(),
                                procedure_date: proc_date,
                            },
                            days_between,
                        }),
                        detected_at: chrono::Local::now().naive_local(),
                        surfaced: false,
                        dismissed: false,
                        dismissal: None,
                    });
                }
            }
        }
    }

    alerts
}

// ---------------------------------------------------------------------------
// [9] ALLERGY detection
// ---------------------------------------------------------------------------

/// Detect allergy cross-matches: new medication contains known allergen.
/// Severity: CRITICAL.
pub fn detect_allergy_conflicts(
    document_id: &Uuid,
    data: &RepositorySnapshot,
    reference: &CoherenceReferenceData,
) -> Vec<CoherenceAlert> {
    let mut alerts = Vec::new();

    if data.allergies.is_empty() {
        return alerts;
    }

    let allergen_map: std::collections::HashMap<String, &_> = data
        .allergies
        .iter()
        .map(|a| (a.allergen.to_lowercase(), a))
        .collect();

    let new_meds: Vec<&Medication> = data
        .medications
        .iter()
        .filter(|m| m.document_id == *document_id)
        .collect();

    for med in &new_meds {
        let mut ingredient_names: Vec<(String, String)> = Vec::new();

        let generic = resolve_generic_name(med, reference);
        if !generic.is_empty() {
            ingredient_names.push((generic.clone(), med.generic_name.clone()));
        }

        if med.is_compound {
            for ing in data.get_compound_ingredients(&med.id) {
                let resolved = ing
                    .maps_to_generic
                    .as_deref()
                    .unwrap_or(&ing.ingredient_name)
                    .to_lowercase();
                ingredient_names.push((resolved, ing.ingredient_name.clone()));
            }
        }

        for (resolved_name, ingredient_display) in &ingredient_names {
            // Direct allergen match
            if let Some(allergy) = allergen_map.get(resolved_name.as_str()) {
                let med_display = display_name(med);
                let message =
                    MessageTemplates::allergy(&allergy.allergen, &med_display, ingredient_display);

                alerts.push(build_allergy_alert(
                    med,
                    allergy,
                    &med_display,
                    ingredient_display,
                    resolved_name,
                    &message,
                ));
            }

            // Drug family match
            for allergy in &data.allergies {
                let allergen_lower = allergy.allergen.to_lowercase();
                if allergen_lower != *resolved_name
                    && is_same_drug_family(&allergen_lower, resolved_name)
                {
                    let med_display = display_name(med);
                    let message = MessageTemplates::allergy(
                        &allergy.allergen,
                        &med_display,
                        ingredient_display,
                    );

                    alerts.push(build_allergy_alert(
                        med,
                        allergy,
                        &med_display,
                        ingredient_display,
                        resolved_name,
                        &message,
                    ));
                }
            }
        }
    }

    dedup_symmetric_alerts(&mut alerts);
    alerts
}

fn build_allergy_alert(
    med: &Medication,
    allergy: &crate::models::Allergy,
    med_display: &str,
    ingredient_display: &str,
    resolved_name: &str,
    message: &str,
) -> CoherenceAlert {
    CoherenceAlert {
        id: Uuid::new_v4(),
        alert_type: AlertType::Allergy,
        severity: AlertSeverity::Critical,
        entity_ids: vec![med.id, allergy.id],
        source_document_ids: vec![
            med.document_id,
            allergy.document_id.unwrap_or(Uuid::nil()),
        ],
        patient_message: message.to_string(),
        detail: AlertDetail::Allergy(AllergyDetail {
            allergen: allergy.allergen.clone(),
            allergy_severity: allergy.severity.as_str().to_string(),
            allergy_id: allergy.id,
            medication_name: med_display.to_string(),
            medication_id: med.id,
            matching_ingredient: ingredient_display.to_string(),
            ingredient_maps_to: resolved_name.to_string(),
        }),
        detected_at: chrono::Local::now().naive_local(),
        surfaced: false,
        dismissed: false,
        dismissal: None,
    }
}

// ---------------------------------------------------------------------------
// [10] DOSE detection
// ---------------------------------------------------------------------------

/// Detect dose plausibility issues: extracted dose outside typical range.
pub fn detect_dose_issues(
    document_id: &Uuid,
    data: &RepositorySnapshot,
    reference: &CoherenceReferenceData,
) -> Vec<CoherenceAlert> {
    let mut alerts = Vec::new();

    let new_meds: Vec<&Medication> = data
        .medications
        .iter()
        .filter(|m| m.document_id == *document_id)
        .collect();

    for med in &new_meds {
        let generic = resolve_generic_name(med, reference);

        let dose_range = match reference.get_dose_range(&generic) {
            Some(range) => range,
            None => continue,
        };

        let extracted_mg = match parse_dose_to_mg(&med.dose) {
            Some(mg) => mg,
            None => continue,
        };

        let outside_range = extracted_mg < dose_range.min_single_dose_mg
            || extracted_mg > dose_range.max_single_dose_mg;

        if outside_range {
            let med_display = display_name(med);
            let message = MessageTemplates::dose(
                &med.dose,
                &med_display,
                &format_dose_mg(dose_range.min_single_dose_mg),
                &format_dose_mg(dose_range.max_single_dose_mg),
            );

            alerts.push(CoherenceAlert {
                id: Uuid::new_v4(),
                alert_type: AlertType::Dose,
                severity: AlertSeverity::Standard,
                entity_ids: vec![med.id],
                source_document_ids: vec![med.document_id],
                patient_message: message,
                detail: AlertDetail::Dose(DoseDetail {
                    medication_name: med_display,
                    medication_id: med.id,
                    extracted_dose: med.dose.clone(),
                    extracted_dose_mg: extracted_mg,
                    typical_range_low_mg: dose_range.min_single_dose_mg,
                    typical_range_high_mg: dose_range.max_single_dose_mg,
                    source: "dose_ranges.json".to_string(),
                }),
                detected_at: chrono::Local::now().naive_local(),
                surfaced: false,
                dismissed: false,
                dismissal: None,
            });
        }
    }

    alerts
}

// ---------------------------------------------------------------------------
// [10b] DAILY DOSE ACCUMULATION detection (RS-L2-03-002)
// ---------------------------------------------------------------------------

/// Detect daily dose accumulation: single_dose × frequency > max daily dose.
pub fn detect_daily_dose_accumulation(
    document_id: &Uuid,
    data: &RepositorySnapshot,
    reference: &CoherenceReferenceData,
) -> Vec<CoherenceAlert> {
    let mut alerts = Vec::new();

    let new_meds: Vec<&Medication> = data
        .medications
        .iter()
        .filter(|m| m.document_id == *document_id || document_id.is_nil())
        .filter(|m| m.status == MedicationStatus::Active)
        .collect();

    for med in &new_meds {
        let single_dose_mg = match parse_dose_to_mg(&med.dose) {
            Some(mg) => mg,
            None => continue,
        };

        let multiplier = match frequency_to_daily_multiplier(&med.frequency) {
            Some(m) => m,
            None => continue, // "as needed" or unparseable — skip
        };

        let daily_total_mg = single_dose_mg * multiplier;

        // Check against reference max daily dose
        let generic = resolve_generic_name(med, reference);
        let max_daily_mg = reference
            .get_dose_range(&generic)
            .map(|r| r.max_daily_dose_mg);

        // Also check prescriber-provided max_daily_dose on the medication itself
        let prescriber_max_mg = med
            .max_daily_dose
            .as_ref()
            .and_then(|d| parse_dose_to_mg(d));

        // Use the lower of the two limits (more conservative)
        let effective_max = match (max_daily_mg, prescriber_max_mg) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };

        if let Some(max_mg) = effective_max {
            if daily_total_mg > max_mg {
                let med_display = display_name(med);
                let message = MessageTemplates::daily_dose(
                    &med_display,
                    &format_dose_mg(daily_total_mg),
                    &format_dose_mg(max_mg),
                );

                alerts.push(CoherenceAlert {
                    id: Uuid::new_v4(),
                    alert_type: AlertType::Dose,
                    severity: AlertSeverity::Standard,
                    entity_ids: vec![med.id],
                    source_document_ids: vec![med.document_id],
                    patient_message: message,
                    detail: AlertDetail::Dose(DoseDetail {
                        medication_name: med_display,
                        medication_id: med.id,
                        extracted_dose: format!(
                            "{}/day ({} × {})",
                            format_dose_mg(daily_total_mg),
                            med.dose,
                            med.frequency
                        ),
                        extracted_dose_mg: daily_total_mg,
                        typical_range_low_mg: 0.0,
                        typical_range_high_mg: max_mg,
                        source: "dose_ranges.json (daily limit)".to_string(),
                    }),
                    detected_at: chrono::Local::now().naive_local(),
                    surfaced: false,
                    dismissed: false,
                    dismissal: None,
                });
            }
        }
    }

    alerts
}

// ---------------------------------------------------------------------------
// [11] CRITICAL detection (lab values)
// ---------------------------------------------------------------------------

/// Detect critical lab values.
pub fn detect_critical_labs(
    document_id: &Uuid,
    data: &RepositorySnapshot,
) -> Vec<CoherenceAlert> {
    let mut alerts = Vec::new();

    let new_labs: Vec<&_> = data
        .lab_results
        .iter()
        .filter(|l| l.document_id == *document_id)
        .collect();

    for lab in &new_labs {
        let is_critical = matches!(
            lab.abnormal_flag,
            AbnormalFlag::CriticalLow | AbnormalFlag::CriticalHigh
        );

        if !is_critical {
            continue;
        }

        let value_display = lab
            .value
            .map(|v| format!("{}", v))
            .or_else(|| lab.value_text.clone())
            .unwrap_or_else(|| "value".to_string());

        let unit_display = lab.unit.as_deref().unwrap_or("");

        let flag_description = match lab.abnormal_flag {
            AbnormalFlag::CriticalLow => "below the expected range",
            AbnormalFlag::CriticalHigh => "above the expected range",
            _ => "outside the expected range",
        };

        let message = format!(
            "Your lab report from {} flags {} as needing prompt attention. \
             The result ({} {}) is {}. \
             Please contact your doctor or pharmacist soon.",
            lab.collection_date, lab.test_name, value_display, unit_display, flag_description,
        );

        alerts.push(CoherenceAlert {
            id: Uuid::new_v4(),
            alert_type: AlertType::Critical,
            severity: AlertSeverity::Critical,
            entity_ids: vec![lab.id],
            source_document_ids: vec![lab.document_id],
            patient_message: message,
            detail: AlertDetail::Critical(CriticalDetail {
                test_name: lab.test_name.clone(),
                lab_result_id: lab.id,
                value: lab.value.unwrap_or(0.0),
                unit: unit_display.to_string(),
                abnormal_flag: lab.abnormal_flag.as_str().to_string(),
                reference_range_low: lab.reference_range_low,
                reference_range_high: lab.reference_range_high,
                collection_date: lab.collection_date,
                document_id: lab.document_id,
            }),
            detected_at: chrono::Local::now().naive_local(),
            surfaced: false,
            dismissed: false,
            dismissal: None,
        });
    }

    alerts
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;
    use crate::models::enums::*;
    use crate::models::*;
    use std::collections::HashSet;

    fn empty_snapshot() -> RepositorySnapshot {
        RepositorySnapshot {
            medications: vec![],
            diagnoses: vec![],
            lab_results: vec![],
            allergies: vec![],
            symptoms: vec![],
            procedures: vec![],
            professionals: vec![],
            dose_changes: vec![],
            compound_ingredients: vec![],
            dismissed_alert_keys: HashSet::new(),
        }
    }

    fn make_medication(
        id: Uuid,
        generic: &str,
        brand: Option<&str>,
        dose: &str,
        freq: &str,
        prescriber: Option<Uuid>,
        doc_id: Uuid,
    ) -> Medication {
        Medication {
            id,
            generic_name: generic.into(),
            brand_name: brand.map(|s| s.into()),
            dose: dose.into(),
            frequency: freq.into(),
            frequency_type: FrequencyType::Scheduled,
            route: "oral".into(),
            prescriber_id: prescriber,
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
            document_id: doc_id,
        }
    }

    /// T-01: Two active Metformin prescriptions, different doses, different prescribers.
    #[test]
    fn conflict_different_dose_different_prescriber() {
        let ref_data = CoherenceReferenceData::load_test();
        let doc1 = Uuid::new_v4();
        let doc2 = Uuid::new_v4();
        let dr_a = Uuid::new_v4();
        let dr_b = Uuid::new_v4();

        let mut data = empty_snapshot();
        data.medications = vec![
            make_medication(Uuid::new_v4(), "Metformin", None, "500mg", "twice daily", Some(dr_a), doc1),
            make_medication(Uuid::new_v4(), "Metformin", None, "1000mg", "twice daily", Some(dr_b), doc2),
        ];

        let alerts = detect_conflicts(&doc2, &data, &ref_data);
        assert!(!alerts.is_empty(), "Expected conflict alert");
        assert!(alerts.iter().all(|a| a.alert_type == AlertType::Conflict));
    }

    /// T-02: Same medication, same prescriber, different dose -> no alert.
    #[test]
    fn conflict_same_prescriber_no_alert() {
        let ref_data = CoherenceReferenceData::load_test();
        let doc1 = Uuid::new_v4();
        let doc2 = Uuid::new_v4();
        let dr = Uuid::new_v4();

        let mut data = empty_snapshot();
        data.medications = vec![
            make_medication(Uuid::new_v4(), "Metformin", None, "500mg", "twice daily", Some(dr), doc1),
            make_medication(Uuid::new_v4(), "Metformin", None, "1000mg", "twice daily", Some(dr), doc2),
        ];

        let alerts = detect_conflicts(&doc2, &data, &ref_data);
        assert!(alerts.is_empty(), "Same prescriber should not trigger conflict");
    }

    /// T-03: Different brand names resolving to same generic, different doses.
    #[test]
    fn conflict_alias_resolution() {
        let ref_data = CoherenceReferenceData::load_test();
        let doc1 = Uuid::new_v4();
        let doc2 = Uuid::new_v4();
        let dr_a = Uuid::new_v4();
        let dr_b = Uuid::new_v4();

        let mut data = empty_snapshot();
        data.medications = vec![
            make_medication(Uuid::new_v4(), "metformin", Some("Glucophage"), "500mg", "twice daily", Some(dr_a), doc1),
            make_medication(Uuid::new_v4(), "metformin", Some("Fortamet"), "1000mg", "twice daily", Some(dr_b), doc2),
        ];

        let alerts = detect_conflicts(&doc2, &data, &ref_data);
        assert!(!alerts.is_empty(), "Alias-resolved conflict should be detected");
    }

    /// T-04: Glucophage and Metformin both active -> duplicate.
    #[test]
    fn duplicate_brand_vs_generic() {
        let ref_data = CoherenceReferenceData::load_test();
        let doc1 = Uuid::new_v4();
        let doc2 = Uuid::new_v4();

        let mut data = empty_snapshot();
        data.medications = vec![
            make_medication(Uuid::new_v4(), "metformin", Some("Glucophage"), "500mg", "twice daily", None, doc1),
            make_medication(Uuid::new_v4(), "metformin", Some("Metformin"), "500mg", "twice daily", None, doc2),
        ];

        let alerts = detect_duplicates(&doc2, &data, &ref_data);
        assert!(!alerts.is_empty(), "Duplicate should be detected for Glucophage vs Metformin");
    }

    /// T-05: Same brand name, same medication -> no duplicate.
    #[test]
    fn duplicate_same_name_no_alert() {
        let ref_data = CoherenceReferenceData::load_test();
        let doc1 = Uuid::new_v4();
        let doc2 = Uuid::new_v4();

        let mut data = empty_snapshot();
        data.medications = vec![
            make_medication(Uuid::new_v4(), "metformin", Some("Metformin"), "500mg", "twice daily", None, doc1),
            make_medication(Uuid::new_v4(), "metformin", Some("Metformin"), "500mg", "twice daily", None, doc2),
        ];

        let alerts = detect_duplicates(&doc2, &data, &ref_data);
        assert!(alerts.is_empty(), "Same display name should not be flagged as duplicate");
    }

    /// T-06: Diagnosis without linked medication -> gap alert.
    #[test]
    fn gap_diagnosis_without_treatment() {
        let doc = Uuid::new_v4();
        let mut data = empty_snapshot();
        data.diagnoses = vec![Diagnosis {
            id: Uuid::new_v4(),
            name: "Type 2 Diabetes".into(),
            icd_code: None,
            date_diagnosed: None,
            diagnosing_professional_id: None,
            status: DiagnosisStatus::Active,
            document_id: doc,
        }];

        let alerts = detect_gaps(&doc, &data);
        assert!(!alerts.is_empty());
        assert!(alerts.iter().any(|a| {
            if let AlertDetail::Gap(ref g) = a.detail {
                g.gap_type == GapType::DiagnosisWithoutTreatment
            } else {
                false
            }
        }));
    }

    /// T-07: Medication without documented diagnosis (non-OTC) -> gap alert.
    #[test]
    fn gap_medication_without_diagnosis() {
        let doc = Uuid::new_v4();
        let mut data = empty_snapshot();
        data.medications = vec![make_medication(
            Uuid::new_v4(), "Metformin", None, "500mg", "twice daily", None, doc,
        )];

        let alerts = detect_gaps(&doc, &data);
        assert!(!alerts.is_empty());
        assert!(alerts.iter().any(|a| {
            if let AlertDetail::Gap(ref g) = a.detail {
                g.gap_type == GapType::MedicationWithoutDiagnosis
            } else {
                false
            }
        }));
    }

    /// T-08: OTC medication without diagnosis -> no alert.
    #[test]
    fn gap_otc_no_alert() {
        let doc = Uuid::new_v4();
        let mut data = empty_snapshot();
        let mut med = make_medication(
            Uuid::new_v4(), "Ibuprofen", None, "200mg", "as needed", None, doc,
        );
        med.is_otc = true;
        data.medications = vec![med];

        let alerts = detect_gaps(&doc, &data);
        assert!(alerts.is_empty(), "OTC medication should not trigger gap alert");
    }

    /// T-09: Metformin dose changed without reason -> drift alert.
    #[test]
    fn drift_dose_changed_no_reason() {
        let ref_data = CoherenceReferenceData::load_test();
        let doc1 = Uuid::new_v4();
        let doc2 = Uuid::new_v4();

        let mut data = empty_snapshot();
        data.medications = vec![
            make_medication(Uuid::new_v4(), "Metformin", None, "500mg", "twice daily", None, doc1),
            make_medication(Uuid::new_v4(), "Metformin", None, "1000mg", "twice daily", None, doc2),
        ];

        let alerts = detect_drift(&doc2, &data, &ref_data);
        assert!(!alerts.is_empty(), "Dose change without reason should trigger drift");
    }

    /// T-10: Medication stopped with documented reason_stop -> no alert.
    #[test]
    fn drift_stopped_with_reason_no_alert() {
        let ref_data = CoherenceReferenceData::load_test();
        let doc1 = Uuid::new_v4();
        let doc2 = Uuid::new_v4();

        let mut data = empty_snapshot();
        let mut old_med = make_medication(
            Uuid::new_v4(), "Metformin", None, "500mg", "twice daily", None, doc1,
        );
        old_med.status = MedicationStatus::Active;

        let mut new_med = make_medication(
            Uuid::new_v4(), "Metformin", None, "500mg", "twice daily", None, doc2,
        );
        new_med.status = MedicationStatus::Stopped;
        new_med.reason_stop = Some("Switching to insulin".into());

        data.medications = vec![old_med, new_med];

        let alerts = detect_drift(&doc2, &data, &ref_data);
        let status_drifts: Vec<_> = alerts.iter().filter(|a| {
            if let AlertDetail::Drift(ref d) = a.detail {
                d.old_value == "active" && d.new_value == "stopped"
            } else {
                false
            }
        }).collect();
        assert!(status_drifts.is_empty(), "Stopped with reason should not trigger drift");
    }

    /// T-11: Symptom onset 3 days after starting new medication -> temporal alert.
    #[test]
    fn temporal_symptom_after_medication_start() {
        let doc = Uuid::new_v4();
        let med_id = Uuid::new_v4();
        let start = NaiveDate::from_ymd_opt(2026, 1, 10).unwrap();
        let onset = NaiveDate::from_ymd_opt(2026, 1, 13).unwrap();

        let mut data = empty_snapshot();
        let mut med = make_medication(med_id, "Lisinopril", None, "10mg", "once daily", None, doc);
        med.start_date = Some(start);
        data.medications = vec![med];
        data.symptoms = vec![Symptom {
            id: Uuid::new_v4(),
            category: "neurological".into(),
            specific: "headache".into(),
            severity: 5,
            body_region: None,
            duration: None,
            character: None,
            aggravating: None,
            relieving: None,
            timing_pattern: None,
            onset_date: onset,
            onset_time: None,
            recorded_date: onset,
            still_active: true,
            resolved_date: None,
            related_medication_id: None,
            related_diagnosis_id: None,
            source: SymptomSource::PatientReported,
            notes: None,
        }];

        let alerts = detect_temporal(&Uuid::nil(), &data);
        assert!(!alerts.is_empty(), "Expected temporal correlation alert");
        if let AlertDetail::Temporal(ref t) = alerts[0].detail {
            assert_eq!(t.days_between, 3);
        }
    }

    /// T-12: Symptom onset 15 days after medication change -> no alert (outside window).
    #[test]
    fn temporal_outside_window_no_alert() {
        let doc = Uuid::new_v4();
        let start = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let onset = NaiveDate::from_ymd_opt(2026, 1, 16).unwrap();

        let mut data = empty_snapshot();
        let mut med = make_medication(Uuid::new_v4(), "Lisinopril", None, "10mg", "once daily", None, doc);
        med.start_date = Some(start);
        data.medications = vec![med];
        data.symptoms = vec![Symptom {
            id: Uuid::new_v4(),
            category: "neurological".into(),
            specific: "headache".into(),
            severity: 5,
            body_region: None,
            duration: None,
            character: None,
            aggravating: None,
            relieving: None,
            timing_pattern: None,
            onset_date: onset,
            onset_time: None,
            recorded_date: onset,
            still_active: true,
            resolved_date: None,
            related_medication_id: None,
            related_diagnosis_id: None,
            source: SymptomSource::PatientReported,
            notes: None,
        }];

        let alerts = detect_temporal(&Uuid::nil(), &data);
        assert!(alerts.is_empty(), "15 days is outside the 14-day window");
    }

    /// T-18: Metformin 5000mg (outside typical range 500-2000mg) -> dose alert.
    #[test]
    fn dose_outside_range() {
        let ref_data = CoherenceReferenceData::load_test();
        let doc = Uuid::new_v4();

        let mut data = empty_snapshot();
        data.medications = vec![make_medication(
            Uuid::new_v4(), "Metformin", None, "5000mg", "twice daily", None, doc,
        )];

        let alerts = detect_dose_issues(&doc, &data, &ref_data);
        assert!(!alerts.is_empty(), "5000mg should be outside Metformin range");
    }

    /// T-19: Metformin 500mg (within range) -> no alert.
    #[test]
    fn dose_within_range_no_alert() {
        let ref_data = CoherenceReferenceData::load_test();
        let doc = Uuid::new_v4();

        let mut data = empty_snapshot();
        data.medications = vec![make_medication(
            Uuid::new_v4(), "Metformin", None, "500mg", "twice daily", None, doc,
        )];

        let alerts = detect_dose_issues(&doc, &data, &ref_data);
        assert!(alerts.is_empty(), "500mg is within Metformin range");
    }

    /// T-20: Unparseable dose string -> no alert.
    #[test]
    fn dose_unparseable_no_alert() {
        let ref_data = CoherenceReferenceData::load_test();
        let doc = Uuid::new_v4();

        let mut data = empty_snapshot();
        data.medications = vec![make_medication(
            Uuid::new_v4(), "Metformin", None, "as directed", "twice daily", None, doc,
        )];

        let alerts = detect_dose_issues(&doc, &data, &ref_data);
        assert!(alerts.is_empty(), "Unparseable dose should be gracefully skipped");
    }

    // ── Daily dose accumulation (RS-L2-03-002) ───────────────────

    /// T-20b: Metformin 1500mg twice daily = 3000mg > 2550mg max → alert.
    #[test]
    fn daily_dose_exceeds_max() {
        let ref_data = CoherenceReferenceData::load_test();
        let doc = Uuid::new_v4();

        let mut data = empty_snapshot();
        data.medications = vec![make_medication(
            Uuid::new_v4(), "Metformin", None, "1500mg", "twice daily", None, doc,
        )];

        let alerts = detect_daily_dose_accumulation(&doc, &data, &ref_data);
        assert!(!alerts.is_empty(), "3000mg/day exceeds Metformin max of 2550mg");
    }

    /// T-20c: Metformin 500mg twice daily = 1000mg < 2550mg max → no alert.
    #[test]
    fn daily_dose_within_max_no_alert() {
        let ref_data = CoherenceReferenceData::load_test();
        let doc = Uuid::new_v4();

        let mut data = empty_snapshot();
        data.medications = vec![make_medication(
            Uuid::new_v4(), "Metformin", None, "500mg", "twice daily", None, doc,
        )];

        let alerts = detect_daily_dose_accumulation(&doc, &data, &ref_data);
        assert!(alerts.is_empty(), "1000mg/day within Metformin max of 2550mg");
    }

    /// T-20d: "as needed" frequency → unparseable, skip accumulation.
    #[test]
    fn daily_dose_prn_skipped() {
        let ref_data = CoherenceReferenceData::load_test();
        let doc = Uuid::new_v4();

        let mut data = empty_snapshot();
        data.medications = vec![make_medication(
            Uuid::new_v4(), "Metformin", None, "2000mg", "as needed", None, doc,
        )];

        let alerts = detect_daily_dose_accumulation(&doc, &data, &ref_data);
        assert!(alerts.is_empty(), "PRN frequency should skip accumulation check");
    }

    /// T-20e: Lisinopril 50mg BID = 100mg > 80mg max → alert.
    #[test]
    fn daily_dose_lisinopril_exceeds() {
        let ref_data = CoherenceReferenceData::load_test();
        let doc = Uuid::new_v4();

        let mut data = empty_snapshot();
        data.medications = vec![make_medication(
            Uuid::new_v4(), "Lisinopril", None, "50mg", "BID", None, doc,
        )];

        let alerts = detect_daily_dose_accumulation(&doc, &data, &ref_data);
        assert!(!alerts.is_empty(), "100mg/day exceeds Lisinopril max of 80mg");
    }

    /// T-21: Lab result with abnormal_flag = critical_high -> CRITICAL alert.
    #[test]
    fn critical_lab_high() {
        let doc = Uuid::new_v4();
        let mut data = empty_snapshot();
        data.lab_results = vec![LabResult {
            id: Uuid::new_v4(),
            test_name: "Potassium".into(),
            test_code: None,
            value: Some(6.5),
            value_text: None,
            unit: Some("mEq/L".into()),
            reference_range_low: Some(3.5),
            reference_range_high: Some(5.0),
            abnormal_flag: AbnormalFlag::CriticalHigh,
            collection_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            lab_facility: None,
            ordering_physician_id: None,
            document_id: doc,
        }];

        let alerts = detect_critical_labs(&doc, &data);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].severity, AlertSeverity::Critical);
        assert_eq!(alerts[0].alert_type, AlertType::Critical);
    }

    /// T-22: Lab result with abnormal_flag = high (not critical) -> no CRITICAL alert.
    #[test]
    fn non_critical_lab_no_alert() {
        let doc = Uuid::new_v4();
        let mut data = empty_snapshot();
        data.lab_results = vec![LabResult {
            id: Uuid::new_v4(),
            test_name: "Cholesterol".into(),
            test_code: None,
            value: Some(240.0),
            value_text: None,
            unit: Some("mg/dL".into()),
            reference_range_low: None,
            reference_range_high: Some(200.0),
            abnormal_flag: AbnormalFlag::High,
            collection_date: NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
            lab_facility: None,
            ordering_physician_id: None,
            document_id: doc,
        }];

        let alerts = detect_critical_labs(&doc, &data);
        assert!(alerts.is_empty(), "Non-critical high should not trigger CRITICAL alert");
    }

    /// T-15: Penicillin allergy + amoxicillin prescribed -> CRITICAL alert.
    #[test]
    fn allergy_penicillin_amoxicillin() {
        let ref_data = CoherenceReferenceData::load_test();
        let doc = Uuid::new_v4();

        let mut data = empty_snapshot();
        data.allergies = vec![Allergy {
            id: Uuid::new_v4(),
            allergen: "penicillin".into(),
            reaction: Some("anaphylaxis".into()),
            severity: AllergySeverity::Severe,
            date_identified: None,
            source: AllergySource::DocumentExtracted,
            document_id: Some(Uuid::new_v4()),
            verified: true,
        }];
        data.medications = vec![make_medication(
            Uuid::new_v4(), "amoxicillin", None, "500mg", "three times daily", None, doc,
        )];

        let alerts = detect_allergy_conflicts(&doc, &data, &ref_data);
        assert!(!alerts.is_empty(), "Penicillin allergy should flag amoxicillin");
        assert!(alerts.iter().all(|a| a.severity == AlertSeverity::Critical));
    }

    /// T-16: Aspirin allergy + ibuprofen prescribed -> CRITICAL alert (NSAID family).
    #[test]
    fn allergy_aspirin_ibuprofen() {
        let ref_data = CoherenceReferenceData::load_test();
        let doc = Uuid::new_v4();

        let mut data = empty_snapshot();
        data.allergies = vec![Allergy {
            id: Uuid::new_v4(),
            allergen: "aspirin".into(),
            reaction: None,
            severity: AllergySeverity::Moderate,
            date_identified: None,
            source: AllergySource::PatientReported,
            document_id: None,
            verified: false,
        }];
        data.medications = vec![make_medication(
            Uuid::new_v4(), "ibuprofen", None, "400mg", "as needed", None, doc,
        )];

        let alerts = detect_allergy_conflicts(&doc, &data, &ref_data);
        assert!(!alerts.is_empty(), "Aspirin allergy should flag ibuprofen (NSAID family)");
    }

    /// Medication relates to diagnosis via condition field.
    #[test]
    fn medication_relates_via_condition() {
        let med = make_medication(
            Uuid::new_v4(), "Metformin", None, "500mg", "twice daily", None, Uuid::new_v4(),
        );
        let mut med_with_condition = med;
        med_with_condition.condition = Some("Type 2 Diabetes".into());

        let diag = Diagnosis {
            id: Uuid::new_v4(),
            name: "Type 2 Diabetes".into(),
            icd_code: None,
            date_diagnosed: None,
            diagnosing_professional_id: None,
            status: DiagnosisStatus::Active,
            document_id: Uuid::new_v4(),
        };

        assert!(super::super::helpers::medication_relates_to_diagnosis(
            &med_with_condition,
            &diag
        ));
    }

    /// Unrelated medication does not relate to diagnosis.
    #[test]
    fn medication_does_not_relate_unlinked() {
        let med = make_medication(
            Uuid::new_v4(), "Atorvastatin", None, "20mg", "once daily", None, Uuid::new_v4(),
        );

        let diag = Diagnosis {
            id: Uuid::new_v4(),
            name: "Asthma".into(),
            icd_code: None,
            date_diagnosed: None,
            diagnosing_professional_id: None,
            status: DiagnosisStatus::Active,
            document_id: Uuid::new_v4(),
        };

        assert!(!super::super::helpers::medication_relates_to_diagnosis(&med, &diag));
    }
}
