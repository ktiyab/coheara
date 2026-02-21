import { invoke } from '@tauri-apps/api/core';
import type { HomeData, DocumentCard, RecentSymptomCard, ExtractionSuggestion } from '$lib/types/home';

export async function getHomeData(): Promise<HomeData> {
  return invoke<HomeData>('get_home_data');
}

export async function getMoreDocuments(
  offset: number,
  limit: number,
): Promise<DocumentCard[]> {
  return invoke<DocumentCard[]>('get_more_documents', { offset, limit });
}

export async function dismissAlert(
  alertId: string,
  alertType: string,
  reason: string,
): Promise<void> {
  return invoke('dismiss_alert', { alertId, alertType, reason });
}

/** LP-07: Fetch recent symptoms for the Home dashboard. */
export async function getRecentSymptoms(limit?: number): Promise<RecentSymptomCard[]> {
  return invoke<RecentSymptomCard[]>('get_recent_symptoms', { limit: limit ?? 5 });
}

/** LP-07: Fetch proactive extraction suggestions. */
export async function getExtractionSuggestions(): Promise<ExtractionSuggestion[]> {
  return invoke<ExtractionSuggestion[]>('get_extraction_suggestions');
}

/** LP-07: Dismiss a suggestion so it does not reappear. */
export async function dismissExtractionSuggestion(
  suggestionType: string,
  entityId: string,
): Promise<void> {
  return invoke('dismiss_extraction_suggestion', { suggestionType, entityId });
}
