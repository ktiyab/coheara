<!-- M1-02: Message bubble — patient (right) vs Coheara (left) -->
<script lang="ts">
	import type { ChatMessage } from '$lib/types/chat.js';
	import CitationChip from './CitationChip.svelte';

	const {
		message,
		onFeedback,
		onCitationTap
	}: {
		message: ChatMessage;
		onFeedback?: (messageId: string, helpful: boolean) => void;
		onCitationTap?: (documentId: string) => void;
	} = $props();

	const isPatient = $derived(message.role === 'patient');
	const showFeedback = $derived(message.role === 'coheara' && message.feedback === null);
	const sourceLabel = $derived(
		message.source === 'live' ? 'From your full records' :
		message.source === 'cached' ? 'From your cached data' : ''
	);
</script>

<div
	class="message-bubble"
	class:patient={isPatient}
	class:coheara={!isPatient}
	role="article"
	aria-label="{isPatient ? 'You' : 'Coheara'}: {message.content.slice(0, 50)}"
>
	{#if !isPatient}
		<div class="avatar" aria-hidden="true">C</div>
	{/if}

	<div class="bubble-content">
		<p class="message-text">{message.content}</p>

		{#if message.citations.length > 0}
			<div class="citations" role="list" aria-label="Source documents">
				{#each message.citations as citation (citation.documentId)}
					<CitationChip {citation} onTap={onCitationTap} />
				{/each}
			</div>
		{/if}

		{#if sourceLabel}
			<span class="source-label">{sourceLabel}</span>
		{/if}

		{#if showFeedback && onFeedback}
			<div class="feedback" role="group" aria-label="Was this helpful?">
				<button
					class="feedback-btn"
					aria-label="Helpful"
					onclick={() => onFeedback(message.id, true)}
				>&#x1F44D;</button>
				<button
					class="feedback-btn"
					aria-label="Not helpful"
					onclick={() => onFeedback(message.id, false)}
				>&#x1F44E;</button>
			</div>
		{/if}

		{#if message.feedback !== null && message.feedback !== undefined}
			<span class="feedback-status">
				{message.feedback === 'helpful' ? 'Marked as helpful' : 'Marked as not helpful'}
			</span>
		{/if}
	</div>
</div>

<style>
	.message-bubble {
		display: flex;
		gap: 8px;
		margin-bottom: 12px;
		max-width: 85%;
	}

	.message-bubble.patient {
		margin-left: auto;
		flex-direction: row-reverse;
	}

	.message-bubble.coheara {
		margin-right: auto;
	}

	.avatar {
		width: 32px;
		height: 32px;
		border-radius: 50%;
		background: var(--color-primary);
		color: white;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 14px;
		font-weight: 700;
		flex-shrink: 0;
	}

	.bubble-content {
		padding: 12px 16px;
		border-radius: 16px;
	}

	.patient .bubble-content {
		background: var(--color-primary);
		color: white;
		border-bottom-right-radius: 4px;
	}

	.coheara .bubble-content {
		background: white;
		border: 1px solid #E7E5E4;
		border-bottom-left-radius: 4px;
	}

	.message-text {
		margin: 0;
		font-size: 16px;
		line-height: 1.5;
		white-space: pre-wrap;
	}

	.citations {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
		margin-top: 10px;
	}

	.source-label {
		display: block;
		margin-top: 8px;
		font-size: 12px;
		color: var(--color-text-muted);
	}

	.patient .source-label { color: rgba(255,255,255,0.7); }

	.feedback {
		display: flex;
		gap: 8px;
		margin-top: 8px;
	}

	.feedback-btn {
		background: none;
		border: 1px solid #E7E5E4;
		border-radius: 8px;
		padding: 6px 12px;
		font-size: 18px;
		cursor: pointer;
		min-height: 36px;
		min-width: 36px;
	}

	.feedback-status {
		display: block;
		margin-top: 6px;
		font-size: 12px;
		color: var(--color-text-muted);
	}
</style>
