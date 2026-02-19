//! Repository layer — entity-scoped database operations.
//!
//! Split from a single 3,178-line file into domain sub-modules (SE-005).
//! All public functions are re-exported here for backwards compatibility.

mod alert;
mod allergy;
mod audit;
mod cached_explanation;
mod consistency;
mod conversation;
mod diagnosis;
mod document;
mod document_search;
mod lab_result;
mod medication;
mod preference;
mod procedure;
mod professional;
mod profile_trust;
mod referral;
mod symptom;
mod vital_sign;

use uuid::Uuid;

use super::DatabaseError;

/// Base repository operations for any entity
pub trait Repository<T, F> {
    fn insert(&self, entity: &T) -> Result<Uuid, DatabaseError>;
    fn get(&self, id: &Uuid) -> Result<Option<T>, DatabaseError>;
    fn delete(&self, id: &Uuid) -> Result<(), DatabaseError>;
    fn list(&self, filter: &F) -> Result<Vec<T>, DatabaseError>;
}

// Re-export all public items from sub-modules
pub use alert::*;
pub use allergy::*;
pub use audit::*;
pub use cached_explanation::*;
pub use consistency::*;
pub use conversation::*;
pub use diagnosis::*;
pub use document::*;
pub use document_search::*;
pub use lab_result::*;
pub use medication::*;
pub use preference::*;
pub use procedure::*;
pub use professional::*;
pub use profile_trust::*;
pub use referral::*;
pub use symptom::*;
pub use vital_sign::*;

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use super::*;
    use chrono::{NaiveDate, NaiveDateTime};
    use crate::db::sqlite::open_memory_database;
    use crate::models::*;
    use crate::models::enums::*;
    use rusqlite::{params, Connection};

    fn test_db() -> Connection {
        open_memory_database().unwrap()
    }

    fn make_document(conn: &Connection, prof_id: Option<Uuid>) -> Uuid {
        let id = Uuid::new_v4();
        insert_document(conn, &Document {
            id,
            doc_type: DocumentType::Prescription,
            title: "Test Prescription".into(),
            document_date: Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()),
            ingestion_date: NaiveDateTime::parse_from_str("2024-01-15 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap(),
            professional_id: prof_id,
            source_file: "/test/file.jpg".into(),
            markdown_file: None,
            ocr_confidence: Some(0.95),
            verified: false,
            source_deleted: false,
            perceptual_hash: Some("abc123hash".into()),
            notes: None,
            pipeline_status: PipelineStatus::Imported,
        }).unwrap();
        id
    }

    #[test]
    fn document_insert_and_retrieve() {
        let conn = test_db();
        let doc_id = make_document(&conn, None);
        let doc = get_document(&conn, &doc_id).unwrap().unwrap();
        assert_eq!(doc.title, "Test Prescription");
        assert_eq!(doc.doc_type, DocumentType::Prescription);
        assert!(!doc.verified);
    }

    #[test]
    fn document_duplicate_detection_by_hash() {
        let conn = test_db();
        make_document(&conn, None);
        let found = get_document_by_hash(&conn, "abc123hash").unwrap();
        assert!(found.is_some());
        let not_found = get_document_by_hash(&conn, "nonexistent").unwrap();
        assert!(not_found.is_none());
    }

    #[test]
    fn medication_insert_and_active_filter() {
        let conn = test_db();
        let doc_id = make_document(&conn, None);

        insert_medication(&conn, &Medication {
            id: Uuid::new_v4(),
            generic_name: "Metformin".into(),
            brand_name: Some("Glucophage".into()),
            dose: "500mg".into(),
            frequency: "twice daily".into(),
            frequency_type: FrequencyType::Scheduled,
            route: "oral".into(),
            prescriber_id: None,
            start_date: Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
            end_date: None,
            reason_start: Some("Type 2 diabetes".into()),
            reason_stop: None,
            is_otc: false,
            status: MedicationStatus::Active,
            administration_instructions: None,
            max_daily_dose: Some("2000mg".into()),
            condition: None,
            dose_type: DoseType::Fixed,
            is_compound: false,
            document_id: doc_id,
        }).unwrap();

        insert_medication(&conn, &Medication {
            id: Uuid::new_v4(),
            generic_name: "Amoxicillin".into(),
            brand_name: None,
            dose: "500mg".into(),
            frequency: "3x daily".into(),
            frequency_type: FrequencyType::Scheduled,
            route: "oral".into(),
            prescriber_id: None,
            start_date: None,
            end_date: None,
            reason_start: None,
            reason_stop: Some("Course completed".into()),
            is_otc: false,
            status: MedicationStatus::Stopped,
            administration_instructions: None,
            max_daily_dose: None,
            condition: None,
            dose_type: DoseType::Fixed,
            is_compound: false,
            document_id: doc_id,
        }).unwrap();

        let active = get_active_medications(&conn).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].generic_name, "Metformin");
    }

    #[test]
    fn lab_result_insert_and_critical_query() {
        let conn = test_db();
        let doc_id = make_document(&conn, None);

        insert_lab_result(&conn, &LabResult {
            id: Uuid::new_v4(),
            test_name: "Potassium".into(),
            test_code: None,
            value: Some(6.5),
            value_text: None,
            unit: Some("mmol/L".into()),
            reference_range_low: Some(3.5),
            reference_range_high: Some(5.0),
            abnormal_flag: AbnormalFlag::CriticalHigh,
            collection_date: NaiveDate::from_ymd_opt(2024, 3, 1).unwrap(),
            lab_facility: None,
            ordering_physician_id: None,
            document_id: doc_id,
        }).unwrap();

        insert_lab_result(&conn, &LabResult {
            id: Uuid::new_v4(),
            test_name: "Glucose".into(),
            test_code: None,
            value: Some(95.0),
            value_text: None,
            unit: Some("mg/dL".into()),
            reference_range_low: Some(70.0),
            reference_range_high: Some(100.0),
            abnormal_flag: AbnormalFlag::Normal,
            collection_date: NaiveDate::from_ymd_opt(2024, 3, 1).unwrap(),
            lab_facility: None,
            ordering_physician_id: None,
            document_id: doc_id,
        }).unwrap();

        let critical = get_critical_labs(&conn).unwrap();
        assert_eq!(critical.len(), 1);
        assert_eq!(critical[0].test_name, "Potassium");
    }

    #[test]
    fn allergy_insert() {
        let conn = test_db();
        insert_allergy(&conn, &Allergy {
            id: Uuid::new_v4(),
            allergen: "Penicillin".into(),
            reaction: Some("Rash".into()),
            severity: AllergySeverity::Severe,
            date_identified: None,
            source: AllergySource::PatientReported,
            document_id: None,
            verified: true,
        }).unwrap();

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM allergies", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn symptom_insert_all_oldcarts_fields() {
        let conn = test_db();
        insert_symptom(&conn, &Symptom {
            id: Uuid::new_v4(),
            category: "pain".into(),
            specific: "headache".into(),
            severity: 3,
            body_region: Some("head".into()),
            duration: Some("2 hours".into()),
            character: Some("throbbing".into()),
            aggravating: Some("bright light".into()),
            relieving: Some("rest".into()),
            timing_pattern: Some("afternoon".into()),
            onset_date: NaiveDate::from_ymd_opt(2024, 3, 10).unwrap(),
            onset_time: Some("14:00".into()),
            recorded_date: NaiveDate::from_ymd_opt(2024, 3, 10).unwrap(),
            still_active: true,
            resolved_date: None,
            related_medication_id: None,
            related_diagnosis_id: None,
            source: SymptomSource::PatientReported,
            notes: Some("Occurs after screen time".into()),
        }).unwrap();

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM symptoms", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn foreign_key_constraint_enforced() {
        let conn = test_db();
        let result = insert_medication(&conn, &Medication {
            id: Uuid::new_v4(),
            generic_name: "Orphan".into(),
            brand_name: None,
            dose: "10mg".into(),
            frequency: "daily".into(),
            frequency_type: FrequencyType::Scheduled,
            route: "oral".into(),
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
            document_id: Uuid::new_v4(), // Non-existent document
        });
        assert!(result.is_err());
    }

    #[test]
    fn cascade_delete_removes_compounds() {
        let conn = test_db();
        let doc_id = make_document(&conn, None);
        let med_id = Uuid::new_v4();

        insert_medication(&conn, &Medication {
            id: med_id,
            generic_name: "Compound Med".into(),
            brand_name: None,
            dose: "10mg".into(),
            frequency: "daily".into(),
            frequency_type: FrequencyType::Scheduled,
            route: "oral".into(),
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
            is_compound: true,
            document_id: doc_id,
        }).unwrap();

        insert_compound_ingredient(&conn, &CompoundIngredient {
            id: Uuid::new_v4(),
            medication_id: med_id,
            ingredient_name: "Ingredient A".into(),
            ingredient_dose: Some("5mg".into()),
            maps_to_generic: None,
        }).unwrap();

        let ingredients = get_compound_ingredients(&conn, &med_id).unwrap();
        assert_eq!(ingredients.len(), 1);

        delete_medication_cascade(&conn, &med_id).unwrap();

        let ingredients_after = get_compound_ingredients(&conn, &med_id).unwrap();
        assert_eq!(ingredients_after.len(), 0);
    }

    #[test]
    fn profile_trust_update_and_retrieve() {
        let conn = test_db();
        let trust = get_profile_trust(&conn).unwrap();
        assert_eq!(trust.total_documents, 0);

        update_profile_trust_verified(&conn).unwrap();
        let trust = get_profile_trust(&conn).unwrap();
        assert_eq!(trust.total_documents, 1);
        assert_eq!(trust.documents_verified, 1);
    }

    #[test]
    fn diagnosis_insert() {
        let conn = test_db();
        let doc_id = make_document(&conn, None);

        insert_diagnosis(&conn, &Diagnosis {
            id: Uuid::new_v4(),
            name: "Type 2 Diabetes".into(),
            icd_code: Some("E11".into()),
            date_diagnosed: Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()),
            diagnosing_professional_id: None,
            status: DiagnosisStatus::Active,
            document_id: doc_id,
        }).unwrap();

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM diagnoses", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn procedure_insert() {
        let conn = test_db();
        let doc_id = make_document(&conn, None);

        insert_procedure(&conn, &Procedure {
            id: Uuid::new_v4(),
            name: "Blood pressure measurement".into(),
            date: Some(NaiveDate::from_ymd_opt(2024, 2, 1).unwrap()),
            performing_professional_id: None,
            facility: Some("City Hospital".into()),
            outcome: Some("Normal".into()),
            follow_up_required: false,
            follow_up_date: None,
            document_id: doc_id,
        }).unwrap();

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM procedures", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn referral_insert() {
        let conn = test_db();
        let doc_id = make_document(&conn, None);
        let referring = Uuid::new_v4();
        let referred_to = Uuid::new_v4();

        insert_professional(&conn, &Professional {
            id: referring,
            name: "Dr. Smith".into(),
            specialty: Some("GP".into()),
            institution: None,
            first_seen_date: None,
            last_seen_date: None,
        }).unwrap();

        insert_professional(&conn, &Professional {
            id: referred_to,
            name: "Dr. Jones".into(),
            specialty: Some("Cardiology".into()),
            institution: None,
            first_seen_date: None,
            last_seen_date: None,
        }).unwrap();

        insert_referral(&conn, &Referral {
            id: Uuid::new_v4(),
            referring_professional_id: referring,
            referred_to_professional_id: referred_to,
            reason: Some("Chest pain evaluation".into()),
            date: NaiveDate::from_ymd_opt(2024, 3, 1).unwrap(),
            status: ReferralStatus::Pending,
            document_id: Some(doc_id),
        }).unwrap();

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM referrals", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn tapering_step_insert() {
        let conn = test_db();
        let doc_id = make_document(&conn, None);
        let med_id = Uuid::new_v4();

        insert_medication(&conn, &Medication {
            id: med_id,
            generic_name: "Prednisone".into(),
            brand_name: None,
            dose: "40mg".into(),
            frequency: "daily".into(),
            frequency_type: FrequencyType::Tapering,
            route: "oral".into(),
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
            document_id: doc_id,
        }).unwrap();

        insert_tapering_step(&conn, &TaperingStep {
            id: Uuid::new_v4(),
            medication_id: med_id,
            step_number: 1,
            dose: "40mg".into(),
            duration_days: 7,
            start_date: None,
            document_id: Some(doc_id),
        }).unwrap();

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM tapering_schedules", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn medication_instruction_insert() {
        let conn = test_db();
        let doc_id = make_document(&conn, None);
        let med_id = Uuid::new_v4();

        insert_medication(&conn, &Medication {
            id: med_id,
            generic_name: "Metformin".into(),
            brand_name: None,
            dose: "500mg".into(),
            frequency: "daily".into(),
            frequency_type: FrequencyType::Scheduled,
            route: "oral".into(),
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
            document_id: doc_id,
        }).unwrap();

        insert_medication_instruction(&conn, &MedicationInstruction {
            id: Uuid::new_v4(),
            medication_id: med_id,
            instruction: "Take with food".into(),
            timing: Some("with meals".into()),
            source_document_id: Some(doc_id),
        }).unwrap();

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM medication_instructions", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn find_or_create_professional_creates_new() {
        let conn = test_db();
        let prof = find_or_create_professional(&conn, "Dr. New", Some("GP")).unwrap();
        assert_eq!(prof.name, "Dr. New");
        assert_eq!(prof.specialty.as_deref(), Some("GP"));
    }

    #[test]
    fn find_or_create_professional_finds_existing() {
        let conn = test_db();
        let prof1 = find_or_create_professional(&conn, "Dr. Existing", Some("GP")).unwrap();
        let prof2 = find_or_create_professional(&conn, "Dr. Existing", Some("GP")).unwrap();
        assert_eq!(prof1.id, prof2.id);
    }

    #[test]
    fn document_update() {
        let conn = test_db();
        let doc_id = make_document(&conn, None);

        let mut doc = get_document(&conn, &doc_id).unwrap().unwrap();
        assert_eq!(doc.title, "Test Prescription");

        doc.title = "Updated Title".into();
        doc.verified = true;
        update_document(&conn, &doc).unwrap();

        let updated = get_document(&conn, &doc_id).unwrap().unwrap();
        assert_eq!(updated.title, "Updated Title");
        assert!(updated.verified);
    }

    #[test]
    fn conversation_insert_and_retrieve() {
        let conn = test_db();
        let conv_id = Uuid::new_v4();

        insert_conversation(
            &conn,
            &Conversation {
                id: conv_id,
                started_at: NaiveDateTime::parse_from_str("2024-03-01 10:00:00", "%Y-%m-%d %H:%M:%S")
                    .unwrap(),
                title: Some("Test conversation".into()),
            },
        )
        .unwrap();

        let conv = get_conversation(&conn, &conv_id).unwrap().unwrap();
        assert_eq!(conv.title.as_deref(), Some("Test conversation"));
    }

    #[test]
    fn message_insert_and_retrieve_by_conversation() {
        let conn = test_db();
        let conv_id = Uuid::new_v4();

        insert_conversation(
            &conn,
            &Conversation {
                id: conv_id,
                started_at: chrono::Local::now().naive_local(),
                title: None,
            },
        )
        .unwrap();

        insert_message(
            &conn,
            &Message {
                id: Uuid::new_v4(),
                conversation_id: conv_id,
                role: MessageRole::Patient,
                content: "What dose of metformin am I on?".into(),
                timestamp: chrono::Local::now().naive_local(),
                source_chunks: None,
                confidence: None,
                feedback: None,
            },
        )
        .unwrap();

        insert_message(
            &conn,
            &Message {
                id: Uuid::new_v4(),
                conversation_id: conv_id,
                role: MessageRole::Coheara,
                content: "Your documents show Metformin 500mg twice daily.".into(),
                timestamp: chrono::Local::now().naive_local(),
                source_chunks: Some("[\"doc-1\"]".into()),
                confidence: Some(0.92),
                feedback: None,
            },
        )
        .unwrap();

        let messages = get_messages_by_conversation(&conn, &conv_id).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, MessageRole::Patient);
        assert_eq!(messages[1].role, MessageRole::Coheara);
        assert_eq!(messages[1].confidence, Some(0.92));
    }

    #[test]
    fn get_medications_by_name_finds_match() {
        let conn = test_db();
        let doc_id = make_document(&conn, None);

        insert_medication(
            &conn,
            &Medication {
                id: Uuid::new_v4(),
                generic_name: "Metformin".into(),
                brand_name: Some("Glucophage".into()),
                dose: "500mg".into(),
                frequency: "twice daily".into(),
                frequency_type: FrequencyType::Scheduled,
                route: "oral".into(),
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
                document_id: doc_id,
            },
        )
        .unwrap();

        let found = get_medications_by_name(&conn, "metformin").unwrap();
        assert_eq!(found.len(), 1);

        let by_brand = get_medications_by_name(&conn, "glucophage").unwrap();
        assert_eq!(by_brand.len(), 1);

        let not_found = get_medications_by_name(&conn, "aspirin").unwrap();
        assert!(not_found.is_empty());
    }

    #[test]
    fn get_active_diagnoses_filters_correctly() {
        let conn = test_db();
        let doc_id = make_document(&conn, None);

        insert_diagnosis(
            &conn,
            &Diagnosis {
                id: Uuid::new_v4(),
                name: "Hypertension".into(),
                icd_code: None,
                date_diagnosed: None,
                diagnosing_professional_id: None,
                status: DiagnosisStatus::Active,
                document_id: doc_id,
            },
        )
        .unwrap();

        insert_diagnosis(
            &conn,
            &Diagnosis {
                id: Uuid::new_v4(),
                name: "Resolved condition".into(),
                icd_code: None,
                date_diagnosed: None,
                diagnosing_professional_id: None,
                status: DiagnosisStatus::Resolved,
                document_id: doc_id,
            },
        )
        .unwrap();

        let active = get_active_diagnoses(&conn).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "Hypertension");
    }

    #[test]
    fn get_all_allergies_returns_all() {
        let conn = test_db();

        insert_allergy(&conn, &Allergy {
            id: Uuid::new_v4(),
            allergen: "Penicillin".into(),
            reaction: Some("Rash".into()),
            severity: AllergySeverity::Severe,
            date_identified: None,
            source: AllergySource::DocumentExtracted,
            document_id: None,
            verified: true,
        }).unwrap();

        insert_allergy(&conn, &Allergy {
            id: Uuid::new_v4(),
            allergen: "Sulfa".into(),
            reaction: None,
            severity: AllergySeverity::Moderate,
            date_identified: None,
            source: AllergySource::PatientReported,
            document_id: None,
            verified: false,
        }).unwrap();

        let all = get_all_allergies(&conn).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn document_default_pipeline_status_is_imported() {
        let conn = test_db();
        let doc_id = make_document(&conn, None);
        let doc = get_document(&conn, &doc_id).unwrap().unwrap();
        assert_eq!(doc.pipeline_status, PipelineStatus::Imported);
    }

    #[test]
    fn update_pipeline_status_transitions() {
        let conn = test_db();
        let doc_id = make_document(&conn, None);

        update_pipeline_status(&conn, &doc_id, &PipelineStatus::Extracting).unwrap();
        let doc = get_document(&conn, &doc_id).unwrap().unwrap();
        assert_eq!(doc.pipeline_status, PipelineStatus::Extracting);

        update_pipeline_status(&conn, &doc_id, &PipelineStatus::Structuring).unwrap();
        let doc = get_document(&conn, &doc_id).unwrap().unwrap();
        assert_eq!(doc.pipeline_status, PipelineStatus::Structuring);

        update_pipeline_status(&conn, &doc_id, &PipelineStatus::Confirmed).unwrap();
        let doc = get_document(&conn, &doc_id).unwrap().unwrap();
        assert_eq!(doc.pipeline_status, PipelineStatus::Confirmed);
    }

    #[test]
    fn update_pipeline_status_not_found() {
        let conn = test_db();
        let fake_id = Uuid::new_v4();
        let result = update_pipeline_status(&conn, &fake_id, &PipelineStatus::Failed);
        assert!(result.is_err());
    }

    #[test]
    fn get_documents_by_pipeline_status_filters() {
        let conn = test_db();
        let doc1 = make_document(&conn, None);
        let doc2 = make_document(&conn, None);

        update_pipeline_status(&conn, &doc1, &PipelineStatus::PendingReview).unwrap();

        let pending = get_documents_by_pipeline_status(&conn, &PipelineStatus::PendingReview).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, doc1);

        let imported = get_documents_by_pipeline_status(&conn, &PipelineStatus::Imported).unwrap();
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].id, doc2);

        let failed = get_documents_by_pipeline_status(&conn, &PipelineStatus::Failed).unwrap();
        assert!(failed.is_empty());
    }

    #[test]
    fn recalculate_profile_trust_matches_actual_data() {
        let conn = test_db();
        let doc1 = make_document(&conn, None);
        let doc2 = make_document(&conn, None);

        let mut d1 = get_document(&conn, &doc1).unwrap().unwrap();
        d1.verified = true;
        update_document(&conn, &d1).unwrap();

        recalculate_profile_trust(&conn).unwrap();
        let trust = get_profile_trust(&conn).unwrap();
        assert_eq!(trust.total_documents, 2);
        assert_eq!(trust.documents_verified, 1);
        assert!((trust.extraction_accuracy - 0.5).abs() < 0.01);

        delete_document_cascade(&conn, &doc2).unwrap();
        let trust = get_profile_trust(&conn).unwrap();
        assert_eq!(trust.total_documents, 1);
        assert_eq!(trust.documents_verified, 1);
        assert!((trust.extraction_accuracy - 1.0).abs() < 0.01);
    }

    #[test]
    fn delete_document_cascade_removes_all_children() {
        let conn = test_db();
        let doc_id = make_document(&conn, None);

        insert_medication(&conn, &Medication {
            id: Uuid::new_v4(),
            generic_name: "TestDrug".into(),
            brand_name: None,
            dose: "10mg".into(),
            frequency: "daily".into(),
            frequency_type: FrequencyType::Scheduled,
            route: "oral".into(),
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
            document_id: doc_id,
        }).unwrap();

        insert_lab_result(&conn, &LabResult {
            id: Uuid::new_v4(),
            test_name: "HbA1c".into(),
            test_code: None,
            value: Some(7.2),
            value_text: None,
            unit: Some("%".into()),
            reference_range_low: None,
            reference_range_high: None,
            abnormal_flag: AbnormalFlag::High,
            collection_date: NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            lab_facility: None,
            ordering_physician_id: None,
            document_id: doc_id,
        }).unwrap();

        insert_diagnosis(&conn, &Diagnosis {
            id: Uuid::new_v4(),
            name: "Diabetes".into(),
            icd_code: None,
            date_diagnosed: None,
            diagnosing_professional_id: None,
            status: DiagnosisStatus::Active,
            document_id: doc_id,
        }).unwrap();

        delete_document_cascade(&conn, &doc_id).unwrap();

        assert!(get_document(&conn, &doc_id).unwrap().is_none());
        let med_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM medications WHERE document_id = ?1",
            params![doc_id.to_string()], |r| r.get(0),
        ).unwrap();
        assert_eq!(med_count, 0);
        let lab_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM lab_results WHERE document_id = ?1",
            params![doc_id.to_string()], |r| r.get(0),
        ).unwrap();
        assert_eq!(lab_count, 0);
        let diag_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM diagnoses WHERE document_id = ?1",
            params![doc_id.to_string()], |r| r.get(0),
        ).unwrap();
        assert_eq!(diag_count, 0);
    }

    #[test]
    fn delete_document_cascade_not_found() {
        let conn = test_db();
        let fake_id = Uuid::new_v4();
        let result = delete_document_cascade(&conn, &fake_id);
        assert!(result.is_err());
    }

    #[test]
    fn delete_document_cascade_preserves_other_documents() {
        let conn = test_db();
        let doc1 = make_document(&conn, None);
        let doc2 = make_document(&conn, None);

        insert_medication(&conn, &Medication {
            id: Uuid::new_v4(),
            generic_name: "Drug1".into(),
            brand_name: None,
            dose: "10mg".into(),
            frequency: "daily".into(),
            frequency_type: FrequencyType::Scheduled,
            route: "oral".into(),
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
            document_id: doc1,
        }).unwrap();

        insert_medication(&conn, &Medication {
            id: Uuid::new_v4(),
            generic_name: "Drug2".into(),
            brand_name: None,
            dose: "20mg".into(),
            frequency: "daily".into(),
            frequency_type: FrequencyType::Scheduled,
            route: "oral".into(),
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
            document_id: doc2,
        }).unwrap();

        delete_document_cascade(&conn, &doc1).unwrap();

        assert!(get_document(&conn, &doc1).unwrap().is_none());
        assert!(get_document(&conn, &doc2).unwrap().is_some());
        let med_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM medications WHERE document_id = ?1",
            params![doc2.to_string()], |r| r.get(0),
        ).unwrap();
        assert_eq!(med_count, 1);
    }

    #[test]
    fn pipeline_status_round_trip() {
        for (variant, s) in [
            (PipelineStatus::Imported, "imported"),
            (PipelineStatus::Extracting, "extracting"),
            (PipelineStatus::Structuring, "structuring"),
            (PipelineStatus::PendingReview, "pending_review"),
            (PipelineStatus::Confirmed, "confirmed"),
            (PipelineStatus::Failed, "failed"),
            (PipelineStatus::Rejected, "rejected"),
        ] {
            assert_eq!(variant.as_str(), s);
            assert_eq!(PipelineStatus::from_str(s).unwrap(), variant);
        }
    }

    #[test]
    fn consistency_check_clean_database() {
        let conn = test_db();
        let report = check_consistency(&conn).unwrap();
        assert!(report.issues.is_empty(), "Clean DB should have no issues");
        assert!(!report.trust_drift_detected);
        assert_eq!(report.documents_checked, 0);
    }

    #[test]
    fn consistency_check_detects_stuck_pipeline() {
        let conn = test_db();
        let doc_id = make_document(&conn, None);

        update_pipeline_status(&conn, &doc_id, &PipelineStatus::Extracting).unwrap();
        recalculate_profile_trust(&conn).unwrap();

        let report = check_consistency(&conn).unwrap();
        let stuck: Vec<_> = report.issues.iter()
            .filter(|i| i.category == "stuck_pipeline")
            .collect();
        assert_eq!(stuck.len(), 1);
        assert_eq!(stuck[0].severity, "high");
        assert_eq!(stuck[0].document_id.as_deref(), Some(&*doc_id.to_string()));
    }

    #[test]
    fn consistency_check_detects_trust_drift() {
        let conn = test_db();
        let _doc_id = make_document(&conn, None);

        let report = check_consistency(&conn).unwrap();
        let trust_issues: Vec<_> = report.issues.iter()
            .filter(|i| i.category == "trust_drift")
            .collect();
        assert_eq!(trust_issues.len(), 1);
        assert!(report.trust_drift_detected);
    }

    #[test]
    fn repair_consistency_fixes_trust_drift() {
        let conn = test_db();
        let _doc_id = make_document(&conn, None);

        let report = check_consistency(&conn).unwrap();
        assert!(report.trust_drift_detected);

        let repaired = repair_consistency(&conn).unwrap();
        assert!(repaired >= 1);

        let report = check_consistency(&conn).unwrap();
        assert!(!report.trust_drift_detected);
    }

    #[test]
    fn repair_consistency_fixes_stuck_pipeline() {
        let conn = test_db();
        let doc_id = make_document(&conn, None);

        update_pipeline_status(&conn, &doc_id, &PipelineStatus::Structuring).unwrap();

        let report = check_consistency(&conn).unwrap();
        let stuck: Vec<_> = report.issues.iter()
            .filter(|i| i.category == "stuck_pipeline")
            .collect();
        assert_eq!(stuck.len(), 1);

        repair_consistency(&conn).unwrap();

        let doc_after = get_document(&conn, &doc_id).unwrap().unwrap();
        assert_eq!(doc_after.pipeline_status, PipelineStatus::Failed);

        let report = check_consistency(&conn).unwrap();
        let stuck_after: Vec<_> = report.issues.iter()
            .filter(|i| i.category == "stuck_pipeline")
            .collect();
        assert!(stuck_after.is_empty());
    }
}
