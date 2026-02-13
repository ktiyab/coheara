<!-- M2-01: SLM Status — download progress + cancel -->
<script lang="ts">
	import { downloadProgress, cancelDownload, isDownloading } from '$lib/stores/slm.js';

	let {
		onCancel
	}: {
		onCancel?: () => void;
	} = $props();

	let progress = $derived($downloadProgress);
	let percent = $derived(progress?.percent ?? 0);
	let downloadedMb = $derived(progress ? Math.round(progress.downloadedBytes / (1024 * 1024)) : 0);
	let totalMb = $derived(progress ? Math.round(progress.totalBytes / (1024 * 1024)) : 0);

	function handleCancel() {
		cancelDownload();
		onCancel?.();
	}
</script>

{#if $isDownloading && progress}
	<div class="slm-status" role="region" aria-label="AI model download progress">
		<h3 class="status-heading">Downloading AI Model</h3>

		<div
			class="progress-bar"
			role="progressbar"
			aria-valuenow={percent}
			aria-valuemin={0}
			aria-valuemax={100}
			aria-label="Download progress: {percent}%"
		>
			<div class="progress-fill" style="width: {percent}%"></div>
		</div>

		<p class="progress-text">
			{downloadedMb} MB / {totalMb} MB · {Math.round(percent)}%
		</p>

		<button class="btn-cancel" onclick={handleCancel}>
			Cancel Download
		</button>

		<p class="status-footer">Download continues in the background.</p>
	</div>
{/if}

<style>
	.slm-status {
		padding: var(--spacing-lg, 1.5rem);
		background: var(--color-surface, #fff);
		border-radius: var(--radius-lg, 12px);
	}

	.status-heading {
		font-size: 1.125rem;
		font-weight: 600;
		margin: 0 0 1rem;
	}

	.progress-bar {
		height: 8px;
		background: var(--color-border, #e0e0e0);
		border-radius: 4px;
		overflow: hidden;
		margin-bottom: 0.5rem;
	}

	.progress-fill {
		height: 100%;
		background: var(--color-primary, #2563eb);
		border-radius: 4px;
		transition: width 0.3s ease;
	}

	.progress-text {
		font-size: 0.875rem;
		color: var(--color-text-muted, #666);
		margin: 0 0 1rem;
	}

	.btn-cancel {
		width: 100%;
		padding: 0.75rem;
		background: transparent;
		color: var(--color-error, #dc2626);
		border: 1px solid var(--color-error, #dc2626);
		border-radius: var(--radius-md, 8px);
		font-size: 0.9375rem;
		cursor: pointer;
		min-height: 48px;
	}

	.status-footer {
		text-align: center;
		font-size: 0.8125rem;
		color: var(--color-text-muted, #666);
		margin-top: 0.75rem;
	}
</style>
