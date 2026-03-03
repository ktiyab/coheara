/** L3-06: Me Screen API — single IPC call for health overview. */

import { invoke } from '@tauri-apps/api/core';
import type { MeOverview } from '$lib/types/me';

/** ME-04: `lang` is the UI locale — ensures backend labels match display language. */
export async function getMeOverview(lang: string): Promise<MeOverview> {
	return invoke('get_me_overview', { lang });
}

/** ME-04: Record a vital sign measurement (manual entry). */
export async function recordVitalSign(
	vitalType: string,
	value: number,
	valueSecondary?: number | null,
	notes?: string | null,
): Promise<void> {
	return invoke('record_vital_sign', {
		vitalType,
		value,
		valueSecondary: valueSecondary ?? null,
		notes: notes ?? null,
	});
}

/** ME-06: Record a screening or vaccination date. */
export async function recordScreening(
	screeningKey: string,
	doseNumber: number,
	completedAt: string,
	provider?: string | null,
	notes?: string | null,
): Promise<string> {
	return invoke('record_screening', {
		screeningKey,
		doseNumber,
		completedAt,
		provider: provider ?? null,
		notes: notes ?? null,
	});
}

/** ME-06: Delete a screening record by ID. */
export async function deleteScreeningRecord(recordId: string): Promise<boolean> {
	return invoke('delete_screening_record', { recordId });
}
