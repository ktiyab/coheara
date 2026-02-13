<!-- M1-05: Document Capture screen — camera → preview → upload → done -->
<script lang="ts">
	import { isConnected } from '$lib/stores/connection.js';
	import {
		captureSession,
		captureStep,
		selectedPageIndex,
		uploadProgress,
		pageCount,
		canAddPage,
		startCaptureSession,
		addPage,
		cancelCapture,
		setUploadStarted,
		updateUploadProgress,
		setUploadComplete,
		setUploadFailed,
		addProcessingDocument,
		queueForOffline
	} from '$lib/stores/capture.js';
	import { makeGoodQuality } from '$lib/utils/capture.js';
	import type { QualityCheck } from '$lib/types/capture.js';
	import CameraOverlay from '$lib/components/capture/CameraOverlay.svelte';
	import QualityIndicator from '$lib/components/capture/QualityIndicator.svelte';
	import CaptureButton from '$lib/components/capture/CaptureButton.svelte';
	import CapturePreview from '$lib/components/capture/CapturePreview.svelte';
	import UploadProgress from '$lib/components/capture/UploadProgress.svelte';

	// Start a session on mount
	$effect(() => {
		if (!$captureSession) {
			startCaptureSession();
		}
	});

	// Simulated quality state (in real Capacitor, this comes from camera preview frames)
	let currentQuality = $state<QualityCheck>(makeGoodQuality());

	function handleCapture(): void {
		// In real Capacitor: Camera.getPhoto() → receives dataUrl
		// For now: simulate capture with placeholder
		const mockDataUrl = 'data:image/jpeg;base64,/9j/placeholder';
		addPage(mockDataUrl, 2400, 3200, 1500000, currentQuality);
		$captureStep = 'preview';
	}

	function handleRetake(): void {
		$captureStep = 'camera';
	}

	function handleAddPage(): void {
		$captureStep = 'camera';
	}

	function handleSend(): void {
		if (!$captureSession || $captureSession.pages.length === 0) return;

		if (!$isConnected) {
			// Queue for offline
			queueForOffline($captureSession.pages);
			$captureStep = 'done';
			return;
		}

		// Start upload
		const pages = $captureSession.pages;
		setUploadStarted(pages.length);

		// Simulate upload progress (in real app: actual fetch with progress events)
		let percent = 0;
		const interval = setInterval(() => {
			percent += 20;
			if (percent >= 100) {
				clearInterval(interval);
				setUploadComplete();
				addProcessingDocument(`doc-${Date.now()}`, pages.length);
			} else {
				updateUploadProgress(percent, 1);
			}
		}, 500);
	}

	function handleCancel(): void {
		cancelCapture();
	}
</script>

<div class="capture-screen">
	{#if $captureStep === 'camera'}
		<div class="camera-header">
			<h1>Capture Document</h1>
			<a class="close-btn" href="/more" aria-label="Close">&times;</a>
		</div>

		{#if !$isConnected}
			<div class="offline-notice">
				<p class="offline-text">
					Camera is available, but you need to be connected to your desktop to send documents.
				</p>
			</div>
		{/if}

		<div class="camera-viewport">
			<div class="camera-placeholder">
				<CameraOverlay quality={currentQuality} />
			</div>
		</div>

		<div class="camera-controls">
			<QualityIndicator quality={currentQuality} />
			<div class="capture-btn-wrapper">
				<CaptureButton disabled={false} onCapture={handleCapture} />
			</div>
		</div>

	{:else if $captureStep === 'preview'}
		<div class="preview-header">
			<a class="back-btn" href="/more" aria-label="Cancel">&larr;</a>
		</div>

		{#if $captureSession}
			<CapturePreview
				pages={$captureSession.pages}
				selectedIndex={$selectedPageIndex}
				onSelectPage={(i) => $selectedPageIndex = i}
				onRetake={handleRetake}
				onAddPage={handleAddPage}
				onSend={handleSend}
			/>
		{/if}

	{:else if $captureStep === 'uploading'}
		<div class="upload-screen">
			<UploadProgress
				progress={$uploadProgress}
				pages={$captureSession?.pages ?? []}
				onCancel={handleCancel}
			/>
		</div>

	{:else if $captureStep === 'done'}
		<div class="done-screen">
			{#if $isConnected}
				<p class="done-check" aria-hidden="true">&#10003;</p>
				<p class="done-title">Document sent!</p>
				<p class="done-detail">
					You'll get a notification when processing is complete.
				</p>
			{:else}
				<p class="done-check" aria-hidden="true">&#128247;</p>
				<p class="done-title">Photo saved</p>
				<p class="done-detail">
					It will be sent when you connect to your desktop.
				</p>
			{/if}
			<a class="done-link" href="/more">Done</a>
		</div>
	{/if}
</div>

<style>
	.capture-screen {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 100dvh;
		background: #1A1A1A;
		color: white;
	}

	.camera-header, .preview-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 16px;
	}

	h1 {
		font-size: 18px;
		font-weight: 600;
		margin: 0;
		color: white;
	}

	.close-btn, .back-btn {
		width: 40px;
		height: 40px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: rgba(255, 255, 255, 0.15);
		border-radius: 50%;
		text-decoration: none;
		color: white;
		font-size: 22px;
	}

	.offline-notice {
		padding: 12px 16px;
		margin: 0 16px;
		background: rgba(255, 255, 255, 0.1);
		border-radius: 8px;
	}

	.offline-text {
		font-size: 14px;
		color: #D6D3D1;
		margin: 0;
		line-height: 1.4;
	}

	.camera-viewport {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 16px;
		position: relative;
	}

	.camera-placeholder {
		width: 100%;
		height: 100%;
		min-height: 300px;
		background: #2A2A2A;
		border-radius: 12px;
		position: relative;
		overflow: hidden;
	}

	.camera-controls {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 16px;
		padding: 16px 16px 32px;
	}

	.capture-btn-wrapper {
		display: flex;
		justify-content: center;
	}

	.upload-screen {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 24px;
	}

	.done-screen {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 24px;
		text-align: center;
	}

	.done-check {
		font-size: 48px;
		margin: 0 0 12px;
	}

	.done-title {
		font-size: 20px;
		font-weight: 600;
		margin: 0 0 8px;
	}

	.done-detail {
		font-size: 15px;
		color: #D6D3D1;
		margin: 0 0 24px;
		line-height: 1.4;
	}

	.done-link {
		padding: 14px 32px;
		background: var(--color-primary);
		color: white;
		border-radius: 12px;
		text-decoration: none;
		font-size: 16px;
		font-weight: 600;
		min-height: var(--min-touch-target);
		display: flex;
		align-items: center;
	}
</style>
