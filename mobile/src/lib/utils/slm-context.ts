// M2-01: SLM Context Assembly — cache → structured prompt, sanitization
// CA-08: Property names aligned with desktop source of truth (viewer.ts CA-05)
import type { CachedMedication, CachedLabResult, CachedTimelineEvent, CachedAlert, CachedAppointment, CachedProfile } from '$lib/types/viewer.js';
import type { CacheScope } from '$lib/types/slm.js';
import { SLM_SYSTEM_PROMPT } from '$lib/types/slm.js';

// === SANITIZATION ===

/** Sanitize cached values before injecting into SLM prompt (prevent prompt injection) */
export function sanitizeForContext(text: string): string {
	return text
		.replace(/[\x00-\x09\x0B-\x1F\x7F]/g, '')
		.replace(/\b(system|assistant|user|human):/gi, '')
		.replace(/\s{3,}/g, '  ')
		.slice(0, 500);
}

/** Sanitize user query before including in prompt */
export function sanitizeQuery(query: string): string {
	return query
		.replace(/[\x00-\x09\x0B-\x1F\x7F]/g, '')
		.replace(/\b(system|assistant|user|human):/gi, '')
		.trim()
		.slice(0, 300);
}

// === FORMATTING ===

/** Format medications for SLM context */
export function formatMedications(meds: CachedMedication[]): string {
	if (meds.length === 0) return '';
	const lines = ['CURRENT MEDICATIONS:'];
	for (const m of meds) {
		const prescriber = m.prescriberName ? ` (prescribed by ${sanitizeForContext(m.prescriberName)})` : '';
		lines.push(`- ${sanitizeForContext(m.genericName)} ${sanitizeForContext(m.dose)}, ${sanitizeForContext(m.frequency)}${prescriber}`);
	}
	return lines.join('\n');
}

/** Format lab results for SLM context */
export function formatLabs(labs: CachedLabResult[]): string {
	if (labs.length === 0) return '';
	const lines = ['RECENT LAB RESULTS:'];
	for (const l of labs) {
		const flag = l.isAbnormal ? ' [ABNORMAL]' : '';
		const low = l.referenceRangeLow ?? '?';
		const high = l.referenceRangeHigh ?? '?';
		const unit = l.unit ?? '';
		const range = `(range: ${low}-${high} ${unit})`;
		const value = l.value != null ? String(l.value) : (l.valueText ?? 'N/A');
		lines.push(`- ${sanitizeForContext(l.testName)}: ${value} ${unit} ${range}${flag} — ${l.collectionDate}`);
	}
	return lines.join('\n');
}

/** Format timeline events for SLM context */
export function formatTimeline(events: CachedTimelineEvent[]): string {
	if (events.length === 0) return '';
	const lines = ['RECENT TIMELINE:'];
	for (const e of events) {
		lines.push(`- ${e.date}: ${e.eventType} — ${sanitizeForContext(e.description)}`);
	}
	return lines.join('\n');
}

/** Format alerts for SLM context */
export function formatAlerts(alerts: CachedAlert[]): string {
	if (alerts.length === 0) return '';
	const lines = ['ACTIVE ALERTS:'];
	for (const a of alerts) {
		lines.push(`- [${a.severity}] ${sanitizeForContext(a.title)}: ${sanitizeForContext(a.description)}`);
	}
	return lines.join('\n');
}

/** Format appointment for SLM context */
export function formatAppointment(appt: CachedAppointment): string {
	return [
		'NEXT APPOINTMENT:',
		`- Date: ${appt.date}`,
		`- Doctor: ${sanitizeForContext(appt.professionalName)}`,
		appt.appointmentType ? `- Type: ${sanitizeForContext(appt.appointmentType)}` : ''
	].filter(Boolean).join('\n');
}

/** Format profile for SLM context */
export function formatProfile(profile: CachedProfile): string {
	const lines = ['PATIENT PROFILE:'];
	lines.push(`- Name: ${sanitizeForContext(profile.profileName)}`);
	if (profile.allergies.length > 0) {
		lines.push(`- Allergies: ${profile.allergies.map((a) => sanitizeForContext(a.allergen)).join(', ')}`);
	}
	return lines.join('\n');
}

/** Format sync age for SLM prompt */
export function formatSyncAge(syncTimestamp: string | null, now?: number): string {
	if (!syncTimestamp) return 'never (no sync data available)';

	const elapsed = (now ?? Date.now()) - new Date(syncTimestamp).getTime();
	const minutes = Math.floor(elapsed / 60_000);

	if (minutes < 1) return 'just now';
	if (minutes < 60) return `${minutes} minute${minutes !== 1 ? 's' : ''} ago`;

	const hours = Math.floor(minutes / 60);
	if (hours < 24) return `${hours} hour${hours !== 1 ? 's' : ''} ago`;

	const days = Math.floor(hours / 24);
	return `${days} day${days !== 1 ? 's' : ''} ago`;
}

// === PROMPT ASSEMBLY ===

export interface CacheData {
	medications: CachedMedication[];
	labs: CachedLabResult[];
	timeline: CachedTimelineEvent[];
	alerts: CachedAlert[];
	appointment: CachedAppointment | null;
	profile: CachedProfile | null;
	syncTimestamp: string | null;
}

/** Assemble the full SLM prompt from cache data, scope, and user query */
export function assemblePrompt(
	query: string,
	data: CacheData,
	scope: CacheScope
): string {
	const sections: string[] = [];

	if (scope.medications) {
		const activeMeds = data.medications.filter((m) => m.status === 'active');
		const block = formatMedications(activeMeds);
		if (block) sections.push(block);
	}

	if (scope.labs) {
		const block = formatLabs(data.labs);
		if (block) sections.push(block);
	}

	if (scope.timeline) {
		const block = formatTimeline(data.timeline.slice(0, 10));
		if (block) sections.push(block);
	}

	if (scope.alerts) {
		const activeAlerts = data.alerts.filter((a) => !a.dismissed);
		const block = formatAlerts(activeAlerts);
		if (block) sections.push(block);
	}

	if (scope.appointment && data.appointment) {
		sections.push(formatAppointment(data.appointment));
	}

	if (scope.profile && data.profile) {
		sections.push(formatProfile(data.profile));
	}

	const syncAge = formatSyncAge(data.syncTimestamp);
	const context = sections.length > 0
		? sections.join('\n\n')
		: 'No health data available in cache.';

	return [
		SLM_SYSTEM_PROMPT,
		'',
		`DATA FRESHNESS: Last synced ${syncAge}.`,
		'',
		context,
		'',
		`User: ${sanitizeQuery(query)}`,
		'',
		'Assistant:'
	].join('\n');
}

/** Estimate token count for a prompt (rough: ~4 chars per token) */
export function estimateTokenCount(text: string): number {
	return Math.ceil(text.length / 4);
}
