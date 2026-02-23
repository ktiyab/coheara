<!-- Documents tab — upload history + capture button -->
<script lang="ts">
	import { isConnected } from '$lib/stores/connection.js';
	import { processingDocuments, hasProcessingDocuments } from '$lib/stores/capture.js';
</script>

<div class="documents-screen">
	<div class="documents-header">
		<h1>Documents</h1>
		<a class="capture-btn" href="/documents/capture" aria-label="Capture document">
			+
		</a>
	</div>

	{#if $hasProcessingDocuments}
		<section class="processing-section" aria-label="Processing documents">
			<h2>Processing</h2>
			{#each $processingDocuments as doc (doc.documentId)}
				<div class="processing-card">
					<span class="processing-icon" aria-hidden="true">&#8987;</span>
					<div class="processing-info">
						<span class="processing-title">{doc.pageCount} page{doc.pageCount > 1 ? 's' : ''}</span>
						<span class="processing-status">Being analyzed by desktop...</span>
					</div>
				</div>
			{/each}
		</section>
	{/if}

	<section class="documents-list" aria-label="Uploaded documents">
		<div class="empty-state">
			<p>Documents you upload will appear here.</p>
			<p class="hint">Tap + to capture a document with your camera.</p>
		</div>
	</section>

	{#if !$isConnected}
		<p class="offline-note">Captured documents will be sent when you reconnect.</p>
	{/if}
</div>

<style>
	.documents-screen {
		padding: 16px;
	}

	.documents-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 20px;
	}

	h1 {
		font-size: 20px;
		font-weight: 700;
		margin: 0;
	}

	.capture-btn {
		width: 40px;
		height: 40px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: var(--color-primary);
		color: white;
		border-radius: 50%;
		text-decoration: none;
		font-size: 24px;
		font-weight: 600;
		min-height: var(--min-touch-target);
	}

	h2 {
		font-size: 13px;
		font-weight: 600;
		color: var(--color-text-muted);
		text-transform: uppercase;
		letter-spacing: 0.5px;
		margin: 0 0 8px;
	}

	.processing-section {
		margin-bottom: 20px;
	}

	.processing-card {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 14px 16px;
		background: white;
		border: 1px solid #E7E5E4;
		border-radius: 12px;
		margin-bottom: 8px;
	}

	.processing-icon {
		font-size: 20px;
	}

	.processing-info {
		display: flex;
		flex-direction: column;
	}

	.processing-title {
		font-size: 16px;
		font-weight: 600;
	}

	.processing-status {
		font-size: 13px;
		color: var(--color-text-muted);
		margin-top: 2px;
	}

	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		min-height: 200px;
		text-align: center;
		padding: 24px;
	}

	.empty-state p {
		color: var(--color-text-muted);
		font-size: 16px;
		margin: 0;
	}

	.hint {
		font-size: 14px;
		margin-top: 8px;
	}

	.offline-note {
		text-align: center;
		font-size: 13px;
		color: var(--color-text-muted);
		margin-top: 16px;
	}
</style>
