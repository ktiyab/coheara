// M1-03: Cache store tests — 12 tests
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
	medicationsBySchedule,
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
		name: 'Metformin',
		dose: '500mg',
		frequency: 'Twice daily',
		prescriber: 'Dr. Chen',
		purpose: 'For blood sugar',
		scheduleGroup: 'morning',
		since: '2024-01-15',
		isActive: true,
		...overrides
	};
}

function makeLab(overrides: Partial<CachedLabResult> = {}): CachedLabResult {
	return {
		id: 'lab-1',
		testName: 'HbA1c',
		value: 7.2,
		unit: '%',
		referenceMin: 4.0,
		referenceMax: 5.6,
		isAbnormal: true,
		trend: 'down',
		trendContext: 'improving',
		testedAt: '2026-02-08T10:00:00Z',
		...overrides
	};
}

describe('cache store — medication grouping', () => {
	beforeEach(() => clearCacheStores());

	it('groups medications by schedule (Dr. Diallo)', () => {
		const meds: CachedMedication[] = [
			makeMed({ id: 'a', scheduleGroup: 'morning', name: 'Metformin' }),
			makeMed({ id: 'b', scheduleGroup: 'morning', name: 'Lisinopril' }),
			makeMed({ id: 'c', scheduleGroup: 'evening', name: 'Amlodipine' }),
			makeMed({ id: 'd', scheduleGroup: 'as_needed', name: 'Paracetamol' }),
			makeMed({ id: 'e', scheduleGroup: 'multiple', name: 'Metoprolol' }),
			makeMed({ id: 'f', name: 'Atorvastatin', isActive: false })
		];

		medications.set(meds);
		const groups = get(medicationsBySchedule);

		expect(groups.morning).toHaveLength(2);
		expect(groups.evening).toHaveLength(1);
		expect(groups.as_needed).toHaveLength(1);
		expect(groups.multiple).toHaveLength(1);
		expect(groups.discontinued).toHaveLength(1);
	});

	it('counts active and discontinued medications separately', () => {
		medications.set([
			makeMed({ id: 'a', isActive: true }),
			makeMed({ id: 'b', isActive: true }),
			makeMed({ id: 'c', isActive: false }),
		]);

		expect(get(activeMedicationCount)).toBe(2);
		expect(get(discontinuedMedicationCount)).toBe(1);
	});

	it('separates active from discontinued (Dr. Diallo)', () => {
		medications.set([
			makeMed({ id: 'active', isActive: true }),
			makeMed({ id: 'disc', isActive: false })
		]);

		const groups = get(medicationsBySchedule);
		expect(groups.morning.every((m) => m.isActive)).toBe(true);
		expect(groups.discontinued.every((m) => !m.isActive)).toBe(true);
	});
});

describe('cache store — lab results', () => {
	beforeEach(() => clearCacheStores());

	it('sorts lab results by date (most recent first, Dr. Diallo)', () => {
		labResults.set([
			makeLab({ id: 'old', testName: 'Glucose', testedAt: '2026-01-15T10:00:00Z' }),
			makeLab({ id: 'new', testName: 'HbA1c', testedAt: '2026-02-08T10:00:00Z' }),
			makeLab({ id: 'mid', testName: 'Creatinine', testedAt: '2026-02-01T10:00:00Z' })
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

	it('reference ranges always present (Dr. Diallo)', () => {
		const lab = makeLab({ referenceMin: 3.5, referenceMax: 5.0 });
		labResults.set([lab]);

		const result = get(labResults)[0];
		expect(result.referenceMin).toBe(3.5);
		expect(result.referenceMax).toBe(5.0);
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
			appointment: { id: 'apt-1', doctorName: 'Dr. Chen', date: '2026-02-14', hasPrepData: false },
			profile: { name: 'Mamadou', allergies: ['Penicillin'], emergencyContacts: [] },
			syncTimestamp: '2026-02-12T09:30:00Z'
		});

		expect(get(medications)).toHaveLength(1);
		expect(get(labResults)).toHaveLength(1);
		expect(get(nextAppointment)?.doctorName).toBe('Dr. Chen');
		expect(get(profile)?.name).toBe('Mamadou');
		expect(get(lastSyncTimestamp)).toBe('2026-02-12T09:30:00Z');
	});

	it('clearCacheStores resets everything', () => {
		loadCacheData({
			medications: [makeMed()],
			labResults: [makeLab()],
			timelineEvents: [],
			alerts: [],
			appointment: { id: 'apt-1', doctorName: 'Dr. Chen', date: '2026-02-14', hasPrepData: false },
			profile: { name: 'Mamadou', allergies: [], emergencyContacts: [] },
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
