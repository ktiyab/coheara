// M1-03: Viewer screen types — medications, labs, timeline, appointments

/** Schedule group for medications (Dr. Diallo: "That's how pill boxes work") */
export type ScheduleGroup = 'morning' | 'evening' | 'as_needed' | 'multiple';

/** Cached medication from sync */
export interface CachedMedication {
	id: string;
	name: string;
	genericName?: string;
	dose: string;
	frequency: string;
	prescriber: string;
	purpose: string;
	scheduleGroup: ScheduleGroup;
	since: string;
	isActive: boolean;
	discontinuedDate?: string;
	discontinuedReason?: string;
	notes?: string;
	sourceDocumentTitle?: string;
}

/** Lab trend direction */
export type LabTrend = 'up' | 'down' | 'stable' | 'first';

/** Lab trend clinical context (Dr. Diallo: "color should reflect clinical meaning") */
export type LabTrendContext =
	| 'worsening'    // up + abnormal OR down + was normal
	| 'improving'    // down + was abnormal OR up + was low
	| 'approaching'  // moving toward limit but still normal
	| 'stable'       // no significant change
	| 'first';       // no comparison available

/** Cached lab result from sync */
export interface CachedLabResult {
	id: string;
	testName: string;
	value: number;
	unit: string;
	referenceMin: number;
	referenceMax: number;
	isAbnormal: boolean;
	trend: LabTrend;
	trendContext: LabTrendContext;
	testedAt: string;
	labName?: string;
	previousValue?: number;
	previousDate?: string;
}

/** Lab history entry (for detail/trend view, fetched from desktop) */
export interface LabHistoryEntry {
	value: number;
	date: string;
	trend: LabTrend;
}

/** Timeline event types */
export type TimelineEventType =
	| 'medication_change'
	| 'lab_result'
	| 'appointment'
	| 'alert'
	| 'document'
	| 'journal';

/** Cached timeline event from sync */
export interface CachedTimelineEvent {
	id: string;
	eventType: TimelineEventType;
	title: string;
	description: string;
	timestamp: string;
	severity?: 'info' | 'warning' | 'critical';
	isPatientReported: boolean;
	metadata?: Record<string, string>;
}

/** Cached alert */
export interface CachedAlert {
	id: string;
	title: string;
	description: string;
	severity: 'info' | 'warning' | 'critical';
	createdAt: string;
	dismissed: boolean;
}

/** Cached appointment */
export interface CachedAppointment {
	id: string;
	doctorName: string;
	date: string;
	location?: string;
	purpose?: string;
	hasPrepData: boolean;
}

/** Emergency contact (from SyncProfile) */
export interface EmergencyContact {
	name: string;
	phone: string;
	relation: string;
}

/** Cached profile */
export interface CachedProfile {
	name: string;
	bloodType?: string;
	allergies: string[];
	dateOfBirth?: string;
	emergencyContacts: EmergencyContact[];
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
	'name', 'genericName', 'dose', 'prescriber', 'purpose'
] as const;
