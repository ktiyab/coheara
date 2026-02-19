use chrono::NaiveDate;
use rusqlite::{params, Connection};

use crate::db::DatabaseError;
use super::correlations::{detect_correlations, fetch_explicit_correlations};
use super::fetch::*;
use super::types::*;

/// Assembles timeline events from ALL entity tables, applies filters,
/// and returns them sorted chronologically (oldest first).
pub fn assemble_timeline_events(
    conn: &Connection,
    filter: &TimelineFilter,
) -> Result<Vec<TimelineEvent>, DatabaseError> {
    let (date_from, date_to) = resolve_date_bounds(conn, filter)?;

    let mut events: Vec<TimelineEvent> = Vec::new();

    events.extend(fetch_medication_starts(conn, &date_from, &date_to)?);
    events.extend(fetch_medication_stops(conn, &date_from, &date_to)?);
    events.extend(fetch_dose_changes(conn, &date_from, &date_to)?);
    events.extend(fetch_lab_events(conn, &date_from, &date_to)?);
    events.extend(fetch_symptom_events(conn, &date_from, &date_to)?);
    events.extend(fetch_procedure_events(conn, &date_from, &date_to)?);
    events.extend(fetch_appointment_events(conn, &date_from, &date_to)?);
    events.extend(fetch_document_events(conn, &date_from, &date_to)?);
    events.extend(fetch_diagnosis_events(conn, &date_from, &date_to)?);

    // Apply event_type filter
    if let Some(ref types) = filter.event_types {
        events.retain(|e| types.contains(&e.event_type));
    }

    // Apply professional filter
    if let Some(ref prof_id) = filter.professional_id {
        events.retain(|e| e.professional_id.as_deref() == Some(prof_id.as_str()));
    }

    // Sort chronologically
    events.sort_by(|a, b| a.date.cmp(&b.date));

    Ok(events)
}

/// Resolves effective date bounds. If "since last visit" mode,
/// date_from = 30 days before appointment date (for context).
fn resolve_date_bounds(
    conn: &Connection,
    filter: &TimelineFilter,
) -> Result<(Option<String>, Option<String>), DatabaseError> {
    let date_from = if let Some(ref appt_id) = filter.since_appointment_id {
        let appt_date: String = conn
            .query_row(
                "SELECT date FROM appointments WHERE id = ?1",
                params![appt_id],
                |row| row.get(0),
            )
            .map_err(|_| DatabaseError::NotFound {
                entity_type: "appointment".into(),
                id: appt_id.clone(),
            })?;

        // Go back 30 days for context
        if let Ok(parsed) = NaiveDate::parse_from_str(&appt_date, "%Y-%m-%d") {
            let context_start = parsed - chrono::Duration::days(30);
            Some(context_start.format("%Y-%m-%d").to_string())
        } else {
            Some(appt_date)
        }
    } else {
        filter.date_from.clone()
    };

    Ok((date_from, filter.date_to.clone()))
}

/// Computes total event counts across all tables (unfiltered).
/// Used for filter badge counts.
pub fn compute_event_counts(conn: &Connection) -> Result<EventCounts, DatabaseError> {
    let count = |sql: &str| -> Result<u32, DatabaseError> {
        conn.query_row(sql, [], |row| row.get(0))
            .map_err(DatabaseError::from)
    };

    Ok(EventCounts {
        medications: count("SELECT COUNT(*) FROM medications WHERE start_date IS NOT NULL")?
            + count("SELECT COUNT(*) FROM dose_changes")?,
        lab_results: count("SELECT COUNT(*) FROM lab_results")?,
        symptoms: count("SELECT COUNT(*) FROM symptoms")?,
        procedures: count("SELECT COUNT(*) FROM procedures WHERE date IS NOT NULL")?,
        appointments: count("SELECT COUNT(*) FROM appointments")?,
        documents: count("SELECT COUNT(*) FROM documents")?,
        diagnoses: count("SELECT COUNT(*) FROM diagnoses WHERE date_diagnosed IS NOT NULL")?,
    })
}

/// Fetches professionals with their event counts for the filter dropdown.
pub fn fetch_professionals_with_counts(
    conn: &Connection,
) -> Result<Vec<ProfessionalSummary>, DatabaseError> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.specialty,
                (SELECT COUNT(*) FROM medications m WHERE m.prescriber_id = p.id)
                + (SELECT COUNT(*) FROM lab_results l WHERE l.ordering_physician_id = p.id)
                + (SELECT COUNT(*) FROM procedures pr WHERE pr.performing_professional_id = p.id)
                + (SELECT COUNT(*) FROM appointments a WHERE a.professional_id = p.id)
                + (SELECT COUNT(*) FROM documents d WHERE d.professional_id = p.id)
                + (SELECT COUNT(*) FROM diagnoses dg WHERE dg.diagnosing_professional_id = p.id)
                AS event_count
         FROM professionals p
         GROUP BY p.id
         HAVING event_count > 0
         ORDER BY event_count DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(ProfessionalSummary {
            id: row.get::<_, String>("id")?,
            name: row.get("name")?,
            specialty: row.get("specialty")?,
            event_count: row.get("event_count")?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::from)
}

/// Top-level assembly: assembles all timeline data in a single call.
pub fn get_timeline_data(
    conn: &Connection,
    filter: &TimelineFilter,
) -> Result<TimelineData, DatabaseError> {
    let events = assemble_timeline_events(conn, filter)?;

    let mut correlations = detect_correlations(&events);
    let explicit = fetch_explicit_correlations(conn)?;
    correlations.extend(explicit);

    // Deduplicate (same source+target pair)
    correlations.sort_by(|a, b| (&a.source_id, &a.target_id).cmp(&(&b.source_id, &b.target_id)));
    correlations.dedup_by(|a, b| a.source_id == b.source_id && a.target_id == b.target_id);

    let date_range = DateRange {
        earliest: events.first().map(|e| e.date.clone()),
        latest: events.last().map(|e| e.date.clone()),
    };

    let event_counts = compute_event_counts(conn)?;
    let professionals = fetch_professionals_with_counts(conn)?;

    Ok(TimelineData {
        events,
        correlations,
        date_range,
        event_counts,
        professionals,
    })
}
