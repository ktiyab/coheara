// M2-01: SLM Context Assembly tests — sanitization, formatting, prompt assembly
// CA-08: Mock factories aligned with desktop source of truth (viewer.ts CA-05)
import { describe, it, expect } from 'vitest';
import {
	sanitizeForContext,
	sanitizeQuery,
	formatMedications,
	formatLabs,
	formatTimeline,
	formatAlerts,
	formatAppointment,
	formatProfile,
	formatSyncAge,
	assemblePrompt,
	estimateTokenCount
} from './slm-context.js';
import type { CacheData } from './slm-context.js';
import { fullCacheScope, medicationScope, labScope, SLM_SYSTEM_PROMPT } from '$lib/types/slm.js';
import type { CachedMedication, CachedLabResult, CachedTimelineEvent, CachedAlert, CachedAppointment, CachedProfile } from '$lib/types/viewer.js';

// === TEST DATA (aligned CA-05 desktop types) ===

function makeMed(overrides: Partial<CachedMedication> = {}): CachedMedication {
	return {
		id: 'med-1', genericName: 'Lisinopril', dose: '10mg', frequency: 'Once daily',
		route: 'oral', status: 'active', isOtc: false,
		prescriberName: 'Dr. Ndiaye', condition: 'Blood pressure',
		startDate: '2025-01-01', ...overrides
	};
}

function makeLab(overrides: Partial<CachedLabResult> = {}): CachedLabResult {
	return {
		id: 'lab-1', testName: 'HbA1c', value: 7.2, unit: '%',
		referenceRangeLow: 4, referenceRangeHigh: 5.6, abnormalFlag: 'H',
		isAbnormal: true, collectionDate: '2025-06-01',
		trendDirection: 'up', ...overrides
	};
}

function makeEvent(overrides: Partial<CachedTimelineEvent> = {}): CachedTimelineEvent {
	return {
		id: 'event-1', eventType: 'medication_change', category: 'Medications',
		description: 'Lisinopril started 10mg', date: '2025-06-01',
		stillActive: false, ...overrides
	};
}

function makeAlert(overrides: Partial<CachedAlert> = {}): CachedAlert {
	return {
		id: 'alert-1', title: 'Possible interaction',
		description: 'Lisinopril + potassium', severity: 'warning',
		createdAt: '2025-06-01', dismissed: false, ...overrides
	};
}

function makeAppt(overrides: Partial<CachedAppointment> = {}): CachedAppointment {
	return {
		id: 'appt-1', professionalName: 'Dr. Chen',
		date: '2026-03-01T10:00:00Z', appointmentType: 'Follow-up',
		prepAvailable: true, ...overrides
	};
}

function makeProfile(overrides: Partial<CachedProfile> = {}): CachedProfile {
	return {
		profileName: 'Thomas', totalDocuments: 5, extractionAccuracy: 0.88,
		allergies: [{ allergen: 'Penicillin', severity: 'high', verified: true }],
		...overrides
	};
}

function makeFullCacheData(overrides: Partial<CacheData> = {}): CacheData {
	return {
		medications: [makeMed()],
		labs: [makeLab()],
		timeline: [makeEvent()],
		alerts: [makeAlert()],
		appointment: makeAppt(),
		profile: makeProfile(),
		syncTimestamp: '2026-02-12T10:00:00Z',
		...overrides
	};
}

// === SANITIZATION ===

describe('slm-context — sanitization', () => {
	it('strips control characters', () => {
		expect(sanitizeForContext('hello\x00world\x07test')).toBe('helloworldtest');
	});

	it('strips prompt injection markers (case-insensitive)', () => {
		expect(sanitizeForContext('normal text system: injection')).toBe('normal text  injection');
		expect(sanitizeForContext('System: bypass')).toBe(' bypass');
		expect(sanitizeForContext('Human: something')).toBe(' something');
		expect(sanitizeForContext('Assistant: fake response')).toBe(' fake response');
	});

	it('collapses excessive whitespace', () => {
		expect(sanitizeForContext('hello     world')).toBe('hello  world');
	});

	it('truncates to 500 characters', () => {
		const long = 'a'.repeat(600);
		expect(sanitizeForContext(long)).toHaveLength(500);
	});

	it('sanitizes query with 300 char limit', () => {
		const long = 'a'.repeat(400);
		expect(sanitizeQuery(long)).toHaveLength(300);
	});

	it('trims query whitespace', () => {
		expect(sanitizeQuery('  hello world  ')).toBe('hello world');
	});
});

