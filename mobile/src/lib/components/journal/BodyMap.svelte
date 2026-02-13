<!-- M1-04: Body map — tappable regions (Viktor: simple SVG, no 3D) -->
<script lang="ts">
	import type { BodyRegion } from '$lib/types/journal.js';
	import { BODY_REGION_LABELS } from '$lib/types/journal.js';

	const { selected, onToggleRegion }: {
		selected: BodyRegion[];
		onToggleRegion: (region: BodyRegion) => void;
	} = $props();

	const selectedSet = $derived(new Set(selected));
	const selectedLabels = $derived(
		selected.map((r) => BODY_REGION_LABELS[r]).join(', ') || 'None selected'
	);

	// Simplified body regions as tappable buttons (not a full SVG silhouette)
	const regionGroups: Array<{ label: string; regions: BodyRegion[] }> = [
		{ label: 'Head & Neck', regions: ['head', 'face', 'neck'] },
		{ label: 'Chest', regions: ['chest_left', 'chest_center', 'chest_right'] },
		{ label: 'Abdomen', regions: ['abdomen_upper', 'abdomen_lower'] },
		{ label: 'Back', regions: ['back_upper', 'back_lower'] },
		{ label: 'Shoulders', regions: ['shoulder_left', 'shoulder_right'] },
		{ label: 'Arms & Hands', regions: ['arm_left', 'arm_right', 'hand_left', 'hand_right'] },
		{ label: 'Hips', regions: ['hip_left', 'hip_right'] },
		{ label: 'Legs & Feet', regions: ['leg_left', 'leg_right', 'knee_left', 'knee_right', 'foot_left', 'foot_right'] }
	];
</script>

<div class="body-map">
	<p class="map-label">Where does it hurt? <span class="optional">(optional)</span></p>
	<p class="map-selection" aria-live="polite">Selected: {selectedLabels}</p>

	<div class="region-groups">
		{#each regionGroups as group (group.label)}
			<div class="region-group">
				<span class="group-label">{group.label}</span>
				<div class="region-buttons">
					{#each group.regions as region (region)}
						<button
							class="region-btn"
							class:active={selectedSet.has(region)}
							role="checkbox"
							aria-checked={selectedSet.has(region)}
							aria-label="{BODY_REGION_LABELS[region]}, {selectedSet.has(region) ? 'selected' : 'not selected'}"
							onclick={() => onToggleRegion(region)}
						>
							{BODY_REGION_LABELS[region]}
						</button>
					{/each}
				</div>
			</div>
		{/each}
	</div>
</div>

<style>
	.body-map {
		margin-bottom: 20px;
	}

	.map-label {
		font-size: 16px;
		font-weight: 600;
		margin: 0 0 4px;
	}

	.optional {
		font-weight: 400;
		color: var(--color-text-muted);
		font-size: 14px;
	}

	.map-selection {
		font-size: 13px;
		color: var(--color-text-muted);
		margin: 0 0 12px;
	}

	.region-groups {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.region-group {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.group-label {
		font-size: 12px;
		font-weight: 600;
		color: var(--color-text-muted);
		text-transform: uppercase;
		letter-spacing: 0.3px;
	}

	.region-buttons {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}

	.region-btn {
		padding: 6px 12px;
		border: 1px solid #D6D3D1;
		border-radius: 8px;
		background: white;
		font-size: 13px;
		cursor: pointer;
		font-family: inherit;
		min-height: 36px;
	}

	.region-btn.active {
		background: var(--color-accent);
		color: white;
		border-color: var(--color-accent);
	}
</style>
