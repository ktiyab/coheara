<!-- M1-04: Sync indicator — "N entries pending sync" -->
<script lang="ts">
	const { count }: { count: number } = $props();
</script>

{#if count > 0}
	<div class="sync-indicator" aria-label="{count} {count === 1 ? 'entry' : 'entries'} pending sync">
		<span class="badge">{count}</span>
		{count === 1 ? 'entry' : 'entries'} pending sync
	</div>
{/if}

<style>
	.sync-indicator {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 16px;
		background: #FFFBEB;
		border-radius: 8px;
		font-size: 14px;
		color: #92400E;
		margin-bottom: 12px;
	}

	.badge {
		background: var(--color-warning);
		color: white;
		width: 22px;
		height: 22px;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 12px;
		font-weight: 700;
	}
</style>
