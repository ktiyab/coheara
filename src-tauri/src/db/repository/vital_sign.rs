use chrono::NaiveDateTime;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::DatabaseError;
use crate::models::{VitalSign, VitalSource, VitalType};

/// Insert a vital sign record.
pub fn insert_vital_sign(conn: &Connection, vs: &VitalSign) -> Result<(), DatabaseError> {
    conn.execute(
        "INSERT INTO vital_signs (id, vital_type, value_primary, value_secondary, unit, recorded_at, notes, source, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            vs.id.to_string(),
            vs.vital_type.as_str(),
            vs.value_primary,
            vs.value_secondary,
            vs.unit,
            vs.recorded_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            vs.notes,
            vs.source.as_str(),
            vs.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        ],
    )?;
    Ok(())
}

/// Get all vital signs of a given type, ordered by recorded_at descending.
pub fn get_vital_signs_by_type(
    conn: &Connection,
    vital_type: &VitalType,
) -> Result<Vec<VitalSign>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, vital_type, value_primary, value_secondary, unit, recorded_at, notes, source, created_at
         FROM vital_signs
         WHERE vital_type = ?1
         ORDER BY recorded_at DESC",
    )?;
    let rows = stmt.query_map(params![vital_type.as_str()], row_to_vital_sign)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

/// Get vital signs within a date range, ordered by recorded_at ascending.
pub fn get_vital_signs_in_range(
    conn: &Connection,
    from: &NaiveDateTime,
    to: &NaiveDateTime,
) -> Result<Vec<VitalSign>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, vital_type, value_primary, value_secondary, unit, recorded_at, notes, source, created_at
         FROM vital_signs
         WHERE recorded_at >= ?1 AND recorded_at <= ?2
         ORDER BY recorded_at ASC",
    )?;
    let rows = stmt.query_map(
        params![
            from.format("%Y-%m-%d %H:%M:%S").to_string(),
            to.format("%Y-%m-%d %H:%M:%S").to_string(),
        ],
        row_to_vital_sign,
    )?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)
}

/// Get the most recent vital sign of a given type.
pub fn get_latest_vital_sign(
    conn: &Connection,
    vital_type: &VitalType,
) -> Result<Option<VitalSign>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT id, vital_type, value_primary, value_secondary, unit, recorded_at, notes, source, created_at
         FROM vital_signs
         WHERE vital_type = ?1
         ORDER BY recorded_at DESC
         LIMIT 1",
    )?;
    let mut rows = stmt.query_map(params![vital_type.as_str()], row_to_vital_sign)?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Delete a vital sign by ID.
pub fn delete_vital_sign(conn: &Connection, id: &Uuid) -> Result<(), DatabaseError> {
    let affected = conn.execute(
        "DELETE FROM vital_signs WHERE id = ?1",
        params![id.to_string()],
    )?;
    if affected == 0 {
        return Err(DatabaseError::NotFound {
            entity_type: "vital_sign".into(),
            id: id.to_string(),
        });
    }
    Ok(())
}

fn row_to_vital_sign(row: &rusqlite::Row) -> Result<VitalSign, rusqlite::Error> {
    let id_str: String = row.get(0)?;
    let type_str: String = row.get(1)?;
    let recorded_str: String = row.get(5)?;
    let source_str: String = row.get(7)?;
    let created_str: String = row.get(8)?;

    Ok(VitalSign {
        id: Uuid::parse_str(&id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        vital_type: VitalType::from_str(&type_str).unwrap_or(VitalType::Temperature),
        value_primary: row.get(2)?,
        value_secondary: row.get(3)?,
        unit: row.get(4)?,
        recorded_at: NaiveDateTime::parse_from_str(&recorded_str, "%Y-%m-%d %H:%M:%S")
            .unwrap_or_default(),
        notes: row.get(6)?,
        source: VitalSource::from_str(&source_str).unwrap_or(VitalSource::Manual),
        created_at: NaiveDateTime::parse_from_str(&created_str, "%Y-%m-%d %H:%M:%S")
            .unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sqlite::open_memory_database;

    fn test_db() -> Connection {
        open_memory_database().unwrap()
    }

    fn make_vital(vtype: VitalType, value: f64) -> VitalSign {
        VitalSign {
            id: Uuid::new_v4(),
            vital_type: vtype,
            value_primary: value,
            value_secondary: None,
            unit: vtype.default_unit().to_string(),
            recorded_at: chrono::Local::now().naive_local(),
            notes: None,
            source: VitalSource::Manual,
            created_at: chrono::Local::now().naive_local(),
        }
    }

    #[test]
    fn insert_and_retrieve_temperature() {
        let conn = test_db();
        let vs = make_vital(VitalType::Temperature, 37.2);
        insert_vital_sign(&conn, &vs).unwrap();

        let results = get_vital_signs_by_type(&conn, &VitalType::Temperature).unwrap();
        assert_eq!(results.len(), 1);
        assert!((results[0].value_primary - 37.2).abs() < 0.01);
    }

    #[test]
    fn insert_blood_pressure_with_secondary() {
        let conn = test_db();
        let vs = VitalSign {
            id: Uuid::new_v4(),
            vital_type: VitalType::BloodPressure,
            value_primary: 120.0,
            value_secondary: Some(80.0),
            unit: "mmHg".to_string(),
            recorded_at: chrono::Local::now().naive_local(),
            notes: Some("After rest".to_string()),
            source: VitalSource::Manual,
            created_at: chrono::Local::now().naive_local(),
        };
        insert_vital_sign(&conn, &vs).unwrap();

        let latest = get_latest_vital_sign(&conn, &VitalType::BloodPressure)
            .unwrap()
            .unwrap();
        assert!((latest.value_primary - 120.0).abs() < 0.01);
        assert!((latest.value_secondary.unwrap() - 80.0).abs() < 0.01);
        assert_eq!(latest.notes.as_deref(), Some("After rest"));
    }

    #[test]
    fn delete_vital_sign_works() {
        let conn = test_db();
        let vs = make_vital(VitalType::HeartRate, 72.0);
        let id = vs.id;
        insert_vital_sign(&conn, &vs).unwrap();

        delete_vital_sign(&conn, &id).unwrap();
        let results = get_vital_signs_by_type(&conn, &VitalType::HeartRate).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn delete_nonexistent_fails() {
        let conn = test_db();
        let result = delete_vital_sign(&conn, &Uuid::new_v4());
        assert!(matches!(result, Err(DatabaseError::NotFound { .. })));
    }

    #[test]
    fn latest_returns_none_for_empty() {
        let conn = test_db();
        let result = get_latest_vital_sign(&conn, &VitalType::Weight).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn filter_by_type_isolates() {
        let conn = test_db();
        insert_vital_sign(&conn, &make_vital(VitalType::Temperature, 37.0)).unwrap();
        insert_vital_sign(&conn, &make_vital(VitalType::Weight, 75.0)).unwrap();
        insert_vital_sign(&conn, &make_vital(VitalType::Temperature, 38.5)).unwrap();

        let temps = get_vital_signs_by_type(&conn, &VitalType::Temperature).unwrap();
        assert_eq!(temps.len(), 2);

        let weights = get_vital_signs_by_type(&conn, &VitalType::Weight).unwrap();
        assert_eq!(weights.len(), 1);
    }
}
