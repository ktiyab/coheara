// M1-03: Viewer screen types — aligned with desktop sync.rs source of truth (CA-05)

/** Cached medication from sync — matches desktop CachedMedication */
export interface CachedMedication {
	id: string;
	genericName: string;
	brandName?: string;
	dose: string;
	frequency: string;
	route: string;
	status: string;
	startDate?: string;
	endDate?: string;
	prescriberName?: string;
	condition?: string;
	isOtc: boolean;
}

/** Lab trend direction */
export type LabTrend = 'up' | 'down' | 'stable';

/** Cached lab result from sync — matches desktop CachedLabResult */
export interface CachedLabResult {
	id: string;
	testName: string;
	value?: number;
	valueText?: string;
	unit?: string;
	referenceRangeLow?: number;
	referenceRangeHigh?: number;
	abnormalFlag: string;
	collectionDate: string;
	isAbnormal: boolean;
	trendDirection?: string;
}

/** Lab history entry (for detail/trend view, fetched from desktop) */
export interface LabHistoryEntry {
	value?: number;
	date: string;
	trendDirection?: string;
}

/** Timeline event types */
export type TimelineEventType =
	| 'medication_change'
	| 'lab_result'
	| 'appointment'
	| 'alert'
	| 'document'
	| 'journal';

/** Cached timeline event from sync — matches desktop CachedTimelineEvent */
export interface CachedTimelineEvent {
	id: string;
	eventType: string;
	category: string;
	description: string;
	severity?: number;
	date: string;
	stillActive: boolean;
}

/** Cached alert — matches desktop CachedAlert */
export interface CachedAlert {
	id: string;
	title: string;
	description: string;
	severity: string;
	createdAt: string;
	dismissed: boolean;
}

/** Cached appointment — matches desktop CachedAppointment */
export interface CachedAppointment {
	id: string;
	professionalName: string;
	professionalSpecialty?: string;
	date: string;
	appointmentType: string;
	prepAvailable: boolean;
}

/** Cached allergy — matches desktop CachedAllergy */
export interface CachedAllergy {
	allergen: string;
	severity: string;
	verified: boolean;
}

/** Cached profile — matches desktop CachedProfile (source of truth) */
export interface CachedProfile {
	profileName: string;
	totalDocuments: number;
	extractionAccuracy: number;
	allergies: CachedAllergy[];
}

/** Appointment prep — patient-facing plain language view */
export interface PrepForPatient {
	thingsToMention: string[];
	questionsToConsider: string[];
}

/** Appointment prep — doctor-facing structured view */
export interface PrepForDoctor {
	lastVisitDate?: string;
	medicationChanges: string[];
	labResults: string[];
	patientReportedSymptoms: string[];
	activeAlerts: string[];
}

/** Combined appointment prep data */
export interface AppointmentPrepData {
	appointmentId: string;
	doctorName: string;
	appointmentDate: string;
	forPatient: PrepForPatient;
	forDoctor: PrepForDoctor;
}

/** Share payload (Nadia: "minimize what leaves the phone") */
export interface SharePayload {
	title: string;
	text: string;
	timestamp: string;
	disclaimer: string;
}

/** Freshness level based on sync timestamp */
export type FreshnessLevel = 'fresh' | 'recent' | 'stale' | 'old';

/** Freshness thresholds */
export const FRESHNESS_THRESHOLDS = {
	FRESH_MS: 15 * 60 * 1000,      // <15 min
	RECENT_MS: 60 * 60 * 1000,     // <1 hour
	STALE_MS: 24 * 60 * 60 * 1000, // <24 hours
} as const;

/** Medication detail (enriched from desktop) */
export interface MedicationDetail extends CachedMedication {
	history: Array<{ date: string; event: string }>;
}

/** Timeline date group */
export interface TimelineDateGroup {
	label: string;
	date: string;
	events: CachedTimelineEvent[];
}

/** Timeline filter options */
export type TimelineFilter = 'all' | TimelineEventType;

/** Medication search fields */
export const MEDICATION_SEARCH_FIELDS = [
	'genericName', 'brandName', 'dose', 'prescriberName', 'condition'
] as const;
