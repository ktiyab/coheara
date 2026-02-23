<!-- M1-05: Document Capture screen — camera → preview → upload → done -->
<script lang="ts">
	import { get } from 'svelte/store';
	import { isConnected } from '$lib/stores/connection.js';
	import {
		captureSession,
		captureStep,
		selectedPageIndex,
		uploadProgress,
		pageCount,
		canAddPage,
		startCaptureSession,
		addPageSafe,
		cancelCapture,
		setUploadStarted,
		setUploadComplete,
		setUploadFailed,
		addProcessingDocument,
		queueForOffline
	} from '$lib/stores/capture.js';
	import { evaluateQuality, makeGoodQuality } from '$lib/utils/capture.js';
	import { uploadDocument } from '$lib/api/capture.js';
	import type { QualityCheck } from '$lib/types/capture.js';
	import { JPEG_QUALITY, TARGET_MAX_WIDTH } from '$lib/types/capture.js';
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

	// Camera state
	let videoEl: HTMLVideoElement | undefined = $state();
	let canvasEl: HTMLCanvasElement | undefined = $state();
	let stream: MediaStream | null = null;
	let qualityFrameId = 0;
	let cameraReady = $state(false);
	let cameraError = $state('');

	let currentQuality = $state<QualityCheck>(makeGoodQuality());

	// Upload abort controller
	let uploadAbort: AbortController | null = null;

	// Start camera when entering camera step
	$effect(() => {
		if ($captureStep === 'camera') {
			startCamera();
		}
		return () => {
			stopCamera();
		};
	});

	async function startCamera(): Promise<void> {
		cameraReady = false;
		cameraError = '';
		try {
			stream = await navigator.mediaDevices.getUserMedia({
				video: { facingMode: 'environment', width: { ideal: 2400 }, height: { ideal: 3200 } }
			});
			if (videoEl) {
				videoEl.srcObject = stream;
				await videoEl.play();
				cameraReady = true;
				startQualityLoop();
			}
		} catch (err) {
			const msg = err instanceof Error ? err.message : 'Unknown error';
			if (msg.includes('NotAllowed') || msg.includes('Permission')) {
				cameraError = 'Camera access denied. Enable camera in your device settings.';
			} else {
				cameraError = `Camera error: ${msg}`;
			}
		}
	}

	function stopCamera(): void {
		cancelAnimationFrame(qualityFrameId);
		if (stream) {
			for (const track of stream.getTracks()) {
				track.stop();
			}
			stream = null;
		}
		if (videoEl) {
			videoEl.srcObject = null;
		}
		cameraReady = false;
	}

	function startQualityLoop(): void {
		if (!videoEl || !canvasEl) return;

		const ctx = canvasEl.getContext('2d', { willReadFrequently: true });
		if (!ctx) return;

		function analyzeFrame(): void {
			if (!videoEl || !canvasEl || !ctx || videoEl.readyState < videoEl.HAVE_ENOUGH_DATA) {
				qualityFrameId = requestAnimationFrame(analyzeFrame);
				return;
			}

			// Downsample for fast analysis
			const w = Math.min(320, videoEl.videoWidth);
			const h = Math.round((w / videoEl.videoWidth) * videoEl.videoHeight);
			canvasEl.width = w;
			canvasEl.height = h;
			ctx.drawImage(videoEl, 0, 0, w, h);

			const imageData = ctx.getImageData(0, 0, w, h);
			const brightness = analyzeBrightness(imageData);

			// Simplified quality — brightness is real, others default to ok
			currentQuality = evaluateQuality(brightness, 150, true, 4, 0.7, 5);

			// Run at ~5 fps for quality feedback
			qualityFrameId = setTimeout(() => requestAnimationFrame(analyzeFrame), 200) as unknown as number;
		}

		qualityFrameId = requestAnimationFrame(analyzeFrame);
	}

	function analyzeBrightness(imageData: ImageData): number {
		const data = imageData.data;
		let sum = 0;
		const step = 16; // Sample every 16th pixel for speed
		let count = 0;
		for (let i = 0; i < data.length; i += 4 * step) {
			// Luminance: 0.299R + 0.587G + 0.114B
			sum += 0.299 * data[i] + 0.587 * data[i + 1] + 0.114 * data[i + 2];
			count++;
		}
		return count > 0 ? sum / count : 128;
	}

	async function handleCapture(): Promise<void> {
		if (!videoEl || !canvasEl) return;

		// Capture full-resolution frame
		const vw = videoEl.videoWidth;
		const vh = videoEl.videoHeight;

		// Scale to target width
		const scale = vw > TARGET_MAX_WIDTH ? TARGET_MAX_WIDTH / vw : 1;
		const outW = Math.round(vw * scale);
		const outH = Math.round(vh * scale);

		canvasEl.width = outW;
		canvasEl.height = outH;
		const ctx = canvasEl.getContext('2d');
		if (!ctx) return;

		ctx.drawImage(videoEl, 0, 0, outW, outH);
		const dataUrl = canvasEl.toDataURL('image/jpeg', JPEG_QUALITY);

		// Estimate size from data URL
		const commaIdx = dataUrl.indexOf(',');
		const sizeBytes = Math.round((dataUrl.length - commaIdx - 1) * 0.75);

		await addPageSafe(dataUrl, outW, outH, sizeBytes, currentQuality);
		stopCamera();
		$captureStep = 'preview';
	}

	function handleRetake(): void {
		$captureStep = 'camera';
	}

	function handleAddPage(): void {
		$captureStep = 'camera';
	}

	async function handleSend(): Promise<void> {
		const session = get(captureSession);
		if (!session || session.pages.length === 0) return;

		if (!get(isConnected)) {
			queueForOffline(session.pages);
			$captureStep = 'done';
			return;
		}

		const pages = session.pages;
		setUploadStarted(pages.length);

		uploadAbort = new AbortController();

		const result = await uploadDocument(pages, 'Mobile Camera', uploadAbort.signal);

		if (result.ok && result.data) {
			setUploadComplete();
			addProcessingDocument(result.data.document_id, pages.length);
		} else {
			setUploadFailed(result.errorMessage ?? 'Upload failed');
		}

		uploadAbort = null;
	}

	function handleCancel(): void {
		if (uploadAbort) {
			uploadAbort.abort();
			uploadAbort = null;
		}
		cancelCapture();
	}
