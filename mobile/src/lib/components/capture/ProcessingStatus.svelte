<!-- M1-05: Processing status card — shows on home screen during/after desktop processing -->
<script lang="ts">
	import type { DocumentStatus } from '$lib/types/capture.js';
	import { PROCESSING_STAGE_LABELS } from '$lib/types/capture.js';
	import { processingStageText } from '$lib/utils/capture.js';

	const { status, onDismiss }: {
		status: DocumentStatus;
		onDismiss: () => void;
	} = $props();
</script>

{#if status.state === 'processing'}
	<div class="status-card processing" role="status" aria-live="polite">
		<div class="status-icon" aria-hidden="true">&#128196;</div>
		<div class="status-content">
			<p class="status-title">Document processing...</p>
			<p class="status-detail">
				{processingStageText(status.stage, status.pageCount)}
			</p>
		</div>
	</div>
{:else if status.state === 'complete'}
	<div class="status-card complete" role="status">
		<div class="status-icon" aria-hidden="true">&#128196;</div>
		<div class="status-content">
			<p class="status-title">New document ready</p>
			<p class="status-detail">{status.title}</p>
			<p class="status-hint">Ready to review on desktop</p>
		</div>
		<button class="dismiss-btn" onclick={onDismiss} aria-label="Dismiss">
			&times;
		</button>
	</div>
{:else if status.state === 'failed'}
	<div class="status-card failed" role="alert">
		<div class="status-icon" aria-hidden="true">&#128196;</div>
		<div class="status-content">
			<p class="status-title error">Processing failed</p>
			<p class="status-detail">{status.reason}</p>
		</div>
		<button class="dismiss-btn" onclick={onDismiss} aria-label="Dismiss">
			&times;
		</button>
	</div>
{/if}

<style>
	.status-card {
		display: flex;
		align-items: flex-start;
		gap: 12px;
		padding: 14px 16px;
		background: white;
		border-radius: 12px;
		border: 1px solid #E7E5E4;
	}

	.status-card.processing {
		border-left: 3px solid var(--color-primary);
	}

	.status-card.complete {
		border-left: 3px solid var(--color-success);
	}

	.status-card.failed {
		border-left: 3px solid var(--color-error);
	}

	.status-icon {
		font-size: 20px;
		flex-shrink: 0;
		margin-top: 2px;
	}

	.status-content {
		flex: 1;
		min-width: 0;
	}

	.status-title {
		font-size: 15px;
		font-weight: 600;
		margin: 0 0 2px;
	}

	.status-title.error {
		color: var(--color-error);
	}

	.status-detail {
		font-size: 14px;
		color: var(--color-text-muted);
		margin: 0;
	}

	.status-hint {
		font-size: 13px;
		color: var(--color-text-muted);
		margin: 4px 0 0;
	}

	.dismiss-btn {
		flex-shrink: 0;
		width: 32px;
		height: 32px;
		display: flex;
		align-items: center;
		justify-content: center;
		border: none;
		background: transparent;
		font-size: 20px;
		color: var(--color-text-muted);
		cursor: pointer;
		border-radius: 50%;
		min-height: var(--min-touch-target);
		min-width: var(--min-touch-target);
	}
</style>
