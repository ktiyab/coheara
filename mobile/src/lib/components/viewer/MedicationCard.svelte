<!-- M1-03: Medication card — single medication row in schedule group -->
<script lang="ts">
	import type { CachedMedication } from '$lib/types/viewer.js';

	const { medication, onTap }: {
		medication: CachedMedication;
		onTap: (id: string) => void;
	} = $props();

	const sinceLabel = $derived(formatSince(medication.since));

	function formatSince(since: string): string {
		const d = new Date(since);
		const month = d.toLocaleDateString('en-US', { month: 'short' });
		const year = d.getFullYear();
		return `Since ${month} ${year}`;
	}
</script>

<button
	class="medication-card"
	class:discontinued={!medication.isActive}
	onclick={() => onTap(medication.id)}
	aria-label="{medication.name} {medication.dose}, {medication.frequency}, prescribed by {medication.prescriber}, {medication.purpose}, {sinceLabel}"
>
	<div class="card-header">
		<span class="name">{medication.name}</span>
		<span class="dose">{medication.dose}</span>
	</div>
	<div class="card-detail">
		{#if medication.isActive}
			<span>{medication.frequency} &middot; {medication.prescriber}</span>
		{:else}
			<span class="discontinued-label">DISCONTINUED {medication.discontinuedDate ?? ''}</span>
			<span>Was: {medication.frequency} &middot; {medication.prescriber}</span>
		{/if}
	</div>
	<div class="card-meta">
		<span>{medication.purpose}</span>
		{#if medication.isActive}
			<span class="separator" aria-hidden="true">&middot;</span>
			<span>{sinceLabel}</span>
		{/if}
	</div>
</button>

<style>
	.medication-card {
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

	.medication-card:active {
		background: #F5F5F4;
	}

	.medication-card.discontinued {
		opacity: 0.7;
	}

	.card-header {
		display: flex;
		align-items: baseline;
		gap: 8px;
		margin-bottom: 4px;
	}

	.name {
		font-size: 16px;
		font-weight: 600;
		color: var(--color-text);
	}

	.dose {
		font-size: 16px;
		color: var(--color-text);
	}

	.card-detail {
		font-size: 14px;
		color: var(--color-text-muted);
		line-height: 1.4;
	}

	.card-meta {
		display: flex;
		align-items: center;
		gap: 4px;
		font-size: 13px;
		color: var(--color-text-muted);
		margin-top: 2px;
	}

	.discontinued-label {
		font-size: 12px;
		font-weight: 600;
		color: var(--color-error);
		text-transform: uppercase;
		display: block;
		margin-bottom: 2px;
	}
</style>
