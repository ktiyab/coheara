<!-- M1-05: Capture action buttons — Retake / Add Page / Send -->
<script lang="ts">
	const { pageCount, canAdd, onRetake, onAddPage, onSend }: {
		pageCount: number;
		canAdd: boolean;
		onRetake: () => void;
		onAddPage: () => void;
		onSend: () => void;
	} = $props();

	const sendLabel = $derived(
		pageCount === 1
			? 'Send to Desktop'
			: `Send ${pageCount} Pages to Desktop`
	);
</script>

<div class="capture-actions">
	<div class="secondary-actions">
		<button class="action-btn" onclick={onRetake}>Retake</button>
		{#if canAdd}
			<button class="action-btn" onclick={onAddPage}>+ Page</button>
		{/if}
	</div>

	<button class="send-btn" onclick={onSend}>
		{sendLabel}
	</button>
</div>

<style>
	.capture-actions {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.secondary-actions {
		display: flex;
		gap: 8px;
	}

	.action-btn {
		flex: 1;
		padding: 12px;
		border: 1px solid #D6D3D1;
		border-radius: 12px;
		background: white;
		font-size: 15px;
		font-weight: 500;
		cursor: pointer;
		font-family: inherit;
		min-height: var(--min-touch-target);
	}

	.send-btn {
		width: 100%;
		padding: 16px;
		background: var(--color-primary);
		color: white;
		border: none;
		border-radius: 12px;
		font-size: 17px;
		font-weight: 600;
		cursor: pointer;
		font-family: inherit;
		min-height: var(--large-touch-target);
	}
</style>
