// M1-03: Viewer utility tests — 39 tests
import { describe, it, expect } from 'vitest';
import {
	searchMedications,
	computeFreshness,
	freshnessLabel,
	freshnessColor,
	trendArrow,
	trendLabel,
	trendColor,
	computeTrendContext,
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

// --- Test data factories ---

function makeMed(overrides: Partial<CachedMedication> = {}): CachedMedication {
	return {
		id: 'med-1', name: 'Metformin', dose: '500mg', frequency: 'Twice daily',
		prescriber: 'Dr. Chen', purpose: 'For blood sugar', scheduleGroup: 'morning',
		since: '2024-01-15', isActive: true, ...overrides
	};
}

function makeLab(overrides: Partial<CachedLabResult> = {}): CachedLabResult {
	return {
		id: 'lab-1', testName: 'HbA1c', value: 7.2, unit: '%',
		referenceMin: 4.0, referenceMax: 5.6, isAbnormal: true,
		trend: 'down', trendContext: 'improving', testedAt: '2026-02-08T10:00:00Z',
		...overrides
	};
}

function makeEvent(overrides: Partial<CachedTimelineEvent> = {}): CachedTimelineEvent {
	return {
		id: 'evt-1', eventType: 'lab_result', title: 'Lab results processed',
		description: 'New blood work from Central Hospital',
		timestamp: '2026-02-12T09:30:00Z', isPatientReported: false,
		...overrides
	};
}

// === MEDICATION SEARCH ===

describe('medication search (in-memory, Viktor)', () => {
	const meds = [
		makeMed({ id: 'a', name: 'Metformin', genericName: 'metformin hydrochloride', purpose: 'For blood sugar' }),
		makeMed({ id: 'b', name: 'Lisinopril', dose: '10mg', prescriber: 'Dr. Ndiaye', purpose: 'For blood pressure' }),
		makeMed({ id: 'c', name: 'Amlodipine', dose: '5mg', prescriber: 'Dr. Ndiaye', purpose: 'For blood pressure' })
	];

	it('returns all medications for empty query', () => {
		expect(searchMedications(meds, '')).toHaveLength(3);
		expect(searchMedications(meds, '  ')).toHaveLength(3);
	});

	it('searches by medication name (case-insensitive)', () => {
		const results = searchMedications(meds, 'metformin');
		expect(results).toHaveLength(1);
		expect(results[0].name).toBe('Metformin');
	});

	it('searches by purpose (Dr. Diallo: "blood" matches purpose)', () => {
		const results = searchMedications(meds, 'blood');
		expect(results).toHaveLength(3); // All have "blood" in purpose or generic
	});

	it('searches by prescriber name', () => {
		const results = searchMedications(meds, 'Ndiaye');
		expect(results).toHaveLength(2);
	});

	it('searches by dose', () => {
		const results = searchMedications(meds, '10mg');
		expect(results).toHaveLength(1);
		expect(results[0].name).toBe('Lisinopril');
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

describe('lab trend indicators (Dr. Diallo: clinical meaning)', () => {
	it('returns correct trend arrows', () => {
		expect(trendArrow('up')).toBe('\u2191');
		expect(trendArrow('down')).toBe('\u2193');
		expect(trendArrow('stable')).toBe('\u2192');
		expect(trendArrow('first')).toBe('\u2014');
	});

	it('returns correct trend labels', () => {
		expect(trendLabel('worsening')).toBe('Worsening');
		expect(trendLabel('improving')).toBe('Improving');
		expect(trendLabel('approaching')).toBe('Approaching limit');
		expect(trendLabel('stable')).toBe('Stable');
		expect(trendLabel('first')).toBe('First result');
	});

	it('maps trend context to clinical colors', () => {
		expect(trendColor('worsening')).toBe('var(--color-error)');
		expect(trendColor('improving')).toBe('var(--color-success)');
		expect(trendColor('approaching')).toBe('var(--color-warning)');
		expect(trendColor('stable')).toBe('var(--color-text-muted)');
	});

	it('computes trend context: up + abnormal = worsening', () => {
		expect(computeTrendContext('up', true, false)).toBe('worsening');
	});

	it('computes trend context: down + was abnormal = improving', () => {
		expect(computeTrendContext('down', false, true)).toBe('improving');
		expect(computeTrendContext('down', true, true)).toBe('improving');
	});

	it('computes trend context: up + normal + was normal = approaching', () => {
		expect(computeTrendContext('up', false, false)).toBe('approaching');
	});

	it('computes trend context: stable and first', () => {
		expect(computeTrendContext('stable', false, false)).toBe('stable');
		expect(computeTrendContext('first', false, false)).toBe('first');
	});
});

// === TIMELINE ===

describe('timeline grouping and filtering', () => {
	it('groups events by date with Today/Yesterday labels', () => {
		const today = new Date('2026-02-12T15:00:00Z');
		const events = [
			makeEvent({ id: 'a', timestamp: '2026-02-12T09:30:00Z' }),
			makeEvent({ id: 'b', timestamp: '2026-02-12T14:00:00Z' }),
			makeEvent({ id: 'c', timestamp: '2026-02-11T11:00:00Z' }),
			makeEvent({ id: 'd', timestamp: '2026-02-09T16:00:00Z' })
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
			makeEvent({ id: 'a', timestamp: '2026-02-12T09:00:00Z' }),
			makeEvent({ id: 'b', timestamp: '2026-02-12T15:00:00Z' })
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

	it('distinguishes patient-reported events (LC-06)', () => {
		const events = [
			makeEvent({ id: 'a', eventType: 'journal', isPatientReported: true }),
			makeEvent({ id: 'b', eventType: 'lab_result', isPatientReported: false })
		];

		const journal = events.filter((e) => e.isPatientReported);
		expect(journal).toHaveLength(1);
		expect(journal[0].eventType).toBe('journal');
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
			makeMed({ id: 'a', name: 'Metformin', dose: '500mg', frequency: 'Twice daily', scheduleGroup: 'morning' }),
			makeMed({ id: 'b', name: 'Amlodipine', dose: '5mg', frequency: 'Once daily', scheduleGroup: 'evening' })
		];

		const payload = shareMedicationList(meds, 'Mamadou', '2026-02-12T09:30:00Z');
		expect(payload.title).toContain('Mamadou');
		expect(payload.text).toContain('Metformin 500mg');
		expect(payload.text).toContain('Amlodipine 5mg');
		expect(payload.disclaimer).toContain('healthcare team');
	});

	it('generates lab share with values + ranges + trends', () => {
		const labs = [
			makeLab({ id: 'a', testName: 'HbA1c', value: 7.2, unit: '%', referenceMin: 4.0, referenceMax: 5.6, trend: 'down', trendContext: 'improving' })
		];

		const payload = shareLabSummary(labs, 'Thomas', null);
		expect(payload.title).toContain('Thomas');
		expect(payload.text).toContain('HbA1c');
		expect(payload.text).toContain('7.2');
		expect(payload.text).toContain('4-5.6');
		expect(payload.text).toContain('Improving');
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
				medicationChanges: ['Lisinopril: 10mg → 20mg'],
				labResults: ['HbA1c: 7.8% → 7.2%'],
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
				medicationChanges: ['Lisinopril: 10mg → 20mg'],
				labResults: ['HbA1c: 7.8% → 7.2%'],
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
