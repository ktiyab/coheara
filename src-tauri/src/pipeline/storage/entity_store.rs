use std::str::FromStr;

use chrono::NaiveDate;
use rusqlite::Connection;
use uuid::Uuid;

use super::StorageError;
use super::types::{EntitiesStoredCount, StorageWarning};
use crate::db::repository;
use crate::models::*;
use crate::models::enums::*;
use crate::pipeline::structuring::types::{ExtractedProfessional, StructuringResult};

/// Store extracted entities into SQLite, returning counts and warnings.
///
/// P.3: Idempotent — clears any existing entities for this document before
/// inserting fresh ones, making reprocessing safe.
pub fn store_entities(
    conn: &Connection,
    result: &StructuringResult,
) -> Result<(EntitiesStoredCount, Vec<StorageWarning>), StorageError> {
    // P.3: Clear existing entities for idempotent reprocessing
    repository::clear_document_entities(conn, &result.document_id)?;

    let mut counts = EntitiesStoredCount::default();
    let mut warnings = Vec::new();

    let professional_id = match &result.professional {
        Some(prof) => match resolve_professional(conn, prof) {
            Ok(id) => Some(id),
            Err(_) => {
                warnings.push(StorageWarning::ProfessionalNameAmbiguous {
                    name: prof.name.clone(),
                });
                None
            }
        },
        None => None,
    };

    counts.medications = store_medications(
        conn,
        &result.extracted_entities.medications,
        &result.document_id,
        professional_id,
        &mut warnings,
    )?;

    counts.lab_results = store_lab_results(
        conn,
        &result.extracted_entities.lab_results,
        &result.document_id,
        professional_id,
        &mut warnings,
    )?;

    counts.diagnoses = store_diagnoses(
        conn,
        &result.extracted_entities.diagnoses,
        &result.document_id,
        professional_id,
    )?;

    counts.allergies = store_allergies(
        conn,
        &result.extracted_entities.allergies,
        &result.document_id,
    )?;

    counts.procedures = store_procedures(
        conn,
        &result.extracted_entities.procedures,
        &result.document_id,
        professional_id,
    )?;

    counts.referrals = store_referrals(
        conn,
        &result.extracted_entities.referrals,
        &result.document_id,
        professional_id,
    )?;

    counts.instructions = store_instructions(
        conn,
        &result.extracted_entities.instructions,
        &result.document_id,
    )?;

    Ok((counts, warnings))
}

fn resolve_professional(
    conn: &Connection,
    prof: &ExtractedProfessional,
) -> Result<Uuid, StorageError> {
    let found = repository::find_or_create_professional(
        conn,
        &prof.name,
        prof.specialty.as_deref(),
    )?;
    Ok(found.id)
}

