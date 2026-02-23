<!-- M1-03: Lab trend indicator — arrow + label + color (aligned CA-05) -->
<script lang="ts">
	import type { LabTrend } from '$lib/types/viewer.js';
	import { trendArrow, trendLabel, trendColor } from '$lib/utils/viewer.js';

	const { trend, isAbnormal }: {
		trend: LabTrend;
		isAbnormal: boolean;
	} = $props();

	const arrow = $derived(trendArrow(trend));
	const label = $derived(trendLabel(trend));
	const color = $derived(trendColor(trend, isAbnormal));
</script>

<span
	class="trend-indicator"
	style="color: {color}"
	aria-label="Trend: {label}"
>
	<span class="arrow" aria-hidden="true">{arrow}</span>
	{label}
</span>

<style>
	.trend-indicator {
		display: inline-flex;
		align-items: center;
		gap: 3px;
		font-size: 13px;
		font-weight: 500;
	}

	.arrow {
		font-size: 14px;
	}
</style>
