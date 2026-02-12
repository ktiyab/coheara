// L3-05: Medication List — Tauri invoke wrappers.

import { invoke } from '@tauri-apps/api/core';
import type {
  MedicationListData,
  MedicationDetail,
  MedicationListFilter,
  DoseChangeView,
  AliasSearchResult,
  OtcMedicationInput,
} from '$lib/types/medication';

export async function getMedications(
  filter: MedicationListFilter,
): Promise<MedicationListData> {
  return invoke<MedicationListData>('get_medications', { filter });
}

export async function getMedicationDetail(
  medicationId: string,
): Promise<MedicationDetail> {
  return invoke<MedicationDetail>('get_medication_detail', { medicationId });
}

export async function addOtcMedication(
  input: OtcMedicationInput,
): Promise<string> {
  return invoke<string>('add_otc_medication', { input });
}

export async function getDoseHistory(
  medicationId: string,
): Promise<DoseChangeView[]> {
  return invoke<DoseChangeView[]>('get_dose_history', { medicationId });
}

export async function searchMedicationAlias(
  query: string,
  limit?: number,
): Promise<AliasSearchResult[]> {
  return invoke<AliasSearchResult[]>('search_medication_alias', { query, limit });
}