fn store_medications(
    conn: &Connection,
    meds: &[crate::pipeline::structuring::types::ExtractedMedication],
    document_id: &Uuid,
    professional_id: Option<Uuid>,
    warnings: &mut Vec<StorageWarning>,
) -> Result<usize, StorageError> {
    let mut count = 0;

    for extracted in meds {
        let generic_name = extracted
            .generic_name
            .clone()
            .unwrap_or_else(|| "Unknown".to_string());

        let freq_type = FrequencyType::from_str(&extracted.frequency_type)
            .unwrap_or(FrequencyType::Scheduled);

        let med_id = Uuid::new_v4();
        let med = Medication {
            id: med_id,
            generic_name,
            brand_name: extracted.brand_name.clone(),
            dose: extracted.dose.clone(),
            frequency: extracted.frequency.clone(),
            frequency_type: freq_type,
            route: extracted.route.clone(),
            prescriber_id: professional_id,
            start_date: None,
            end_date: None,
            reason_start: extracted.reason.clone(),
            reason_stop: None,
            is_otc: false,
            status: MedicationStatus::Active,
            administration_instructions: None,
            max_daily_dose: extracted.max_daily_dose.clone(),
            condition: extracted.condition.clone(),
            dose_type: DoseType::Fixed,
            is_compound: extracted.is_compound,
            document_id: *document_id,
        };

        if let Err(e) = repository::insert_medication(conn, &med) {
            warnings.push(StorageWarning::DuplicateMedication {
                name: med.generic_name.clone(),
                existing_id: med_id,
            });
            tracing::warn!(medication_id = %med_id, "Failed to insert medication: {e}");
            continue;
        }

        // Store compound ingredients
        for ingredient in &extracted.compound_ingredients {
            let ci = CompoundIngredient {
                id: Uuid::new_v4(),
                medication_id: med_id,
                ingredient_name: ingredient.name.clone(),
                ingredient_dose: ingredient.dose.clone(),
                maps_to_generic: None,
            };
            if let Err(e) = repository::insert_compound_ingredient(conn, &ci) {
                tracing::warn!(
                    medication_id = %med_id,
                    ingredient = %ci.ingredient_name,
                    error = %e,
                    "Failed to insert compound ingredient"
                );
            }
        }

        // Store tapering steps
        for step in &extracted.tapering_steps {
            let ts = TaperingStep {
                id: Uuid::new_v4(),
                medication_id: med_id,
                step_number: step.step_number as i32,
                dose: step.dose.clone(),
                duration_days: step.duration_days.unwrap_or(0) as i32,
                start_date: None,
                document_id: Some(*document_id),
            };
            if let Err(e) = repository::insert_tapering_step(conn, &ts) {
                tracing::warn!(
                    medication_id = %med_id,
                    step = ts.step_number,
                    error = %e,
                    "Failed to insert tapering step"
                );
            }
        }

        // Store instructions
        for instruction_text in &extracted.instructions {
            let instr = MedicationInstruction {
                id: Uuid::new_v4(),
                medication_id: med_id,
                instruction: instruction_text.clone(),
                timing: None,
                source_document_id: Some(*document_id),
            };
            if let Err(e) = repository::insert_medication_instruction(conn, &instr) {
                tracing::warn!(
                    medication_id = %med_id,
                    error = %e,
                    "Failed to insert medication instruction"
                );
            }
        }

        count += 1;
    }

    Ok(count)
}

fn store_lab_results(
    conn: &Connection,
    labs: &[crate::pipeline::structuring::types::ExtractedLabResult],
    document_id: &Uuid,
    professional_id: Option<Uuid>,
    warnings: &mut Vec<StorageWarning>,
) -> Result<usize, StorageError> {
    let mut count = 0;

    for extracted in labs {
        let collection_date = extracted
            .collection_date
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
            .unwrap_or_else(|| chrono::Local::now().date_naive());

        let abnormal_flag = extracted
            .abnormal_flag
            .as_deref()
            .and_then(|f| AbnormalFlag::from_str(f).ok())
            .unwrap_or(AbnormalFlag::Normal);

        if let Some(date_str) = &extracted.collection_date {
            if NaiveDate::parse_from_str(date_str, "%Y-%m-%d").is_err() {
                warnings.push(StorageWarning::DateParsingFailed {
                    field: "collection_date".into(),
                    value: date_str.clone(),
                });
            }
        }

        let lab = LabResult {
            id: Uuid::new_v4(),
            test_name: extracted.test_name.clone(),
            test_code: extracted.test_code.clone(),
            value: extracted.value,
            value_text: extracted.value_text.clone(),
            unit: extracted.unit.clone(),
            reference_range_low: extracted.reference_range_low,
            reference_range_high: extracted.reference_range_high,
            abnormal_flag,
            collection_date,
            lab_facility: None,
            ordering_physician_id: professional_id,
            document_id: *document_id,
        };

        repository::insert_lab_result(conn, &lab)?;
        count += 1;
    }

    Ok(count)
}

fn store_diagnoses(
    conn: &Connection,
    diagnoses: &[crate::pipeline::structuring::types::ExtractedDiagnosis],
    document_id: &Uuid,
    professional_id: Option<Uuid>,
) -> Result<usize, StorageError> {
    let mut count = 0;

    for extracted in diagnoses {
        let date_diagnosed = extracted
            .date
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

        let status = DiagnosisStatus::from_str(&extracted.status)
            .unwrap_or(DiagnosisStatus::Active);

        let diag = Diagnosis {
            id: Uuid::new_v4(),
            name: extracted.name.clone(),
            icd_code: extracted.icd_code.clone(),
            date_diagnosed,
            diagnosing_professional_id: professional_id,
            status,
            document_id: *document_id,
        };

        repository::insert_diagnosis(conn, &diag)?;
        count += 1;
    }

    Ok(count)
}

