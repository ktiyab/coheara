use std::str::FromStr;

use chrono::NaiveDate;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::DatabaseError;
use crate::models::*;
use crate::models::enums::*;

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

    let rows = stmt.query_map([], |row| Ok(medication_row_from_rusqlite(row)))?;

    let mut meds = Vec::new();
    for row in rows {
        meds.push(medication_from_row(row??)?);
    }
    Ok(meds)
}

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

    let rows = stmt.query_map(params![pattern], |row| Ok(medication_row_from_rusqlite(row)))?;

    let mut meds = Vec::new();
    for row in rows {
        meds.push(medication_from_row(row??)?);
    }
    Ok(meds)
}

/// All medications (active + stopped + paused) for coherence drift/conflict detection.
pub fn get_all_medications(conn: &Connection) -> Result<Vec<Medication>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, generic_name, brand_name, dose, frequency, frequency_type, route,
         prescriber_id, start_date, end_date, reason_start, reason_stop, is_otc, status,
         administration_instructions, max_daily_dose, condition, dose_type, is_compound, document_id
         FROM medications"
    )?;

    let rows = stmt.query_map([], |row| Ok(medication_row_from_rusqlite(row)))?;

    let mut meds = Vec::new();
    for row in rows {
        meds.push(medication_from_row(row??)?);
    }
    Ok(meds)
}

pub fn delete_medication_cascade(conn: &Connection, med_id: &Uuid) -> Result<(), DatabaseError> {
    conn.execute("DELETE FROM medications WHERE id = ?1", params![med_id.to_string()])?;
    Ok(())
}

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

/// All compound ingredients for coherence allergy cross-referencing.
pub fn get_all_compound_ingredients(conn: &Connection) -> Result<Vec<CompoundIngredient>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, medication_id, ingredient_name, ingredient_dose, maps_to_generic
         FROM compound_ingredients"
    )?;

    let rows = stmt.query_map([], |row| {
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

/// All dose changes for coherence drift/temporal detection.
pub fn get_all_dose_changes(conn: &Connection) -> Result<Vec<DoseChange>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, medication_id, old_dose, new_dose, old_frequency, new_frequency,
         change_date, changed_by_id, reason, document_id
         FROM dose_changes ORDER BY change_date DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, Option<String>>(7)?,
            row.get::<_, Option<String>>(8)?,
            row.get::<_, Option<String>>(9)?,
        ))
    })?;

    let mut changes = Vec::new();
    for row in rows {
        let (id, med_id, old_dose, new_dose, old_freq, new_freq, change_date, changed_by, reason, doc_id) = row?;
        changes.push(DoseChange {
            id: Uuid::parse_str(&id)
                .map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
            medication_id: Uuid::parse_str(&med_id)
                .map_err(|e| DatabaseError::ConstraintViolation(e.to_string()))?,
            old_dose,
            new_dose,
            old_frequency: old_freq,
            new_frequency: new_freq,
            change_date: NaiveDate::parse_from_str(&change_date, "%Y-%m-%d").unwrap_or_default(),
            changed_by_id: changed_by.and_then(|s| Uuid::parse_str(&s).ok()),
            reason,
            document_id: doc_id.and_then(|s| Uuid::parse_str(&s).ok()),
        });
    }
    Ok(changes)
}

// Internal row type for Medication mapping
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

fn medication_row_from_rusqlite(row: &rusqlite::Row<'_>) -> Result<MedicationRow, rusqlite::Error> {
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
