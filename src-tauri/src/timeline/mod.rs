//! L4-04: Timeline View — chronological visualization of the patient's medical journey.
//!
//! Assembles events from ALL entity tables (medications, dose_changes, lab_results,
//! symptoms, procedures, appointments, documents, diagnoses) into a unified
//! `Vec<TimelineEvent>`, sorted by date. Detects temporal correlations between
//! symptom onset and medication changes. Returns everything in a single payload.

mod aggregates;
mod correlations;
mod fetch;
mod types;

pub use aggregates::*;
pub use correlations::*;
pub use types::*;

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory_database;
    use rusqlite::{params, Connection};

    fn setup_db() -> Connection {
        open_memory_database().expect("Failed to open test DB")
    }

    fn insert_professional(conn: &Connection, id: &str, name: &str, specialty: &str) {
        conn.execute(
            "INSERT INTO professionals (id, name, specialty) VALUES (?1, ?2, ?3)",
            params![id, name, specialty],
        )
        .unwrap();
    }

    fn insert_document(conn: &Connection, id: &str, title: &str, date: &str, prof_id: Option<&str>) {
        conn.execute(
            "INSERT INTO documents (id, type, title, document_date, ingestion_date, source_file, professional_id)
             VALUES (?1, 'clinical_note', ?2, ?3, ?3, 'test.pdf', ?4)",
            params![id, title, date, prof_id],
        )
        .unwrap();
    }

    fn insert_medication(
        conn: &Connection,
        id: &str,
        generic: &str,
        dose: &str,
        start: &str,
        end: Option<&str>,
        status: &str,
        doc_id: &str,
        prescriber: Option<&str>,
    ) {
        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, start_date, end_date, document_id, prescriber_id)
             VALUES (?1, ?2, ?3, 'daily', 'scheduled', ?4, ?5, ?6, ?7, ?8)",
            params![id, generic, dose, status, start, end, doc_id, prescriber],
        )
        .unwrap();
    }

    // ── Assembly Tests ─────────────────────────────────────────────────

    #[test]
    fn test_assemble_empty_database() {
        let conn = setup_db();
        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_assemble_medications_start() {
        let conn = setup_db();
        insert_professional(&conn, "prof-1", "Dr. Chen", "Endocrinology");
        insert_document(&conn, "doc-1", "Prescription", "2026-01-15", Some("prof-1"));
        insert_medication(&conn, "med-1", "Metformin", "500mg", "2026-01-15", None, "active", "doc-1", Some("prof-1"));

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        assert_eq!(events.len(), 2); // 1 med start + 1 document
        let med = events.iter().find(|e| e.event_type == EventType::MedicationStart).unwrap();
        assert_eq!(med.title, "Started Metformin");
        assert_eq!(med.date, "2026-01-15");
        assert_eq!(med.professional_name.as_deref(), Some("Dr. Chen"));
    }

    #[test]
    fn test_assemble_medications_stop() {
        let conn = setup_db();
        insert_document(&conn, "doc-1", "Note", "2026-01-01", None);
        insert_medication(&conn, "med-1", "Aspirin", "100mg", "2026-01-01", Some("2026-02-01"), "stopped", "doc-1", None);

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        let stops: Vec<_> = events.iter().filter(|e| e.event_type == EventType::MedicationStop).collect();
        assert_eq!(stops.len(), 1);
        assert_eq!(stops[0].title, "Stopped Aspirin");
        assert_eq!(stops[0].date, "2026-02-01");
    }

    #[test]
    fn test_assemble_dose_changes() {
        let conn = setup_db();
        insert_document(&conn, "doc-1", "Note", "2026-01-01", None);
        insert_medication(&conn, "med-1", "Metformin", "500mg", "2026-01-01", None, "active", "doc-1", None);

        conn.execute(
            "INSERT INTO dose_changes (id, medication_id, old_dose, new_dose, change_date)
             VALUES ('dc-1', 'med-1', '500mg', '1000mg', '2026-02-01')",
            [],
        ).unwrap();

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        let dcs: Vec<_> = events.iter().filter(|e| e.event_type == EventType::MedicationDoseChange).collect();
        assert_eq!(dcs.len(), 1);
        assert_eq!(dcs[0].title, "Metformin dose changed");
    }

    #[test]
    fn test_assemble_lab_results() {
        let conn = setup_db();
        insert_document(&conn, "doc-1", "Lab Report", "2026-01-10", None);

        conn.execute(
            "INSERT INTO lab_results (id, test_name, value, unit, abnormal_flag, collection_date, document_id)
             VALUES ('lab-1', 'HbA1c', 6.5, '%', 'high', '2026-01-10', 'doc-1')",
            [],
        ).unwrap();

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        let labs: Vec<_> = events.iter().filter(|e| e.event_type == EventType::LabResult).collect();
        assert_eq!(labs.len(), 1);
        assert_eq!(labs[0].title, "HbA1c");
        assert_eq!(labs[0].severity, Some(EventSeverity::High));
    }

    #[test]
    fn test_assemble_symptoms() {
        let conn = setup_db();

        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, source)
             VALUES ('sym-1', 'Digestive', 'Nausea', 3, '2026-01-20', '2026-01-20', 'patient_reported')",
            [],
        ).unwrap();

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        let syms: Vec<_> = events.iter().filter(|e| e.event_type == EventType::Symptom).collect();
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].title, "Nausea");
        assert_eq!(syms[0].severity, Some(EventSeverity::Moderate));
    }

    #[test]
    fn test_assemble_procedures() {
        let conn = setup_db();
        insert_document(&conn, "doc-1", "Report", "2026-01-05", None);

        conn.execute(
            "INSERT INTO procedures (id, name, date, facility, document_id)
             VALUES ('proc-1', 'Blood Draw', '2026-01-05', 'City Lab', 'doc-1')",
            [],
        ).unwrap();

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        let procs: Vec<_> = events.iter().filter(|e| e.event_type == EventType::Procedure).collect();
        assert_eq!(procs.len(), 1);
        assert_eq!(procs[0].title, "Blood Draw");
    }

    #[test]
    fn test_assemble_appointments() {
        let conn = setup_db();
        insert_professional(&conn, "prof-1", "Dr. Smith", "Cardiology");

        conn.execute(
            "INSERT INTO appointments (id, professional_id, date, type)
             VALUES ('appt-1', 'prof-1', '2026-01-25', 'completed')",
            [],
        ).unwrap();

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        let appts: Vec<_> = events.iter().filter(|e| e.event_type == EventType::Appointment).collect();
        assert_eq!(appts.len(), 1);
        assert_eq!(appts[0].title, "Visit with Dr. Smith");
    }

    #[test]
    fn test_assemble_documents() {
        let conn = setup_db();
        insert_document(&conn, "doc-1", "Lab Report", "2026-01-10", None);

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        let docs: Vec<_> = events.iter().filter(|e| e.event_type == EventType::Document).collect();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].title, "Lab Report");
    }

    #[test]
    fn test_assemble_diagnoses() {
        let conn = setup_db();
        insert_professional(&conn, "prof-1", "Dr. Chen", "Endocrinology");
        insert_document(&conn, "doc-1", "Clinical Note", "2026-01-15", Some("prof-1"));

        conn.execute(
            "INSERT INTO diagnoses (id, name, icd_code, date_diagnosed, status, diagnosing_professional_id, document_id)
             VALUES ('dx-1', 'Type 2 Diabetes', 'E11.9', '2026-01-15', 'active', 'prof-1', 'doc-1')",
            [],
        ).unwrap();

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        let dx: Vec<_> = events.iter().filter(|e| e.event_type == EventType::Diagnosis).collect();
        assert_eq!(dx.len(), 1);
        assert_eq!(dx[0].title, "Type 2 Diabetes");
    }

    #[test]
    fn test_events_sorted_by_date() {
        let conn = setup_db();
        insert_document(&conn, "doc-1", "Note", "2026-01-01", None);
        insert_medication(&conn, "med-1", "Aspirin", "100mg", "2026-02-01", None, "active", "doc-1", None);

        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, source)
             VALUES ('sym-1', 'Pain', 'Headache', 2, '2026-01-15', '2026-01-15', 'patient_reported')",
            [],
        ).unwrap();

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        // Should be sorted: doc (Jan 1), symptom (Jan 15), med (Feb 1)
        assert!(events.len() >= 3);
        for i in 1..events.len() {
            assert!(events[i].date >= events[i - 1].date, "Events not sorted at index {i}");
        }
    }

    // ── Filter Tests ───────────────────────────────────────────────────

    #[test]
    fn test_filter_by_event_type() {
        let conn = setup_db();
        insert_document(&conn, "doc-1", "Note", "2026-01-01", None);
        insert_medication(&conn, "med-1", "Aspirin", "100mg", "2026-01-01", None, "active", "doc-1", None);

        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, source)
             VALUES ('sym-1', 'Pain', 'Headache', 2, '2026-01-15', '2026-01-15', 'patient_reported')",
            [],
        ).unwrap();

        let filter = TimelineFilter {
            event_types: Some(vec![EventType::Symptom]),
            ..Default::default()
        };
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, EventType::Symptom);
    }

    #[test]
    fn test_filter_by_professional() {
        let conn = setup_db();
        insert_professional(&conn, "prof-1", "Dr. Chen", "Endo");
        insert_professional(&conn, "prof-2", "Dr. Smith", "Cardio");
        insert_document(&conn, "doc-1", "Note1", "2026-01-01", Some("prof-1"));
        insert_document(&conn, "doc-2", "Note2", "2026-01-02", Some("prof-2"));
        insert_medication(&conn, "med-1", "Metformin", "500mg", "2026-01-01", None, "active", "doc-1", Some("prof-1"));
        insert_medication(&conn, "med-2", "Lisinopril", "10mg", "2026-01-02", None, "active", "doc-2", Some("prof-2"));

        let filter = TimelineFilter {
            professional_id: Some("prof-1".into()),
            ..Default::default()
        };
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        // Only events with prof-1
        for ev in &events {
            assert_eq!(ev.professional_id.as_deref(), Some("prof-1"));
        }
    }

    #[test]
    fn test_filter_by_date_range() {
        let conn = setup_db();
        insert_document(&conn, "doc-1", "Note", "2026-01-01", None);
        insert_medication(&conn, "med-1", "Aspirin", "100mg", "2026-01-01", None, "active", "doc-1", None);
        insert_medication(&conn, "med-2", "Metformin", "500mg", "2026-03-01", None, "active", "doc-1", None);

        let filter = TimelineFilter {
            date_from: Some("2026-02-01".into()),
            ..Default::default()
        };
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        // Only events on or after Feb 1 should be included
        for ev in &events {
            assert!(ev.date.as_str() >= "2026-02-01", "Event {} has date {} before filter", ev.title, ev.date);
        }
    }

    #[test]
    fn test_since_appointment_resolves_date() {
        let conn = setup_db();
        insert_professional(&conn, "prof-1", "Dr. Chen", "Endo");
        conn.execute(
            "INSERT INTO appointments (id, professional_id, date, type) VALUES ('appt-1', 'prof-1', '2026-01-15', 'completed')",
            [],
        ).unwrap();

        let filter = TimelineFilter {
            since_appointment_id: Some("appt-1".into()),
            ..Default::default()
        };
        // Test resolve_date_bounds indirectly via assemble
        let events = assemble_timeline_events(&conn, &filter).unwrap();
        // The appointment itself should appear (its date Jan 15 is after Dec 16)
        let appts: Vec<_> = events.iter().filter(|e| e.event_type == EventType::Appointment).collect();
        assert_eq!(appts.len(), 1);
    }

    #[test]
    fn test_since_appointment_not_found() {
        let conn = setup_db();
        let filter = TimelineFilter {
            since_appointment_id: Some("nonexistent".into()),
            ..Default::default()
        };
        let result = assemble_timeline_events(&conn, &filter);
        assert!(result.is_err());
    }

    // ── Correlation Tests ──────────────────────────────────────────────

    #[test]
    fn test_detect_correlations_within_window() {
        let events = vec![
            TimelineEvent {
                id: "med-1".into(),
                event_type: EventType::MedicationStart,
                date: "2026-01-10".into(),
                title: "Started Metformin".into(),
                subtitle: None,
                professional_id: None,
                professional_name: None,
                document_id: None,
                severity: None,
                metadata: EventMetadata::Medication {
                    generic_name: "Metformin".into(),
                    brand_name: None,
                    dose: "500mg".into(),
                    frequency: "daily".into(),
                    status: "active".into(),
                    reason: None,
                    route: None,
                    frequency_type: None,
                    is_otc: None,
                    condition: None,
                    administration_instructions: None,
                },
            },
            TimelineEvent {
                id: "sym-1".into(),
                event_type: EventType::Symptom,
                date: "2026-01-15".into(),
                title: "Nausea".into(),
                subtitle: None,
                professional_id: None,
                professional_name: None,
                document_id: None,
                severity: Some(EventSeverity::Moderate),
                metadata: EventMetadata::Symptom {
                    category: "Digestive".into(),
                    specific: "Nausea".into(),
                    severity: 3,
                    body_region: None,
                    still_active: true,
                    duration: None,
                    character: None,
                    aggravating: None,
                    relieving: None,
                    timing_pattern: None,
                    resolved_date: None,
                    notes: None,
                    source: None,
                    related_medication_id: None,
                    related_diagnosis_id: None,
                },
            },
        ];

        let corrs = detect_correlations(&events);
        assert_eq!(corrs.len(), 1);
        assert_eq!(corrs[0].source_id, "sym-1");
        assert_eq!(corrs[0].target_id, "med-1");
        assert_eq!(corrs[0].correlation_type, CorrelationType::SymptomAfterMedicationStart);
        assert!(corrs[0].description.contains("5 day(s)"));
    }

    #[test]
    fn test_detect_correlations_outside_window() {
        let events = vec![
            TimelineEvent {
                id: "med-1".into(),
                event_type: EventType::MedicationStart,
                date: "2026-01-01".into(),
                title: "Started Metformin".into(),
                subtitle: None,
                professional_id: None,
                professional_name: None,
                document_id: None,
                severity: None,
                metadata: EventMetadata::Medication {
                    generic_name: "Metformin".into(),
                    brand_name: None,
                    dose: "500mg".into(),
                    frequency: "daily".into(),
                    status: "active".into(),
                    reason: None,
                    route: None,
                    frequency_type: None,
                    is_otc: None,
                    condition: None,
                    administration_instructions: None,
                },
            },
            TimelineEvent {
                id: "sym-1".into(),
                event_type: EventType::Symptom,
                date: "2026-02-15".into(),
                title: "Nausea".into(),
                subtitle: None,
                professional_id: None,
                professional_name: None,
                document_id: None,
                severity: Some(EventSeverity::Moderate),
                metadata: EventMetadata::Symptom {
                    category: "Digestive".into(),
                    specific: "Nausea".into(),
                    severity: 3,
                    body_region: None,
                    still_active: true,
                    duration: None,
                    character: None,
                    aggravating: None,
                    relieving: None,
                    timing_pattern: None,
                    resolved_date: None,
                    notes: None,
                    source: None,
                    related_medication_id: None,
                    related_diagnosis_id: None,
                },
            },
        ];

        let corrs = detect_correlations(&events);
        assert!(corrs.is_empty(), "Should not detect correlation outside 14-day window");
    }

    #[test]
    fn test_explicit_correlations() {
        let conn = setup_db();
        insert_document(&conn, "doc-1", "Note", "2026-01-01", None);
        insert_medication(&conn, "med-1", "Metformin", "500mg", "2026-01-01", None, "active", "doc-1", None);

        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, source, related_medication_id)
             VALUES ('sym-1', 'Digestive', 'Nausea', 3, '2026-01-20', '2026-01-20', 'patient_reported', 'med-1')",
            [],
        ).unwrap();

        let corrs = fetch_explicit_correlations(&conn).unwrap();
        assert_eq!(corrs.len(), 1);
        assert_eq!(corrs[0].correlation_type, CorrelationType::ExplicitLink);
        assert_eq!(corrs[0].source_id, "sym-1");
        assert_eq!(corrs[0].target_id, "med-1");
    }

    #[test]
    fn test_correlation_deduplication() {
        let conn = setup_db();
        insert_document(&conn, "doc-1", "Note", "2026-01-01", None);
        insert_medication(&conn, "med-1", "Metformin", "500mg", "2026-01-10", None, "active", "doc-1", None);

        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, source, related_medication_id)
             VALUES ('sym-1', 'Digestive', 'Nausea', 3, '2026-01-15', '2026-01-15', 'patient_reported', 'med-1')",
            [],
        ).unwrap();

        let filter = TimelineFilter::default();
        let data = get_timeline_data(&conn, &filter).unwrap();

        // Both temporal and explicit should detect the same pair — but dedup to 1
        let pair_count = data
            .correlations
            .iter()
            .filter(|c| c.source_id == "sym-1" && c.target_id == "med-1")
            .count();
        assert_eq!(pair_count, 1, "Duplicate correlations should be deduped");
    }

    // ── Aggregate Tests ────────────────────────────────────────────────

    #[test]
    fn test_event_counts_all_tables() {
        let conn = setup_db();
        insert_professional(&conn, "prof-1", "Dr. Chen", "Endo");
        insert_document(&conn, "doc-1", "Note", "2026-01-01", None);
        insert_medication(&conn, "med-1", "Aspirin", "100mg", "2026-01-01", None, "active", "doc-1", None);

        conn.execute(
            "INSERT INTO lab_results (id, test_name, abnormal_flag, collection_date, document_id)
             VALUES ('lab-1', 'CBC', 'normal', '2026-01-05', 'doc-1')",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, source)
             VALUES ('sym-1', 'Pain', 'Headache', 2, '2026-01-10', '2026-01-10', 'patient_reported')",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO appointments (id, professional_id, date, type) VALUES ('appt-1', 'prof-1', '2026-01-20', 'completed')",
            [],
        ).unwrap();

        let counts = compute_event_counts(&conn).unwrap();
        assert_eq!(counts.medications, 1); // 1 med start
        assert_eq!(counts.lab_results, 1);
        assert_eq!(counts.symptoms, 1);
        assert_eq!(counts.appointments, 1);
        assert_eq!(counts.documents, 1);
    }

    #[test]
    fn test_professionals_with_counts() {
        let conn = setup_db();
        insert_professional(&conn, "prof-1", "Dr. Chen", "Endo");
        insert_professional(&conn, "prof-2", "Dr. Idle", "Derm");
        insert_document(&conn, "doc-1", "Note", "2026-01-01", Some("prof-1"));
        insert_medication(&conn, "med-1", "Metformin", "500mg", "2026-01-01", None, "active", "doc-1", Some("prof-1"));

        let profs = fetch_professionals_with_counts(&conn).unwrap();

        // prof-1 has events (1 med + 1 doc), prof-2 has none
        assert!(profs.iter().any(|p| p.id == "prof-1"));
        assert!(!profs.iter().any(|p| p.id == "prof-2"), "Prof with 0 events should be excluded");

        let chen = profs.iter().find(|p| p.id == "prof-1").unwrap();
        assert!(chen.event_count > 0);
    }

    // ── Severity Mapping Tests ─────────────────────────────────────────

    #[test]
    fn test_severity_from_lab_flag() {
        assert_eq!(fetch::severity_from_lab_flag("normal"), EventSeverity::Normal);
        assert_eq!(fetch::severity_from_lab_flag("low"), EventSeverity::Low);
        assert_eq!(fetch::severity_from_lab_flag("high"), EventSeverity::High);
        assert_eq!(fetch::severity_from_lab_flag("critical_low"), EventSeverity::Critical);
        assert_eq!(fetch::severity_from_lab_flag("critical_high"), EventSeverity::Critical);
        assert_eq!(fetch::severity_from_lab_flag("unknown"), EventSeverity::Normal);
    }

    #[test]
    fn test_severity_from_symptom() {
        assert_eq!(fetch::severity_from_symptom(1), EventSeverity::Low);
        assert_eq!(fetch::severity_from_symptom(2), EventSeverity::Low);
        assert_eq!(fetch::severity_from_symptom(3), EventSeverity::Moderate);
        assert_eq!(fetch::severity_from_symptom(4), EventSeverity::High);
        assert_eq!(fetch::severity_from_symptom(5), EventSeverity::Critical);
    }

    // ── Type Serialization Test ────────────────────────────────────────

    #[test]
    fn test_event_type_serialization_roundtrip() {
        let types = vec![
            EventType::MedicationStart,
            EventType::MedicationStop,
            EventType::MedicationDoseChange,
            EventType::LabResult,
            EventType::Symptom,
            EventType::Procedure,
            EventType::Appointment,
            EventType::Document,
            EventType::Diagnosis,
            EventType::CoherenceAlert,
            EventType::VitalSign,
        ];

        for t in types {
            let json = serde_json::to_string(&t).unwrap();
            let back: EventType = serde_json::from_str(&json).unwrap();
            assert_eq!(back, t);
        }
    }

    // ── Full Pipeline Test ─────────────────────────────────────────────

    #[test]
    fn test_timeline_data_structure() {
        let conn = setup_db();
        let filter = TimelineFilter::default();
        let data = get_timeline_data(&conn, &filter).unwrap();

        // Empty DB should return valid structure with empty vecs
        assert!(data.events.is_empty());
        assert!(data.correlations.is_empty());
        assert!(data.date_range.earliest.is_none());
        assert!(data.date_range.latest.is_none());
        assert_eq!(data.event_counts.medications, 0);
        assert_eq!(data.event_counts.coherence_alerts, 0);
        assert_eq!(data.event_counts.vital_signs, 0);
        assert!(data.professionals.is_empty());
    }

    // ── LP-08 New Tests ─────────────────────────────────────────────────

    fn insert_coherence_alert(
        conn: &Connection,
        id: &str,
        alert_type: &str,
        severity: &str,
        detected_at: &str,
        dismissed: bool,
    ) {
        conn.execute(
            "INSERT INTO coherence_alerts (id, alert_type, severity, entity_ids, source_document_ids, patient_message, detail_json, detected_at, dismissed, two_step_confirmed)
             VALUES (?1, ?2, ?3, '[\"med-1\"]', '[\"doc-1\"]', 'Check this interaction', '{}', ?4, ?5, 0)",
            params![id, alert_type, severity, detected_at, dismissed as i32],
        )
        .unwrap();
    }

    fn insert_vital_sign(
        conn: &Connection,
        id: &str,
        vital_type: &str,
        value_primary: f64,
        value_secondary: Option<f64>,
        unit: &str,
        recorded_at: &str,
    ) {
        conn.execute(
            "INSERT INTO vital_signs (id, vital_type, value_primary, value_secondary, unit, recorded_at, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'manual')",
            params![id, vital_type, value_primary, value_secondary, unit, recorded_at],
        )
        .unwrap();
    }

    #[test]
    fn test_assemble_coherence_alerts() {
        let conn = setup_db();
        insert_coherence_alert(&conn, "ca-1", "conflict", "critical", "2026-01-15", false);

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        let alerts: Vec<_> = events.iter().filter(|e| e.event_type == EventType::CoherenceAlert).collect();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].severity, Some(EventSeverity::Critical));
        assert_eq!(alerts[0].title, "Medication Conflict");
    }

    #[test]
    fn test_assemble_vital_signs() {
        let conn = setup_db();
        insert_vital_sign(&conn, "vs-1", "blood_pressure", 120.0, Some(80.0), "mmHg", "2026-01-20");

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        let vitals: Vec<_> = events.iter().filter(|e| e.event_type == EventType::VitalSign).collect();
        assert_eq!(vitals.len(), 1);
        assert_eq!(vitals[0].title, "Blood Pressure");
        assert_eq!(vitals[0].subtitle.as_deref(), Some("120/80 mmHg"));
    }

    #[test]
    fn test_symptom_oldcarts_fields() {
        let conn = setup_db();

        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, source,
                duration, character, aggravating, relieving, timing_pattern, notes)
             VALUES ('sym-1', 'Pain', 'Migraine', 4, '2026-01-20', '2026-01-20', 'patient_reported',
                '3 hours', 'throbbing', 'light, noise', 'rest, dark room', 'afternoon', 'occurs after stress')",
            [],
        ).unwrap();

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        let sym = events.iter().find(|e| e.event_type == EventType::Symptom).unwrap();
        if let EventMetadata::Symptom { duration, character, aggravating, relieving, timing_pattern, notes, .. } = &sym.metadata {
            assert_eq!(duration.as_deref(), Some("3 hours"));
            assert_eq!(character.as_deref(), Some("throbbing"));
            assert_eq!(aggravating.as_deref(), Some("light, noise"));
            assert_eq!(relieving.as_deref(), Some("rest, dark room"));
            assert_eq!(timing_pattern.as_deref(), Some("afternoon"));
            assert_eq!(notes.as_deref(), Some("occurs after stress"));
        } else {
            panic!("Expected Symptom metadata");
        }
    }

    #[test]
    fn test_appointment_post_notes() {
        let conn = setup_db();
        insert_professional(&conn, "prof-1", "Dr. Smith", "Cardiology");

        conn.execute(
            "INSERT INTO appointments (id, professional_id, date, type, pre_summary_generated, post_notes)
             VALUES ('appt-1', 'prof-1', '2026-01-25', 'completed', 1, 'Patient doing well')",
            [],
        ).unwrap();

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        let appt = events.iter().find(|e| e.event_type == EventType::Appointment).unwrap();
        if let EventMetadata::Appointment { pre_summary_generated, post_notes, .. } = &appt.metadata {
            assert_eq!(*pre_summary_generated, Some(true));
            assert_eq!(post_notes.as_deref(), Some("Patient doing well"));
        } else {
            panic!("Expected Appointment metadata");
        }
    }

    #[test]
    fn test_medication_route_fields() {
        let conn = setup_db();
        insert_document(&conn, "doc-1", "Rx", "2026-01-01", None);

        conn.execute(
            "INSERT INTO medications (id, generic_name, dose, frequency, frequency_type, status, start_date, document_id, route, is_otc, condition, administration_instructions)
             VALUES ('med-1', 'Ibuprofen', '400mg', 'every 6 hours', 'as_needed', 'active', '2026-01-01', 'doc-1', 'oral', 1, 'headache', 'Take with food')",
            [],
        ).unwrap();

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        let med = events.iter().find(|e| e.event_type == EventType::MedicationStart).unwrap();
        if let EventMetadata::Medication { route, frequency_type, is_otc, condition, administration_instructions, .. } = &med.metadata {
            assert_eq!(route.as_deref(), Some("oral"));
            assert_eq!(frequency_type.as_deref(), Some("as_needed"));
            assert_eq!(*is_otc, Some(true));
            assert_eq!(condition.as_deref(), Some("headache"));
            assert_eq!(administration_instructions.as_deref(), Some("Take with food"));
        } else {
            panic!("Expected Medication metadata");
        }
    }

    #[test]
    fn test_filter_coherence_alerts_dismissed() {
        let conn = setup_db();
        insert_coherence_alert(&conn, "ca-1", "conflict", "critical", "2026-01-15", false);
        insert_coherence_alert(&conn, "ca-2", "dose", "standard", "2026-01-16", true);

        // Default: exclude dismissed
        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();
        let alerts: Vec<_> = events.iter().filter(|e| e.event_type == EventType::CoherenceAlert).collect();
        assert_eq!(alerts.len(), 1);

        // Include dismissed
        let filter_incl = TimelineFilter {
            include_dismissed_alerts: Some(true),
            ..Default::default()
        };
        let events_incl = assemble_timeline_events(&conn, &filter_incl).unwrap();
        let alerts_incl: Vec<_> = events_incl.iter().filter(|e| e.event_type == EventType::CoherenceAlert).collect();
        assert_eq!(alerts_incl.len(), 2);
    }

    #[test]
    fn test_filter_by_new_event_types() {
        let conn = setup_db();
        insert_coherence_alert(&conn, "ca-1", "conflict", "critical", "2026-01-15", false);
        insert_vital_sign(&conn, "vs-1", "heart_rate", 72.0, None, "bpm", "2026-01-20");

        // Filter only VitalSign
        let filter = TimelineFilter {
            event_types: Some(vec![EventType::VitalSign]),
            ..Default::default()
        };
        let events = assemble_timeline_events(&conn, &filter).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, EventType::VitalSign);

        // Filter only CoherenceAlert
        let filter2 = TimelineFilter {
            event_types: Some(vec![EventType::CoherenceAlert]),
            ..Default::default()
        };
        let events2 = assemble_timeline_events(&conn, &filter2).unwrap();
        assert_eq!(events2.len(), 1);
        assert_eq!(events2[0].event_type, EventType::CoherenceAlert);
    }

    #[test]
    fn test_event_counts_with_new_types() {
        let conn = setup_db();
        insert_coherence_alert(&conn, "ca-1", "conflict", "critical", "2026-01-15", false);
        insert_coherence_alert(&conn, "ca-2", "dose", "standard", "2026-01-16", true); // dismissed
        insert_vital_sign(&conn, "vs-1", "heart_rate", 72.0, None, "bpm", "2026-01-20");
        insert_vital_sign(&conn, "vs-2", "blood_pressure", 120.0, Some(80.0), "mmHg", "2026-01-21");

        let counts = compute_event_counts(&conn).unwrap();
        assert_eq!(counts.coherence_alerts, 1); // Only non-dismissed
        assert_eq!(counts.vital_signs, 2);
    }

    #[test]
    fn test_severity_from_alert_level() {
        assert_eq!(fetch::severity_from_alert_level("critical"), EventSeverity::Critical);
        assert_eq!(fetch::severity_from_alert_level("standard"), EventSeverity::Moderate);
        assert_eq!(fetch::severity_from_alert_level("info"), EventSeverity::Low);
        assert_eq!(fetch::severity_from_alert_level("unknown"), EventSeverity::Low);
    }

    #[test]
    fn test_symptom_diagnosis_correlation() {
        let conn = setup_db();
        insert_professional(&conn, "prof-1", "Dr. Chen", "Endo");
        insert_document(&conn, "doc-1", "Note", "2026-01-01", Some("prof-1"));

        conn.execute(
            "INSERT INTO diagnoses (id, name, date_diagnosed, status, diagnosing_professional_id, document_id)
             VALUES ('dx-1', 'Migraine', '2026-01-01', 'active', 'prof-1', 'doc-1')",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO symptoms (id, category, specific, severity, onset_date, recorded_date, source, related_diagnosis_id)
             VALUES ('sym-1', 'Neurological', 'Headache', 4, '2026-01-15', '2026-01-15', 'patient_reported', 'dx-1')",
            [],
        ).unwrap();

        let corrs = fetch_explicit_correlations(&conn).unwrap();
        let diag_links: Vec<_> = corrs.iter()
            .filter(|c| c.correlation_type == CorrelationType::SymptomLinkedToDiagnosis)
            .collect();
        assert_eq!(diag_links.len(), 1);
        assert_eq!(diag_links[0].source_id, "sym-1");
        assert_eq!(diag_links[0].target_id, "dx-1");
        assert!(diag_links[0].description.contains("Headache"));
        assert!(diag_links[0].description.contains("Migraine"));
    }

    #[test]
    fn test_vital_sign_without_secondary() {
        let conn = setup_db();
        insert_vital_sign(&conn, "vs-1", "heart_rate", 72.0, None, "bpm", "2026-01-20");

        let filter = TimelineFilter::default();
        let events = assemble_timeline_events(&conn, &filter).unwrap();

        let vital = events.iter().find(|e| e.event_type == EventType::VitalSign).unwrap();
        assert_eq!(vital.title, "Heart Rate");
        assert_eq!(vital.subtitle.as_deref(), Some("72 bpm"));
    }
}
