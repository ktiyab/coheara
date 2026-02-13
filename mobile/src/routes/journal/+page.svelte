<!-- M1-04: Journal tab — entry history list + new entry button -->
<script lang="ts">
	import {
		entriesByDate,
		unsyncedCount,
		hasEntries
	} from '$lib/stores/journal.js';
	import JournalEntryList from '$lib/components/journal/JournalEntryList.svelte';
	import SyncIndicator from '$lib/components/journal/SyncIndicator.svelte';

	function handleTapEntry(id: string): void {
		// Future: open entry detail view
	}
</script>

<div class="journal-screen">
	<div class="journal-header">
		<h1>Journal</h1>
		<a class="new-btn" href="/journal/new" aria-label="New journal entry">+ New</a>
	</div>

	<SyncIndicator count={$unsyncedCount} />

	{#if !$hasEntries}
		<div class="empty-state">
			<p class="empty-title">No journal entries yet</p>
			<p class="empty-text">Track how you're feeling. Entries sync to your desktop for your healthcare team.</p>
			<a class="empty-cta" href="/journal/new">Create your first entry</a>
		</div>
	{:else}
		<JournalEntryList groups={$entriesByDate} onTapEntry={handleTapEntry} />
	{/if}
</div>

<style>
	.journal-screen {
		padding: 16px;
	}

	.journal-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 16px;
	}

	h1 {
		font-size: 20px;
		font-weight: 700;
		margin: 0;
	}

	.new-btn {
		padding: 8px 20px;
		background: var(--color-primary);
		color: white;
		border-radius: 20px;
		text-decoration: none;
		font-size: 15px;
		font-weight: 600;
		min-height: var(--min-touch-target);
		display: flex;
		align-items: center;
	}

	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		min-height: 300px;
		text-align: center;
		padding: 24px;
	}

	.empty-title {
		font-size: 18px;
		font-weight: 600;
		margin: 0 0 8px;
	}

	.empty-text {
		color: var(--color-text-muted);
		font-size: 15px;
		line-height: 1.5;
		margin: 0 0 20px;
	}

	.empty-cta {
		padding: 14px 28px;
		background: var(--color-primary);
		color: white;
		border-radius: 12px;
		text-decoration: none;
		font-size: 16px;
		font-weight: 600;
		min-height: var(--min-touch-target);
		display: flex;
		align-items: center;
	}
</style>
