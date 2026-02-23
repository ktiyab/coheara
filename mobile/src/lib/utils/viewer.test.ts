// M1-03: Viewer utility tests — aligned CA-05 desktop types
import { describe, it, expect } from 'vitest';
import {
	searchMedications,
	computeFreshness,
	freshnessLabel,
	freshnessColor,
	trendArrow,
	trendLabel,
	trendColor,
	groupTimelineByDate,
	filterTimelineEvents,
	timelineEventIcon,
	timelineEventColor,
	shareMedicationList,
	shareLabSummary,
	shareAppointmentPrep,
	formatShareText,
	emptyStateMessage
} from './viewer.js';
import type {
	CachedMedication,
	CachedLabResult,
	CachedTimelineEvent,
	AppointmentPrepData
} from '$lib/types/viewer.js';
import { FRESHNESS_THRESHOLDS } from '$lib/types/viewer.js';

// --- Test data factories (aligned CA-05 desktop types) ---

function makeMed(overrides: Partial<CachedMedication> = {}): CachedMedication {
	return {
		id: 'med-1', genericName: 'Metformin', dose: '500mg', frequency: 'Twice daily',
		route: 'oral', status: 'active', isOtc: false,
		prescriberName: 'Dr. Chen', condition: 'For blood sugar',
		startDate: '2024-01-15', ...overrides
	};
}

function makeLab(overrides: Partial<CachedLabResult> = {}): CachedLabResult {
	return {
		id: 'lab-1', testName: 'HbA1c', value: 7.2, unit: '%',
		referenceRangeLow: 4.0, referenceRangeHigh: 5.6, abnormalFlag: 'H',
		isAbnormal: true, collectionDate: '2026-02-08T10:00:00Z',
		trendDirection: 'down', ...overrides
	};
}

function makeEvent(overrides: Partial<CachedTimelineEvent> = {}): CachedTimelineEvent {
	return {
		id: 'evt-1', eventType: 'lab_result', category: 'Lab Results',
		description: 'New blood work from Central Hospital',
		date: '2026-02-12T09:30:00Z', stillActive: false,
		...overrides
	};
}

// === MEDICATION SEARCH ===

describe('medication search (in-memory, Viktor)', () => {
	const meds = [
		makeMed({ id: 'a', genericName: 'Metformin', brandName: 'Glucophage', condition: 'For blood sugar' }),
		makeMed({ id: 'b', genericName: 'Lisinopril', dose: '10mg', prescriberName: 'Dr. Ndiaye', condition: 'For blood pressure' }),
		makeMed({ id: 'c', genericName: 'Amlodipine', dose: '5mg', prescriberName: 'Dr. Ndiaye', condition: 'For blood pressure' })
	];

	it('returns all medications for empty query', () => {
		expect(searchMedications(meds, '')).toHaveLength(3);
		expect(searchMedications(meds, '  ')).toHaveLength(3);
	});

	it('searches by generic name (case-insensitive)', () => {
		const results = searchMedications(meds, 'metformin');
		expect(results).toHaveLength(1);
		expect(results[0].genericName).toBe('Metformin');
	});

	it('searches by brand name', () => {
		const results = searchMedications(meds, 'glucophage');
		expect(results).toHaveLength(1);
		expect(results[0].genericName).toBe('Metformin');
	});

	it('searches by condition (Dr. Diallo: "blood" matches condition)', () => {
		const results = searchMedications(meds, 'blood');
		expect(results).toHaveLength(3); // All have "blood" in condition
	});

	it('searches by prescriber name', () => {
		const results = searchMedications(meds, 'Ndiaye');
		expect(results).toHaveLength(2);
	});

	it('searches by dose', () => {
		const results = searchMedications(meds, '10mg');
		expect(results).toHaveLength(1);
		expect(results[0].genericName).toBe('Lisinopril');
	});

	it('returns empty for no match', () => {
		expect(searchMedications(meds, 'aspirin')).toHaveLength(0);
	});
});

// === FRESHNESS INDICATOR ===

