use std::str::FromStr;

use chrono::{NaiveDate, NaiveDateTime};
use rusqlite::{params, Connection};
use uuid::Uuid;

use super::DatabaseError;
use crate::models::*;
use crate::models::enums::*;

/// Base repository operations for any entity
pub trait Repository<T, F> {
    fn insert(&self, entity: &T) -> Result<Uuid, DatabaseError>;
    fn get(&self, id: &Uuid) -> Result<Option<T>, DatabaseError>;
    fn delete(&self, id: &Uuid) -> Result<(), DatabaseError>;
    fn list(&self, filter: &F) -> Result<Vec<T>, DatabaseError>;
}

// ═══════════════════════════════════════════
// Document Repository
// ═══════════════════════════════════════════

pub fn insert_document(conn: &Connection, doc: &Document) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO documents (id, type, title, document_date, ingestion_date, professional_id,
         source_file, markdown_file, ocr_confidence, verified, source_deleted, perceptual_hash, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            doc.id.to_string(),
            doc.doc_type.as_str(),
            doc.title,
            doc.document_date.map(|d| d.to_string()),
            doc.ingestion_date.to_string(),
            doc.professional_id.map(|id| id.to_string()),
            doc.source_file,
            doc.markdown_file,
            doc.ocr_confidence,
            doc.verified as i32,
            doc.source_deleted as i32,
            doc.perceptual_hash,
            doc.notes,
        ],
    )?;
    Ok(())
}

pub fn get_document(conn: &Connection, id: &Uuid) -> Result<Option<Document>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, type, title, document_date, ingestion_date, professional_id,
         source_file, markdown_file, ocr_confidence, verified, source_deleted, perceptual_hash, notes
         FROM documents WHERE id = ?1"
    )?;

    let result = stmt.query_row(params![id.to_string()], |row| {
        Ok(DocumentRow {
            id: row.get::<_, String>(0)?,
            doc_type: row.get::<_, String>(1)?,
            title: row.get::<_, String>(2)?,
            document_date: row.get::<_, Option<String>>(3)?,
            ingestion_date: row.get::<_, String>(4)?,
            professional_id: row.get::<_, Option<String>>(5)?,
            source_file: row.get::<_, String>(6)?,
            markdown_file: row.get::<_, Option<String>>(7)?,
            ocr_confidence: row.get::<_, Option<f32>>(8)?,
            verified: row.get::<_, i32>(9)?,
            source_deleted: row.get::<_, i32>(10)?,
            perceptual_hash: row.get::<_, Option<String>>(11)?,
            notes: row.get::<_, Option<String>>(12)?,
        })
    });

    match result {
        Ok(row) => Ok(Some(document_from_row(row)?)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn get_document_by_hash(conn: &Connection, hash: &str) -> Result<Option<Document>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, type, title, document_date, ingestion_date, professional_id,
         source_file, markdown_file, ocr_confidence, verified, source_deleted, perceptual_hash, notes
         FROM documents WHERE perceptual_hash = ?1 LIMIT 1"
    )?;

    let result = stmt.query_row(params![hash], |row| {
        Ok(DocumentRow {
            id: row.get::<_, String>(0)?,
            doc_type: row.get::<_, String>(1)?,
            title: row.get::<_, String>(2)?,
            document_date: row.get::<_, Option<String>>(3)?,
            ingestion_date: row.get::<_, String>(4)?,
            professional_id: row.get::<_, Option<String>>(5)?,
            source_file: row.get::<_, String>(6)?,
            markdown_file: row.get::<_, Option<String>>(7)?,
            ocr_confidence: row.get::<_, Option<f32>>(8)?,
            verified: row.get::<_, i32>(9)?,
            source_deleted: row.get::<_, i32>(10)?,
            perceptual_hash: row.get::<_, Option<String>>(11)?,
            notes: row.get::<_, Option<String>>(12)?,
        })
    });

    match result {
        Ok(row) => Ok(Some(document_from_row(row)?)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

// Internal row type for Document mapping
struct DocumentRow {
    id: String,
    doc_type: String,
    title: String,
    document_date: Option<String>,
    ingestion_date: String,
    professional_id: Option<String>,
    source_file: String,
    markdown_file: Option<String>,
    ocr_confidence: Option<f32>,
    verified: i32,
    source_deleted: i32,
    perceptual_hash: Option<String>,
    notes: Option<String>,
}

fn document_from_row(row: DocumentRow) -> Result<Document, DatabaseError> {
    Ok(Document {
        id: Uuid::parse_str(&row.id).map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
        doc_type: DocumentType::from_str(&row.doc_type)?,
        title: row.title,
        document_date: row.document_date.and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
        ingestion_date: NaiveDateTime::parse_from_str(&row.ingestion_date, "%Y-%m-%d %H:%M:%S")
            .or_else(|_| NaiveDateTime::parse_from_str(&row.ingestion_date, "%Y-%m-%dT%H:%M:%S"))
            .unwrap_or_default(),
        professional_id: row.professional_id.and_then(|s| Uuid::parse_str(&s).ok()),
        source_file: row.source_file,
        markdown_file: row.markdown_file,
        ocr_confidence: row.ocr_confidence,
        verified: row.verified != 0,
        source_deleted: row.source_deleted != 0,
        perceptual_hash: row.perceptual_hash,
        notes: row.notes,
    })
}

// ═══════════════════════════════════════════
// Professional Repository
// ═══════════════════════════════════════════

pub fn insert_professional(conn: &Connection, prof: &Professional) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO professionals (id, name, specialty, institution, first_seen_date, last_seen_date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            prof.id.to_string(),
            prof.name,
            prof.specialty,
            prof.institution,
            prof.first_seen_date.map(|d| d.to_string()),
            prof.last_seen_date.map(|d| d.to_string()),
        ],
    )?;
    Ok(())
}

// ═══════════════════════════════════════════
// Medication Repository
// ═══════════════════════════════════════════

pub fn insert_medication(conn: &Connection, med: &Medication) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO medications (id, generic_name, brand_name, dose, frequency, frequency_type,
         route, prescriber_id, start_date, end_date, reason_start, reason_stop, is_otc, status,
         administration_instructions, max_daily_dose, condition, dose_type, is_compound, document_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
        params![
            med.id.to_string(),
            med.generic_name,
            med.brand_name,
            med.dose,
            med.frequency,
            med.frequency_type.as_str(),
            med.route,
            med.prescriber_id.map(|id| id.to_string()),
            med.start_date.map(|d| d.to_string()),
            med.end_date.map(|d| d.to_string()),
            med.reason_start,
            med.reason_stop,
            med.is_otc as i32,
            med.status.as_str(),
            med.administration_instructions,
            med.max_daily_dose,
            med.condition,
            med.dose_type.as_str(),
            med.is_compound as i32,
            med.document_id.to_string(),
        ],
    )?;
    Ok(())
}

