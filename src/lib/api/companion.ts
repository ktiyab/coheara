/** MP-02: Companion access & data sharing API layer. */

import { invoke } from '@tauri-apps/api/core';

// ── Types ─────────────────────────────────────────────────

export interface CompanionProfileInfo {
	profile_id: string;
	profile_name: string;
	is_active: boolean;
}

export interface EnrichedGrant {
	id: string;
	granter_profile_id: string;
	grantee_profile_id: string;
	granter_name: string;
	grantee_name: string;
	access_level: string;
	granted_at: string;
}

// ── Companion unlock (E6) ─────────────────────────────────

export async function unlockForCompanion(
	profileId: string,
	password: string
): Promise<CompanionProfileInfo> {
	return invoke<CompanionProfileInfo>('unlock_for_companion', { profileId, password });
}

export async function revokeCompanionAccess(profileId: string): Promise<void> {
	return invoke<void>('revoke_companion_access', { profileId });
}

export async function listCompanionProfiles(): Promise<CompanionProfileInfo[]> {
	return invoke<CompanionProfileInfo[]>('list_companion_profiles');
}

// ── Data sharing grants (MP-02) ───────────────────────────

export async function listMyGrants(): Promise<EnrichedGrant[]> {
	return invoke<EnrichedGrant[]>('list_my_grants');
}

export async function listGrantsToMe(): Promise<EnrichedGrant[]> {
	return invoke<EnrichedGrant[]>('list_grants_to_me');
}

export async function grantProfileAccess(
	granterProfileId: string,
	granteeProfileId: string,
	accessLevel: 'full' | 'read_only'
): Promise<void> {
	return invoke<void>('grant_profile_access', {
		granterProfileId,
		granteeProfileId,
		accessLevel
	});
}

export async function revokeProfileAccess(
	granterProfileId: string,
	granteeProfileId: string
): Promise<boolean> {
	return invoke<boolean>('revoke_profile_access', {
		granterProfileId,
		granteeProfileId
	});
}
