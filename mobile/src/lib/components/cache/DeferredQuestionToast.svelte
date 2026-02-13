<!-- M1-06: Deferred question toast — "You asked while offline: '...'" -->
<script lang="ts">
	import type { DeferredQuestion } from '$lib/types/cache-manager.js';

	const { question, onAskNow, onDismiss }: {
		question: DeferredQuestion;
		onAskNow: (question: DeferredQuestion) => void;
		onDismiss: (id: string) => void;
	} = $props();

	const preview = $derived(
		question.questionText.length > 60
			? question.questionText.slice(0, 57) + '...'
			: question.questionText
	);
</script>

<div class="deferred-toast" role="alert">
	<p class="toast-label">You asked while offline:</p>
	<p class="toast-question">"{preview}"</p>
	<div class="toast-actions">
		<button class="ask-btn" onclick={() => onAskNow(question)}>Ask Now</button>
		<button class="dismiss-btn" onclick={() => onDismiss(question.id)}>Dismiss</button>
	</div>
</div>

<style>
	.deferred-toast {
		padding: 14px 16px;
		background: white;
		border-radius: 12px;
		border: 1px solid #E7E5E4;
		border-left: 3px solid var(--color-primary);
	}

	.toast-label {
		font-size: 13px;
		color: var(--color-text-muted);
		margin: 0 0 4px;
	}

	.toast-question {
		font-size: 15px;
		font-weight: 500;
		margin: 0 0 12px;
		line-height: 1.4;
	}

	.toast-actions {
		display: flex;
		gap: 8px;
	}

	.ask-btn {
		padding: 8px 16px;
		background: var(--color-primary);
		color: white;
		border: none;
		border-radius: 8px;
		font-size: 14px;
		font-weight: 500;
		cursor: pointer;
		font-family: inherit;
		min-height: var(--min-touch-target);
	}

	.dismiss-btn {
		padding: 8px 16px;
		background: transparent;
		color: var(--color-text-muted);
		border: 1px solid #D6D3D1;
		border-radius: 8px;
		font-size: 14px;
		cursor: pointer;
		font-family: inherit;
		min-height: var(--min-touch-target);
	}
</style>
