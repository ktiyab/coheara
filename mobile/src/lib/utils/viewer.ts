// M1-03: Viewer utility functions — search, freshness, share, timeline grouping, lab trends
import type {
	CachedMedication,
	CachedLabResult,
	CachedTimelineEvent,
	CachedAppointment,
	FreshnessLevel,
	SharePayload,
	TimelineDateGroup,
	TimelineFilter,
	LabTrend,
	AppointmentPrepData
} from '$lib/types/viewer.js';
import { FRESHNESS_THRESHOLDS } from '$lib/types/viewer.js';

// --- Medication search (Viktor: "simple in-memory filter, instant") ---

/** Search medications by generic name, brand name, dose, prescriber, condition */
export function searchMedications(
	medications: CachedMedication[],
	query: string
): CachedMedication[] {
	if (!query.trim()) return medications;

	const lower = query.toLowerCase();
	return medications.filter((med) =>
		med.genericName.toLowerCase().includes(lower) ||
		(med.brandName?.toLowerCase().includes(lower) ?? false) ||
		med.dose.toLowerCase().includes(lower) ||
		(med.prescriberName?.toLowerCase().includes(lower) ?? false) ||
		(med.condition?.toLowerCase().includes(lower) ?? false)
	);
}

// --- Freshness indicator ---

/** Compute freshness level from sync timestamp */
export function computeFreshness(syncTimestamp: string | null, now?: number): FreshnessLevel {
	if (!syncTimestamp) return 'old';

	const elapsed = (now ?? Date.now()) - new Date(syncTimestamp).getTime();

	if (elapsed < FRESHNESS_THRESHOLDS.FRESH_MS) return 'fresh';
	if (elapsed < FRESHNESS_THRESHOLDS.RECENT_MS) return 'recent';
	if (elapsed < FRESHNESS_THRESHOLDS.STALE_MS) return 'stale';
	return 'old';
}

/** Freshness display label */
export function freshnessLabel(syncTimestamp: string | null, now?: number): string {
	if (!syncTimestamp) return 'Not synced';

	const elapsed = (now ?? Date.now()) - new Date(syncTimestamp).getTime();
	const minutes = Math.floor(elapsed / 60_000);

	if (minutes < 1) return 'Synced just now';
	if (minutes < 60) return `Synced ${minutes}m ago`;

	const hours = Math.floor(minutes / 60);
	if (hours < 24) return `Synced ${hours}h ago`;

	const days = Math.floor(hours / 24);
	return `Synced ${days}d ago`;
}

/** Freshness CSS color variable */
export function freshnessColor(level: FreshnessLevel): string {
	switch (level) {
		case 'fresh': return 'var(--color-success)';
		case 'recent': return 'var(--color-text-muted)';
		case 'stale': return 'var(--color-warning)';
		case 'old': return 'var(--color-error)';
	}
}

// --- Lab trend display ---

/** Get trend arrow character */
export function trendArrow(trend: LabTrend): string {
	switch (trend) {
		case 'up': return '\u2191';     // ↑
		case 'down': return '\u2193';   // ↓
		case 'stable': return '\u2192'; // →
	}
}

/** Get trend display label */
export function trendLabel(trend: LabTrend): string {
	switch (trend) {
		case 'up': return 'Rising';
		case 'down': return 'Falling';
		case 'stable': return 'Stable';
	}
}

/** Get trend color for display (abnormal context from lab's isAbnormal) */
export function trendColor(trend: LabTrend, isAbnormal: boolean): string {
	if (trend === 'stable') return 'var(--color-text-muted)';
	// Rising/falling on abnormal = warning; on normal = neutral
	if (isAbnormal) return 'var(--color-error)';
	return 'var(--color-text-muted)';
}

// --- Timeline grouping ---

/** Group timeline events by date */
export function groupTimelineByDate(
	events: CachedTimelineEvent[],
	now?: Date
): TimelineDateGroup[] {
	const today = now ?? new Date();
	const todayStr = formatDateKey(today);

	const yesterday = new Date(today);
	yesterday.setDate(yesterday.getDate() - 1);
	const yesterdayStr = formatDateKey(yesterday);

	const groups = new Map<string, CachedTimelineEvent[]>();

	// Sort events by date descending (most recent first)
	const sorted = [...events].sort(
		(a, b) => new Date(b.date).getTime() - new Date(a.date).getTime()
	);

	for (const event of sorted) {
		const dateKey = formatDateKey(new Date(event.date));
		const existing = groups.get(dateKey);
		if (existing) {
			existing.push(event);
		} else {
			groups.set(dateKey, [event]);
		}
	}

	const result: TimelineDateGroup[] = [];
	for (const [dateKey, groupEvents] of groups) {
		let label: string;
		if (dateKey === todayStr) {
			label = 'Today';
		} else if (dateKey === yesterdayStr) {
			label = 'Yesterday';
		} else {
			label = formatDisplayDate(dateKey);
		}

		result.push({ label, date: dateKey, events: groupEvents });
	}

	return result;
}

/** Filter timeline events by type */
export function filterTimelineEvents(
	events: CachedTimelineEvent[],
	filter: TimelineFilter
): CachedTimelineEvent[] {
	if (filter === 'all') return events;
	return events.filter((e) => e.eventType === filter);
}

/** Get timeline event type icon text */
export function timelineEventIcon(eventType: string): string {
	switch (eventType) {
		case 'medication_change': return 'Pill';
		case 'lab_result': return 'Lab';
		case 'appointment': return 'Cal';
		case 'alert': return 'Alert';
		case 'document': return 'Doc';
		case 'journal': return 'Note';
		default: return 'Event';
	}
}

