/**
 * E2E-F06 + S.5: Global profile store.
 *
 * S.5: aiStatus is kept for backward compatibility but is now synced
 * from the ai store (single source of truth). Use `ai.isAiAvailable`
 * for the canonical check.
 */

import type { AiStatus } from '$lib/api/profile';

class ProfileStore {
	name = $state('');
	/** Spec 45 [PU-04]: Profile color index from 8-color palette. */
	colorIndex = $state<number | null>(null);
	/** @deprecated S.5: Use ai store directly. Kept for backward compat in +page.svelte. */
	aiStatus = $state<AiStatus | null>(null);

	get isAiAvailable(): boolean {
		return this.aiStatus?.ollama_available ?? false;
	}
}

export const profile = new ProfileStore();
