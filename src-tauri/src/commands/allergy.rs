//! ALLERGY-01 B5: Allergy CRUD + reference data IPC commands.
//!
//! Commands:
//! - `add_allergy`: manual entry with auto-classification
//! - `update_allergy`: partial update of existing allergy
//! - `delete_allergy`: remove an allergy by ID
//! - `verify_allergy`: mark an allergy as verified
//! - `get_allergen_references`: canonical allergen catalog for autocomplete

use std::str::FromStr;
use std::sync::Arc;

use tauri::State;

use crate::core_state::CoreState;
use crate::invariants::allergens::CanonicalAllergen;
use crate::models::enums::*;

/// DTO for canonical allergen reference data returned to the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AllergenReferenceDto {
    pub key: String,
    pub label: String,
    pub category: String,
    pub mechanism: String,
    pub source: String,
}

impl AllergenReferenceDto {
    fn from_canonical(allergen: &CanonicalAllergen, lang: &str) -> Self {
        Self {
            key: allergen.key.to_string(),
            label: allergen.label.get(lang).to_string(),
            category: allergen.category.to_string(),
            mechanism: allergen.mechanism.as_str().to_string(),
            source: allergen.source.to_string(),
        }
    }
}

/// ALLERGY-01 B5: Add an allergy (manual patient entry).
///
/// Auto-classifies the allergen via the invariant registry.
/// Sets source to `PatientReported`.
#[tauri::command]
pub fn add_allergy(
    allergen: String,
    reaction: Option<String>,
    severity: String,
    allergen_category: Option<String>,
    date_identified: Option<String>,
    state: State<'_, Arc<CoreState>>,
) -> Result<String, String> {
    let severity = AllergySeverity::from_str(&severity)
        .map_err(|_| format!("Invalid severity: {severity}"))?;

    let date = date_identified
        .as_deref()
        .map(|d| {
            chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
                .map_err(|_| format!("Invalid date format: {d}. Expected YYYY-MM-DD"))
        })
        .transpose()?;

    // Auto-classify via registry if no explicit category provided
    let category = if let Some(cat_str) = allergen_category.as_deref() {
        AllergenCategory::from_str(cat_str).ok()
    } else {
        state
            .invariants()
            .classify_allergen(&allergen)
            .and_then(|a| AllergenCategory::from_str(a.category).ok())
    };

    let conn = state.open_db().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4();

    let allergy = crate::models::Allergy {
        id,
        allergen,
        reaction,
        severity,
        allergen_category: category,
        date_identified: date,
        source: AllergySource::PatientReported,
        document_id: None,
        verified: false,
    };

    crate::db::insert_allergy(&conn, &allergy).map_err(|e| e.to_string())?;
    state.update_activity();
    Ok(id.to_string())
}

/// ALLERGY-01 B5: Update an existing allergy (partial update).
#[tauri::command]
pub fn update_allergy(
    allergy_id: String,
    allergen: Option<String>,
    reaction: Option<String>,
    severity: Option<String>,
    allergen_category: Option<String>,
    date_identified: Option<String>,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    let id = uuid::Uuid::parse_str(&allergy_id)
        .map_err(|_| format!("Invalid allergy ID: {allergy_id}"))?;

    let sev = severity
        .as_deref()
        .map(|s| {
            AllergySeverity::from_str(s)
                .map_err(|_| format!("Invalid severity: {s}"))
        })
        .transpose()?;

    let cat = allergen_category
        .as_deref()
        .map(|c| {
            if c.is_empty() {
                Ok(None)
            } else {
                AllergenCategory::from_str(c)
                    .map(Some)
                    .map_err(|_| format!("Invalid category: {c}"))
            }
        })
        .transpose()?;

    let date = date_identified
        .as_deref()
        .map(|d| {
            if d.is_empty() {
                Ok(None)
            } else {
                chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
                    .map(Some)
                    .map_err(|_| format!("Invalid date format: {d}"))
            }
        })
        .transpose()?;

    let conn = state.open_db().map_err(|e| e.to_string())?;

    crate::db::update_allergy(
        &conn,
        &id,
        allergen.as_deref(),
        reaction.as_deref().map(|r| if r.is_empty() { None } else { Some(r) }),
        sev.as_ref(),
        cat.as_ref().map(|c| c.as_ref()),
        date,
    )
    .map_err(|e| e.to_string())?;

    state.update_activity();
    Ok(())
}

/// ALLERGY-01 B5: Delete an allergy by ID.
#[tauri::command]
pub fn delete_allergy(
    allergy_id: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<bool, String> {
    let id = uuid::Uuid::parse_str(&allergy_id)
        .map_err(|_| format!("Invalid allergy ID: {allergy_id}"))?;

    let conn = state.open_db().map_err(|e| e.to_string())?;
    let deleted = crate::db::delete_allergy(&conn, &id).map_err(|e| e.to_string())?;
    state.update_activity();
    Ok(deleted)
}

/// ALLERGY-01 B5: Mark an allergy as verified.
#[tauri::command]
pub fn verify_allergy(
    allergy_id: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<(), String> {
    let id = uuid::Uuid::parse_str(&allergy_id)
        .map_err(|_| format!("Invalid allergy ID: {allergy_id}"))?;

    let conn = state.open_db().map_err(|e| e.to_string())?;
    crate::db::verify_allergy(&conn, &id).map_err(|e| e.to_string())?;
    state.update_activity();
    Ok(())
}

/// ALLERGY-01 B5: Get canonical allergen references for autocomplete.
///
/// Returns the full catalog (46 entries) optionally filtered by category.
/// Labels are resolved for the requested language.
#[tauri::command]
pub fn get_allergen_references(
    category: Option<String>,
    lang: String,
    state: State<'_, Arc<CoreState>>,
) -> Result<Vec<AllergenReferenceDto>, String> {
    let lang = match lang.as_str() {
        "fr" | "de" => lang.as_str(),
        _ => "en",
    };

    let references = state.invariants().allergen_references();
    let filtered: Vec<AllergenReferenceDto> = references
        .iter()
        .filter(|a| {
            category
                .as_deref()
                .map_or(true, |c| a.category == c)
        })
        .map(|a| AllergenReferenceDto::from_canonical(a, lang))
        .collect();

    Ok(filtered)
}
