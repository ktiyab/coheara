<!-- M0-02: Pairing route — flow is handled by root layout when unpaired -->
<script lang="ts">
	import { connection } from '$lib/stores/connection.js';
	import { goto } from '$app/navigation';

	// If already paired, redirect to home
	$effect(() => {
		if ($connection.status !== 'unpaired' && $connection.status !== 'connecting') {
			goto('/');
		}
	});
</script>

<!-- Pairing flow is rendered by +layout.svelte when connection.status is 'unpaired' -->
