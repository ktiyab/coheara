<!-- M1-05: Large capture button — 72dp minimum (Mamadou accessibility) -->
<script lang="ts">
	const { disabled, onCapture }: {
		disabled: boolean;
		onCapture: () => void;
	} = $props();
</script>

<button
	class="capture-btn"
	class:disabled
	{disabled}
	onclick={onCapture}
	aria-label="Capture document"
>
	<span class="capture-ring" aria-hidden="true">
		<span class="capture-inner"></span>
	</span>
</button>

<style>
	.capture-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 72px;
		height: 72px;
		border: none;
		background: transparent;
		cursor: pointer;
		padding: 0;
	}

	.capture-btn.disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.capture-ring {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 72px;
		height: 72px;
		border-radius: 50%;
		border: 4px solid white;
	}

	.capture-inner {
		width: 56px;
		height: 56px;
		border-radius: 50%;
		background: white;
	}

	.capture-btn:active:not(.disabled) .capture-inner {
		background: #E7E5E4;
		transform: scale(0.92);
	}
</style>
