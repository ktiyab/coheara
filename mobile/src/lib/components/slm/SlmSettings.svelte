<!-- M2-01: SLM Settings — model management (info, delete, change) -->
<script lang="ts">
	import { modelState, modelInfo, deleteModel } from '$lib/stores/slm.js';

	let {
		onDownloadModel,
		onChangeModel
	}: {
		onDownloadModel: () => void;
		onChangeModel: () => void;
	} = $props();

	let info = $derived($modelInfo);
	let currentState = $derived($modelState);
	let isInstalled = $derived(currentState === 'downloaded' || currentState === 'loading' || currentState === 'ready' || currentState === 'generating');
	let sizeGb = $derived(info ? (info.sizeBytes / (1024 * 1024 * 1024)).toFixed(1) : '0');

	let showConfirmDelete = $state(false);

	function handleDelete() {
		deleteModel();
		showConfirmDelete = false;
	}

	function formatLastUsed(lastUsed: string | null): string {
		if (!lastUsed) return 'Never';
		const elapsed = Date.now() - new Date(lastUsed).getTime();
		const minutes = Math.floor(elapsed / 60_000);
		if (minutes < 1) return 'Just now';
		if (minutes < 60) return `${minutes} min ago`;
		const hours = Math.floor(minutes / 60);
		if (hours < 24) return `${hours} hour${hours !== 1 ? 's' : ''} ago`;
		const days = Math.floor(hours / 24);
		return `${days} day${days !== 1 ? 's' : ''} ago`;
	}
</script>

<section class="slm-settings" aria-label="Offline AI settings">
	<h3 class="settings-heading">Offline AI</h3>

	{#if isInstalled && info}
		<div class="model-info">
			<div class="info-row">
				<span class="info-label">Model</span>
				<span class="info-value">{info.name}</span>
			</div>
			<div class="info-row">
				<span class="info-label">Size</span>
				<span class="info-value">{sizeGb} GB</span>
			</div>
			<div class="info-row">
				<span class="info-label">Last used</span>
				<span class="info-value">{formatLastUsed(info.lastUsed)}</span>
			</div>
		</div>

		<div class="settings-actions">
			<button class="btn-outline" onclick={onChangeModel}>
				Change Model
			</button>
			{#if showConfirmDelete}
				<div class="confirm-delete" role="alert">
					<p>Remove {info.name}? This frees {sizeGb} GB.</p>
					<div class="confirm-actions">
						<button class="btn-danger" onclick={handleDelete}>
							Remove
						</button>
						<button class="btn-outline" onclick={() => showConfirmDelete = false}>
							Keep
						</button>
					</div>
				</div>
			{:else}
				<button class="btn-danger-outline" onclick={() => showConfirmDelete = true}>
					Delete Model · {sizeGb} GB
				</button>
			{/if}
		</div>
	{:else if currentState === 'not_capable'}
		<p class="not-capable">Your device doesn't support offline AI (insufficient RAM).</p>
	{:else}
		<button class="btn-primary" onclick={onDownloadModel}>
			Download Model
		</button>
	{/if}

	<p class="settings-description">
		The AI model answers questions from your saved health data when your desktop isn't available.
	</p>
</section>

<style>
	.slm-settings {
		padding: var(--spacing-lg, 1.5rem);
	}

	.settings-heading {
		font-size: 1.125rem;
		font-weight: 600;
		margin: 0 0 1rem;
	}

	.model-info {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		margin-bottom: 1rem;
	}

	.info-row {
		display: flex;
		justify-content: space-between;
		font-size: 0.9375rem;
	}

	.info-label {
		color: var(--color-text-muted, #666);
	}

	.info-value {
		font-weight: 500;
	}

	.settings-actions {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		margin-bottom: 1rem;
	}

	.btn-outline {
		padding: 0.75rem;
		background: transparent;
		color: var(--color-text, #1a1a1a);
		border: 1px solid var(--color-border, #e0e0e0);
		border-radius: var(--radius-md, 8px);
		font-size: 0.9375rem;
		cursor: pointer;
		min-height: 48px;
	}

	.btn-danger-outline {
		padding: 0.75rem;
		background: transparent;
		color: var(--color-error, #dc2626);
		border: 1px solid var(--color-error, #dc2626);
		border-radius: var(--radius-md, 8px);
		font-size: 0.9375rem;
		cursor: pointer;
		min-height: 48px;
	}

	.btn-danger {
		padding: 0.75rem;
		background: var(--color-error, #dc2626);
		color: white;
		border: none;
		border-radius: var(--radius-md, 8px);
		font-size: 0.9375rem;
		cursor: pointer;
		min-height: 48px;
		flex: 1;
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
		margin-bottom: 1rem;
	}

	.confirm-delete {
		padding: 1rem;
		background: var(--color-error-subtle, #fef2f2);
		border-radius: var(--radius-md, 8px);
	}

	.confirm-delete p {
		margin: 0 0 0.75rem;
		font-size: 0.9375rem;
	}

	.confirm-actions {
		display: flex;
		gap: 0.5rem;
	}

	.not-capable {
		color: var(--color-text-muted, #666);
		font-size: 0.9375rem;
		margin-bottom: 1rem;
	}

	.settings-description {
		color: var(--color-text-muted, #666);
		font-size: 0.8125rem;
		line-height: 1.5;
	}
</style>