pub fn get_active_medications(conn: &Connection) -> Result<Vec<Medication>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, generic_name, brand_name, dose, frequency, frequency_type, route,
         prescriber_id, start_date, end_date, reason_start, reason_stop, is_otc, status,
         administration_instructions, max_daily_dose, condition, dose_type, is_compound, document_id
         FROM medications WHERE status = 'active'"
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(MedicationRow {
            id: row.get(0)?,
            generic_name: row.get(1)?,
            brand_name: row.get(2)?,
            dose: row.get(3)?,
            frequency: row.get(4)?,
            frequency_type: row.get(5)?,
            route: row.get(6)?,
            prescriber_id: row.get(7)?,
            start_date: row.get(8)?,
            end_date: row.get(9)?,
            reason_start: row.get(10)?,
            reason_stop: row.get(11)?,
            is_otc: row.get(12)?,
            status: row.get(13)?,
            administration_instructions: row.get(14)?,
            max_daily_dose: row.get(15)?,
            condition: row.get(16)?,
            dose_type: row.get(17)?,
            is_compound: row.get(18)?,
            document_id: row.get(19)?,
        })
    })?;

    let mut meds = Vec::new();
    for row in rows {
        meds.push(medication_from_row(row?)?);
    }
    Ok(meds)
}

struct MedicationRow {
    id: String,
    generic_name: String,
    brand_name: Option<String>,
    dose: String,
    frequency: String,
    frequency_type: String,
    route: String,
    prescriber_id: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    reason_start: Option<String>,
    reason_stop: Option<String>,
    is_otc: i32,
    status: String,
    administration_instructions: Option<String>,
    max_daily_dose: Option<String>,
    condition: Option<String>,
    dose_type: String,
    is_compound: i32,
    document_id: String,
}

