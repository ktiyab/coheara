<!-- M1-02: Quick question chips — Mamadou's suggested questions -->
<script lang="ts">
	import type { QuickQuestion } from '$lib/types/chat.js';
	import { DEFAULT_QUICK_QUESTIONS } from '$lib/stores/chat.js';

	const {
		questions = DEFAULT_QUICK_QUESTIONS,
		onSelect,
		disabled = false
	}: {
		questions?: readonly QuickQuestion[];
		onSelect: (text: string) => void;
		disabled?: boolean;
	} = $props();
</script>

<div class="quick-questions" role="group" aria-label="Suggested questions">
	{#each questions as question (question.text)}
		<button
			class="quick-btn"
			disabled={disabled}
			onclick={() => onSelect(question.text)}
			aria-label="Ask: {question.text}"
		>
			{question.text}
		</button>
	{/each}
</div>

<style>
	.quick-questions {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		padding: 12px 16px;
	}

	.quick-btn {
		padding: 10px 16px;
		background: white;
		border: 1px solid var(--color-primary);
		border-radius: 20px;
		color: var(--color-primary);
		font-size: 15px;
		font-weight: 500;
		cursor: pointer;
		min-height: var(--min-touch-target);
		transition: background-color 0.15s;
	}

	.quick-btn:hover:not(:disabled) {
		background: #EFF6FF;
	}

	.quick-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
</style>
