<!-- M1-02: Source indicator — Live / Cached / Offline badge -->
<script lang="ts">
	import type { ChatSource } from '$lib/types/chat.js';

	const { source }: { source: ChatSource } = $props();

	const label = $derived(
		source === 'live' ? 'Live' :
		source === 'cached' ? 'Cached' : 'Offline'
	);

	const dotClass = $derived(
		source === 'live' ? 'dot-live' :
		source === 'cached' ? 'dot-cached' : 'dot-offline'
	);
</script>

<span class="source-indicator" aria-label="Chat source: {label}">
	<span class="dot {dotClass}" aria-hidden="true"></span>
	{label}
</span>

<style>
	.source-indicator {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		font-size: 13px;
		font-weight: 500;
		color: var(--color-text-muted);
	}

	.dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
	}

	.dot-live { background: var(--color-success); }
	.dot-cached { background: var(--color-warning); }
	.dot-offline { background: var(--color-text-muted); }
</style>
