<!-- M1-01: Persistent status bar — shows connection + profile name + last sync -->
<script lang="ts">
	import { statusText, statusColor, statusLabel, connection } from '$lib/stores/connection.js';

	const { onPairAction }: { onPairAction?: () => void } = $props();
</script>

<header
	class="status-bar"
	role="status"
	aria-live="polite"
	aria-label={`Connection status: ${$statusLabel}`}
>
	<div class="status-indicator">
		<span
			class="status-dot"
			style="background-color: {$statusColor}"
			aria-hidden="true"
		></span>
		<span class="status-text">{$statusText}</span>
	</div>

	{#if $connection.status === 'unpaired' && onPairAction}
		<button
			class="pair-action"
			onclick={onPairAction}
			aria-label="Scan QR code to connect to desktop"
		>
			Scan QR Code
		</button>
	{/if}

	{#if $connection.status === 'desktop_locked'}
		<p class="status-hint">Unlock your desktop to get updates</p>
	{/if}
</header>

<style>
	.status-bar {
		display: flex;
		flex-direction: column;
		padding: 12px 16px;
		background: var(--color-surface);
		border-bottom: 1px solid #E7E5E4;
		padding-top: max(12px, env(safe-area-inset-top));
	}

	.status-indicator {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.status-dot {
		width: 10px;
		height: 10px;
		border-radius: 50%;
		flex-shrink: 0;
	}

	.status-text {
		font-size: 14px;
		font-weight: 500;
		color: var(--color-text);
	}

	.status-hint {
		margin: 4px 0 0 18px;
		font-size: 13px;
		color: var(--color-text-muted);
	}

	.pair-action {
		margin-top: 8px;
		padding: 10px 16px;
		background: var(--color-primary);
		color: white;
		border: none;
		border-radius: 8px;
		font-size: 15px;
		font-weight: 600;
		cursor: pointer;
		min-height: var(--min-touch-target);
	}
</style>
