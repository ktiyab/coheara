<!-- M1-03: Medication card — single medication row (aligned CA-05 desktop types) -->
<script lang="ts">
	import type { CachedMedication } from '$lib/types/viewer.js';

	const { medication, onTap }: {
		medication: CachedMedication;
		onTap: (id: string) => void;
	} = $props();

	const isActive = $derived(medication.status === 'active');
	const startLabel = $derived(medication.startDate ? formatSince(medication.startDate) : '');

	function formatSince(since: string): string {
		const d = new Date(since);
		const month = d.toLocaleDateString('en-US', { month: 'short' });
		const year = d.getFullYear();
		return `Since ${month} ${year}`;
	}
</script>

<button
	class="medication-card"
	class:discontinued={!isActive}
	onclick={() => onTap(medication.id)}
	aria-label="{medication.genericName} {medication.dose}, {medication.frequency}, {medication.prescriberName ? `prescribed by ${medication.prescriberName},` : ''} {medication.condition ?? ''}, {startLabel}"
>
	<div class="card-header">
		<span class="name">{medication.genericName}</span>
		<span class="dose">{medication.dose}</span>
	</div>
	<div class="card-detail">
		{#if isActive}
			<span>{medication.frequency}{medication.prescriberName ? ` \u00B7 ${medication.prescriberName}` : ''}</span>
		{:else}
			<span class="discontinued-label">DISCONTINUED {medication.endDate ?? ''}</span>
			<span>Was: {medication.frequency}{medication.prescriberName ? ` \u00B7 ${medication.prescriberName}` : ''}</span>
		{/if}
	</div>
	<div class="card-meta">
		{#if medication.condition}
			<span>{medication.condition}</span>
		{/if}
		{#if isActive && startLabel}
			{#if medication.condition}
				<span class="separator" aria-hidden="true">&middot;</span>
			{/if}
			<span>{startLabel}</span>
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
