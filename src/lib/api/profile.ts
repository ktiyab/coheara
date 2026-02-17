import { invoke } from '@tauri-apps/api/core';
import type { ProfileInfo, ProfileCreateResult } from '$lib/types/profile';

export async function listProfiles(): Promise<ProfileInfo[]> {
  return invoke<ProfileInfo[]>('list_profiles');
}

export async function createProfile(
  name: string,
  password: string,
  managedBy: string | null,
): Promise<ProfileCreateResult> {
  return invoke<ProfileCreateResult>('create_profile', {
    name,
    password,
    managedBy,
  });
}

export async function unlockProfile(
  profileId: string,
  password: string,
): Promise<ProfileInfo> {
  return invoke<ProfileInfo>('unlock_profile', { profileId, password });
}

export async function lockProfile(): Promise<void> {
  return invoke('lock_profile');
}

export async function recoverProfile(
  profileId: string,
  recoveryPhrase: string,
  newPassword: string,
): Promise<void> {
  return invoke('recover_profile', { profileId, recoveryPhrase, newPassword });
}

export async function isProfileActive(): Promise<boolean> {
  return invoke<boolean>('is_profile_active');
}

export async function getActiveProfileName(): Promise<string> {
  return invoke<string>('get_active_profile_name');
}

import type { ResolvedModel } from '$lib/types/ai';

/** S.1: Granular AI status level */
export type StatusLevel = 'unknown' | 'reachable' | 'configured' | 'verified' | 'degraded' | 'error';

export interface AiStatus {
  ollama_available: boolean;
  active_model: ResolvedModel | null;
  embedder_type: string;
  summary: string;
  /** S.1: Granular status level for frontend routing */
  level: StatusLevel;
}

export async function checkAiStatus(): Promise<AiStatus> {
  return invoke<AiStatus>('check_ai_status');
}

/** S.1+S.7: Verify AI generation and update cached status.
 * Runs a lightweight test generation. Promotes level from 'configured' to 'verified' on success.
 * Frontend should call this ~30s after startup and periodically (every 60s). */
export async function verifyAiStatus(): Promise<AiStatus> {
  return invoke<AiStatus>('verify_ai_status');
}

export async function deleteProfile(profileId: string): Promise<void> {
  return invoke('delete_profile', { profileId });
}

export async function changeProfilePassword(
  currentPassword: string,
  newPassword: string,
): Promise<void> {
  return invoke('change_profile_password', { currentPassword, newPassword });
}
