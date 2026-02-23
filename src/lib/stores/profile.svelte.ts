/**
 * E2E-F06 + S.5 + F6: Global profile store.
 *
 * S.5: aiStatus is kept for backward compatibility but is now synced
 * from the ai store (single source of truth). Use `ai.isAiAvailable`
 * for the canonical check.
 *
 * F6: activeInfo holds full ProfileInfo for the active session,
 * enabling "viewing as" indicator and self-managed checks.
 */

import type { AiStatus } from '$lib/api/profile';
import type { ProfileInfo } from '$lib/types/profile';

class ProfileStore {
	name = $state('');
	/** Spec 45 [PU-04]: Profile color index from 8-color palette. */
	colorIndex = $state<number | null>(null);
	/** @deprecated S.5: Use ai store directly. Kept for backward compat in +page.svelte. */
	aiStatus = $state<AiStatus | null>(null);
	/** F6: Full ProfileInfo for the active session. */
	activeInfo = $state<ProfileInfo | null>(null);

	/** F6: True if the active profile is self-managed (no caregiver). */
	get isSelfManaged(): boolean {
		return this.activeInfo?.managed_by == null;
	}

	/** F6: Caregiver name if this is a managed profile, null otherwise. */
	get managedBy(): string | null {
		return this.activeInfo?.managed_by ?? null;
	}

	get isAiAvailable(): boolean {
		return this.aiStatus?.ollama_available ?? false;
	}
}

export const profile = new ProfileStore();
