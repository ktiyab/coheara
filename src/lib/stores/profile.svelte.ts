/**
 * E2E-F06: Global profile store.
 *
 * Replaces prop-drilled `profileName` and `aiStatus` with a singleton
 * reactive store. Loaded once on app mount, accessible everywhere.
 */

import type { AiStatus } from '$lib/api/profile';

class ProfileStore {
	name = $state('');
	aiStatus = $state<AiStatus | null>(null);

	get isAiAvailable(): boolean {
		return this.aiStatus?.ollama_available ?? false;
	}
}

export const profile = new ProfileStore();
