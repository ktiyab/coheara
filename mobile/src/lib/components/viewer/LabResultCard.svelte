<!-- M1-03: Lab result card — single lab row with trend arrow + reference range -->
<script lang="ts">
	import type { CachedLabResult } from '$lib/types/viewer.js';
	import LabTrendIndicator from './LabTrendIndicator.svelte';

	const { lab, onTap }: {
		lab: CachedLabResult;
		onTap: (testName: string) => void;
	} = $props();

	const dateFormatted = $derived(formatLabDate(lab.testedAt));

	function formatLabDate(iso: string): string {
		return new Date(iso).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
	}
</script>

<button
	class="lab-card"
	class:abnormal={lab.isAbnormal}
	onclick={() => onTap(lab.testName)}
	aria-label="{lab.testName}, {lab.value} {lab.unit}, reference range {lab.referenceMin} to {lab.referenceMax}, {lab.isAbnormal ? 'abnormal' : 'normal'}, trend {lab.trendContext}, tested {dateFormatted}"
>
	<div class="card-top">
		{#if lab.isAbnormal}
			<span class="abnormal-icon" aria-hidden="true">&#9888;</span>
		{/if}
		<span class="test-name">{lab.testName}</span>
		<span class="value">{lab.value} {lab.unit}</span>
	</div>
	<div class="card-bottom">
		<span class="reference">Ref: {lab.referenceMin}-{lab.referenceMax}</span>
		<LabTrendIndicator trend={lab.trend} context={lab.trendContext} />
	</div>
	<div class="card-date">
		{dateFormatted}
		{#if lab.labName}
			<span class="separator" aria-hidden="true">&middot;</span>
			{lab.labName}
		{/if}
	</div>
</button>

<style>
	.lab-card {
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

	.lab-card:active {
		background: #F5F5F4;
	}

	.lab-card.abnormal {
		border-color: var(--color-error);
		border-width: 2px;
	}

	.card-top {
		display: flex;
		align-items: baseline;
		gap: 8px;
		margin-bottom: 4px;
	}

	.abnormal-icon {
		color: var(--color-error);
		font-size: 14px;
	}

	.test-name {
		font-size: 16px;
		font-weight: 600;
		color: var(--color-text);
	}

	.value {
		font-size: 16px;
		color: var(--color-text);
		margin-left: auto;
	}

	.card-bottom {
		display: flex;
		align-items: center;
		gap: 12px;
		margin-bottom: 2px;
	}

	.reference {
		font-size: 13px;
		color: var(--color-text-muted);
	}

	.card-date {
		font-size: 13px;
		color: var(--color-text-muted);
	}
</style>
