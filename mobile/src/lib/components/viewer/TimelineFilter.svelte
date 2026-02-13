<!-- M1-03: Timeline filter — single filter: event type -->
<script lang="ts">
	import type { TimelineFilter as FilterType } from '$lib/types/viewer.js';

	const { active, onChange }: {
		active: FilterType;
		onChange: (filter: FilterType) => void;
	} = $props();

	const filters: Array<{ value: FilterType; label: string }> = [
		{ value: 'all', label: 'All' },
		{ value: 'medication_change', label: 'Meds' },
		{ value: 'lab_result', label: 'Labs' },
		{ value: 'alert', label: 'Alerts' },
		{ value: 'journal', label: 'Journal' },
		{ value: 'document', label: 'Docs' }
	];
</script>

<div class="filter-row" role="radiogroup" aria-label="Filter timeline events">
	{#each filters as f (f.value)}
		<button
			class="filter-chip"
			class:active={active === f.value}
			role="radio"
			aria-checked={active === f.value}
			onclick={() => onChange(f.value)}
		>
			{f.label}
		</button>
	{/each}
</div>

<style>
	.filter-row {
		display: flex;
		gap: 6px;
		padding: 8px 0;
		overflow-x: auto;
		-webkit-overflow-scrolling: touch;
	}

	.filter-chip {
		padding: 6px 14px;
		border: 1px solid #D6D3D1;
		border-radius: 20px;
		background: white;
		font-size: 14px;
		font-family: inherit;
		cursor: pointer;
		white-space: nowrap;
		min-height: 36px;
	}

	.filter-chip.active {
		background: var(--color-primary);
		color: white;
		border-color: var(--color-primary);
	}
</style>
