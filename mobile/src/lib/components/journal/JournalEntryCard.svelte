<!-- M1-04: Journal entry card — single entry in history list -->
<script lang="ts">
	import type { JournalEntry } from '$lib/types/journal.js';
	import { SEVERITY_FACE_EMOJI, SEVERITY_FACES, SEVERITY_FACE_VALUES, BODY_REGION_LABELS } from '$lib/types/journal.js';

	const { entry, onTap }: {
		entry: JournalEntry;
		onTap: (id: string) => void;
	} = $props();

	const faceEmoji = $derived(getSeverityEmoji(entry.severity));
	const chipLabel = $derived(entry.symptomChip
		? entry.symptomChip.charAt(0).toUpperCase() + entry.symptomChip.slice(1)
		: ''
	);
	const bodyLabel = $derived(
		entry.bodyLocations.map((r) => BODY_REGION_LABELS[r]).join(', ') || ''
	);
	const timeLabel = $derived(formatTime(entry.createdAt));
	const syncLabel = $derived(entry.synced ? 'Synced' : 'Pending');
	const truncatedText = $derived(
		entry.freeText.length > 50 ? entry.freeText.slice(0, 50) + '...' : entry.freeText
	);

	function getSeverityEmoji(severity: number): string {
		const face = SEVERITY_FACES.find((f) => SEVERITY_FACE_VALUES[f] >= severity)
			?? SEVERITY_FACES[SEVERITY_FACES.length - 1];
		return SEVERITY_FACE_EMOJI[face];
	}

	function formatTime(iso: string): string {
		return new Date(iso).toLocaleTimeString('en-US', { hour: 'numeric', minute: '2-digit' });
	}
</script>

<button
	class="entry-card"
	onclick={() => onTap(entry.id)}
	aria-label="Journal entry, severity {entry.severity} out of 10, {chipLabel || 'no category'}, {timeLabel}"
>
	<div class="card-top">
		<span class="face" aria-hidden="true">{faceEmoji}</span>
		<span class="chip-label">{chipLabel || 'General'}</span>
		<span class="severity">{entry.severity}/10</span>
	</div>
	{#if entry.freeText}
		<div class="card-text">"{truncatedText}"</div>
	{/if}
	<div class="card-bottom">
		{#if bodyLabel}
			<span class="body-location">{bodyLabel}</span>
			<span class="separator" aria-hidden="true">&middot;</span>
		{/if}
		<span class="time">{timeLabel}</span>
		<span class="sync-badge" class:synced={entry.synced} class:pending={!entry.synced}>
			[{syncLabel}]
		</span>
	</div>
</button>

<style>
	.entry-card {
		display: block;
		width: 100%;
		text-align: left;
		padding: 14px 16px;
		background: white;
		border: 1px solid #E7E5E4;
		border-radius: 12px;
		cursor: pointer;
		min-height: var(--min-touch-target);
		font-family: inherit;
	}

	.entry-card:active {
		background: #F5F5F4;
	}

	.card-top {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-bottom: 4px;
	}

	.face {
		font-size: 22px;
	}

	.chip-label {
		font-size: 16px;
		font-weight: 600;
		color: var(--color-text);
	}

	.severity {
		margin-left: auto;
		font-size: 15px;
		font-weight: 600;
		color: var(--color-text-muted);
	}

	.card-text {
		font-size: 14px;
		color: var(--color-text-muted);
		font-style: italic;
		margin-bottom: 4px;
		line-height: 1.4;
	}

	.card-bottom {
		display: flex;
		align-items: center;
		gap: 4px;
		font-size: 13px;
		color: var(--color-text-muted);
	}

	.sync-badge {
		margin-left: auto;
		font-size: 11px;
		font-weight: 500;
	}

	.sync-badge.synced {
		color: var(--color-success);
	}

	.sync-badge.pending {
		color: var(--color-warning);
	}
</style>
