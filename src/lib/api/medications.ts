// L3-05: Medication List — Tauri invoke wrappers.

import { invoke } from '@tauri-apps/api/core';
import type { MedicationListData, MedicationListFilter } from '$lib/types/medication';

export async function getMedications(
  filter: MedicationListFilter,
): Promise<MedicationListData> {
  return invoke<MedicationListData>('get_medications', { filter });
}
