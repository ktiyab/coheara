/**
 * F6: All-profiles list store.
 *
 * Separate from the active-profile store (profile.svelte.ts).
 * Holds the full list of profiles from disk (unencrypted metadata).
 * Used by the sidebar popover and profiles management screen.
 */

import { listProfiles } from '$lib/api/profile';
import type { ProfileInfo } from '$lib/types/profile';

class ProfilesStore {
	all = $state<ProfileInfo[]>([]);
	loading = $state(false);

	/** Profiles managed by the given caregiver name. */
	managedBy(caregiverName: string): ProfileInfo[] {
		return this.all.filter((p) => p.managed_by === caregiverName);
	}

	/** Check if a profile has dependents (managed profiles referencing it). */
	hasDependents(profileName: string): boolean {
		return this.all.some((p) => p.managed_by === profileName);
	}

	/** Refresh from backend (reads unencrypted profiles.json). */
	async refresh() {
		this.loading = true;
		try {
			this.all = await listProfiles();
		} finally {
			this.loading = false;
		}
	}
}

export const profiles = new ProfilesStore();
