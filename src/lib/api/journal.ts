// L4-01: Symptom Journal — Tauri invoke wrappers.

import { invoke } from '@tauri-apps/api/core';
import type {
  SymptomEntry,
  StoredSymptom,
  SymptomFilter,
  RecordResult,
  NudgeDecision,
  CategoryInfo,
} from '$lib/types/journal';

export async function recordSymptom(entry: SymptomEntry): Promise<RecordResult> {
  return invoke<RecordResult>('record_symptom', { entry });
}

export async function getSymptomHistory(
  filter?: SymptomFilter,
): Promise<StoredSymptom[]> {
  return invoke<StoredSymptom[]>('get_symptom_history', {
    filter: filter ?? null,
  });
}

export async function resolveSymptom(symptomId: string): Promise<void> {
  return invoke('resolve_symptom', { symptomId });
}

export async function deleteSymptom(symptomId: string): Promise<void> {
  return invoke('delete_symptom', { symptomId });
}

export async function checkJournalNudge(): Promise<NudgeDecision> {
  return invoke<NudgeDecision>('check_journal_nudge');
}

export async function getSymptomCategories(): Promise<CategoryInfo[]> {
  return invoke<CategoryInfo[]>('get_symptom_categories');
}
