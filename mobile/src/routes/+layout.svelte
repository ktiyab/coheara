<!-- M1-01: Root layout — status bar + content area + bottom tabs -->
<script lang="ts">
	import '../app.css';
	import StatusBar from '$lib/components/StatusBar.svelte';
	import BottomTabs from '$lib/components/BottomTabs.svelte';
	import BiometricGate from '$lib/components/BiometricGate.svelte';
	import { connection } from '$lib/stores/connection.js';
	import { isAuthenticated, authState } from '$lib/stores/session.js';

	const { children } = $props();

	function handleUnlock(): void {
		// Biometric verification triggered — handled by lifecycle
	}
</script>

<div class="app-shell">
	<!-- Biometric gate overlay (shown when locked) -->
	{#if $authState.state === 'locked' || $authState.state === 'locked_out'}
		<BiometricGate onUnlock={handleUnlock} />
	{/if}

	<!-- Status bar -->
	<StatusBar />

	<!-- Content area -->
	<main class="content-area">
		{#if $connection.status === 'unpaired'}
			<div class="unpaired-message">
				<h1>Welcome to Coheara</h1>
				<p>Connect to your desktop to get started.</p>
			</div>
		{:else}
			{@render children()}
		{/if}
	</main>

	<!-- Bottom tabs (hidden when unpaired) -->
	{#if $connection.status !== 'unpaired'}
		<BottomTabs />
	{/if}
</div>

<style>
	.app-shell {
		display: flex;
		flex-direction: column;
		height: 100dvh;
		overflow: hidden;
	}

	.content-area {
		flex: 1;
		overflow-y: auto;
		-webkit-overflow-scrolling: touch;
	}

	.unpaired-message {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		height: 100%;
		text-align: center;
		padding: 24px;
	}

	.unpaired-message h1 {
		font-size: var(--font-header);
		font-weight: 700;
		margin: 0 0 8px;
	}

	.unpaired-message p {
		font-size: 16px;
		color: var(--color-text-muted);
	}
</style>
