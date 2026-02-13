<!-- M1-04: Severity picker — face icons + slider (Mamadou: one-tap) -->
<script lang="ts">
	import {
		SEVERITY_FACES,
		SEVERITY_FACE_VALUES,
		SEVERITY_FACE_LABELS,
		SEVERITY_FACE_EMOJI
	} from '$lib/types/journal.js';
	import type { SeverityFace } from '$lib/types/journal.js';

	const { severity, onSeverityChange }: {
		severity: number;
		onSeverityChange: (value: number) => void;
	} = $props();

	const activeFace = $derived(
		SEVERITY_FACES.find((f) => SEVERITY_FACE_VALUES[f] === severity) ?? null
	);

	function handleFaceTap(face: SeverityFace): void {
		onSeverityChange(SEVERITY_FACE_VALUES[face]);
	}

	function handleSlider(event: Event): void {
		const target = event.target as HTMLInputElement;
		onSeverityChange(parseInt(target.value, 10));
	}
</script>

<div class="severity-picker">
	<p class="picker-label">How are you feeling?</p>

	<div class="face-row" role="radiogroup" aria-label="Severity">
		{#each SEVERITY_FACES as face (face)}
			<button
				class="face-btn"
				class:active={activeFace === face}
				role="radio"
				aria-checked={activeFace === face}
				aria-label="{SEVERITY_FACE_LABELS[face]}, severity {SEVERITY_FACE_VALUES[face]} out of 10"
				onclick={() => handleFaceTap(face)}
			>
				<span class="face-emoji" aria-hidden="true">{SEVERITY_FACE_EMOJI[face]}</span>
				<span class="face-label">{SEVERITY_FACE_LABELS[face]}</span>
			</button>
		{/each}
	</div>

	<div class="slider-row">
		<input
			type="range"
			min="1"
			max="10"
			value={severity}
			class="severity-slider"
			oninput={handleSlider}
			aria-label="Severity slider, {severity} out of 10"
		/>
		<span class="severity-value">{severity}/10</span>
	</div>
</div>

<style>
	.severity-picker {
		margin-bottom: 20px;
	}

	.picker-label {
		font-size: 16px;
		font-weight: 600;
		margin: 0 0 12px;
	}

	.face-row {
		display: flex;
		justify-content: space-between;
		gap: 4px;
		margin-bottom: 12px;
	}

	.face-btn {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 4px;
		padding: 8px;
		border: 2px solid transparent;
		border-radius: 12px;
		background: #F5F5F4;
		cursor: pointer;
		flex: 1;
		min-width: 0;
		min-height: var(--min-touch-target);
		font-family: inherit;
	}

	.face-btn.active {
		border-color: var(--color-primary);
		background: #EFF6FF;
	}

	.face-emoji {
		font-size: 28px;
		line-height: 1;
	}

	.face-label {
		font-size: 11px;
		color: var(--color-text-muted);
		white-space: nowrap;
	}

	.slider-row {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.severity-slider {
		flex: 1;
		height: 32px;
		accent-color: var(--color-primary);
	}

	.severity-value {
		font-size: 16px;
		font-weight: 600;
		min-width: 40px;
		text-align: right;
	}
</style>
