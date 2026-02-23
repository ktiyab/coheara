<!-- M1-03: Timeline screen — scrollable cards, date-grouped, type-filtered -->
<script lang="ts">
	import { isConnected } from '$lib/stores/connection.js';
	import { timelineEvents, lastSyncTimestamp, profile } from '$lib/stores/cache.js';
	import {
		groupTimelineByDate,
		filterTimelineEvents,
		emptyStateMessage
	} from '$lib/utils/viewer.js';
	import type { TimelineFilter as FilterType } from '$lib/types/viewer.js';
	import FreshnessIndicator from '$lib/components/viewer/FreshnessIndicator.svelte';
	import TimelineCard from '$lib/components/viewer/TimelineCard.svelte';
	import TimelineFilter from '$lib/components/viewer/TimelineFilter.svelte';

	let activeFilter = $state<FilterType>('all');

	const filtered = $derived(filterTimelineEvents($timelineEvents, activeFilter));
	const grouped = $derived(groupTimelineByDate(filtered));
</script>

<div class="timeline-screen">
	<div class="timeline-header">
		<FreshnessIndicator
			syncTimestamp={$lastSyncTimestamp}
			profileName={$profile?.name}
		/>
	</div>

	<div class="title-row">
		<h1>Timeline</h1>
	</div>

	<TimelineFilter active={activeFilter} onChange={(f) => activeFilter = f} />

	{#if $timelineEvents.length === 0}
		<div class="empty-state">
			<p>{emptyStateMessage('timeline')}</p>
		</div>
	{:else if filtered.length === 0}
		<div class="empty-state">
			<p>No events match this filter</p>
		</div>
	{:else}
		<div class="timeline-list" role="list" aria-label="Timeline events">
			{#each grouped as group (group.date)}
				<div class="date-group">
					<h3 class="date-label">{group.label}</h3>
					<div class="group-events">
						{#each group.events as event (event.id)}
							<TimelineCard {event} />
						{/each}
					</div>
				</div>
			{/each}
		</div>

		{#if !$isConnected}
			<p class="cached-note">Showing last {$timelineEvents.length} events</p>
		{/if}
	{/if}
</div>

<style>
	.timeline-screen {
		padding: 16px;
	}

	.timeline-header {
		margin-bottom: 8px;
	}

	.title-row {
		margin-bottom: 4px;
	}

	h1 {
		font-size: 20px;
		font-weight: 700;
		margin: 0;
	}

	.date-group {
		margin-bottom: 16px;
	}

	.date-label {
		font-size: 13px;
		font-weight: 600;
		color: var(--color-text-muted);
		text-transform: uppercase;
		letter-spacing: 0.5px;
		margin: 0 0 8px 4px;
		padding-bottom: 4px;
		border-bottom: 1px solid #E7E5E4;
	}

	.group-events {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.cached-note {
		text-align: center;
		font-size: 13px;
		color: var(--color-text-muted);
		margin-top: 16px;
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
