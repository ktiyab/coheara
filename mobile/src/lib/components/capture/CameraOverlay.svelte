<!-- M1-05: Camera overlay — guide frame with quality-reactive border color -->
<script lang="ts">
	import type { QualityCheck } from '$lib/types/capture.js';
	import { frameBorderColor } from '$lib/utils/capture.js';

	const { quality }: {
		quality: QualityCheck;
	} = $props();

	const borderColor = $derived(frameBorderColor(quality));
</script>

<div class="camera-overlay">
	<div class="guide-frame" style="--frame-color: {borderColor}" aria-hidden="true">
		<!-- Four corner markers -->
		<div class="corner top-left"></div>
		<div class="corner top-right"></div>
		<div class="corner bottom-left"></div>
		<div class="corner bottom-right"></div>
	</div>
	<p class="guide-text">Place one document inside the frame</p>
</div>

<style>
	.camera-overlay {
		position: absolute;
		inset: 0;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		pointer-events: none;
	}

	.guide-frame {
		position: relative;
		width: 85%;
		aspect-ratio: 0.707;  /* A4 portrait ratio */
		border: 2px solid var(--frame-color);
		border-radius: 8px;
	}

	.corner {
		position: absolute;
		width: 24px;
		height: 24px;
		border-color: var(--frame-color);
		border-style: solid;
	}

	.corner.top-left {
		top: -2px;
		left: -2px;
		border-width: 4px 0 0 4px;
		border-radius: 4px 0 0 0;
	}

	.corner.top-right {
		top: -2px;
		right: -2px;
		border-width: 4px 4px 0 0;
		border-radius: 0 4px 0 0;
	}

	.corner.bottom-left {
		bottom: -2px;
		left: -2px;
		border-width: 0 0 4px 4px;
		border-radius: 0 0 0 4px;
	}

	.corner.bottom-right {
		bottom: -2px;
		right: -2px;
		border-width: 0 4px 4px 0;
		border-radius: 0 0 4px 0;
	}

	.guide-text {
		margin-top: 16px;
		color: white;
		font-size: 15px;
		text-align: center;
		text-shadow: 0 1px 3px rgba(0, 0, 0, 0.6);
	}
</style>
