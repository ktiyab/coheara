// E2E-B04: Coherence Engine Types

export type AlertSeverity = 'Info' | 'Standard' | 'Critical';

export type AlertType =
	| 'conflict'
	| 'duplicate'
	| 'gap'
	| 'drift'
	| 'temporal'
	| 'allergy'
	| 'dose'
	| 'critical';

export interface AlertCounts {
	conflicts: number;
	duplicates: number;
	gaps: number;
	drifts: number;
	temporals: number;
	allergies: number;
	doses: number;
	criticals: number;
}

export interface CoherenceResult {
	new_alerts: CoherenceAlert[];
	counts: AlertCounts;
	processing_time_ms: number;
}

export interface CoherenceAlert {
	id: string;
	alert_type: AlertType;
	severity: AlertSeverity;
	entity_ids: string[];
	source_document_ids: string[];
	patient_message: string;
	detail: AlertDetail;
	detected_at: string;
	surfaced: boolean;
	dismissed: boolean;
	dismissal: AlertDismissal | null;
}

export interface AlertDismissal {
	dismissed_date: string;
	reason: string;
	dismissed_by: string;
	two_step_confirmed: boolean;
}

// Alert detail variants (tagged union)
export type AlertDetail =
	| { Conflict: ConflictDetail }
	| { Duplicate: DuplicateDetail }
	| { Gap: GapDetail }
	| { Drift: DriftDetail }
	| { Temporal: TemporalDetail }
	| { Allergy: AllergyDetail }
	| { Dose: DoseDetail }
	| { Critical: CriticalDetail };

export interface PrescriberRef {
	professional_id: string;
	name: string;
	document_id: string;
	document_date: string | null;
}

export interface ConflictDetail {
	medication_name: string;
	prescriber_a: PrescriberRef;
	prescriber_b: PrescriberRef;
	field_conflicted: string;
	value_a: string;
	value_b: string;
}

export interface DuplicateDetail {
	generic_name: string;
	brand_a: string;
	brand_b: string;
	medication_id_a: string;
	medication_id_b: string;
}

export interface GapDetail {
	gap_type: 'DiagnosisWithoutTreatment' | 'MedicationWithoutDiagnosis';
	entity_name: string;
	entity_id: string;
	expected: string;
	document_id: string;
}

export interface DriftDetail {
	entity_type: string;
	entity_name: string;
	old_value: string;
	new_value: string;
	change_date: string | null;
	reason_documented: boolean;
}

export interface TemporalDetail {
	symptom_id: string;
	symptom_name: string;
	symptom_onset: string;
	correlated_event: TemporalEvent;
	days_between: number;
}

export type TemporalEvent =
	| {
			MedicationStarted: {
				medication_id: string;
				medication_name: string;
				start_date: string;
			};
		}
	| {
			DoseChanged: {
				medication_id: string;
				medication_name: string;
				old_dose: string;
				new_dose: string;
				change_date: string;
			};
		}
	| {
			ProcedurePerformed: {
				procedure_id: string;
				procedure_name: string;
				procedure_date: string;
			};
		};

export interface AllergyDetail {
	allergen: string;
	allergy_severity: string;
	allergy_id: string;
	medication_name: string;
	medication_id: string;
	matching_ingredient: string;
	ingredient_maps_to: string;
}

export interface DoseDetail {
	medication_name: string;
	medication_id: string;
	extracted_dose: string;
	extracted_dose_mg: number;
	typical_range_low_mg: number;
	typical_range_high_mg: number;
	source: string;
}

export interface CriticalDetail {
	test_name: string;
	lab_result_id: string;
	value: number;
	unit: string;
	abnormal_flag: string;
	reference_range_low: number | null;
	reference_range_high: number | null;
	collection_date: string;
	document_id: string;
}

export type EmergencyActionType = 'LabCritical' | 'AllergyMatch' | 'Other';

export interface EmergencyAction {
	alert_id: string;
	action_type: EmergencyActionType;
	ingestion_message: string;
	home_banner: string;
	appointment_priority: boolean;
	dismissal_steps: number;
	dismissal_prompt_1: string;
	dismissal_prompt_2: string;
}
