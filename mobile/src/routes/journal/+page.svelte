<!-- M1-04: Journal tab — entry history list + new entry button -->
<script lang="ts">
	import {
		journalEntries,
		entriesByDate,
		unsyncedCount,
		hasEntries
	} from '$lib/stores/journal.js';
	import JournalEntryCard from '$lib/components/journal/JournalEntryCard.svelte';
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
		<div class="entry-list" role="list" aria-label="Journal entries">
			{#each $entriesByDate as group (group.date)}
				<div class="date-group">
					<h3 class="date-label">{group.label}</h3>
					<div class="group-entries">
						{#each group.entries as entry (entry.id)}
							<JournalEntryCard {entry} onTap={handleTapEntry} />
						{/each}
					</div>
				</div>
			{/each}
		</div>
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

	.date-group {
		margin-bottom: 16px;
	}

	.date-label {
		font-size: 13px;
		font-weight: 600;
		color: var(--color-text-muted);
		text-transform: uppercase;
		letter-spacing: 0.5px;
		margin: 0 0 8px 4px;
		padding-bottom: 4px;
		border-bottom: 1px solid #E7E5E4;
	}

	.group-entries {
		display: flex;
		flex-direction: column;
		gap: 8px;
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
