<!-- M1-03: Freshness indicator — sync timestamp display -->
<script lang="ts">
	import { computeFreshness, freshnessLabel, freshnessColor } from '$lib/utils/viewer.js';

	const { syncTimestamp, profileName }: { syncTimestamp: string | null; profileName?: string } = $props();

	const level = $derived(computeFreshness(syncTimestamp));
	const label = $derived(freshnessLabel(syncTimestamp));
	const color = $derived(freshnessColor(level));
</script>

<div class="freshness" aria-label="Sync status: {label}">
	{#if profileName}
		<span class="profile-name">{profileName}</span>
		<span class="separator" aria-hidden="true">&middot;</span>
	{/if}
	<span class="dot" style="background: {color}" aria-hidden="true"></span>
	<span class="label">{label}</span>
</div>

<style>
	.freshness {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 13px;
		color: var(--color-text-muted);
	}

	.profile-name {
		font-weight: 600;
		color: var(--color-text);
	}

	.dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		flex-shrink: 0;
	}

	.label {
		white-space: nowrap;
	}
</style>
