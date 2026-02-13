// M1-03: Cache stores — reactive stores backed by cached health data
import { writable, derived } from 'svelte/store';
import type {
	CachedMedication,
	CachedLabResult,
	CachedTimelineEvent,
	CachedAlert,
	CachedAppointment,
	CachedProfile,
	ScheduleGroup
} from '$lib/types/viewer.js';

// --- Core cache stores ---

/** All medications (active + discontinued) */
export const medications = writable<CachedMedication[]>([]);

/** All lab results */
export const labResults = writable<CachedLabResult[]>([]);

/** Timeline events (last 30 from cache) */
export const timelineEvents = writable<CachedTimelineEvent[]>([]);

/** Active alerts */
export const activeAlerts = writable<CachedAlert[]>([]);

/** Next upcoming appointment */
export const nextAppointment = writable<CachedAppointment | null>(null);

/** Patient profile */
export const profile = writable<CachedProfile | null>(null);

/** Last sync timestamp (ISO 8601) */
export const lastSyncTimestamp = writable<string | null>(null);

// --- Derived stores ---

/** Medications grouped by schedule (Dr. Diallo: "That's how pill boxes work") */
export const medicationsBySchedule = derived(medications, ($meds) => {
	const groups: Record<ScheduleGroup | 'discontinued', CachedMedication[]> = {
		morning: [],
		evening: [],
		as_needed: [],
		multiple: [],
		discontinued: []
	};

	for (const med of $meds) {
		if (!med.isActive) {
			groups.discontinued.push(med);
		} else {
			groups[med.scheduleGroup].push(med);
		}
	}

	return groups;
});

/** Active medication count */
export const activeMedicationCount = derived(medications, ($meds) =>
	$meds.filter((m) => m.isActive).length
);

/** Discontinued medication count */
export const discontinuedMedicationCount = derived(medications, ($meds) =>
	$meds.filter((m) => !m.isActive).length
);

/** Abnormal lab results (shown in top banner) */
export const abnormalLabs = derived(labResults, ($labs) =>
	$labs.filter((l) => l.isAbnormal)
);

/** Lab results sorted by date (most recent first — Dr. Diallo: "I want to see what's new") */
export const labResultsSorted = derived(labResults, ($labs) =>
	[...$labs].sort((a, b) => new Date(b.testedAt).getTime() - new Date(a.testedAt).getTime())
);

/** Undismissed alert count */
export const activeAlertCount = derived(activeAlerts, ($alerts) =>
	$alerts.filter((a) => !a.dismissed).length
);

// --- Store management ---

/** Load all cached data into stores */
export function loadCacheData(data: {
	medications: CachedMedication[];
	labResults: CachedLabResult[];
	timelineEvents: CachedTimelineEvent[];
	alerts: CachedAlert[];
	appointment: CachedAppointment | null;
	profile: CachedProfile | null;
	syncTimestamp: string | null;
}): void {
	medications.set(data.medications);
	labResults.set(data.labResults);
	timelineEvents.set(data.timelineEvents);
	activeAlerts.set(data.alerts);
	nextAppointment.set(data.appointment);
	profile.set(data.profile);
	lastSyncTimestamp.set(data.syncTimestamp);
}

/** Clear all cache stores (for logout/reset) */
export function clearCacheStores(): void {
	medications.set([]);
	labResults.set([]);
	timelineEvents.set([]);
	activeAlerts.set([]);
	nextAppointment.set(null);
	profile.set(null);
	lastSyncTimestamp.set(null);
}
