<!-- M0-02: QR Pairing Flow — camera viewfinder + pairing handshake -->
<script lang="ts">
	import jsQR from 'jsqr';
	import type { PairingPhase } from '$lib/types/pairing.js';
	import { parseQrData, executePairing, PairingError } from '$lib/utils/pairing.js';

	let phase = $state<PairingPhase>({ phase: 'idle' });
	let videoEl: HTMLVideoElement | undefined = $state();
	let canvasEl: HTMLCanvasElement | undefined = $state();
	let stream: MediaStream | null = null;
	let scanFrameId = 0;

	function startScanning(): void {
		phase = { phase: 'scanning' };
		requestCamera();
	}

	async function requestCamera(): Promise<void> {
		try {
			stream = await navigator.mediaDevices.getUserMedia({
				video: { facingMode: 'environment', width: { ideal: 1280 }, height: { ideal: 720 } }
			});

			if (videoEl) {
				videoEl.srcObject = stream;
				await videoEl.play();
				scanLoop();
			}
		} catch (err) {
			const message = err instanceof Error ? err.message : 'Unknown error';
			if (message.includes('NotAllowed') || message.includes('Permission')) {
				phase = { phase: 'camera_denied' };
			} else {
				phase = { phase: 'error', message: `Camera error: ${message}`, retryable: true };
			}
		}
	}

	function scanLoop(): void {
		if (phase.phase !== 'scanning' || !videoEl || !canvasEl) return;

		const ctx = canvasEl.getContext('2d', { willReadFrequently: true });
		if (!ctx || videoEl.readyState < videoEl.HAVE_ENOUGH_DATA) {
			scanFrameId = requestAnimationFrame(scanLoop);
			return;
		}

		canvasEl.width = videoEl.videoWidth;
		canvasEl.height = videoEl.videoHeight;
		ctx.drawImage(videoEl, 0, 0);

		const imageData = ctx.getImageData(0, 0, canvasEl.width, canvasEl.height);
		const code = jsQR(imageData.data, imageData.width, imageData.height, {
			inversionAttempts: 'dontInvert'
		});

		if (code) {
			const qrData = parseQrData(code.data);
			if (qrData) {
				stopCamera();
				handleQrDecoded(qrData);
				return;
			}
		}

		scanFrameId = requestAnimationFrame(scanLoop);
	}

	async function handleQrDecoded(qrData: ReturnType<typeof parseQrData>): Promise<void> {
		if (!qrData) return;

		phase = { phase: 'connecting', message: 'Connecting to desktop\u2026' };

		try {
			const result = await executePairing(
				qrData,
				getDeviceName(),
				getDeviceModel(),
				(message) => {
					phase = { phase: 'connecting', message };
				}
			);

			phase = { phase: 'success', profileName: result.profileName };
		} catch (err) {
			const message = err instanceof PairingError
				? err.message
				: err instanceof Error
					? err.message
					: 'Pairing failed. Please try again.';

			const retryable = !message.includes('denied') && !message.includes('Maximum');
			phase = { phase: 'error', message, retryable };
		}
	}

	function retry(): void {
		phase = { phase: 'idle' };
	}

	function stopCamera(): void {
		cancelAnimationFrame(scanFrameId);
		if (stream) {
			for (const track of stream.getTracks()) {
				track.stop();
			}
			stream = null;
		}
		if (videoEl) {
			videoEl.srcObject = null;
		}
	}

	function getDeviceName(): string {
		const ua = navigator.userAgent;
		if (/iPhone/.test(ua)) return 'iPhone';
		if (/iPad/.test(ua)) return 'iPad';
		if (/Android/.test(ua)) return 'Android Phone';
		return 'Mobile Device';
	}

	function getDeviceModel(): string {
		const ua = navigator.userAgent;
		const match = ua.match(/\(([^)]+)\)/);
		return match ? match[1].slice(0, 50) : 'Unknown';
	}

	// Cleanup on component destroy
	$effect(() => {
		return () => {
			stopCamera();
		};
	});
</script>