fn medication_from_row(row: MedicationRow) -> Result<Medication, DatabaseError> {
    Ok(Medication {
        id: Uuid::parse_str(&row.id).map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
        generic_name: row.generic_name,
        brand_name: row.brand_name,
        dose: row.dose,
        frequency: row.frequency,
        frequency_type: FrequencyType::from_str(&row.frequency_type)?,
        route: row.route,
        prescriber_id: row.prescriber_id.and_then(|s| Uuid::parse_str(&s).ok()),
        start_date: row.start_date.and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
        end_date: row.end_date.and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
        reason_start: row.reason_start,
        reason_stop: row.reason_stop,
        is_otc: row.is_otc != 0,
        status: MedicationStatus::from_str(&row.status)?,
        administration_instructions: row.administration_instructions,
        max_daily_dose: row.max_daily_dose,
        condition: row.condition,
        dose_type: DoseType::from_str(&row.dose_type)?,
        is_compound: row.is_compound != 0,
        document_id: Uuid::parse_str(&row.document_id).map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
    })
}

// ═══════════════════════════════════════════
// Lab Result Repository
// ═══════════════════════════════════════════

pub fn insert_lab_result(conn: &Connection, lab: &LabResult) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO lab_results (id, test_name, test_code, value, value_text, unit,
         reference_range_low, reference_range_high, abnormal_flag, collection_date,
         lab_facility, ordering_physician_id, document_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            lab.id.to_string(),
            lab.test_name,
            lab.test_code,
            lab.value,
            lab.value_text,
            lab.unit,
            lab.reference_range_low,
            lab.reference_range_high,
            lab.abnormal_flag.as_str(),
            lab.collection_date.to_string(),
            lab.lab_facility,
            lab.ordering_physician_id.map(|id| id.to_string()),
            lab.document_id.to_string(),
        ],
    )?;
    Ok(())
}

pub fn get_critical_labs(conn: &Connection) -> Result<Vec<LabResult>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, test_name, test_code, value, value_text, unit,
         reference_range_low, reference_range_high, abnormal_flag, collection_date,
         lab_facility, ordering_physician_id, document_id
         FROM lab_results WHERE abnormal_flag IN ('critical_low', 'critical_high')"
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(LabRow {
            id: row.get(0)?,
            test_name: row.get(1)?,
            test_code: row.get(2)?,
            value: row.get(3)?,
            value_text: row.get(4)?,
            unit: row.get(5)?,
            reference_range_low: row.get(6)?,
            reference_range_high: row.get(7)?,
            abnormal_flag: row.get(8)?,
            collection_date: row.get(9)?,
            lab_facility: row.get(10)?,
            ordering_physician_id: row.get(11)?,
            document_id: row.get(12)?,
        })
    })?;

    let mut labs = Vec::new();
    for row in rows {
        labs.push(lab_from_row(row?)?);
    }
    Ok(labs)
}

struct LabRow {
    id: String,
    test_name: String,
    test_code: Option<String>,
    value: Option<f64>,
    value_text: Option<String>,
    unit: Option<String>,
    reference_range_low: Option<f64>,
    reference_range_high: Option<f64>,
    abnormal_flag: String,
    collection_date: String,
    lab_facility: Option<String>,
    ordering_physician_id: Option<String>,
    document_id: String,
}

fn lab_from_row(row: LabRow) -> Result<LabResult, DatabaseError> {
    Ok(LabResult {
        id: Uuid::parse_str(&row.id).map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
        test_name: row.test_name,
        test_code: row.test_code,
        value: row.value,
        value_text: row.value_text,
        unit: row.unit,
        reference_range_low: row.reference_range_low,
        reference_range_high: row.reference_range_high,
        abnormal_flag: AbnormalFlag::from_str(&row.abnormal_flag)?,
        collection_date: NaiveDate::parse_from_str(&row.collection_date, "%Y-%m-%d").unwrap_or_default(),
        lab_facility: row.lab_facility,
        ordering_physician_id: row.ordering_physician_id.and_then(|s| Uuid::parse_str(&s).ok()),
        document_id: Uuid::parse_str(&row.document_id).map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
    })
}

// ═══════════════════════════════════════════
// Allergy Repository
// ═══════════════════════════════════════════

pub fn insert_allergy(conn: &Connection, allergy: &Allergy) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO allergies (id, allergen, reaction, severity, date_identified, source, document_id, verified)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            allergy.id.to_string(),
            allergy.allergen,
            allergy.reaction,
            allergy.severity.as_str(),
            allergy.date_identified.map(|d| d.to_string()),
            allergy.source.as_str(),
            allergy.document_id.map(|id| id.to_string()),
            allergy.verified as i32,
        ],
    )?;
    Ok(())
}

// ═══════════════════════════════════════════
// Symptom Repository
// ═══════════════════════════════════════════