describe('freshness indicator', () => {
	const baseTime = new Date('2026-02-12T10:00:00Z').getTime();

	it('returns fresh for <15 minutes', () => {
		const sync = new Date(baseTime - 5 * 60_000).toISOString();
		expect(computeFreshness(sync, baseTime)).toBe('fresh');
	});

	it('returns recent for <1 hour', () => {
		const sync = new Date(baseTime - 30 * 60_000).toISOString();
		expect(computeFreshness(sync, baseTime)).toBe('recent');
	});

	it('returns stale for <24 hours', () => {
		const sync = new Date(baseTime - 3 * 3600_000).toISOString();
		expect(computeFreshness(sync, baseTime)).toBe('stale');
	});

	it('returns old for >24 hours', () => {
		const sync = new Date(baseTime - 48 * 3600_000).toISOString();
		expect(computeFreshness(sync, baseTime)).toBe('old');
	});

	it('returns old for null timestamp', () => {
		expect(computeFreshness(null)).toBe('old');
	});

	it('generates human-readable label', () => {
		expect(freshnessLabel(null)).toBe('Not synced');
		const justNow = new Date(baseTime - 30_000).toISOString();
		expect(freshnessLabel(justNow, baseTime)).toBe('Synced just now');
		const minutesAgo = new Date(baseTime - 45 * 60_000).toISOString();
		expect(freshnessLabel(minutesAgo, baseTime)).toBe('Synced 45m ago');
		const hoursAgo = new Date(baseTime - 2 * 3600_000).toISOString();
		expect(freshnessLabel(hoursAgo, baseTime)).toBe('Synced 2h ago');
	});

	it('maps freshness to correct colors', () => {
		expect(freshnessColor('fresh')).toBe('var(--color-success)');
		expect(freshnessColor('recent')).toBe('var(--color-text-muted)');
		expect(freshnessColor('stale')).toBe('var(--color-warning)');
		expect(freshnessColor('old')).toBe('var(--color-error)');
	});
});

// === LAB TREND INDICATORS ===

describe('lab trend indicators', () => {
	it('returns correct trend arrows', () => {
		expect(trendArrow('up')).toBe('\u2191');
		expect(trendArrow('down')).toBe('\u2193');
		expect(trendArrow('stable')).toBe('\u2192');
	});

	it('returns correct trend labels', () => {
		expect(trendLabel('up')).toBe('Rising');
		expect(trendLabel('down')).toBe('Falling');
		expect(trendLabel('stable')).toBe('Stable');
	});

	it('maps trend to clinical colors (abnormal context)', () => {
		expect(trendColor('up', true)).toBe('var(--color-error)');
		expect(trendColor('up', false)).toBe('var(--color-text-muted)');
		expect(trendColor('stable', false)).toBe('var(--color-text-muted)');
		expect(trendColor('down', true)).toBe('var(--color-error)');
	});
});

// === TIMELINE ===

describe('timeline grouping and filtering', () => {
	it('groups events by date with Today/Yesterday labels', () => {
		const today = new Date('2026-02-12T15:00:00Z');
		const events = [
			makeEvent({ id: 'a', date: '2026-02-12T09:30:00Z' }),
			makeEvent({ id: 'b', date: '2026-02-12T14:00:00Z' }),
			makeEvent({ id: 'c', date: '2026-02-11T11:00:00Z' }),
			makeEvent({ id: 'd', date: '2026-02-09T16:00:00Z' })
		];

		const groups = groupTimelineByDate(events, today);
		expect(groups).toHaveLength(3);
		expect(groups[0].label).toBe('Today');
		expect(groups[0].events).toHaveLength(2);
		expect(groups[1].label).toBe('Yesterday');
		expect(groups[1].events).toHaveLength(1);
		expect(groups[2].label).toContain('Feb');
	});

	it('sorts events within groups by time descending', () => {
		const today = new Date('2026-02-12T18:00:00Z');
		const events = [
			makeEvent({ id: 'a', date: '2026-02-12T09:00:00Z' }),
			makeEvent({ id: 'b', date: '2026-02-12T15:00:00Z' })
		];

		const groups = groupTimelineByDate(events, today);
		expect(groups[0].events[0].id).toBe('b'); // 15:00 before 09:00
		expect(groups[0].events[1].id).toBe('a');
	});

	it('filters events by type', () => {
		const events = [
			makeEvent({ id: 'a', eventType: 'lab_result' }),
			makeEvent({ id: 'b', eventType: 'medication_change' }),
			makeEvent({ id: 'c', eventType: 'journal' }),
			makeEvent({ id: 'd', eventType: 'lab_result' })
		];

		expect(filterTimelineEvents(events, 'all')).toHaveLength(4);
		expect(filterTimelineEvents(events, 'lab_result')).toHaveLength(2);
		expect(filterTimelineEvents(events, 'journal')).toHaveLength(1);
		expect(filterTimelineEvents(events, 'appointment')).toHaveLength(0);
	});

	it('tracks stillActive status on events', () => {
		const events = [
			makeEvent({ id: 'a', eventType: 'journal', stillActive: true }),
			makeEvent({ id: 'b', eventType: 'lab_result', stillActive: false })
		];

		const active = events.filter((e) => e.stillActive);
		expect(active).toHaveLength(1);
		expect(active[0].eventType).toBe('journal');
	});

	it('maps event types to icons and colors', () => {
		expect(timelineEventIcon('medication_change')).toBe('Pill');
		expect(timelineEventIcon('lab_result')).toBe('Lab');
		expect(timelineEventIcon('journal')).toBe('Note');
		expect(timelineEventColor('medication_change')).toBe('var(--color-primary)');
		expect(timelineEventColor('journal')).toBe('var(--color-accent)');
	});
});

