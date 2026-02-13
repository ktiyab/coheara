<!-- M1-03: Lab results screen — abnormal banner, sorted list, share -->
<script lang="ts">
	import { isConnected } from '$lib/stores/connection.js';
	import {
		labResultsSorted,
		abnormalLabs,
		lastSyncTimestamp,
		profile
	} from '$lib/stores/cache.js';
	import { shareLabSummary, emptyStateMessage } from '$lib/utils/viewer.js';
	import type { SharePayload } from '$lib/types/viewer.js';
	import FreshnessIndicator from '$lib/components/viewer/FreshnessIndicator.svelte';
	import LabAbnormalBanner from '$lib/components/viewer/LabAbnormalBanner.svelte';
	import LabResultCard from '$lib/components/viewer/LabResultCard.svelte';
	import ShareSheet from '$lib/components/viewer/ShareSheet.svelte';

	let sharePayload = $state<SharePayload | null>(null);

	const normalLabs = $derived(
		$labResultsSorted.filter((l) => !l.isAbnormal)
	);

	function handleTapLab(testName: string): void {
		// Lab detail/trend view — requires desktop connection for history
		// Future: open lab detail bottom sheet
	}

	function handleShare(): void {
		sharePayload = shareLabSummary(
			$labResultsSorted,
			$profile?.name ?? 'Patient',
			$lastSyncTimestamp
		);
	}
</script>

<div class="labs-screen">
	<div class="labs-header">
		<FreshnessIndicator
			syncTimestamp={$lastSyncTimestamp}
			profileName={$profile?.name}
		/>
	</div>

	<h1>Lab Results</h1>

	{#if $labResultsSorted.length === 0}
		<div class="empty-state">
			<p>{emptyStateMessage('labs')}</p>
		</div>
	{:else}
		<LabAbnormalBanner labs={$abnormalLabs} onTapLab={handleTapLab} />

		{#if normalLabs.length > 0}
			<section aria-label="Recent lab results">
				<h3 class="section-label">Recent Results</h3>
				<div class="lab-list">
					{#each normalLabs as lab (lab.id)}
						<LabResultCard {lab} onTap={handleTapLab} />
					{/each}
				</div>
			</section>
		{/if}

		<div class="share-area">
			<button class="share-btn" onclick={handleShare}>
				Share Lab Summary
			</button>
		</div>
	{/if}
</div>

{#if sharePayload}
	<ShareSheet payload={sharePayload} onClose={() => sharePayload = null} />
{/if}

<style>
	.labs-screen {
		padding: 16px;
	}

	.labs-header {
		margin-bottom: 8px;
	}

	h1 {
		font-size: 20px;
		font-weight: 700;
		margin: 0 0 16px;
	}

	.section-label {
		font-size: 13px;
		font-weight: 600;
		color: var(--color-text-muted);
		text-transform: uppercase;
		letter-spacing: 0.5px;
		margin: 0 0 8px 4px;
	}

	.lab-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.share-area {
		padding: 16px 0;
	}

	.share-btn {
		width: 100%;
		padding: 14px;
		background: white;
		color: var(--color-primary);
		border: 2px solid var(--color-primary);
		border-radius: 12px;
		font-size: 16px;
		font-weight: 600;
		cursor: pointer;
		font-family: inherit;
		min-height: var(--min-touch-target);
	}

	.empty-state {
		display: flex;
		align-items: center;
		justify-content: center;
		min-height: 200px;
		text-align: center;
		padding: 24px;
	}

	.empty-state p {
		color: var(--color-text-muted);
		font-size: 16px;
		line-height: 1.5;
	}
</style>
