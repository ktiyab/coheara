// M1-03: Viewer API tests — 9 tests
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
	fetchMedicationDetail,
	fetchLabResults,
	fetchLabHistory,
	fetchAppointmentPrep
} from './viewer.js';
import type { CachedLabResult, MedicationDetail, LabHistoryEntry, AppointmentPrepData } from '$lib/types/viewer.js';

vi.mock('./client.js', () => ({
	apiClient: {
		get: vi.fn()
	}
}));

import { apiClient } from './client.js';

const mockGet = vi.mocked(apiClient.get);

describe('viewer API — fetchMedicationDetail', () => {
	beforeEach(() => vi.clearAllMocks());

	it('returns medication detail on success', async () => {
		const detail: MedicationDetail = {
			id: 'med-1', name: 'Metformin', dose: '500mg', frequency: 'Twice daily',
			prescriber: 'Dr. Chen', purpose: 'Blood sugar', scheduleGroup: 'morning',
			since: '2024-01-15', isActive: true, history: [{ date: '2024-01-15', event: 'Started' }]
		};
		mockGet.mockResolvedValue({ ok: true, status: 200, data: detail });

		const result = await fetchMedicationDetail('med-1');
		expect(result).toEqual(detail);
		expect(mockGet).toHaveBeenCalledWith('/api/medications/med-1');
	});

	it('returns null on failure', async () => {
		mockGet.mockResolvedValue({ ok: false, status: 404, error: 'Not found' });

		const result = await fetchMedicationDetail('med-999');
		expect(result).toBeNull();
	});
});

describe('viewer API — fetchLabResults', () => {
	beforeEach(() => vi.clearAllMocks());

	it('returns lab results array on success', async () => {
		const labs: CachedLabResult[] = [
			{
				id: 'lab-1', testName: 'HbA1c', value: 7.2, unit: '%',
				referenceMin: 4.0, referenceMax: 5.6, isAbnormal: true,
				trend: 'down', trendContext: 'improving', testedAt: '2026-02-08T10:00:00Z'
			}
		];
		mockGet.mockResolvedValue({ ok: true, status: 200, data: labs });

		const result = await fetchLabResults();
		expect(result).toEqual(labs);
		expect(mockGet).toHaveBeenCalledWith('/api/labs');
	});

	it('returns empty array on failure', async () => {
		mockGet.mockResolvedValue({ ok: false, status: 500, error: 'Server error' });

		const result = await fetchLabResults();
		expect(result).toEqual([]);
	});
});

describe('viewer API — fetchLabHistory', () => {
	beforeEach(() => vi.clearAllMocks());

	it('returns history entries on success', async () => {
		const history: LabHistoryEntry[] = [
			{ value: 7.8, date: '2025-11-01', trend: 'up' },
			{ value: 7.2, date: '2026-02-08', trend: 'down' }
		];
		mockGet.mockResolvedValue({ ok: true, status: 200, data: history });

		const result = await fetchLabHistory('HbA1c');
		expect(result).toEqual(history);
		expect(mockGet).toHaveBeenCalledWith('/api/labs/history/HbA1c');
	});

	it('encodes test names with special characters', async () => {
		mockGet.mockResolvedValue({ ok: true, status: 200, data: [] });

		await fetchLabHistory('Vitamin B12/Folate');
		expect(mockGet).toHaveBeenCalledWith('/api/labs/history/Vitamin%20B12%2FFolate');
	});

	it('returns empty array on failure', async () => {
		mockGet.mockResolvedValue({ ok: false, status: 404, error: 'Not found' });

		const result = await fetchLabHistory('Unknown');
		expect(result).toEqual([]);
	});
});

describe('viewer API — fetchAppointmentPrep', () => {
	beforeEach(() => vi.clearAllMocks());

	it('returns prep data on success', async () => {
		const prep: AppointmentPrepData = {
			appointmentId: 'apt-1', doctorName: 'Dr. Chen', appointmentDate: 'Feb 14, 2026',
			forPatient: { thingsToMention: ['Blood sugar improved'], questionsToConsider: ['Dose change?'] },
			forDoctor: { medicationChanges: [], labResults: [], patientReportedSymptoms: [], activeAlerts: [] }
		};
		mockGet.mockResolvedValue({ ok: true, status: 200, data: prep });

		const result = await fetchAppointmentPrep('apt-1');
		expect(result).toEqual(prep);
		expect(mockGet).toHaveBeenCalledWith('/api/appointments/apt-1/prep');
	});

	it('returns null on failure', async () => {
		mockGet.mockResolvedValue({ ok: false, status: 500, error: 'Generation failed' });

		const result = await fetchAppointmentPrep('apt-1');
		expect(result).toBeNull();
	});
});