fn store_allergies(
    conn: &Connection,
    allergies: &[crate::pipeline::structuring::types::ExtractedAllergy],
    document_id: &Uuid,
) -> Result<usize, StorageError> {
    let mut count = 0;

    for extracted in allergies {
        let severity = extracted
            .severity
            .as_deref()
            .and_then(|s| AllergySeverity::from_str(s).ok())
            .unwrap_or(AllergySeverity::Moderate);

        let allergy = Allergy {
            id: Uuid::new_v4(),
            allergen: extracted.allergen.clone(),
            reaction: extracted.reaction.clone(),
            severity,
            date_identified: None,
            source: AllergySource::DocumentExtracted,
            document_id: Some(*document_id),
            verified: false,
        };

        repository::insert_allergy(conn, &allergy)?;
        count += 1;
    }

    Ok(count)
}

fn store_procedures(
    conn: &Connection,
    procedures: &[crate::pipeline::structuring::types::ExtractedProcedure],
    document_id: &Uuid,
    professional_id: Option<Uuid>,
) -> Result<usize, StorageError> {
    let mut count = 0;

    for extracted in procedures {
        let date = extracted
            .date
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

        let follow_up_date = extracted
            .follow_up_date
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

        let proc = Procedure {
            id: Uuid::new_v4(),
            name: extracted.name.clone(),
            date,
            performing_professional_id: professional_id,
            facility: None,
            outcome: extracted.outcome.clone(),
            follow_up_required: extracted.follow_up_required,
            follow_up_date,
            document_id: *document_id,
        };

        repository::insert_procedure(conn, &proc)?;
        count += 1;
    }

    Ok(count)
}

fn store_referrals(
    conn: &Connection,
    referrals: &[crate::pipeline::structuring::types::ExtractedReferral],
    document_id: &Uuid,
    referring_professional_id: Option<Uuid>,
) -> Result<usize, StorageError> {
    let mut count = 0;

    for extracted in referrals {
        let referred_to = repository::find_or_create_professional(
            conn,
            &extracted.referred_to,
            extracted.specialty.as_deref(),
        )?;

        let referring_id = referring_professional_id
            .unwrap_or_else(Uuid::new_v4);

        let referral = Referral {
            id: Uuid::new_v4(),
            referring_professional_id: referring_id,
            referred_to_professional_id: referred_to.id,
            reason: extracted.reason.clone(),
            date: chrono::Local::now().date_naive(),
            status: ReferralStatus::Pending,
            document_id: Some(*document_id),
        };

        repository::insert_referral(conn, &referral)?;
        count += 1;
    }

    Ok(count)
}