pub fn insert_symptom(conn: &Connection, symptom: &Symptom) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO symptoms (id, category, specific, severity, body_region, duration, character,
         aggravating, relieving, timing_pattern, onset_date, onset_time, recorded_date, still_active,
         resolved_date, related_medication_id, related_diagnosis_id, source, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
        params![
            symptom.id.to_string(),
            symptom.category,
            symptom.specific,
            symptom.severity,
            symptom.body_region,
            symptom.duration,
            symptom.character,
            symptom.aggravating,
            symptom.relieving,
            symptom.timing_pattern,
            symptom.onset_date.to_string(),
            symptom.onset_time,
            symptom.recorded_date.to_string(),
            symptom.still_active as i32,
            symptom.resolved_date.map(|d| d.to_string()),
            symptom.related_medication_id.map(|id| id.to_string()),
            symptom.related_diagnosis_id.map(|id| id.to_string()),
            symptom.source.as_str(),
            symptom.notes,
        ],
    )?;
    Ok(())
}

// ═══════════════════════════════════════════
// Profile Trust Repository
// ═══════════════════════════════════════════

pub fn get_profile_trust(conn: &Connection) -> Result<ProfileTrust, DatabaseError> {
    conn.query_row(
        "SELECT total_documents, documents_verified, documents_corrected, extraction_accuracy, last_updated
         FROM profile_trust WHERE id = 1",
        [],
        |row| {
            Ok(ProfileTrust {
                total_documents: row.get(0)?,
                documents_verified: row.get(1)?,
                documents_corrected: row.get(2)?,
                extraction_accuracy: row.get(3)?,
                last_updated: row.get::<_, String>(4)
                    .map(|s| NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S").unwrap_or_default())?,
            })
        },
    ).map_err(|e| e.into())
}

pub fn update_profile_trust_verified(conn: &Connection) -> Result<(), DatabaseError> {
    conn.execute(
        "UPDATE profile_trust SET
         documents_verified = documents_verified + 1,
         total_documents = total_documents + 1,
         extraction_accuracy = CASE
           WHEN total_documents > 0
           THEN CAST(documents_verified + 1 AS REAL) / CAST(total_documents + 1 AS REAL)
           ELSE 1.0
         END,
         last_updated = datetime('now')
         WHERE id = 1",
        [],
    )?;
    Ok(())
}

pub fn update_profile_trust_corrected(conn: &Connection) -> Result<(), DatabaseError> {
    conn.execute(
        "UPDATE profile_trust SET
         documents_corrected = documents_corrected + 1,
         documents_verified = documents_verified + 1,
         total_documents = total_documents + 1,
         extraction_accuracy = CASE
           WHEN total_documents > 0
           THEN CAST(documents_verified + 1 AS REAL) / CAST(total_documents + 1 AS REAL)
           ELSE 1.0
         END,
         last_updated = datetime('now')
         WHERE id = 1",
        [],
    )?;
    Ok(())
}

// ═══════════════════════════════════════════
// Compound Ingredient Repository
// ═══════════════════════════════════════════

pub fn insert_compound_ingredient(conn: &Connection, ing: &CompoundIngredient) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO compound_ingredients (id, medication_id, ingredient_name, ingredient_dose, maps_to_generic)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            ing.id.to_string(),
            ing.medication_id.to_string(),
            ing.ingredient_name,
            ing.ingredient_dose,
            ing.maps_to_generic,
        ],
    )?;
    Ok(())
}

pub fn delete_medication_cascade(conn: &Connection, med_id: &Uuid) -> Result<(), DatabaseError> {
    conn.execute("DELETE FROM medications WHERE id = ?1", params![med_id.to_string()])?;
    Ok(())
}

pub fn get_compound_ingredients(conn: &Connection, med_id: &Uuid) -> Result<Vec<CompoundIngredient>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, medication_id, ingredient_name, ingredient_dose, maps_to_generic
         FROM compound_ingredients WHERE medication_id = ?1"
    )?;

    let rows = stmt.query_map(params![med_id.to_string()], |row| {
        Ok(CompoundIngredient {
            id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
            medication_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or_default(),
            ingredient_name: row.get(2)?,
            ingredient_dose: row.get(3)?,
            maps_to_generic: row.get(4)?,
        })
    })?;

    rows.map(|r| r.map_err(DatabaseError::from)).collect()
}

// ═══════════════════════════════════════════
// Diagnosis Repository
// ═══════════════════════════════════════════

