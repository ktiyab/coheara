<!-- M1-01: Root layout — status bar + content area + bottom tabs -->
<script lang="ts">
	import '../app.css';
	import StatusBar from '$lib/components/StatusBar.svelte';
	import BottomTabs from '$lib/components/BottomTabs.svelte';
	import BiometricGate from '$lib/components/BiometricGate.svelte';
	import QrPairingFlow from '$lib/components/QrPairingFlow.svelte';
	import { connection } from '$lib/stores/connection.js';
	import { isAuthenticated, authState } from '$lib/stores/session.js';

	const { children } = $props();

	const showPairing = $derived(
		$connection.status === 'unpaired' || $connection.status === 'connecting'
	);

	function handleUnlock(): void {
		// Biometric verification triggered — handled by lifecycle
	}
</script>

<div class="app-shell">
	<!-- Biometric gate overlay (shown when locked) -->
	{#if $authState.state === 'locked' || $authState.state === 'locked_out'}
		<BiometricGate onUnlock={handleUnlock} />
	{/if}

	<!-- Status bar (hidden during pairing scanner) -->
	{#if !showPairing}
		<StatusBar />
	{/if}

	<!-- Content area -->
	<main class="content-area">
		{#if showPairing}
			<QrPairingFlow />
		{:else}
			{@render children()}
		{/if}
	</main>

	<!-- Bottom tabs (hidden when unpaired/pairing) -->
	{#if !showPairing}
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

</style>
