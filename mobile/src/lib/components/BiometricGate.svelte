<!-- M1-01: Biometric gate overlay — shown when auth required -->
<script lang="ts">
	import { authState, isAuthenticated } from '$lib/stores/session.js';

	const {
		onUnlock,
		onSkip
	}: {
		onUnlock: () => void;
		onSkip?: () => void;
	} = $props();
</script>

{#if !$isAuthenticated}
	<div class="gate-overlay" role="dialog" aria-modal="true" aria-label="Authentication required">
		<div class="gate-content">
			<div class="gate-icon" aria-hidden="true">&#x1F512;</div>

			{#if $authState.state === 'locked_out'}
				<h2 class="gate-title">Too many attempts</h2>
				<p class="gate-message">
					Please wait before trying again, or unlock from your desktop.
				</p>
			{:else if $authState.state === 'locked'}
				<h2 class="gate-title">Session expired</h2>
				<p class="gate-message">Verify your identity to continue.</p>
				<button class="gate-button primary" onclick={onUnlock}>
					Unlock with biometric
				</button>
			{:else}
				<h2 class="gate-title">Welcome to Coheara</h2>
				<p class="gate-message">Verify your identity to access your health data.</p>
				<button class="gate-button primary" onclick={onUnlock}>
					Unlock with biometric
				</button>
			{/if}

			{#if onSkip}
				<button class="gate-button secondary" onclick={onSkip}>
					Not now — anyone who picks up your phone can see your health data
				</button>
			{/if}
		</div>
	</div>
{/if}

<style>
	.gate-overlay {
		position: fixed;
		inset: 0;
		background: var(--color-surface);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
		padding: 24px;
	}

	.gate-content {
		text-align: center;
		max-width: 320px;
	}

	.gate-icon {
		font-size: 48px;
		margin-bottom: 16px;
	}

	.gate-title {
		font-size: var(--font-header);
		font-weight: 700;
		margin: 0 0 8px;
		color: var(--color-text);
	}

	.gate-message {
		font-size: 16px;
		color: var(--color-text-muted);
		margin: 0 0 24px;
		line-height: 1.5;
	}

	.gate-button {
		display: block;
		width: 100%;
		padding: 14px 20px;
		border: none;
		border-radius: 12px;
		font-size: 16px;
		font-weight: 600;
		cursor: pointer;
		min-height: var(--large-touch-target);
		margin-bottom: 12px;
	}

	.gate-button.primary {
		background: var(--color-primary);
		color: white;
	}

	.gate-button.secondary {
		background: transparent;
		color: var(--color-text-muted);
		font-size: 14px;
		font-weight: 400;
		text-align: center;
		line-height: 1.4;
	}
</style>
