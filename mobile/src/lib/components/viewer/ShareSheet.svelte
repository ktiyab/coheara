<!-- M1-03: Share sheet — reusable text-only share component (BP-02) -->
<script lang="ts">
	import type { SharePayload } from '$lib/types/viewer.js';
	import { formatShareText } from '$lib/utils/viewer.js';

	const { payload, onClose }: {
		payload: SharePayload;
		onClose: () => void;
	} = $props();

	const fullText = $derived(formatShareText(payload));

	let copied = $state(false);

	async function handleCopy(): Promise<void> {
		try {
			await navigator.clipboard.writeText(fullText);
			copied = true;
			setTimeout(() => { copied = false; }, 2000);
		} catch {
			// Fallback: select text for manual copy
		}
	}
</script>

<div class="share-overlay" role="dialog" aria-label="Share {payload.title}">
	<div class="share-sheet">
		<div class="sheet-header">
			<h2 class="sheet-title">Share</h2>
			<button class="close-btn" onclick={onClose} aria-label="Close share">&times;</button>
		</div>

		<div class="sheet-body">
			<div class="preview">
				<p class="preview-title">{payload.title}</p>
				<pre class="preview-text">{payload.text}</pre>
				<p class="preview-meta">{payload.timestamp}</p>
				<p class="preview-disclaimer">{payload.disclaimer}</p>
			</div>

			<button class="copy-btn" onclick={handleCopy}>
				{copied ? 'Copied!' : 'Copy to Clipboard'}
			</button>
		</div>
	</div>
</div>

<style>
	.share-overlay {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.4);
		display: flex;
		align-items: flex-end;
		z-index: 50;
	}

	.share-sheet {
		width: 100%;
		max-height: 80vh;
		background: white;
		border-radius: 16px 16px 0 0;
		overflow-y: auto;
		-webkit-overflow-scrolling: touch;
	}

	.sheet-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 16px;
		border-bottom: 1px solid #E7E5E4;
		position: sticky;
		top: 0;
		background: white;
	}

	.sheet-title {
		font-size: 18px;
		font-weight: 700;
		margin: 0;
	}

	.close-btn {
		width: 36px;
		height: 36px;
		border: none;
		background: #F5F5F4;
		border-radius: 50%;
		font-size: 20px;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.sheet-body {
		padding: 16px;
	}

	.preview {
		background: #F5F5F4;
		border-radius: 12px;
		padding: 16px;
		margin-bottom: 16px;
	}

	.preview-title {
		font-weight: 600;
		font-size: 15px;
		margin: 0 0 12px;
	}

	.preview-text {
		font-size: 14px;
		line-height: 1.5;
		white-space: pre-wrap;
		font-family: inherit;
		margin: 0 0 12px;
	}

	.preview-meta {
		font-size: 12px;
		color: var(--color-text-muted);
		margin: 0 0 4px;
	}

	.preview-disclaimer {
		font-size: 12px;
		color: #92400E;
		font-style: italic;
		margin: 0;
	}

	.copy-btn {
		width: 100%;
		padding: 14px;
		background: var(--color-primary);
		color: white;
		border: none;
		border-radius: 12px;
		font-size: 16px;
		font-weight: 600;
		cursor: pointer;
		min-height: var(--min-touch-target);
	}
</style>