// === SHARE SHEET ===

describe('share sheet (Nadia: reduced subset)', () => {
	it('generates medication share with names + doses only', () => {
		const meds = [
			makeMed({ id: 'a', genericName: 'Metformin', dose: '500mg', frequency: 'Twice daily' }),
			makeMed({ id: 'b', genericName: 'Amlodipine', dose: '5mg', frequency: 'Once daily' })
		];

		const payload = shareMedicationList(meds, 'Mamadou', '2026-02-12T09:30:00Z');
		expect(payload.title).toContain('Mamadou');
		expect(payload.text).toContain('Metformin 500mg');
		expect(payload.text).toContain('Amlodipine 5mg');
		expect(payload.disclaimer).toContain('healthcare team');
	});

	it('generates lab share with values + ranges + trends', () => {
		const labs = [
			makeLab({ id: 'a', testName: 'HbA1c', value: 7.2, unit: '%', referenceRangeLow: 4.0, referenceRangeHigh: 5.6, trendDirection: 'down' })
		];

		const payload = shareLabSummary(labs, 'Thomas', null);
		expect(payload.title).toContain('Thomas');
		expect(payload.text).toContain('HbA1c');
		expect(payload.text).toContain('7.2');
		expect(payload.text).toContain('4-5.6');
	});

	it('generates appointment prep share for patient view', () => {
		const prep: AppointmentPrepData = {
			appointmentId: 'apt-1',
			doctorName: 'Dr. Chen',
			appointmentDate: 'Feb 14, 2026',
			forPatient: {
				thingsToMention: ['Blood sugar improved', 'Feeling dizzy'],
				questionsToConsider: ['Should Lisinopril dose be adjusted?']
			},
			forDoctor: {
				lastVisitDate: 'Nov 15, 2025',
				medicationChanges: ['Lisinopril: 10mg \u2192 20mg'],
				labResults: ['HbA1c: 7.8% \u2192 7.2%'],
				patientReportedSymptoms: ['Dizziness (3 entries)'],
				activeAlerts: ['Potassium rising']
			}
		};

		const patientPayload = shareAppointmentPrep(prep, 'patient', '2026-02-12T09:30:00Z');
		expect(patientPayload.text).toContain('Blood sugar improved');
		expect(patientPayload.text).toContain('Should Lisinopril');
		expect(patientPayload.disclaimer).toContain('healthcare team');
	});

	it('generates appointment prep share for doctor view', () => {
		const prep: AppointmentPrepData = {
			appointmentId: 'apt-1',
			doctorName: 'Dr. Chen',
			appointmentDate: 'Feb 14, 2026',
			forPatient: { thingsToMention: [], questionsToConsider: [] },
			forDoctor: {
				lastVisitDate: 'Nov 15, 2025',
				medicationChanges: ['Lisinopril: 10mg \u2192 20mg'],
				labResults: ['HbA1c: 7.8% \u2192 7.2%'],
				patientReportedSymptoms: ['Dizziness (3 entries)'],
				activeAlerts: ['Potassium rising']
			}
		};

		const doctorPayload = shareAppointmentPrep(prep, 'doctor', null);
		expect(doctorPayload.text).toContain('Lisinopril: 10mg');
		expect(doctorPayload.text).toContain('HbA1c: 7.8%');
		expect(doctorPayload.text).toContain('Dizziness');
		expect(doctorPayload.text).toContain('Potassium rising');
	});

	it('includes disclaimer in formatted share text', () => {
		const payload = shareMedicationList([makeMed()], 'Test', null);
		const fullText = formatShareText(payload);
		expect(fullText).toContain(payload.title);
		expect(fullText).toContain(payload.text);
		expect(fullText).toContain(payload.disclaimer);
	});
});

// === EMPTY STATES ===

describe('empty state messages', () => {
	it('provides helpful messages for all screen types', () => {
		expect(emptyStateMessage('medications')).toContain('No medications');
		expect(emptyStateMessage('labs')).toContain('No lab results');
		expect(emptyStateMessage('timeline')).toContain('No events');
		expect(emptyStateMessage('appointments')).toContain('No upcoming');
	});

	it('empty messages suggest using the desktop', () => {
		expect(emptyStateMessage('medications')).toContain('desktop');
		expect(emptyStateMessage('labs')).toContain('desktop');
	});
});
