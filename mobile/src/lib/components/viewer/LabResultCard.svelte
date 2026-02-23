<!-- M1-03: Lab result card — single lab row with trend + reference range (aligned CA-05) -->
<script lang="ts">
	import type { CachedLabResult, LabTrend } from '$lib/types/viewer.js';
	import LabTrendIndicator from './LabTrendIndicator.svelte';

	const { lab, onTap }: {
		lab: CachedLabResult;
		onTap: (testName: string) => void;
	} = $props();

	const dateFormatted = $derived(formatLabDate(lab.collectionDate));
	const displayValue = $derived(lab.valueText ?? (lab.value != null ? String(lab.value) : ''));
	const hasReference = $derived(lab.referenceRangeLow != null && lab.referenceRangeHigh != null);

	function formatLabDate(iso: string): string {
		return new Date(iso).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
	}
</script>

<button
	class="lab-card"
	class:abnormal={lab.isAbnormal}
	onclick={() => onTap(lab.testName)}
	aria-label="{lab.testName}, {displayValue} {lab.unit ?? ''}, {hasReference ? `reference range ${lab.referenceRangeLow} to ${lab.referenceRangeHigh},` : ''} {lab.isAbnormal ? 'abnormal' : 'normal'}, tested {dateFormatted}"
>
	<div class="card-top">
		{#if lab.isAbnormal}
			<span class="abnormal-icon" aria-hidden="true">&#9888;</span>
		{/if}
		<span class="test-name">{lab.testName}</span>
		<span class="value">{displayValue} {lab.unit ?? ''}</span>
	</div>
	<div class="card-bottom">
		{#if hasReference}
			<span class="reference">Ref: {lab.referenceRangeLow}-{lab.referenceRangeHigh}</span>
		{/if}
		{#if lab.trendDirection}
			<LabTrendIndicator trend={lab.trendDirection as LabTrend} isAbnormal={lab.isAbnormal} />
		{/if}
	</div>
	<div class="card-date">
		{dateFormatted}
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