/** Get timeline event type background color */
export function timelineEventColor(eventType: string): string {
	switch (eventType) {
		case 'medication_change': return 'var(--color-primary)';
		case 'lab_result': return '#7C3AED';
		case 'appointment': return 'var(--color-success)';
		case 'alert': return 'var(--color-warning)';
		case 'document': return 'var(--color-text-muted)';
		case 'journal': return 'var(--color-accent)';
		default: return 'var(--color-text-muted)';
	}
}

// --- Share text generation (Nadia: "reduced subset") ---

const SHARE_DISCLAIMER = 'This summary is for reference. Confirm with your healthcare team.';

/** Generate share payload for medication list */
export function shareMedicationList(
	medications: CachedMedication[],
	profileName: string,
	syncTimestamp: string | null
): SharePayload {
	const active = medications.filter((m) => m.status === 'active');
	const lines: string[] = [];

	for (const med of active) {
		lines.push(`${med.genericName} ${med.dose} - ${med.frequency}`);
	}

	return {
		title: `Medication List \u2014 ${profileName}`,
		text: lines.join('\n').trim(),
		timestamp: syncTimestamp
			? `Generated from data synced ${formatTimestamp(syncTimestamp)}`
			: 'Generated from cached data',
		disclaimer: SHARE_DISCLAIMER
	};
}

/** Generate share payload for lab summary */
export function shareLabSummary(
	labs: CachedLabResult[],
	profileName: string,
	syncTimestamp: string | null
): SharePayload {
	const lines = labs.map((lab) => {
		const value = lab.valueText ?? String(lab.value ?? '');
		const ref = (lab.referenceRangeLow != null && lab.referenceRangeHigh != null)
			? ` (ref: ${lab.referenceRangeLow}-${lab.referenceRangeHigh})`
			: '';
		const trend = lab.trendDirection ? ` ${trendArrow(lab.trendDirection as LabTrend)}` : '';
		return `${lab.testName}: ${value} ${lab.unit ?? ''}${ref}${trend}`;
	});

	return {
		title: `Lab Summary \u2014 ${profileName}`,
		text: lines.join('\n'),
		timestamp: syncTimestamp
			? `Generated from data synced ${formatTimestamp(syncTimestamp)}`
			: 'Generated from cached data',
		disclaimer: SHARE_DISCLAIMER
	};
}

/** Generate share payload for appointment prep */
export function shareAppointmentPrep(
	prep: AppointmentPrepData,
	view: 'patient' | 'doctor',
	syncTimestamp: string | null
): SharePayload {
	let text: string;

	if (view === 'patient') {
		const mention = prep.forPatient.thingsToMention.map((t) => `- ${t}`).join('\n');
		const questions = prep.forPatient.questionsToConsider.map((q) => `- ${q}`).join('\n');
		text = `Things to mention:\n${mention}\n\nQuestions to consider:\n${questions}`;
	} else {
		const parts: string[] = [];
		if (prep.forDoctor.lastVisitDate) {
			parts.push(`Changes since last visit (${prep.forDoctor.lastVisitDate}):`);
		}
		if (prep.forDoctor.medicationChanges.length > 0) {
			parts.push(`\nMedications:\n${prep.forDoctor.medicationChanges.map((c) => `- ${c}`).join('\n')}`);
		}
		if (prep.forDoctor.labResults.length > 0) {
			parts.push(`\nLab Results:\n${prep.forDoctor.labResults.map((l) => `- ${l}`).join('\n')}`);
		}
		if (prep.forDoctor.patientReportedSymptoms.length > 0) {
			parts.push(`\nPatient-Reported Symptoms:\n${prep.forDoctor.patientReportedSymptoms.map((s) => `- ${s}`).join('\n')}`);
		}
		if (prep.forDoctor.activeAlerts.length > 0) {
			parts.push(`\nActive Alerts:\n${prep.forDoctor.activeAlerts.map((a) => `- ${a}`).join('\n')}`);
		}
		text = parts.join('\n');
	}

	return {
		title: `Appointment Prep \u2014 ${prep.doctorName} (${prep.appointmentDate})`,
		text,
		timestamp: syncTimestamp
			? `Generated from data synced ${formatTimestamp(syncTimestamp)}`
			: 'Generated from cached data',
		disclaimer: SHARE_DISCLAIMER
	};
}

/** Generate full share text from payload */
export function formatShareText(payload: SharePayload): string {
	return `${payload.title}\n\n${payload.text}\n\n${payload.timestamp}\n${payload.disclaimer}`;
}

// --- Empty state messages ---

export function emptyStateMessage(screen: 'medications' | 'labs' | 'timeline' | 'appointments'): string {
	switch (screen) {
		case 'medications':
			return 'No medications found. Import prescriptions on your desktop to see them here.';
		case 'labs':
			return 'No lab results available. Import lab reports on your desktop to see them here.';
		case 'timeline':
			return 'No events yet. Your timeline will build as you import documents and log journal entries.';
		case 'appointments':
			return 'No upcoming appointments. Add appointments on your desktop.';
	}
}

// --- Helpers ---

function formatDateKey(date: Date): string {
	return date.toISOString().split('T')[0];
}

function formatDisplayDate(dateKey: string): string {
	const date = new Date(dateKey + 'T00:00:00');
	return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
}

function formatTimestamp(isoTimestamp: string): string {
	const d = new Date(isoTimestamp);
	return d.toLocaleDateString('en-US', {
		month: 'short', day: 'numeric', year: 'numeric',
		hour: 'numeric', minute: '2-digit'
	});
}
