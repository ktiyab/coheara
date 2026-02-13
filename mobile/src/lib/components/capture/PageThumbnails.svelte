<!-- M1-05: Multi-page thumbnail strip (Thomas flow) -->
<script lang="ts">
	import type { CapturedPage } from '$lib/types/capture.js';
	import { canAddPage } from '$lib/stores/capture.js';

	const { pages, selectedIndex, onSelectPage, onAddPage }: {
		pages: CapturedPage[];
		selectedIndex: number;
		onSelectPage: (index: number) => void;
		onAddPage: () => void;
	} = $props();
</script>

<div class="thumbnail-strip" role="tablist" aria-label="Captured pages">
	{#each pages as page, i (page.id)}
		<button
			class="thumbnail"
			class:selected={i === selectedIndex}
			onclick={() => onSelectPage(i)}
			role="tab"
			aria-selected={i === selectedIndex}
			aria-label={`Page ${i + 1}`}
		>
			<img src={page.dataUrl} alt={`Page ${i + 1}`} class="thumb-img" />
			<span class="thumb-number">{i + 1}</span>
		</button>
	{/each}

	{#if $canAddPage}
		<button
			class="thumbnail add-page"
			onclick={onAddPage}
			aria-label="Add another page"
		>
			<span class="add-icon">+</span>
		</button>
	{/if}
</div>

<style>
	.thumbnail-strip {
		display: flex;
		gap: 8px;
		padding: 8px 16px;
		overflow-x: auto;
		-webkit-overflow-scrolling: touch;
	}

	.thumbnail {
		position: relative;
		flex-shrink: 0;
		width: 56px;
		height: 72px;
		border: 2px solid #D6D3D1;
		border-radius: 6px;
		overflow: hidden;
		padding: 0;
		cursor: pointer;
		background: white;
	}

	.thumbnail.selected {
		border-color: var(--color-primary);
		border-width: 3px;
	}

	.thumb-img {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}

	.thumb-number {
		position: absolute;
		bottom: 2px;
		right: 2px;
		background: rgba(0, 0, 0, 0.6);
		color: white;
		font-size: 11px;
		font-weight: 600;
		padding: 1px 5px;
		border-radius: 4px;
	}

	.add-page {
		display: flex;
		align-items: center;
		justify-content: center;
		background: #F5F5F4;
		border-style: dashed;
	}

	.add-icon {
		font-size: 24px;
		color: var(--color-text-muted);
	}
</style>
