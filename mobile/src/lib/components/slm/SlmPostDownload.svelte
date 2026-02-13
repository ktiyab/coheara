<!-- M2-01: SLM Post-Download — completion confirmation dialog -->
<script lang="ts">
	let {
		modelName,
		onTryNow,
		onGotIt
	}: {
		modelName: string;
		onTryNow: () => void;
		onGotIt: () => void;
	} = $props();
</script>

<div class="slm-complete" role="dialog" aria-label="AI model ready" aria-modal="true">
	<div class="checkmark" aria-hidden="true">
		<svg width="48" height="48" viewBox="0 0 48 48" fill="none">
			<circle cx="24" cy="24" r="24" fill="var(--color-success, #16a34a)" opacity="0.15" />
			<path d="M14 24L21 31L34 18" stroke="var(--color-success, #16a34a)" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" />
		</svg>
	</div>

	<h2 class="complete-heading">AI Model Ready</h2>
	<p class="complete-model">{modelName} installed</p>
	<p class="complete-description">
		Your offline AI is ready to answer questions from your saved data.
	</p>

	<div class="complete-actions">
		<button class="btn-primary" onclick={onTryNow}>
			Try It Now
		</button>
		<button class="btn-secondary" onclick={onGotIt}>
			Got It
		</button>
	</div>
</div>

<style>
	.slm-complete {
		padding: var(--spacing-xl, 2rem);
		text-align: center;
		background: var(--color-surface, #fff);
		border-radius: var(--radius-lg, 12px);
	}

	.checkmark {
		margin-bottom: 1rem;
	}

	.complete-heading {
		font-size: 1.25rem;
		font-weight: 600;
		margin: 0 0 0.25rem;
	}

	.complete-model {
		font-size: 0.9375rem;
		color: var(--color-text-muted, #666);
		margin: 0 0 0.75rem;
	}

	.complete-description {
		color: var(--color-text-muted, #666);
		line-height: 1.5;
		margin: 0 0 1.5rem;
	}

	.complete-actions {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.btn-primary {
		width: 100%;
		padding: 0.875rem;
		background: var(--color-primary, #2563eb);
		color: white;
		border: none;
		border-radius: var(--radius-md, 8px);
		font-size: 1rem;
		font-weight: 500;
		cursor: pointer;
		min-height: 48px;
	}

	.btn-secondary {
		width: 100%;
		padding: 0.875rem;
		background: transparent;
		color: var(--color-text-muted, #666);
		border: 1px solid var(--color-border, #e0e0e0);
		border-radius: var(--radius-md, 8px);
		font-size: 1rem;
		cursor: pointer;
		min-height: 48px;
	}
</style>
