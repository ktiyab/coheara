<!-- M1-03: Lab abnormal banner — top section if any abnormal results exist -->
<script lang="ts">
	import type { CachedLabResult } from '$lib/types/viewer.js';
	import LabResultCard from './LabResultCard.svelte';

	const { labs, onTapLab }: {
		labs: CachedLabResult[];
		onTapLab: (testName: string) => void;
	} = $props();
</script>

{#if labs.length > 0}
	<section class="abnormal-section" aria-label="Abnormal lab results">
		<h3 class="section-label">Abnormal</h3>
		<div class="abnormal-list">
			{#each labs as lab (lab.id)}
				<LabResultCard {lab} onTap={onTapLab} />
			{/each}
		</div>
	</section>
{/if}

<style>
	.abnormal-section {
		margin-bottom: 20px;
		padding: 12px;
		background: #FEF2F2;
		border-radius: 12px;
	}

	.section-label {
		font-size: 13px;
		font-weight: 600;
		color: var(--color-error);
		text-transform: uppercase;
		letter-spacing: 0.5px;
		margin: 0 0 8px 4px;
	}

	.abnormal-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
</style>
