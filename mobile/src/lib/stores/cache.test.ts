// M1-03: Cache store tests — aligned CA-05 desktop types
import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
	medications,
	labResults,
	timelineEvents,
	activeAlerts,
	nextAppointment,
	profile,
	lastSyncTimestamp,
	activeMedicationCount,
	discontinuedMedicationCount,
	abnormalLabs,
	labResultsSorted,
	activeAlertCount,
	loadCacheData,
	clearCacheStores
} from './cache.js';
import type { CachedMedication, CachedLabResult, CachedAlert } from '$lib/types/viewer.js';

function makeMed(overrides: Partial<CachedMedication> = {}): CachedMedication {
	return {
		id: 'med-1',
		genericName: 'Metformin',
		dose: '500mg',
		frequency: 'Twice daily',
		route: 'oral',
		status: 'active',
		isOtc: false,
		prescriberName: 'Dr. Chen',
		condition: 'For blood sugar',
		startDate: '2024-01-15',
		...overrides
	};
}

function makeLab(overrides: Partial<CachedLabResult> = {}): CachedLabResult {
	return {
		id: 'lab-1',
		testName: 'HbA1c',
		value: 7.2,
		unit: '%',
		referenceRangeLow: 4.0,
		referenceRangeHigh: 5.6,
		abnormalFlag: 'H',
		isAbnormal: true,
		collectionDate: '2026-02-08T10:00:00Z',
		trendDirection: 'down',
		...overrides
	};
}

describe('cache store — medication counts', () => {
	beforeEach(() => clearCacheStores());

	it('counts active and discontinued medications separately', () => {
		medications.set([
			makeMed({ id: 'a', status: 'active' }),
			makeMed({ id: 'b', status: 'active' }),
			makeMed({ id: 'c', status: 'discontinued' }),
		]);

		expect(get(activeMedicationCount)).toBe(2);
		expect(get(discontinuedMedicationCount)).toBe(1);
	});
});

describe('cache store — lab results', () => {
	beforeEach(() => clearCacheStores());

	it('sorts lab results by collection date (most recent first, Dr. Diallo)', () => {
		labResults.set([
			makeLab({ id: 'old', testName: 'Glucose', collectionDate: '2026-01-15T10:00:00Z' }),
			makeLab({ id: 'new', testName: 'HbA1c', collectionDate: '2026-02-08T10:00:00Z' }),
			makeLab({ id: 'mid', testName: 'Creatinine', collectionDate: '2026-02-01T10:00:00Z' })
		]);

		const sorted = get(labResultsSorted);
		expect(sorted[0].testName).toBe('HbA1c');
		expect(sorted[1].testName).toBe('Creatinine');
		expect(sorted[2].testName).toBe('Glucose');
	});

	it('filters abnormal labs for top banner', () => {
		labResults.set([
			makeLab({ id: 'a', isAbnormal: true, testName: 'Potassium' }),
			makeLab({ id: 'b', isAbnormal: false, testName: 'Creatinine' }),
			makeLab({ id: 'c', isAbnormal: true, testName: 'HbA1c' })
		]);

		const abnormal = get(abnormalLabs);
		expect(abnormal).toHaveLength(2);
		expect(abnormal.map((l) => l.testName)).toContain('Potassium');
		expect(abnormal.map((l) => l.testName)).toContain('HbA1c');
	});

	it('reference ranges present when provided', () => {
		const lab = makeLab({ referenceRangeLow: 3.5, referenceRangeHigh: 5.0 });
		labResults.set([lab]);

		const result = get(labResults)[0];
		expect(result.referenceRangeLow).toBe(3.5);
		expect(result.referenceRangeHigh).toBe(5.0);
	});
});

describe('cache store — alerts and load/clear', () => {
	beforeEach(() => clearCacheStores());

	it('counts undismissed alerts', () => {
		const alerts: CachedAlert[] = [
			{ id: 'a1', title: 'Alert 1', description: 'Desc', severity: 'warning', createdAt: '', dismissed: false },
			{ id: 'a2', title: 'Alert 2', description: 'Desc', severity: 'info', createdAt: '', dismissed: true },
			{ id: 'a3', title: 'Alert 3', description: 'Desc', severity: 'critical', createdAt: '', dismissed: false }
		];

		activeAlerts.set(alerts);
		expect(get(activeAlertCount)).toBe(2);
	});

	it('loadCacheData populates all stores at once', () => {
		loadCacheData({
			medications: [makeMed()],
			labResults: [makeLab()],
			timelineEvents: [],
			alerts: [],
			appointment: { id: 'apt-1', professionalName: 'Dr. Chen', date: '2026-02-14', prepAvailable: false, appointmentType: 'Follow-up' },
			profile: { profileName: 'Mamadou', totalDocuments: 5, extractionAccuracy: 0.88, allergies: [{ allergen: 'Penicillin', severity: 'high', verified: true }] },
			syncTimestamp: '2026-02-12T09:30:00Z'
		});

		expect(get(medications)).toHaveLength(1);
		expect(get(labResults)).toHaveLength(1);
		expect(get(nextAppointment)?.professionalName).toBe('Dr. Chen');
		expect(get(profile)?.profileName).toBe('Mamadou');
		expect(get(lastSyncTimestamp)).toBe('2026-02-12T09:30:00Z');
	});

	it('clearCacheStores resets everything', () => {
		loadCacheData({
			medications: [makeMed()],
			labResults: [makeLab()],
			timelineEvents: [],
			alerts: [],
			appointment: { id: 'apt-1', professionalName: 'Dr. Chen', date: '2026-02-14', prepAvailable: false, appointmentType: 'Follow-up' },
			profile: { profileName: 'Mamadou', totalDocuments: 3, extractionAccuracy: 0.85, allergies: [] },
			syncTimestamp: '2026-02-12T09:30:00Z'
		});

		clearCacheStores();

		expect(get(medications)).toHaveLength(0);
		expect(get(labResults)).toHaveLength(0);
		expect(get(nextAppointment)).toBeNull();
		expect(get(profile)).toBeNull();
		expect(get(lastSyncTimestamp)).toBeNull();
	});

	it('empty state returns zero counts', () => {
		expect(get(activeMedicationCount)).toBe(0);
		expect(get(discontinuedMedicationCount)).toBe(0);
		expect(get(abnormalLabs)).toHaveLength(0);
		expect(get(activeAlertCount)).toBe(0);
	});
});