<div class="pairing-flow">
	{#if phase.phase === 'idle'}
		<!-- Welcome + start scanning -->
		<div class="center-content">
			<div class="icon-circle" aria-hidden="true">
				<svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<rect x="3" y="3" width="7" height="7" rx="1" />
					<rect x="14" y="3" width="7" height="7" rx="1" />
					<rect x="3" y="14" width="7" height="7" rx="1" />
					<rect x="14" y="14" width="3" height="3" />
					<line x1="21" y1="14" x2="21" y2="21" />
					<line x1="14" y1="21" x2="21" y2="21" />
				</svg>
			</div>
			<h1>Connect to Desktop</h1>
			<p class="subtitle">Scan the QR code shown on your Coheara desktop app to pair this device.</p>

			<button class="primary-button" onclick={startScanning}>
				Scan QR Code
			</button>

			<div class="instructions">
				<h2>How to pair</h2>
				<ol>
					<li>Open Coheara on your desktop</li>
					<li>Go to Settings &rarr; Mobile Companion</li>
					<li>Click "Pair New Device"</li>
					<li>Point your phone camera at the QR code</li>
				</ol>
			</div>
		</div>

	{:else if phase.phase === 'scanning'}
		<!-- Camera viewfinder -->
		<div class="scanner-container">
			<video bind:this={videoEl} class="camera-video" playsinline muted></video>
			<canvas bind:this={canvasEl} class="scan-canvas"></canvas>

			<!-- QR guide overlay -->
			<div class="scan-overlay" aria-hidden="true">
				<div class="scan-frame">
					<div class="corner tl"></div>
					<div class="corner tr"></div>
					<div class="corner bl"></div>
					<div class="corner br"></div>
				</div>
			</div>

			<p class="scan-hint">Point camera at the QR code on your desktop</p>

			<button class="cancel-button" onclick={() => { stopCamera(); phase = { phase: 'idle' }; }}>
				Cancel
			</button>
		</div>

	{:else if phase.phase === 'connecting'}
		<!-- Connecting / waiting for approval -->
		<div class="center-content">
			<div class="spinner" role="status" aria-label={phase.message}></div>
			<h1>Pairing</h1>
			<p class="subtitle">{phase.message}</p>
			<p class="hint">Approve the connection on your desktop when prompted.</p>
		</div>

	{:else if phase.phase === 'success'}
		<!-- Pairing complete -->
		<div class="center-content">
			<div class="success-icon" aria-hidden="true">
				<svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
					<polyline points="20 6 9 17 4 12" />
				</svg>
			</div>
			<h1>Connected</h1>
			<p class="subtitle">Paired with {phase.profileName}'s profile.</p>
			<p class="hint">Your health data is now syncing securely.</p>
		</div>

	{:else if phase.phase === 'error'}
		<!-- Error with optional retry -->
		<div class="center-content">
			<div class="error-icon" aria-hidden="true">
				<svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<circle cx="12" cy="12" r="10" />
					<line x1="15" y1="9" x2="9" y2="15" />
					<line x1="9" y1="9" x2="15" y2="15" />
				</svg>
			</div>
			<h1>Pairing Failed</h1>
			<p class="error-message" role="alert">{phase.message}</p>
			{#if phase.retryable}
				<button class="primary-button" onclick={retry}>
					Try Again
				</button>
			{:else}
				<button class="secondary-button" onclick={retry}>
					Back
				</button>
			{/if}
		</div>

	{:else if phase.phase === 'camera_denied'}
		<!-- Camera permission denied -->
		<div class="center-content">
			<div class="error-icon" aria-hidden="true">
				<svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M16.5 12a4.5 4.5 0 1 1-9 0 4.5 4.5 0 0 1 9 0z" />
					<path d="M2 2l20 20" />
				</svg>
			</div>
			<h1>Camera Access Needed</h1>
			<p class="subtitle">Coheara needs camera access to scan the QR code from your desktop.</p>
			<p class="hint">Open your device settings and enable camera access for Coheara, then try again.</p>
			<button class="primary-button" onclick={retry}>
				Try Again
			</button>
		</div>
	{/if}
</div>

<style>
	.pairing-flow {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.center-content {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		flex: 1;
		text-align: center;
		padding: 24px;
		max-width: 360px;
		margin: 0 auto;
	}

	.icon-circle {
		width: 80px;
		height: 80px;
		border-radius: 50%;
		background: #EEF2FF;
		display: flex;
		align-items: center;
		justify-content: center;
		margin-bottom: 20px;
		color: var(--color-primary);
	}

	h1 {
		font-size: var(--font-header);
		font-weight: 700;
		margin: 0 0 8px;
	}

	.subtitle {
		color: var(--color-text-muted);
		font-size: 16px;
		line-height: 1.5;
		margin: 0 0 24px;
	}

	.hint {
		color: var(--color-text-muted);
		font-size: 14px;
		line-height: 1.5;
		margin: 8px 0 0;
	}

	.primary-button {
		width: 100%;
		padding: 14px 20px;
		background: var(--color-primary);
		color: white;
		border: none;
		border-radius: 12px;
		font-size: 16px;
		font-weight: 600;
		cursor: pointer;
		min-height: var(--min-touch-target);
	}

	.secondary-button {
		width: 100%;
		padding: 14px 20px;
		background: transparent;
		color: var(--color-primary);
		border: 2px solid var(--color-primary);
		border-radius: 12px;
		font-size: 16px;
		font-weight: 600;
		cursor: pointer;
		min-height: var(--min-touch-target);
	}

	.instructions {
		width: 100%;
		text-align: left;
		margin-top: 32px;
	}

	h2 {
		font-size: 18px;
		font-weight: 600;
		margin: 0 0 12px;
	}

	ol {
		padding-left: 20px;
		margin: 0;
	}

	li {
		font-size: 15px;
		line-height: 1.8;
		color: var(--color-text);
	}

	/* Scanner */
	.scanner-container {
		position: relative;
		flex: 1;
		background: black;
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

	.scan-overlay {
		position: absolute;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		pointer-events: none;
	}

	.scan-frame {
		position: relative;
		width: 250px;
		height: 250px;
		border: 2px solid rgba(255, 255, 255, 0.3);
		border-radius: 16px;
	}

	.corner {
		position: absolute;
		width: 28px;
		height: 28px;
		border-color: white;
		border-style: solid;
	}

	.corner.tl {
		top: -2px;
		left: -2px;
		border-width: 4px 0 0 4px;
		border-radius: 8px 0 0 0;
	}

	.corner.tr {
		top: -2px;
		right: -2px;
		border-width: 4px 4px 0 0;
		border-radius: 0 8px 0 0;
	}

	.corner.bl {
		bottom: -2px;
		left: -2px;
		border-width: 0 0 4px 4px;
		border-radius: 0 0 0 8px;
	}

	.corner.br {
		bottom: -2px;
		right: -2px;
		border-width: 0 4px 4px 0;
		border-radius: 0 0 8px 0;
	}

	.scan-hint {
		position: absolute;
		bottom: 100px;
		left: 0;
		right: 0;
		text-align: center;
		color: white;
		font-size: 15px;
		text-shadow: 0 1px 3px rgba(0, 0, 0, 0.6);
	}

	.cancel-button {
		position: absolute;
		bottom: 40px;
		left: 50%;
		transform: translateX(-50%);
		padding: 12px 32px;
		background: rgba(0, 0, 0, 0.5);
		color: white;
		border: 1px solid rgba(255, 255, 255, 0.3);
		border-radius: 24px;
		font-size: 16px;
		font-weight: 500;
		cursor: pointer;
		min-height: var(--min-touch-target);
		backdrop-filter: blur(8px);
		-webkit-backdrop-filter: blur(8px);
	}

	/* Spinner */
	.spinner {
		width: 48px;
		height: 48px;
		border: 4px solid #E7E5E4;
		border-top-color: var(--color-primary);
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
		margin-bottom: 20px;
	}

	@keyframes spin {
		to { transform: rotate(360deg); }
	}

	/* Success */
	.success-icon {
		width: 80px;
		height: 80px;
		border-radius: 50%;
		background: #DCFCE7;
		display: flex;
		align-items: center;
		justify-content: center;
		margin-bottom: 20px;
		color: var(--color-success);
	}

	/* Error */
	.error-icon {
		width: 80px;
		height: 80px;
		border-radius: 50%;
		background: #FEF2F2;
		display: flex;
		align-items: center;
		justify-content: center;
		margin-bottom: 20px;
		color: var(--color-error);
	}

	.error-message {
		color: var(--color-error);
		font-size: 15px;
		line-height: 1.5;
		margin: 0 0 24px;
	}

	@media (prefers-color-scheme: dark) {
		.icon-circle {
			background: #1E3A5F;
		}

		.success-icon {
			background: #14532D;
		}

		.error-icon {
			background: #450A0A;
		}
	}
</style>
