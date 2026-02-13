<!-- M1-03: Timeline card — single event row with type icon -->
<script lang="ts">
	import type { CachedTimelineEvent } from '$lib/types/viewer.js';
	import { timelineEventIcon, timelineEventColor } from '$lib/utils/viewer.js';

	const { event }: { event: CachedTimelineEvent } = $props();

	const icon = $derived(timelineEventIcon(event.eventType));
	const bgColor = $derived(timelineEventColor(event.eventType));
	const timeFormatted = $derived(formatTime(event.timestamp));

	function formatTime(iso: string): string {
		return new Date(iso).toLocaleTimeString('en-US', { hour: 'numeric', minute: '2-digit' });
	}
</script>

<div
	class="timeline-card"
	class:patient-reported={event.isPatientReported}
	role="listitem"
	aria-label="{event.title}, {event.description}, {timeFormatted}"
>
	<div class="icon-badge" style="background: {bgColor}" aria-hidden="true">
		{icon}
	</div>
	<div class="card-content">
		<div class="card-title">{event.title}</div>
		<div class="card-description">{event.description}</div>
		{#if event.isPatientReported}
			<div class="patient-label">Your note</div>
		{/if}
		<div class="card-time">{timeFormatted}</div>
	</div>
</div>

<style>
	.timeline-card {
		display: flex;
		gap: 12px;
		padding: 12px;
		background: white;
		border: 1px solid #E7E5E4;
		border-radius: 12px;
	}

	.timeline-card.patient-reported {
		border-left: 3px solid var(--color-accent);
	}

	.icon-badge {
		width: 36px;
		height: 36px;
		border-radius: 8px;
		display: flex;
		align-items: center;
		justify-content: center;
		color: white;
		font-size: 11px;
		font-weight: 700;
		flex-shrink: 0;
	}

	.card-content {
		flex: 1;
		min-width: 0;
	}

	.card-title {
		font-size: 15px;
		font-weight: 600;
		color: var(--color-text);
		margin-bottom: 2px;
	}

	.card-description {
		font-size: 14px;
		color: var(--color-text-muted);
		line-height: 1.4;
	}

	.patient-label {
		font-size: 12px;
		font-weight: 500;
		color: var(--color-accent);
		margin-top: 4px;
	}

	.card-time {
		font-size: 12px;
		color: var(--color-text-muted);
		margin-top: 4px;
	}
</style>
