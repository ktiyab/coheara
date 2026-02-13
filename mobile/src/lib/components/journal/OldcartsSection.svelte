<!-- M1-04: OLDCARTS progressive disclosure — chip-based (Lena: not free text) -->
<script lang="ts">
	import type { OldcartsData, OnsetQuick, DurationQuick } from '$lib/types/journal.js';
	import { CHARACTER_OPTIONS, AGGRAVATING_OPTIONS, RELIEVING_OPTIONS, TIMING_OPTIONS } from '$lib/types/journal.js';

	const { oldcarts, onOldcartsChange }: {
		oldcarts: OldcartsData | null;
		onOldcartsChange: (data: OldcartsData) => void;
	} = $props();

	let expanded = $state(false);

	const data = $derived(oldcarts ?? {});

	function setOnset(quick: OnsetQuick): void {
		onOldcartsChange({ ...data, onset: { quick } });
	}

	function setDuration(quick: DurationQuick): void {
		onOldcartsChange({ ...data, duration: { quick } });
	}

	function toggleMulti(field: 'character' | 'aggravating' | 'relieving' | 'timing', value: string): void {
		const current = data[field] ?? [];
		const updated = current.includes(value)
			? current.filter((v) => v !== value)
			: [...current, value];
		onOldcartsChange({ ...data, [field]: updated });
	}
</script>

<div class="oldcarts">
	<button
		class="expand-toggle"
		onclick={() => expanded = !expanded}
		aria-expanded={expanded}
	>
		{expanded ? 'Less details \u25B2' : 'More details \u25BC'}
	</button>

	{#if expanded}
		<div class="oldcarts-fields">
			<!-- Onset -->
			<div class="field-group" role="group" aria-label="When did it start?">
				<p class="field-label">When did it start?</p>
				<div class="chip-row">
					{#each ['today', 'yesterday', 'this_week'] as opt (opt)}
						<button
							class="chip"
							class:active={data.onset?.quick === opt}
							onclick={() => setOnset(opt as OnsetQuick)}
						>
							{opt === 'today' ? 'Today' : opt === 'yesterday' ? 'Yesterday' : 'This week'}
						</button>
					{/each}
				</div>
			</div>

			<!-- Duration -->
			<div class="field-group" role="group" aria-label="How long does it last?">
				<p class="field-label">How long does it last?</p>
				<div class="chip-row">
					{#each ['minutes', 'hours', 'constant'] as opt (opt)}
						<button
							class="chip"
							class:active={data.duration?.quick === opt}
							onclick={() => setDuration(opt as DurationQuick)}
						>
							{opt === 'minutes' ? 'Minutes' : opt === 'hours' ? 'Hours' : 'Constant'}
						</button>
					{/each}
				</div>
			</div>

			<!-- Character -->
			<div class="field-group" role="group" aria-label="What does it feel like?">
				<p class="field-label">What does it feel like?</p>
				<div class="chip-row">
					{#each CHARACTER_OPTIONS as opt (opt)}
						<button
							class="chip"
							class:active={data.character?.includes(opt)}
							onclick={() => toggleMulti('character', opt)}
						>
							{opt.charAt(0).toUpperCase() + opt.slice(1)}
						</button>
					{/each}
				</div>
			</div>

			<!-- Aggravating -->
			<div class="field-group" role="group" aria-label="What makes it worse?">
				<p class="field-label">What makes it worse?</p>
				<div class="chip-row">
					{#each AGGRAVATING_OPTIONS as opt (opt)}
						<button
							class="chip"
							class:active={data.aggravating?.includes(opt)}
							onclick={() => toggleMulti('aggravating', opt)}
						>
							{opt.split('_').map((w) => w.charAt(0).toUpperCase() + w.slice(1)).join(' ')}
						</button>
					{/each}
				</div>
			</div>

			<!-- Relieving -->
			<div class="field-group" role="group" aria-label="What makes it better?">
				<p class="field-label">What makes it better?</p>
				<div class="chip-row">
					{#each RELIEVING_OPTIONS as opt (opt)}
						<button
							class="chip"
							class:active={data.relieving?.includes(opt)}
							onclick={() => toggleMulti('relieving', opt)}
						>
							{opt.split('_').map((w) => w.charAt(0).toUpperCase() + w.slice(1)).join(' ')}
						</button>
					{/each}
				</div>
			</div>

			<!-- Timing -->
			<div class="field-group" role="group" aria-label="Does it come and go?">
				<p class="field-label">Does it come and go?</p>
				<div class="chip-row">
					{#each TIMING_OPTIONS as opt (opt)}
						<button
							class="chip"
							class:active={data.timing?.includes(opt)}
							onclick={() => toggleMulti('timing', opt)}
						>
							{opt.split('_').map((w) => w.charAt(0).toUpperCase() + w.slice(1)).join(' ')}
						</button>
					{/each}
				</div>
			</div>
		</div>
	{/if}
</div>

<style>
	.expand-toggle {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 10px 0;
		border: none;
		background: transparent;
		font-size: 15px;
		font-weight: 500;
		color: var(--color-primary);
		cursor: pointer;
		font-family: inherit;
		min-height: var(--min-touch-target);
	}

	.oldcarts-fields {
		display: flex;
		flex-direction: column;
		gap: 16px;
		padding: 16px;
		background: #F5F5F4;
		border-radius: 12px;
		margin-top: 4px;
	}

	.field-group {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.field-label {
		font-size: 15px;
		font-weight: 500;
	}

	.chip-row {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}

	.chip {
		padding: 6px 14px;
		border: 1px solid #D6D3D1;
		border-radius: 20px;
		background: white;
		font-size: 14px;
		cursor: pointer;
		font-family: inherit;
		min-height: 36px;
	}

	.chip.active {
		background: var(--color-primary);
		color: white;
		border-color: var(--color-primary);
	}
</style>
