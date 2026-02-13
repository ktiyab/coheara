<!-- M1-04: Symptom chips — quick symptom selection (Dr. Diallo: "clinical short-circuits") -->
<script lang="ts">
	import { SYMPTOM_CHIPS, SYMPTOM_CHIP_LABELS } from '$lib/types/journal.js';
	import type { SymptomChip } from '$lib/types/journal.js';

	const { selected, onSelect }: {
		selected: SymptomChip | null;
		onSelect: (chip: SymptomChip | null) => void;
	} = $props();

	function handleTap(chip: SymptomChip): void {
		onSelect(selected === chip ? null : chip);
	}
</script>

<div class="symptom-chips">
	<p class="chips-label">What's bothering you? <span class="optional">(optional)</span></p>
	<div class="chips-row" role="radiogroup" aria-label="Symptom type">
		{#each SYMPTOM_CHIPS as chip (chip)}
			<button
				class="chip"
				class:active={selected === chip}
				role="radio"
				aria-checked={selected === chip}
				onclick={() => handleTap(chip)}
			>
				{SYMPTOM_CHIP_LABELS[chip]}
			</button>
		{/each}
	</div>
</div>

<style>
	.symptom-chips {
		margin-bottom: 20px;
	}

	.chips-label {
		font-size: 16px;
		font-weight: 600;
		margin: 0 0 8px;
	}

	.optional {
		font-weight: 400;
		color: var(--color-text-muted);
		font-size: 14px;
	}

	.chips-row {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}

	.chip {
		padding: 8px 16px;
		border: 1px solid #D6D3D1;
		border-radius: 20px;
		background: white;
		font-size: 15px;
		cursor: pointer;
		font-family: inherit;
		min-height: 40px;
	}

	.chip.active {
		background: var(--color-primary);
		color: white;
		border-color: var(--color-primary);
	}
</style>
