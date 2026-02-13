<!-- M1-05: Real-time quality indicator — "Ready to capture" / "Move closer" etc -->
<script lang="ts">
	import type { QualityCheck } from '$lib/types/capture.js';
	import { getQualityHint, qualityColor } from '$lib/utils/capture.js';

	const { quality }: {
		quality: QualityCheck;
	} = $props();

	const hint = $derived(getQualityHint(quality));
	const color = $derived(qualityColor(quality));
</script>

<div
	class="quality-indicator"
	class:ready={quality.ready}
	style="--indicator-color: {color}"
	role="status"
	aria-live="polite"
>
	<span class="quality-dot" aria-hidden="true"></span>
	<span class="quality-text">{hint.message}</span>
</div>

<style>
	.quality-indicator {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 10px 16px;
		background: rgba(0, 0, 0, 0.6);
		border-radius: 20px;
		color: white;
	}

	.quality-dot {
		width: 10px;
		height: 10px;
		border-radius: 50%;
		background: var(--indicator-color);
		flex-shrink: 0;
	}

	.quality-text {
		font-size: 15px;
		font-weight: 500;
	}
</style>