</script>

<div class="capture-screen">
	{#if $captureStep === 'camera'}
		<div class="camera-header">
			<h1>Capture Document</h1>
			<a class="close-btn" href="/documents" aria-label="Close">&times;</a>
		</div>

		{#if !$isConnected}
			<div class="offline-notice">
				<p class="offline-text">
					Camera is available, but you need to be connected to your desktop to send documents.
				</p>
			</div>
		{/if}

		{#if cameraError}
			<div class="camera-error">
				<p>{cameraError}</p>
				<button class="retry-btn" onclick={startCamera}>Try Again</button>
			</div>
		{:else}
			<div class="camera-viewport">
				<video bind:this={videoEl} class="camera-video" playsinline muted></video>
				<canvas bind:this={canvasEl} class="scan-canvas"></canvas>
				{#if cameraReady}
					<CameraOverlay quality={currentQuality} />
				{:else}
					<div class="camera-loading">
						<p>Starting camera...</p>
					</div>
				{/if}
			</div>

			<div class="camera-controls">
				<QualityIndicator quality={currentQuality} />
				<div class="capture-btn-wrapper">
					<CaptureButton disabled={!cameraReady} onCapture={handleCapture} />
				</div>
			</div>
		{/if}

	{:else if $captureStep === 'preview'}
		<div class="preview-header">
			<a class="back-btn" href="/documents" aria-label="Cancel">&larr;</a>
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
			<a class="done-link" href="/documents">Done</a>
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
		position: relative;
		overflow: hidden;
	}

	.camera-video {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}

	.scan-canvas {
		display: none;
	}

	.camera-loading {
		position: absolute;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		color: #D6D3D1;
		font-size: 15px;
	}

	.camera-error {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 24px;
		text-align: center;
	}

	.camera-error p {
		color: #D6D3D1;
		font-size: 15px;
		line-height: 1.5;
		margin: 0 0 16px;
	}

	.retry-btn {
		padding: 12px 24px;
		background: var(--color-primary);
		color: white;
		border: none;
		border-radius: 12px;
		font-size: 16px;
		font-weight: 600;
		cursor: pointer;
		min-height: var(--min-touch-target);
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