pub fn insert_diagnosis(conn: &Connection, diag: &Diagnosis) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO diagnoses (id, name, icd_code, date_diagnosed, diagnosing_professional_id, status, document_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            diag.id.to_string(),
            diag.name,
            diag.icd_code,
            diag.date_diagnosed.map(|d| d.to_string()),
            diag.diagnosing_professional_id.map(|id| id.to_string()),
            diag.status.as_str(),
            diag.document_id.to_string(),
        ],
    )?;
    Ok(())
}

// ═══════════════════════════════════════════
// Procedure Repository
// ═══════════════════════════════════════════

pub fn insert_procedure(conn: &Connection, proc: &Procedure) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO procedures (id, name, date, performing_professional_id, facility, outcome,
         follow_up_required, follow_up_date, document_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            proc.id.to_string(),
            proc.name,
            proc.date.map(|d| d.to_string()),
            proc.performing_professional_id.map(|id| id.to_string()),
            proc.facility,
            proc.outcome,
            proc.follow_up_required as i32,
            proc.follow_up_date.map(|d| d.to_string()),
            proc.document_id.to_string(),
        ],
    )?;
    Ok(())
}

// ═══════════════════════════════════════════
// Referral Repository
// ═══════════════════════════════════════════

pub fn insert_referral(conn: &Connection, referral: &Referral) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO referrals (id, referring_professional_id, referred_to_professional_id,
         reason, date, status, document_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            referral.id.to_string(),
            referral.referring_professional_id.to_string(),
            referral.referred_to_professional_id.to_string(),
            referral.reason,
            referral.date.to_string(),
            referral.status.as_str(),
            referral.document_id.map(|id| id.to_string()),
        ],
    )?;
    Ok(())
}

// ═══════════════════════════════════════════
// Tapering Step Repository
// ═══════════════════════════════════════════

pub fn insert_tapering_step(conn: &Connection, step: &TaperingStep) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO tapering_schedules (id, medication_id, step_number, dose, duration_days,
         start_date, document_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            step.id.to_string(),
            step.medication_id.to_string(),
            step.step_number,
            step.dose,
            step.duration_days,
            step.start_date.map(|d| d.to_string()),
            step.document_id.map(|id| id.to_string()),
        ],
    )?;
    Ok(())
}

// ═══════════════════════════════════════════
// Medication Instruction Repository
// ═══════════════════════════════════════════

pub fn insert_medication_instruction(
    conn: &Connection,
    instr: &MedicationInstruction,
) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO medication_instructions (id, medication_id, instruction, timing, source_document_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            instr.id.to_string(),
            instr.medication_id.to_string(),
            instr.instruction,
            instr.timing,
            instr.source_document_id.map(|id| id.to_string()),
        ],
    )?;
    Ok(())
}

// ═══════════════════════════════════════════
// Professional — find or create
// ═══════════════════════════════════════════

pub fn find_or_create_professional(
    conn: &Connection,
    name: &str,
    specialty: Option<&str>,
) -> Result<Professional, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, specialty, institution, first_seen_date, last_seen_date
         FROM professionals WHERE name = ?1 LIMIT 1",
    )?;

    let result = stmt.query_row(params![name], |row| {
        Ok(Professional {
            id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
            name: row.get(1)?,
            specialty: row.get(2)?,
            institution: row.get(3)?,
            first_seen_date: row
                .get::<_, Option<String>>(4)?
                .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
            last_seen_date: row
                .get::<_, Option<String>>(5)?
                .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
        })
    });

    match result {
        Ok(prof) => Ok(prof),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            let prof = Professional {
                id: Uuid::new_v4(),
                name: name.to_string(),
                specialty: specialty.map(|s| s.to_string()),
                institution: None,
                first_seen_date: Some(chrono::Local::now().date_naive()),
                last_seen_date: Some(chrono::Local::now().date_naive()),
            };
            insert_professional(conn, &prof)?;
            Ok(prof)
        }
        Err(e) => Err(e.into()),
    }
}

// ═══════════════════════════════════════════
// Document — update
// ═══════════════════════════════════════════

pub fn update_document(conn: &Connection, doc: &Document) -> Result<(), DatabaseError> {
    conn.execute(
        "UPDATE documents SET type = ?2, title = ?3, document_date = ?4,
         professional_id = ?5, markdown_file = ?6, ocr_confidence = ?7,
         verified = ?8, notes = ?9
         WHERE id = ?1",
        params![
            doc.id.to_string(),
            doc.doc_type.as_str(),
            doc.title,
            doc.document_date.map(|d| d.to_string()),
            doc.professional_id.map(|id| id.to_string()),
            doc.markdown_file,
            doc.ocr_confidence,
            doc.verified as i32,
            doc.notes,
        ],
    )?;
    Ok(())
}

