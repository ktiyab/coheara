<!-- M1-06: Staleness warning banner — amber/red based on freshness tier -->
<script lang="ts">
	import type { FreshnessTier } from '$lib/types/cache-manager.js';
	import {
		shouldShowStalenessWarning,
		stalenessWarningMessage,
		freshnessTierColor
	} from '$lib/stores/cache-manager.js';

	const { tier }: {
		tier: FreshnessTier;
	} = $props();

	const show = $derived(shouldShowStalenessWarning(tier));
	const message = $derived(stalenessWarningMessage(tier));
	const color = $derived(freshnessTierColor(tier));
</script>

{#if show}
	<div
		class="staleness-warning"
		class:amber={tier === 'amber'}
		class:red={tier === 'red'}
		style="--warning-color: {color}"
		role="alert"
	>
		<span class="warning-icon" aria-hidden="true">&#9888;</span>
		<p class="warning-text">{message}</p>
	</div>
{/if}

<style>
	.staleness-warning {
		display: flex;
		align-items: flex-start;
		gap: 8px;
		padding: 12px 16px;
		border-radius: 8px;
		margin-bottom: 12px;
	}

	.staleness-warning.amber {
		background: #FEF3C7;
		border: 1px solid #F59E0B;
	}

	.staleness-warning.red {
		background: #FEE2E2;
		border: 1px solid #EF4444;
	}

	.warning-icon {
		font-size: 16px;
		flex-shrink: 0;
		margin-top: 1px;
		color: var(--warning-color);
	}

	.warning-text {
		font-size: 14px;
		line-height: 1.4;
		margin: 0;
		color: var(--color-text);
	}
</style>
