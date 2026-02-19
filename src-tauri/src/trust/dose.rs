use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use super::TrustError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DosePlausibility {
    pub medication_name: String,
    pub extracted_dose: String,
    pub extracted_value: f64,
    pub extracted_unit: String,
    pub typical_range_low: f64,
    pub typical_range_high: f64,
    pub typical_unit: String,
    pub plausibility: PlausibilityResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlausibilityResult {
    Plausible,
    HighDose { message: String },
    VeryHighDose { message: String },
    LowDose { message: String },
    UnknownMedication,
}

/// Internal reference row from dose_references table.
#[derive(Debug, Clone)]
struct DoseReference {
    pub typical_min_mg: f64,
    pub typical_max_mg: f64,
    pub absolute_max_mg: f64,
    pub unit: String,
}

/// Resolve a medication name (possibly brand name) to its generic name.
fn resolve_to_generic(conn: &Connection, medication_name: &str) -> String {
    let lower = medication_name.trim().to_lowercase();

    // First check if it's already a generic name in dose_references
    let is_generic: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM dose_references WHERE LOWER(generic_name) = ?1",
            params![lower],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if is_generic {
        return lower;
    }

    // Try medication_aliases: brand_name → generic_name
    conn.query_row(
        "SELECT LOWER(generic_name) FROM medication_aliases
         WHERE LOWER(brand_name) = ?1 LIMIT 1",
        params![lower],
        |row| row.get::<_, String>(0),
    )
    .unwrap_or(lower)
}

/// Convert a dose value to milligrams.
pub fn convert_to_mg(value: f64, unit: &str) -> f64 {
    match unit.to_lowercase().as_str() {
        "mg" => value,
        "g" => value * 1000.0,
        "mcg" | "ug" | "µg" => value / 1000.0,
        _ => value, // Assume mg if unknown
    }
}

/// Check if an extracted dose is plausible against known reference ranges.
pub fn check_dose_plausibility(
    conn: &Connection,
    medication_name: &str,
    dose_value: f64,
    dose_unit: &str,
) -> Result<DosePlausibility, TrustError> {
    let generic = resolve_to_generic(conn, medication_name);

    let reference = conn.query_row(
        "SELECT generic_name, typical_min_mg, typical_max_mg, absolute_max_mg, unit
         FROM dose_references WHERE LOWER(generic_name) = ?1",
        params![generic],
        |row| {
            Ok(DoseReference {
                typical_min_mg: row.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                typical_max_mg: row.get::<_, Option<f64>>(2)?.unwrap_or(f64::MAX),
                absolute_max_mg: row.get::<_, Option<f64>>(3)?.unwrap_or(f64::MAX),
                unit: row.get(4)?,
            })
        },
    );

    match reference {
        Ok(ref_data) => {
            let dose_mg = convert_to_mg(dose_value, dose_unit);

            let plausibility = if dose_mg > ref_data.absolute_max_mg * 5.0 {
                PlausibilityResult::VeryHighDose {
                    message: format!(
                        "I extracted {dose_mg}mg for {medication_name} but the typical maximum is {}mg. \
                         This may be an extraction error — please double-check this value.",
                        ref_data.absolute_max_mg
                    ),
                }
            } else if dose_mg > ref_data.typical_max_mg {
                PlausibilityResult::HighDose {
                    message: format!(
                        "I extracted {dose_mg}mg for {medication_name} but the typical range is \
                         {}-{}mg. Please verify this value.",
                        ref_data.typical_min_mg, ref_data.typical_max_mg
                    ),
                }
            } else if ref_data.typical_min_mg > 0.0 && dose_mg < ref_data.typical_min_mg * 0.5 {
                PlausibilityResult::LowDose {
                    message: format!(
                        "I extracted {dose_mg}mg for {medication_name} but the typical minimum is \
                         {}mg. Please verify this value.",
                        ref_data.typical_min_mg
                    ),
                }
            } else {
                PlausibilityResult::Plausible
            };

            Ok(DosePlausibility {
                medication_name: medication_name.into(),
                extracted_dose: format!("{dose_value}{dose_unit}"),
                extracted_value: dose_value,
                extracted_unit: dose_unit.into(),
                typical_range_low: ref_data.typical_min_mg,
                typical_range_high: ref_data.typical_max_mg,
                typical_unit: ref_data.unit,
                plausibility,
            })
        }
        Err(_) => Ok(DosePlausibility {
            medication_name: medication_name.into(),
            extracted_dose: format!("{dose_value}{dose_unit}"),
            extracted_value: dose_value,
            extracted_unit: dose_unit.into(),
            typical_range_low: 0.0,
            typical_range_high: 0.0,
            typical_unit: "mg".into(),
            plausibility: PlausibilityResult::UnknownMedication,
        }),
    }
}
