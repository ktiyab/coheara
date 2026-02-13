<!-- M1-05: Capture preview — review before send (single or multi-page) -->
<script lang="ts">
	import type { CapturedPage } from '$lib/types/capture.js';
	import { qualityLabel } from '$lib/utils/capture.js';
	import PageThumbnails from './PageThumbnails.svelte';

	const { pages, selectedIndex, onSelectPage, onRetake, onAddPage, onSend }: {
		pages: CapturedPage[];
		selectedIndex: number;
		onSelectPage: (index: number) => void;
		onRetake: () => void;
		onAddPage: () => void;
		onSend: () => void;
	} = $props();

	const currentPage = $derived(pages[selectedIndex]);
	const qualitySummary = $derived(currentPage ? qualityLabel(currentPage.quality) : '');
	const sendLabel = $derived(
		pages.length === 1
			? 'Send to Desktop'
			: `Send ${pages.length} Pages to Desktop`
	);
</script>

<div class="capture-preview">
	<div class="preview-header">
		<h2>Preview</h2>
		<span class="page-info">
			{pages.length === 1 ? '1 page' : `${pages.length} pages`}
		</span>
	</div>

	{#if pages.length > 1}
		<PageThumbnails
			{pages}
			{selectedIndex}
			{onSelectPage}
			{onAddPage}
		/>
	{/if}

	{#if currentPage}
		<div class="preview-image-container">
			<img
				src={currentPage.dataUrl}
				alt={`Page ${selectedIndex + 1} preview`}
				class="preview-image"
			/>
		</div>

		<p class="quality-summary">Quality: {qualitySummary}</p>
	{/if}

	<div class="preview-actions">
		<button class="action-btn secondary" onclick={onRetake}>
			Retake{pages.length > 1 ? ' This Page' : ''}
		</button>
		{#if pages.length <= 1}
			<button class="action-btn secondary" onclick={onAddPage}>
				+ Page
			</button>
		{/if}
	</div>

	<button class="send-btn" onclick={onSend}>
		{sendLabel}
	</button>

	<p class="send-note">
		Send to your desktop for processing. You'll be notified when it's ready to review.
	</p>
</div>

<style>
	.capture-preview {
		display: flex;
		flex-direction: column;
		padding: 16px;
		height: 100%;
	}

	.preview-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 12px;
	}

	h2 {
		font-size: 18px;
		font-weight: 700;
		margin: 0;
	}

	.page-info {
		font-size: 14px;
		color: var(--color-text-muted);
	}

	.preview-image-container {
		flex: 1;
		min-height: 200px;
		max-height: 400px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: #F5F5F4;
		border-radius: 12px;
		overflow: hidden;
		margin-bottom: 12px;
	}

	.preview-image {
		max-width: 100%;
		max-height: 100%;
		object-fit: contain;
	}

	.quality-summary {
		font-size: 14px;
		color: var(--color-text-muted);
		margin: 0 0 12px;
	}

	.preview-actions {
		display: flex;
		gap: 8px;
		margin-bottom: 12px;
	}

	.action-btn {
		flex: 1;
		padding: 12px;
		border: 1px solid #D6D3D1;
		border-radius: 12px;
		background: white;
		font-size: 15px;
		font-weight: 500;
		cursor: pointer;
		font-family: inherit;
		min-height: var(--min-touch-target);
	}

	.send-btn {
		width: 100%;
		padding: 16px;
		background: var(--color-primary);
		color: white;
		border: none;
		border-radius: 12px;
		font-size: 17px;
		font-weight: 600;
		cursor: pointer;
		font-family: inherit;
		min-height: var(--large-touch-target);
	}

	.send-note {
		font-size: 13px;
		color: var(--color-text-muted);
		text-align: center;
		margin: 8px 0 0;
		line-height: 1.4;
	}
</style>
