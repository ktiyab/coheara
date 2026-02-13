<!-- M1-02: Tappable citation chip — links to source document -->
<script lang="ts">
	import type { Citation } from '$lib/types/chat.js';

	const {
		citation,
		onTap
	}: {
		citation: Citation;
		onTap?: (documentId: string) => void;
	} = $props();
</script>

<button
	class="citation-chip"
	aria-label="Source: {citation.documentTitle}, {citation.documentDate}"
	onclick={() => onTap?.(citation.documentId)}
>
	<span class="chip-icon" aria-hidden="true">&#x1F4C4;</span>
	<span class="chip-text">
		{citation.documentTitle}
		{#if citation.documentDate}
			<span class="chip-date">{citation.documentDate}</span>
		{/if}
	</span>
</button>

<style>
	.citation-chip {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 6px 12px;
		background: #F0F9FF;
		border: 1px solid #BAE6FD;
		border-radius: 20px;
		font-size: 13px;
		color: #0369A1;
		cursor: pointer;
		min-height: 32px;
	}

	.chip-icon {
		font-size: 14px;
	}

	.chip-text {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		max-width: 200px;
	}

	.chip-date {
		color: #7DD3FC;
		margin-left: 4px;
		font-size: 12px;
	}
</style>