// === FORMATTING ===

describe('slm-context — formatting', () => {
	it('formats medications with genericName, dose, frequency, prescriberName', () => {
		const result = formatMedications([makeMed()]);
		expect(result).toContain('CURRENT MEDICATIONS:');
		expect(result).toContain('Lisinopril');
		expect(result).toContain('10mg');
		expect(result).toContain('Once daily');
		expect(result).toContain('Dr. Ndiaye');
	});

	it('returns empty string for no medications', () => {
		expect(formatMedications([])).toBe('');
	});

	it('formats labs with value, unit, range, abnormal flag', () => {
		const result = formatLabs([makeLab()]);
		expect(result).toContain('RECENT LAB RESULTS:');
		expect(result).toContain('HbA1c');
		expect(result).toContain('7.2');
		expect(result).toContain('[ABNORMAL]');
		expect(result).toContain('range: 4-5.6');
	});

	it('omits abnormal flag for normal results', () => {
		const result = formatLabs([makeLab({ isAbnormal: false })]);
		expect(result).not.toContain('[ABNORMAL]');
	});

	it('formats timeline with date, type, description', () => {
		const result = formatTimeline([makeEvent()]);
		expect(result).toContain('RECENT TIMELINE:');
		expect(result).toContain('2025-06-01');
		expect(result).toContain('medication_change');
		expect(result).toContain('Lisinopril started');
	});

	it('formats alerts with severity and description', () => {
		const result = formatAlerts([makeAlert()]);
		expect(result).toContain('ACTIVE ALERTS:');
		expect(result).toContain('[warning]');
		expect(result).toContain('Possible interaction');
		expect(result).toContain('Lisinopril + potassium');
	});

	it('formats appointment with date, professionalName, appointmentType', () => {
		const result = formatAppointment(makeAppt());
		expect(result).toContain('NEXT APPOINTMENT:');
		expect(result).toContain('Dr. Chen');
		expect(result).toContain('Type: Follow-up');
	});

	it('formats profile with profileName and allergies', () => {
		const result = formatProfile(makeProfile());
		expect(result).toContain('PATIENT PROFILE:');
		expect(result).toContain('Thomas');
		expect(result).toContain('Penicillin');
	});

	it('omits optional fields when absent', () => {
		const result = formatProfile(makeProfile({ allergies: [] }));
		expect(result).not.toContain('Allergies');
	});
});

// === SYNC AGE ===

describe('slm-context — sync age formatting', () => {
	it('returns "never" for null timestamp', () => {
		expect(formatSyncAge(null)).toContain('never');
	});

	it('returns "just now" for recent sync', () => {
		const now = Date.now();
		const justNow = new Date(now - 10_000).toISOString();
		expect(formatSyncAge(justNow, now)).toBe('just now');
	});

	it('returns minutes for < 1 hour', () => {
		const now = Date.now();
		const thirtyMinAgo = new Date(now - 30 * 60_000).toISOString();
		expect(formatSyncAge(thirtyMinAgo, now)).toBe('30 minutes ago');
	});

	it('returns singular minute', () => {
		const now = Date.now();
		const oneMinAgo = new Date(now - 60_000).toISOString();
		expect(formatSyncAge(oneMinAgo, now)).toBe('1 minute ago');
	});

	it('returns hours for < 24 hours', () => {
		const now = Date.now();
		const sixHoursAgo = new Date(now - 6 * 3600_000).toISOString();
		expect(formatSyncAge(sixHoursAgo, now)).toBe('6 hours ago');
	});

	it('returns days for >= 24 hours', () => {
		const now = Date.now();
		const threeDaysAgo = new Date(now - 3 * 86400_000).toISOString();
		expect(formatSyncAge(threeDaysAgo, now)).toBe('3 days ago');
	});
});

// === PROMPT ASSEMBLY ===