// ═══════════════════════════════════════════
// Conversation Repository
// ═══════════════════════════════════════════

pub fn insert_conversation(conn: &Connection, conv: &Conversation) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO conversations (id, started_at, title) VALUES (?1, ?2, ?3)",
        params![
            conv.id.to_string(),
            conv.started_at.to_string(),
            conv.title,
        ],
    )?;
    Ok(())
}

pub fn get_conversation(conn: &Connection, id: &Uuid) -> Result<Option<Conversation>, DatabaseError> {
    let result = conn.query_row(
        "SELECT id, started_at, title FROM conversations WHERE id = ?1",
        params![id.to_string()],
        |row| {
            Ok(Conversation {
                id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
                started_at: NaiveDateTime::parse_from_str(
                    &row.get::<_, String>(1)?,
                    "%Y-%m-%d %H:%M:%S",
                )
                .unwrap_or_default(),
                title: row.get(2)?,
            })
        },
    );

    match result {
        Ok(conv) => Ok(Some(conv)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

// ═══════════════════════════════════════════
// Message Repository
// ═══════════════════════════════════════════

pub fn insert_message(conn: &Connection, msg: &Message) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO messages (id, conversation_id, role, content, timestamp, source_chunks, confidence, feedback)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            msg.id.to_string(),
            msg.conversation_id.to_string(),
            msg.role.as_str(),
            msg.content,
            msg.timestamp.to_string(),
            msg.source_chunks,
            msg.confidence,
            msg.feedback.as_ref().map(|f| f.as_str().to_string()),
        ],
    )?;
    Ok(())
}

pub fn get_messages_by_conversation(
    conn: &Connection,
    conversation_id: &Uuid,
) -> Result<Vec<Message>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, conversation_id, role, content, timestamp, source_chunks, confidence, feedback
         FROM messages WHERE conversation_id = ?1 ORDER BY timestamp ASC",
    )?;

    let rows = stmt.query_map(params![conversation_id.to_string()], |row| {
        Ok(MessageRow {
            id: row.get(0)?,
            conversation_id: row.get(1)?,
            role: row.get(2)?,
            content: row.get(3)?,
            timestamp: row.get(4)?,
            source_chunks: row.get(5)?,
            confidence: row.get(6)?,
            feedback: row.get(7)?,
        })
    })?;

    let mut messages = Vec::new();
    for row in rows {
        messages.push(message_from_row(row?)?);
    }
    Ok(messages)
}

struct MessageRow {
    id: String,
    conversation_id: String,
    role: String,
    content: String,
    timestamp: String,
    source_chunks: Option<String>,
    confidence: Option<f32>,
    feedback: Option<String>,
}

fn message_from_row(row: MessageRow) -> Result<Message, DatabaseError> {
    Ok(Message {
        id: Uuid::parse_str(&row.id)
            .map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
        conversation_id: Uuid::parse_str(&row.conversation_id)
            .map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
        role: MessageRole::from_str(&row.role)?,
        content: row.content,
        timestamp: NaiveDateTime::parse_from_str(&row.timestamp, "%Y-%m-%d %H:%M:%S")
            .unwrap_or_default(),
        source_chunks: row.source_chunks,
        confidence: row.confidence,
        feedback: row
            .feedback
            .as_deref()
            .map(MessageFeedback::from_str)
            .transpose()?,
    })
}

// ═══════════════════════════════════════════
// Dismissed Alert Repository
// ═══════════════════════════════════════════

pub fn get_dismissed_alerts(conn: &Connection) -> Result<Vec<DismissedAlert>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, alert_type, entity_ids, dismissed_date, reason, dismissed_by
         FROM dismissed_alerts",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, String>(5)?,
        ))
    })?;

    let mut alerts = Vec::new();
    for row in rows {
        let (id, alert_type, entity_ids, dismissed_date, reason, dismissed_by) = row?;
        alerts.push(DismissedAlert {
            id: Uuid::parse_str(&id)
                .map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
            alert_type: AlertType::from_str(&alert_type)?,
            entity_ids,
            dismissed_date: NaiveDateTime::parse_from_str(&dismissed_date, "%Y-%m-%d %H:%M:%S")
                .unwrap_or_default(),
            reason,
            dismissed_by: DismissedBy::from_str(&dismissed_by)?,
        });
    }
    Ok(alerts)
}

