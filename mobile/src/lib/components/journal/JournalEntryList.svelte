<!-- M1-04: Journal entry list — grouped by date with cards -->
<script lang="ts">
	import type { JournalDateGroup } from '$lib/types/journal.js';
	import JournalEntryCard from './JournalEntryCard.svelte';

	const { groups, onTapEntry }: {
		groups: JournalDateGroup[];
		onTapEntry: (id: string) => void;
	} = $props();
</script>

<div class="entry-list" role="list" aria-label="Journal entries">
	{#each groups as group (group.date)}
		<section class="date-group" aria-label="{group.label} entries">
			<h3 class="date-label">{group.label}</h3>
			<div class="group-entries">
				{#each group.entries as entry (entry.id)}
					<JournalEntryCard {entry} onTap={onTapEntry} />
				{/each}
			</div>
		</section>
	{/each}
</div>

<style>
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
</style>
