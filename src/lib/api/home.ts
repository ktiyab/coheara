import { invoke } from '@tauri-apps/api/core';
import type { HomeData, DocumentCard } from '$lib/types/home';

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
