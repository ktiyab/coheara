<!-- M1-01: Pairing flow — QR code scanning (full implementation depends on M0-02) -->
<div class="pairing-screen">
	<div class="pairing-content">
		<h1>Connect to Desktop</h1>
		<p>Open Coheara on your desktop and scan the QR code to pair this device.</p>

		<div class="qr-placeholder" aria-label="QR code scanner area">
			<p>Camera scanner loads from M1-05 Document Capture</p>
		</div>

		<div class="instructions">
			<h2>How to pair</h2>
			<ol>
				<li>Open Coheara on your desktop</li>
				<li>Go to Settings &rarr; Mobile Companion</li>
				<li>Click "Pair New Device"</li>
				<li>Scan the QR code shown on your desktop</li>
			</ol>
		</div>
	</div>
</div>

<style>
	.pairing-screen {
		display: flex;
		align-items: center;
		justify-content: center;
		min-height: 100%;
		padding: 24px;
	}

	.pairing-content {
		text-align: center;
		max-width: 360px;
	}

	h1 {
		font-size: var(--font-header);
		font-weight: 700;
		margin: 0 0 8px;
	}

	p {
		color: var(--color-text-muted);
		font-size: 16px;
		line-height: 1.5;
		margin: 0 0 24px;
	}

	.qr-placeholder {
		width: 250px;
		height: 250px;
		margin: 0 auto 24px;
		background: #F5F5F4;
		border-radius: 16px;
		display: flex;
		align-items: center;
		justify-content: center;
		border: 2px dashed #D6D3D1;
	}

	.qr-placeholder p {
		font-size: 14px;
		padding: 16px;
	}

	.instructions {
		text-align: left;
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
</style>
