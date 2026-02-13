<!-- M1-02: Citation detail bottom sheet — shows source document info (RS-M1-02-003) -->
<script lang="ts">
	import type { Citation } from '$lib/types/chat.js';

	const { citation, onClose }: {
		citation: Citation | null;
		onClose: () => void;
	} = $props();

	const relevancePercent = $derived(citation ? Math.round(citation.relevanceScore * 100) : 0);
</script>

{#if citation}
	<div class="sheet-backdrop" onclick={onClose} role="presentation" aria-hidden="true">
	</div>
	<div
		class="sheet-panel"
		role="dialog"
		aria-label="Source document details"
		aria-modal="true"
	>
		<div class="sheet-handle" aria-hidden="true"></div>

		<h2 class="sheet-title">{citation.documentTitle}</h2>

		{#if citation.documentDate}
			<span class="sheet-date">{citation.documentDate}</span>
		{/if}

		{#if citation.professionalName}
			<div class="detail-row">
				<span class="detail-label">Professional</span>
				<span class="detail-value">{citation.professionalName}</span>
			</div>
		{/if}

		<div class="detail-row">
			<span class="detail-label">Relevance</span>
			<div class="relevance-group">
				<div
					class="relevance-bar"
					role="progressbar"
					aria-valuenow={relevancePercent}
					aria-valuemin={0}
					aria-valuemax={100}
					aria-label="Relevance: {relevancePercent}%"
				>
					<div class="relevance-fill" style="width: {relevancePercent}%"></div>
				</div>
				<span class="relevance-value">{relevancePercent}%</span>
			</div>
		</div>

		{#if citation.chunkText}
			<div class="excerpt-section">
				<span class="detail-label">Excerpt</span>
				<blockquote class="excerpt-text">{citation.chunkText}</blockquote>
			</div>
		{/if}

		<button class="close-btn" onclick={onClose} aria-label="Close citation detail">
			Close
		</button>
	</div>
{/if}

<style>
	.sheet-backdrop {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.4);
		z-index: 100;
	}

	.sheet-panel {
		position: fixed;
		bottom: 0;
		left: 0;
		right: 0;
		background: white;
		border-radius: 16px 16px 0 0;
		padding: 12px 20px 24px;
		z-index: 101;
		max-height: 70vh;
		overflow-y: auto;
		-webkit-overflow-scrolling: touch;
	}

	.sheet-handle {
		width: 40px;
		height: 4px;
		background: #D6D3D1;
		border-radius: 2px;
		margin: 0 auto 16px;
	}

	.sheet-title {
		font-size: 18px;
		font-weight: 700;
		margin: 0 0 4px;
	}

	.sheet-date {
		font-size: 14px;
		color: var(--color-text-muted);
		display: block;
		margin-bottom: 16px;
	}

	.detail-row {
		margin-bottom: 14px;
	}

	.detail-label {
		display: block;
		font-size: 12px;
		font-weight: 600;
		color: var(--color-text-muted);
		text-transform: uppercase;
		letter-spacing: 0.5px;
		margin-bottom: 4px;
	}

	.detail-value {
		font-size: 16px;
	}

	.relevance-group {
		display: flex;
		align-items: center;
		gap: 10px;
	}

	.relevance-bar {
		flex: 1;
		height: 8px;
		background: #E7E5E4;
		border-radius: 4px;
		overflow: hidden;
	}

	.relevance-fill {
		height: 100%;
		background: var(--color-primary);
		border-radius: 4px;
		transition: width 0.3s ease;
	}

	.relevance-value {
		font-size: 14px;
		font-weight: 600;
		color: var(--color-primary);
		min-width: 40px;
		text-align: right;
	}

	.excerpt-section {
		margin-bottom: 16px;
	}

	.excerpt-text {
		margin: 4px 0 0;
		padding: 12px;
		background: #F5F5F4;
		border-left: 3px solid var(--color-primary);
		border-radius: 0 8px 8px 0;
		font-size: 14px;
		line-height: 1.6;
		color: #44403C;
	}

	.close-btn {
		width: 100%;
		padding: 14px;
		background: var(--color-surface);
		border: 1px solid #D6D3D1;
		border-radius: 12px;
		font-size: 16px;
		font-weight: 600;
		cursor: pointer;
		min-height: 44px;
	}
</style>