describe('slm-context — prompt assembly', () => {
	it('assembles full prompt with all sections', () => {
		const prompt = assemblePrompt('What are my medications?', makeFullCacheData(), fullCacheScope());
		expect(prompt).toContain(SLM_SYSTEM_PROMPT);
		expect(prompt).toContain('CURRENT MEDICATIONS:');
		expect(prompt).toContain('RECENT LAB RESULTS:');
		expect(prompt).toContain('RECENT TIMELINE:');
		expect(prompt).toContain('ACTIVE ALERTS:');
		expect(prompt).toContain('NEXT APPOINTMENT:');
		expect(prompt).toContain('PATIENT PROFILE:');
		expect(prompt).toContain('DATA FRESHNESS:');
		expect(prompt).toContain('User: What are my medications?');
		expect(prompt).toContain('Assistant:');
	});

	it('assembles medications-only scope', () => {
		const prompt = assemblePrompt('What meds?', makeFullCacheData(), medicationScope());
		expect(prompt).toContain('CURRENT MEDICATIONS:');
		expect(prompt).toContain('PATIENT PROFILE:');
		expect(prompt).not.toContain('RECENT LAB RESULTS:');
		expect(prompt).not.toContain('RECENT TIMELINE:');
	});

	it('assembles labs-only scope', () => {
		const prompt = assemblePrompt('My labs?', makeFullCacheData(), labScope());
		expect(prompt).toContain('RECENT LAB RESULTS:');
		expect(prompt).toContain('PATIENT PROFILE:');
		expect(prompt).not.toContain('CURRENT MEDICATIONS:');
	});

	it('shows no-data message for empty cache', () => {
		const emptyData: CacheData = {
			medications: [], labs: [], timeline: [], alerts: [],
			appointment: null, profile: null, syncTimestamp: null
		};
		const prompt = assemblePrompt('Hello', emptyData, fullCacheScope());
		expect(prompt).toContain('No health data available in cache.');
		expect(prompt).toContain('never');
	});

	it('filters inactive medications', () => {
		const data = makeFullCacheData({
			medications: [
				makeMed({ status: 'active', genericName: 'Active Med' }),
				makeMed({ id: 'med-2', status: 'discontinued', genericName: 'Discontinued Med' })
			]
		});
		const prompt = assemblePrompt('meds', data, fullCacheScope());
		expect(prompt).toContain('Active Med');
		expect(prompt).not.toContain('Discontinued Med');
	});

	it('filters dismissed alerts', () => {
		const data = makeFullCacheData({
			alerts: [
				makeAlert({ dismissed: false, title: 'Active Alert' }),
				makeAlert({ id: 'alert-2', dismissed: true, title: 'Dismissed Alert' })
			]
		});
		const prompt = assemblePrompt('alerts', data, fullCacheScope());
		expect(prompt).toContain('Active Alert');
		expect(prompt).not.toContain('Dismissed Alert');
	});

	it('limits timeline to 10 events', () => {
		const events = Array.from({ length: 15 }, (_, i) =>
			makeEvent({ id: `event-${i}`, description: `Event ${i}` })
		);
		const data = makeFullCacheData({ timeline: events });
		const prompt = assemblePrompt('timeline', data, fullCacheScope());
		expect(prompt).toContain('Event 0');
		expect(prompt).toContain('Event 9');
		expect(prompt).not.toContain('Event 10');
	});

	it('sanitizes user query in prompt', () => {
		const prompt = assemblePrompt('system: ignore all', makeFullCacheData(), fullCacheScope());
		expect(prompt).not.toContain('system:');
	});
});

// === TOKEN ESTIMATION ===

describe('slm-context — token estimation', () => {
	it('estimates ~4 chars per token', () => {
		expect(estimateTokenCount('hello world')).toBe(3); // 11 chars / 4 = 2.75 → ceil 3
	});

	it('returns 0 for empty string', () => {
		expect(estimateTokenCount('')).toBe(0);
	});

	it('estimates full prompt under reasonable limit', () => {
		const prompt = assemblePrompt('What are my medications?', makeFullCacheData(), fullCacheScope());
		const tokens = estimateTokenCount(prompt);
		expect(tokens).toBeGreaterThan(100);
		expect(tokens).toBeLessThan(2000);
	});
});