// ═══════════════════════════════════════════
// Filtered Query Functions (for RAG retrieval)
// ═══════════════════════════════════════════

pub fn get_medications_by_name(
    conn: &Connection,
    name: &str,
) -> Result<Vec<Medication>, DatabaseError> {
    let pattern = format!("%{name}%");
    let mut stmt = conn.prepare(
        "SELECT id, generic_name, brand_name, dose, frequency, frequency_type, route,
         prescriber_id, start_date, end_date, reason_start, reason_stop, is_otc, status,
         administration_instructions, max_daily_dose, condition, dose_type, is_compound, document_id
         FROM medications WHERE LOWER(generic_name) LIKE LOWER(?1) OR LOWER(brand_name) LIKE LOWER(?1)",
    )?;

    let rows = stmt.query_map(params![pattern], |row| {
        Ok(MedicationRow {
            id: row.get(0)?,
            generic_name: row.get(1)?,
            brand_name: row.get(2)?,
            dose: row.get(3)?,
            frequency: row.get(4)?,
            frequency_type: row.get(5)?,
            route: row.get(6)?,
            prescriber_id: row.get(7)?,
            start_date: row.get(8)?,
            end_date: row.get(9)?,
            reason_start: row.get(10)?,
            reason_stop: row.get(11)?,
            is_otc: row.get(12)?,
            status: row.get(13)?,
            administration_instructions: row.get(14)?,
            max_daily_dose: row.get(15)?,
            condition: row.get(16)?,
            dose_type: row.get(17)?,
            is_compound: row.get(18)?,
            document_id: row.get(19)?,
        })
    })?;

    let mut meds = Vec::new();
    for row in rows {
        meds.push(medication_from_row(row?)?);
    }
    Ok(meds)
}

pub fn get_lab_results_since(
    conn: &Connection,
    since: &NaiveDate,
) -> Result<Vec<LabResult>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, test_name, test_code, value, value_text, unit,
         reference_range_low, reference_range_high, abnormal_flag, collection_date,
         lab_facility, ordering_physician_id, document_id
         FROM lab_results WHERE collection_date >= ?1 ORDER BY collection_date DESC",
    )?;

    let rows = stmt.query_map(params![since.to_string()], |row| {
        Ok(LabRow {
            id: row.get(0)?,
            test_name: row.get(1)?,
            test_code: row.get(2)?,
            value: row.get(3)?,
            value_text: row.get(4)?,
            unit: row.get(5)?,
            reference_range_low: row.get(6)?,
            reference_range_high: row.get(7)?,
            abnormal_flag: row.get(8)?,
            collection_date: row.get(9)?,
            lab_facility: row.get(10)?,
            ordering_physician_id: row.get(11)?,
            document_id: row.get(12)?,
        })
    })?;

    let mut labs = Vec::new();
    for row in rows {
        labs.push(lab_from_row(row?)?);
    }
    Ok(labs)
}

pub fn get_lab_results_by_test_name(
    conn: &Connection,
    test_name: &str,
) -> Result<Vec<LabResult>, DatabaseError> {
    let pattern = format!("%{test_name}%");
    let mut stmt = conn.prepare(
        "SELECT id, test_name, test_code, value, value_text, unit,
         reference_range_low, reference_range_high, abnormal_flag, collection_date,
         lab_facility, ordering_physician_id, document_id
         FROM lab_results WHERE LOWER(test_name) LIKE LOWER(?1) ORDER BY collection_date DESC",
    )?;

    let rows = stmt.query_map(params![pattern], |row| {
        Ok(LabRow {
            id: row.get(0)?,
            test_name: row.get(1)?,
            test_code: row.get(2)?,
            value: row.get(3)?,
            value_text: row.get(4)?,
            unit: row.get(5)?,
            reference_range_low: row.get(6)?,
            reference_range_high: row.get(7)?,
            abnormal_flag: row.get(8)?,
            collection_date: row.get(9)?,
            lab_facility: row.get(10)?,
            ordering_physician_id: row.get(11)?,
            document_id: row.get(12)?,
        })
    })?;

    let mut labs = Vec::new();
    for row in rows {
        labs.push(lab_from_row(row?)?);
    }
    Ok(labs)
}