fn store_instructions(
    _conn: &Connection,
    instructions: &[crate::pipeline::structuring::types::ExtractedInstruction],
    _document_id: &Uuid,
) -> Result<usize, StorageError> {
    // General instructions (non-medication-specific) are stored
    // in the structured Markdown. Count them for reporting.
    Ok(instructions.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;
    use crate::pipeline::structuring::types::*;

    fn test_db() -> Connection {
        open_memory_database().unwrap()
    }

    fn make_document(conn: &Connection) -> Uuid {
        let id = Uuid::new_v4();
        repository::insert_document(
            conn,
            &Document {
                id,
                doc_type: DocumentType::Prescription,
                title: "Test".into(),
                document_date: None,
                ingestion_date: chrono::Local::now().naive_local(),
                professional_id: None,
                source_file: "/test.jpg".into(),
                markdown_file: None,
                ocr_confidence: Some(0.9),
                verified: false,
                source_deleted: false,
                perceptual_hash: None,
                notes: None,
                pipeline_status: PipelineStatus::Imported,
            },
        )
        .unwrap();
        id
    }

    fn minimal_structuring_result(document_id: Uuid) -> StructuringResult {
        StructuringResult {
            document_id,
            document_type: DocumentType::Prescription,
            document_date: None,
            professional: Some(ExtractedProfessional {
                name: "Dr. Test".into(),
                specialty: Some("GP".into()),
                institution: None,
            }),
            structured_markdown: "# Test".into(),
            extracted_entities: ExtractedEntities::default(),
            structuring_confidence: 0.85,
            markdown_file_path: None,
            validation_warnings: vec![],
            raw_llm_response: None,
        }
    }

    #[test]
    fn store_empty_entities() {
        let conn = test_db();
        let doc_id = make_document(&conn);
        let result = minimal_structuring_result(doc_id);

        let (counts, warnings) = store_entities(&conn, &result).unwrap();
        assert_eq!(counts.medications, 0);
        assert_eq!(counts.lab_results, 0);
        assert_eq!(counts.diagnoses, 0);
        assert_eq!(counts.allergies, 0);
        assert_eq!(counts.procedures, 0);
        assert_eq!(counts.referrals, 0);
        assert!(warnings.is_empty());
    }

    #[test]
    fn store_medication_entities() {
        let conn = test_db();
        let doc_id = make_document(&conn);
        let mut result = minimal_structuring_result(doc_id);

        result.extracted_entities.medications.push(ExtractedMedication {
            generic_name: Some("Metformin".into()),
            brand_name: Some("Glucophage".into()),
            dose: "500mg".into(),
            frequency: "twice daily".into(),
            frequency_type: "scheduled".into(),
            route: "oral".into(),
            reason: Some("Type 2 diabetes".into()),
            instructions: vec!["Take with food".into()],
            is_compound: false,
            compound_ingredients: vec![],
            tapering_steps: vec![],
            max_daily_dose: Some("2000mg".into()),
            condition: None,
            confidence: 0.9,
        });

        let (counts, _warnings) = store_entities(&conn, &result).unwrap();
        assert_eq!(counts.medications, 1);
    }

    #[test]
    fn store_lab_result_entities() {
        let conn = test_db();
        let doc_id = make_document(&conn);
        let mut result = minimal_structuring_result(doc_id);

        result.extracted_entities.lab_results.push(ExtractedLabResult {
            test_name: "HbA1c".into(),
            test_code: None,
            value: Some(7.2),
            value_text: None,
            unit: Some("%".into()),
            reference_range_low: Some(4.0),
            reference_range_high: Some(5.6),
            reference_range_text: None,
            abnormal_flag: Some("high".into()),
            collection_date: Some("2024-01-15".into()),
            confidence: 0.95,
        });

        let (counts, _warnings) = store_entities(&conn, &result).unwrap();
        assert_eq!(counts.lab_results, 1);
    }

    #[test]
    fn store_diagnosis_entities() {
        let conn = test_db();
        let doc_id = make_document(&conn);
        let mut result = minimal_structuring_result(doc_id);

        result.extracted_entities.diagnoses.push(ExtractedDiagnosis {
            name: "Type 2 Diabetes".into(),
            icd_code: Some("E11".into()),
            date: Some("2024-01-15".into()),
            status: "active".into(),
            confidence: 0.9,
        });

        let (counts, _warnings) = store_entities(&conn, &result).unwrap();
        assert_eq!(counts.diagnoses, 1);
    }

    #[test]
    fn store_allergy_entities() {
        let conn = test_db();
        let doc_id = make_document(&conn);
        let mut result = minimal_structuring_result(doc_id);

        result.extracted_entities.allergies.push(ExtractedAllergy {
            allergen: "Penicillin".into(),
            reaction: Some("Rash".into()),
            severity: Some("severe".into()),
            confidence: 0.85,
        });

        let (counts, _warnings) = store_entities(&conn, &result).unwrap();
        assert_eq!(counts.allergies, 1);
    }

    #[test]
    fn store_procedure_entities() {
        let conn = test_db();
        let doc_id = make_document(&conn);
        let mut result = minimal_structuring_result(doc_id);

        result.extracted_entities.procedures.push(ExtractedProcedure {
            name: "Blood pressure check".into(),
            date: Some("2024-02-01".into()),
            outcome: Some("Normal".into()),
            follow_up_required: false,
            follow_up_date: None,
            confidence: 0.9,
        });

        let (counts, _warnings) = store_entities(&conn, &result).unwrap();
        assert_eq!(counts.procedures, 1);
    }

    #[test]
    fn store_referral_entities() {
        let conn = test_db();
        let doc_id = make_document(&conn);
        let mut result = minimal_structuring_result(doc_id);

        result.extracted_entities.referrals.push(ExtractedReferral {
            referred_to: "Dr. Cardio".into(),
            specialty: Some("Cardiology".into()),
            reason: Some("Chest pain evaluation".into()),
            confidence: 0.8,
        });

        let (counts, _warnings) = store_entities(&conn, &result).unwrap();
        assert_eq!(counts.referrals, 1);
    }

    #[test]
    fn professional_created_once_for_document() {
        let conn = test_db();
        let doc_id = make_document(&conn);
        let mut result = minimal_structuring_result(doc_id);

        result.extracted_entities.medications.push(ExtractedMedication {
            generic_name: Some("DrugA".into()),
            brand_name: None,
            dose: "10mg".into(),
            frequency: "daily".into(),
            frequency_type: "scheduled".into(),
            route: "oral".into(),
            reason: None,
            instructions: vec![],
            is_compound: false,
            compound_ingredients: vec![],
            tapering_steps: vec![],
            max_daily_dose: None,
            condition: None,
            confidence: 0.9,
        });
        result.extracted_entities.medications.push(ExtractedMedication {
            generic_name: Some("DrugB".into()),
            brand_name: None,
            dose: "20mg".into(),
            frequency: "daily".into(),
            frequency_type: "scheduled".into(),
            route: "oral".into(),
            reason: None,
            instructions: vec![],
            is_compound: false,
            compound_ingredients: vec![],
            tapering_steps: vec![],
            max_daily_dose: None,
            condition: None,
            confidence: 0.9,
        });

        let (counts, _) = store_entities(&conn, &result).unwrap();
        assert_eq!(counts.medications, 2);

        // Professional should only be created once
        let prof_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM professionals WHERE name = 'Dr. Test'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(prof_count, 1);
    }

    #[test]
    fn no_professional_still_works() {
        let conn = test_db();
        let doc_id = make_document(&conn);
        let mut result = minimal_structuring_result(doc_id);
        result.professional = None;

        result.extracted_entities.diagnoses.push(ExtractedDiagnosis {
            name: "Hypertension".into(),
            icd_code: None,
            date: None,
            status: "active".into(),
            confidence: 0.8,
        });

        let (counts, warnings) = store_entities(&conn, &result).unwrap();
        assert_eq!(counts.diagnoses, 1);
        assert!(warnings.is_empty());
    }

    #[test]
    fn compound_medication_stores_ingredients() {
        let conn = test_db();
        let doc_id = make_document(&conn);
        let mut result = minimal_structuring_result(doc_id);

        result.extracted_entities.medications.push(ExtractedMedication {
            generic_name: Some("Custom Compound".into()),
            brand_name: None,
            dose: "1 capsule".into(),
            frequency: "daily".into(),
            frequency_type: "scheduled".into(),
            route: "oral".into(),
            reason: None,
            instructions: vec![],
            is_compound: true,
            compound_ingredients: vec![
                ExtractedCompoundIngredient {
                    name: "IngredientA".into(),
                    dose: Some("5mg".into()),
                },
                ExtractedCompoundIngredient {
                    name: "IngredientB".into(),
                    dose: Some("10mg".into()),
                },
            ],
            tapering_steps: vec![],
            max_daily_dose: None,
            condition: None,
            confidence: 0.85,
        });

        let (counts, _) = store_entities(&conn, &result).unwrap();
        assert_eq!(counts.medications, 1);

        let ingredient_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM compound_ingredients", [], |r| r.get(0))
            .unwrap();
        assert_eq!(ingredient_count, 2);
    }
}
