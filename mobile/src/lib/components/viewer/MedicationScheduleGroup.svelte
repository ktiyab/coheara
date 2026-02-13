<!-- M1-03: Medication schedule group — "Morning", "Evening", etc. -->
<script lang="ts">
	import type { CachedMedication } from '$lib/types/viewer.js';
	import MedicationCard from './MedicationCard.svelte';

	const { label, medications, onTapMedication }: {
		label: string;
		medications: CachedMedication[];
		onTapMedication: (id: string) => void;
	} = $props();
</script>

{#if medications.length > 0}
	<section class="schedule-group" aria-label="{label} medications">
		<h3 class="group-label">{label}</h3>
		<div class="group-list">
			{#each medications as med (med.id)}
				<MedicationCard medication={med} onTap={onTapMedication} />
			{/each}
		</div>
	</section>
{/if}

<style>
	.schedule-group {
		margin-bottom: 20px;
	}

	.group-label {
		font-size: 13px;
		font-weight: 600;
		color: var(--color-text-muted);
		text-transform: uppercase;
		letter-spacing: 0.5px;
		margin: 0 0 8px 4px;
		padding-bottom: 4px;
		border-bottom: 1px solid #E7E5E4;
	}

	.group-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
</style>
