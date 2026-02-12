// L5-01: Trust & Safety — Tauri invoke wrappers

import { invoke } from '@tauri-apps/api/core';
import type {
  CriticalLabAlert,
  CriticalDismissRequest,
  DosePlausibility,
  BackupResult,
  RestorePreview,
  RestoreResult,
  ErasureRequest,
  ErasureResult,
  PrivacyInfo,
} from '$lib/types/trust';

export async function getCriticalAlerts(): Promise<CriticalLabAlert[]> {
  return invoke<CriticalLabAlert[]>('get_critical_alerts');
}

export async function dismissCritical(request: CriticalDismissRequest): Promise<void> {
  return invoke('dismiss_critical', { request });
}

export async function checkDose(
  medicationName: string,
  doseValue: number,
  doseUnit: string,
): Promise<DosePlausibility> {
  return invoke<DosePlausibility>('check_dose', { medicationName, doseValue, doseUnit });
}

export async function createBackup(outputPath: string): Promise<BackupResult> {
  return invoke<BackupResult>('create_backup', { outputPath });
}

export async function previewBackup(backupPath: string): Promise<RestorePreview> {
  return invoke<RestorePreview>('preview_backup_file', { backupPath });
}

export async function restoreFromBackup(
  backupPath: string,
  password: string,
): Promise<RestoreResult> {
  return invoke<RestoreResult>('restore_from_backup', { backupPath, password });
}

export async function eraseProfile(request: ErasureRequest): Promise<ErasureResult> {
  return invoke<ErasureResult>('erase_profile_data', { request });
}

export async function getPrivacyInfo(): Promise<PrivacyInfo> {
  return invoke<PrivacyInfo>('get_privacy_info_cmd');
}

export async function openDataFolder(): Promise<void> {
  return invoke('open_data_folder');
}
