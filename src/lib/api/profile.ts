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

export interface AiStatus {
  ollama_available: boolean;
  ollama_model: string | null;
  embedder_type: string;
  summary: string;
}

export async function checkAiStatus(): Promise<AiStatus> {
  return invoke<AiStatus>('check_ai_status');
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