pub fn get_active_diagnoses(conn: &Connection) -> Result<Vec<Diagnosis>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, icd_code, date_diagnosed, diagnosing_professional_id, status, document_id
         FROM diagnoses WHERE status = 'active'",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
        ))
    })?;

    let mut diagnoses = Vec::new();
    for row in rows {
        let (id, name, icd_code, date_diagnosed, prof_id, status, doc_id) = row?;
        diagnoses.push(Diagnosis {
            id: Uuid::parse_str(&id)
                .map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
            name,
            icd_code,
            date_diagnosed: date_diagnosed
                .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
            diagnosing_professional_id: prof_id.and_then(|s| Uuid::parse_str(&s).ok()),
            status: DiagnosisStatus::from_str(&status)?,
            document_id: Uuid::parse_str(&doc_id)
                .map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
        });
    }
    Ok(diagnoses)
}

pub fn get_all_allergies(conn: &Connection) -> Result<Vec<Allergy>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, allergen, reaction, severity, date_identified, source, document_id, verified
         FROM allergies",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, i32>(7)?,
        ))
    })?;

    let mut allergies = Vec::new();
    for row in rows {
        let (id, allergen, reaction, severity, date_identified, source, document_id, verified) =
            row?;
        allergies.push(Allergy {
            id: Uuid::parse_str(&id)
                .map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
            allergen,
            reaction,
            severity: AllergySeverity::from_str(&severity)?,
            date_identified: date_identified
                .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
            source: AllergySource::from_str(&source)?,
            document_id: document_id.and_then(|s| Uuid::parse_str(&s).ok()),
            verified: verified != 0,
        });
    }
    Ok(allergies)
}

pub fn get_symptoms_in_date_range(
    conn: &Connection,
    from: &NaiveDate,
    to: &NaiveDate,
) -> Result<Vec<Symptom>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, category, specific, severity, body_region, duration, character,
         aggravating, relieving, timing_pattern, onset_date, onset_time, recorded_date,
         still_active, resolved_date, related_medication_id, related_diagnosis_id, source, notes
         FROM symptoms WHERE onset_date BETWEEN ?1 AND ?2 ORDER BY onset_date DESC",
    )?;

    let rows = stmt.query_map(params![from.to_string(), to.to_string()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i32>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Option<String>>(7)?,
            row.get::<_, Option<String>>(8)?,
            row.get::<_, Option<String>>(9)?,
            row.get::<_, String>(10)?,
            row.get::<_, Option<String>>(11)?,
            row.get::<_, String>(12)?,
            row.get::<_, i32>(13)?,
            row.get::<_, Option<String>>(14)?,
            row.get::<_, Option<String>>(15)?,
            row.get::<_, Option<String>>(16)?,
            row.get::<_, String>(17)?,
            row.get::<_, Option<String>>(18)?,
        ))
    })?;

    let mut symptoms = Vec::new();
    for row in rows {
        let (
            id, category, specific, severity, body_region, duration, character,
            aggravating, relieving, timing_pattern, onset_date, onset_time, recorded_date,
            still_active, resolved_date, related_med_id, related_diag_id, source, notes,
        ) = row?;
        symptoms.push(Symptom {
            id: Uuid::parse_str(&id)
                .map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
            category,
            specific,
            severity,
            body_region,
            duration,
            character,
            aggravating,
            relieving,
            timing_pattern,
            onset_date: NaiveDate::parse_from_str(&onset_date, "%Y-%m-%d").unwrap_or_default(),
            onset_time,
            recorded_date: NaiveDate::parse_from_str(&recorded_date, "%Y-%m-%d")
                .unwrap_or_default(),
            still_active: still_active != 0,
            resolved_date: resolved_date
                .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
            related_medication_id: related_med_id.and_then(|s| Uuid::parse_str(&s).ok()),
            related_diagnosis_id: related_diag_id.and_then(|s| Uuid::parse_str(&s).ok()),
            source: SymptomSource::from_str(&source)?,
            notes,
        });
    }
    Ok(symptoms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;

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

        insert_allergy(
            &conn,
            &Allergy {
                id: Uuid::new_v4(),
                allergen: "Penicillin".into(),
                reaction: Some("Rash".into()),
                severity: AllergySeverity::Severe,
                date_identified: None,
                source: AllergySource::DocumentExtracted,
                document_id: None,
                verified: true,
            },
        )
        .unwrap();

        insert_allergy(
            &conn,
            &Allergy {
                id: Uuid::new_v4(),
                allergen: "Sulfa".into(),
                reaction: None,
                severity: AllergySeverity::Moderate,
                date_identified: None,
                source: AllergySource::PatientReported,
                document_id: None,
                verified: false,
            },
        )
        .unwrap();

        let all = get_all_allergies(&conn).unwrap();
        assert_eq!(all.len(), 2);
    }
}
