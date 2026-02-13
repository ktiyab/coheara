// M1-03: Viewer API — desktop enrichment endpoints
import { apiClient } from './client.js';
import type { MedicationDetail, LabHistoryEntry, AppointmentPrepData } from '$lib/types/viewer.js';

/** Fetch enriched medication detail from desktop */
export async function fetchMedicationDetail(medicationId: string): Promise<MedicationDetail | null> {
	const response = await apiClient.get<MedicationDetail>(`/api/medications/${medicationId}`);
	return response.ok && response.data ? response.data : null;
}

/** Fetch lab result history for trend view */
export async function fetchLabHistory(testName: string): Promise<LabHistoryEntry[]> {
	const response = await apiClient.get<LabHistoryEntry[]>(
		`/api/labs/history/${encodeURIComponent(testName)}`
	);
	return response.ok && response.data ? response.data : [];
}

/** Fetch appointment prep data */
export async function fetchAppointmentPrep(appointmentId: string): Promise<AppointmentPrepData | null> {
	const response = await apiClient.get<AppointmentPrepData>(
		`/api/appointments/${appointmentId}/prep`
	);
	return response.ok && response.data ? response.data : null;
}
