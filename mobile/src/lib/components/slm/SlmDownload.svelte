<!-- M2-01: SLM Download — discovery card + download trigger -->
<script lang="ts">
	import { selectedModelSpec, isDeviceCapable, hasEnoughStorage, shouldShowSlmPromotion, startDownload } from '$lib/stores/slm.js';
	import { MODEL_SPECS, type ModelChoice, DEFAULT_MODEL } from '$lib/types/slm.js';

	let {
		onDownloadStart,
		onNotNow,
		freeStorageGb = 0
	}: {
		onDownloadStart: () => void;
		onNotNow: () => void;
		freeStorageGb: number;
	} = $props();

	let selectedId: ModelChoice = $state(DEFAULT_MODEL);
	let spec = $derived(MODEL_SPECS[selectedId]);
	let sizeGb = $derived((spec.sizeBytes / (1024 * 1024 * 1024)).toFixed(1));
	let canDownload = $derived(isDeviceCapable() && hasEnoughStorage(selectedId));

	function handleDownload() {
		startDownload(selectedId);
		onDownloadStart();
	}

	const modelOptions: ModelChoice[] = ['gemma-2b-q4', 'gemma-2b-q5', 'phi3-mini-q4'];
</script>

<div class="slm-download" role="region" aria-label="Offline AI download">
	<h2 class="download-heading">Offline AI (Optional)</h2>
	<p class="download-description">
		Get faster answers when your desktop isn't nearby. The AI uses only your saved health data.
	</p>

	<fieldset class="model-selector">
		<legend class="sr-only">Choose AI model</legend>
		{#each modelOptions as id}
			{@const s = MODEL_SPECS[id]}
			<label class="model-option" class:selected={selectedId === id}>
				<input
					type="radio"
					name="model"
					value={id}
					checked={selectedId === id}
					onchange={() => selectedId = id}
				/>
				<span class="model-name">{s.name}</span>
				<span class="model-meta">
					{(s.sizeBytes / (1024 * 1024 * 1024)).toFixed(1)} GB · ~{s.tokensPerSecond} tok/s
				</span>
				{#if id === DEFAULT_MODEL}
					<span class="badge-recommended">Recommended</span>
				{/if}
			</label>
		{/each}
	</fieldset>

	<div class="storage-info">
		<span>Phone storage: {freeStorageGb.toFixed(0)} GB free</span>
		<span>Download size: ~{sizeGb} GB</span>
	</div>

	{#if !canDownload}
		<p class="warning" role="alert">
			{#if !isDeviceCapable()}
				Your device doesn't have enough RAM for offline AI.
			{:else}
				Not enough storage space for this model.
			{/if}
		</p>
	{/if}

	<div class="download-actions">
		<button
			class="btn-primary"
			disabled={!canDownload}
			onclick={handleDownload}
			aria-describedby={!canDownload ? 'download-disabled-reason' : undefined}
		>
			Download
		</button>
		<button class="btn-secondary" onclick={onNotNow}>
			Not Now
		</button>
	</div>

	<p class="download-footer">You can remove this anytime in Settings.</p>
</div>

<style>
	.slm-download {
		padding: var(--spacing-lg, 1.5rem);
		background: var(--color-surface, #fff);
		border-radius: var(--radius-lg, 12px);
	}

	.download-heading {
		font-size: 1.25rem;
		font-weight: 600;
		margin: 0 0 0.5rem;
	}

	.download-description {
		color: var(--color-text-muted, #666);
		margin: 0 0 1rem;
		line-height: 1.5;
	}

	.model-selector {
		border: none;
		padding: 0;
		margin: 0 0 1rem;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip: rect(0, 0, 0, 0);
		white-space: nowrap;
	}

	.model-option {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.75rem;
		border: 1px solid var(--color-border, #e0e0e0);
		border-radius: var(--radius-md, 8px);
		cursor: pointer;
		flex-wrap: wrap;
	}

	.model-option.selected {
		border-color: var(--color-primary, #2563eb);
		background: var(--color-primary-subtle, #eff6ff);
	}

	.model-option input[type="radio"] {
		accent-color: var(--color-primary, #2563eb);
	}

	.model-name {
		font-weight: 500;
	}

	.model-meta {
		color: var(--color-text-muted, #666);
		font-size: 0.875rem;
	}

	.badge-recommended {
		font-size: 0.75rem;
		color: var(--color-primary, #2563eb);
		background: var(--color-primary-subtle, #eff6ff);
		padding: 0.125rem 0.5rem;
		border-radius: 9999px;
		margin-left: auto;
	}

	.storage-info {
		display: flex;
		justify-content: space-between;
		font-size: 0.875rem;
		color: var(--color-text-muted, #666);
		margin-bottom: 1rem;
	}

	.warning {
		color: var(--color-error, #dc2626);
		font-size: 0.875rem;
		margin-bottom: 0.5rem;
	}

	.download-actions {
		display: flex;
		gap: 0.75rem;
	}

	.btn-primary {
		flex: 1;
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

	.btn-primary:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.btn-secondary {
		flex: 1;
		padding: 0.875rem;
		background: transparent;
		color: var(--color-text-muted, #666);
		border: 1px solid var(--color-border, #e0e0e0);
		border-radius: var(--radius-md, 8px);
		font-size: 1rem;
		cursor: pointer;
		min-height: 48px;
	}

	.download-footer {
		text-align: center;
		font-size: 0.8125rem;
		color: var(--color-text-muted, #666);
		margin-top: 1rem;
	}
</style>
