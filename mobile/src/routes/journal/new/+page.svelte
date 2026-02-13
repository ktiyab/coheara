<!-- M1-04: New journal entry — funnel flow (Lena: severity faces first) -->
<script lang="ts">
	import { isConnected } from '$lib/stores/connection.js';
	import { saveEntry, emptyDraft, isDraftValid } from '$lib/stores/journal.js';
	import type { JournalEntryDraft, SymptomChip, BodyRegion, OldcartsData } from '$lib/types/journal.js';
	import SeverityPicker from '$lib/components/journal/SeverityPicker.svelte';
	import SymptomChips from '$lib/components/journal/SymptomChips.svelte';
	import BodyMap from '$lib/components/journal/BodyMap.svelte';
	import OldcartsSection from '$lib/components/journal/OldcartsSection.svelte';

	let draft = $state<JournalEntryDraft>(emptyDraft());
	let saved = $state(false);
	let saveMessage = $state('');

	const canSave = $derived(isDraftValid(draft));

	function handleSeverityChange(value: number): void {
		draft.severity = value;
	}

	function handleSymptomSelect(chip: SymptomChip | null): void {
		draft.symptomChip = chip;
	}

	function handleToggleRegion(region: BodyRegion): void {
		if (draft.bodyLocations.includes(region)) {
			draft.bodyLocations = draft.bodyLocations.filter((r) => r !== region);
		} else {
			draft.bodyLocations = [...draft.bodyLocations, region];
		}
	}

	function handleOldcartsChange(data: OldcartsData): void {
		draft.oldcarts = data;
	}

	function handleSave(): void {
		if (!canSave) return;

		const result = saveEntry(draft, $isConnected);
		saved = true;

		switch (result) {
			case 'saved_offline':
				saveMessage = 'Saved. Will sync when connected.';
				break;
			case 'saved_synced':
				saveMessage = 'Saved and synced.';
				break;
			case 'saved_sync_failed':
				saveMessage = 'Saved locally. Sync will retry.';
				break;
		}
	}
</script>

<div class="new-entry-screen">
	<div class="entry-header">
		<h1>New Journal Entry</h1>
		<a class="close-btn" href="/journal" aria-label="Close">&times;</a>
	</div>

	{#if saved}
		<div class="save-confirmation">
			<p class="confirm-check" aria-hidden="true">&#10003;</p>
			<p class="confirm-message">{saveMessage}</p>
			<a class="confirm-link" href="/journal">Back to Journal</a>
		</div>
	{:else}
		<div class="entry-form">
			<SeverityPicker
				severity={draft.severity}
				onSeverityChange={handleSeverityChange}
			/>

			<SymptomChips
				selected={draft.symptomChip}
				onSelect={handleSymptomSelect}
			/>

			<BodyMap
				selected={draft.bodyLocations}
				onToggleRegion={handleToggleRegion}
			/>

			<!-- Free text -->
			<div class="field-group">
				<label class="field-label" for="free-text">What's happening?</label>
				<textarea
					id="free-text"
					class="text-input"
					placeholder="Describe how you're feeling..."
					bind:value={draft.freeText}
					rows={3}
				></textarea>
			</div>

			<!-- Activity context (Dr. Diallo MI-20) -->
			<div class="field-group">
				<label class="field-label" for="activity">
					What were you doing? <span class="recommended">(recommended)</span>
				</label>
				<input
					id="activity"
					class="text-input single-line"
					placeholder="Walking, eating, resting..."
					bind:value={draft.activityContext}
				/>
			</div>

			<!-- OLDCARTS progressive disclosure -->
			<OldcartsSection
				oldcarts={draft.oldcarts}
				onOldcartsChange={handleOldcartsChange}
			/>

			<!-- Save button -->
			<button
				class="save-btn"
				disabled={!canSave}
				onclick={handleSave}
			>
				Save Entry
			</button>
		</div>
	{/if}
</div>

<style>
	.new-entry-screen {
		padding: 16px;
	}

	.entry-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 20px;
	}

	h1 {
		font-size: 20px;
		font-weight: 700;
		margin: 0;
	}

	.close-btn {
		width: 40px;
		height: 40px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: #F5F5F4;
		border-radius: 50%;
		text-decoration: none;
		color: var(--color-text);
		font-size: 22px;
	}

	.entry-form {
		display: flex;
		flex-direction: column;
	}

	.field-group {
		margin-bottom: 20px;
	}

	.field-label {
		display: block;
		font-size: 16px;
		font-weight: 600;
		margin-bottom: 8px;
	}

	.recommended {
		font-weight: 400;
		color: var(--color-text-muted);
		font-size: 14px;
	}

	.text-input {
		width: 100%;
		padding: 12px 14px;
		border: 1px solid #D6D3D1;
		border-radius: 12px;
		font-size: 16px;
		font-family: inherit;
		resize: vertical;
		box-sizing: border-box;
	}

	.text-input.single-line {
		min-height: var(--min-touch-target);
	}

	.text-input:focus {
		outline: 2px solid var(--color-primary);
		outline-offset: -1px;
	}

	.save-btn {
		width: 100%;
		padding: 16px;
		margin-top: 8px;
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

	.save-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.save-confirmation {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		min-height: 300px;
		text-align: center;
	}

	.confirm-check {
		font-size: 48px;
		color: var(--color-success);
		margin: 0 0 12px;
	}

	.confirm-message {
		font-size: 16px;
		color: var(--color-text-muted);
		margin: 0 0 24px;
	}

	.confirm-link {
		padding: 14px 28px;
		background: var(--color-primary);
		color: white;
		border-radius: 12px;
		text-decoration: none;
		font-size: 16px;
		font-weight: 600;
	}
</style>
