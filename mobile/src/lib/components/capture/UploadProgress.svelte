<!-- M1-05: Upload progress indicator — progress bar + cancel -->
<script lang="ts">
	import type { UploadProgress } from '$lib/types/capture.js';
	import { uploadProgressText, formatFileSize, totalUploadSize } from '$lib/utils/capture.js';
	import type { CapturedPage } from '$lib/types/capture.js';

	const { progress, pages, onCancel }: {
		progress: UploadProgress;
		pages: CapturedPage[];
		onCancel: () => void;
	} = $props();

	const progressText = $derived(
		uploadProgressText(progress.currentPage, progress.totalPages, progress.percent)
	);
	const totalSize = $derived(formatFileSize(totalUploadSize(pages)));
</script>

<div class="upload-progress">
	{#if progress.status === 'uploading'}
		<div class="progress-card">
			<p class="progress-title">Sending to desktop...</p>
			<div class="progress-bar-container">
				<div class="progress-bar" style="width: {progress.percent}%"></div>
			</div>
			<p class="progress-detail">{progressText}</p>
			<p class="progress-size">{totalSize}</p>
			<button class="cancel-btn" onclick={onCancel}>Cancel Upload</button>
		</div>
	{:else if progress.status === 'upload_complete'}
		<div class="complete-card">
			<p class="complete-check" aria-hidden="true">&#10003;</p>
			<p class="complete-title">Document sent!</p>
			<p class="complete-detail">
				{progress.totalPages} page{progress.totalPages !== 1 ? 's' : ''} uploaded.
			</p>
			<p class="complete-note">
				You'll get a notification when processing is complete.
				Review this document on your desktop.
			</p>
		</div>
	{:else if progress.status === 'upload_failed'}
		<div class="error-card">
			<p class="error-title">Upload failed</p>
			<p class="error-detail">{progress.errorMessage}</p>
		</div>
	{/if}
</div>

<style>
	.progress-card, .complete-card, .error-card {
		padding: 24px;
		background: white;
		border-radius: 16px;
		text-align: center;
		border: 1px solid #E7E5E4;
	}

	.progress-title {
		font-size: 17px;
		font-weight: 600;
		margin: 0 0 16px;
	}

	.progress-bar-container {
		height: 8px;
		background: #E7E5E4;
		border-radius: 4px;
		overflow: hidden;
		margin-bottom: 8px;
	}

	.progress-bar {
		height: 100%;
		background: var(--color-primary);
		border-radius: 4px;
		transition: width 0.3s ease;
	}

	.progress-detail {
		font-size: 15px;
		color: var(--color-text);
		margin: 0 0 4px;
	}

	.progress-size {
		font-size: 13px;
		color: var(--color-text-muted);
		margin: 0 0 16px;
	}

	.cancel-btn {
		padding: 10px 20px;
		border: 1px solid #D6D3D1;
		border-radius: 8px;
		background: white;
		font-size: 14px;
		cursor: pointer;
		font-family: inherit;
		color: var(--color-text-muted);
		min-height: var(--min-touch-target);
	}

	.complete-check {
		font-size: 48px;
		color: var(--color-success);
		margin: 0 0 12px;
	}

	.complete-title {
		font-size: 18px;
		font-weight: 600;
		margin: 0 0 4px;
	}

	.complete-detail {
		font-size: 15px;
		color: var(--color-text);
		margin: 0 0 12px;
	}

	.complete-note {
		font-size: 14px;
		color: var(--color-text-muted);
		line-height: 1.4;
		margin: 0;
	}

	.error-title {
		font-size: 17px;
		font-weight: 600;
		color: var(--color-error);
		margin: 0 0 8px;
	}

	.error-detail {
		font-size: 15px;
		color: var(--color-text-muted);
		margin: 0;
	}
</style>
