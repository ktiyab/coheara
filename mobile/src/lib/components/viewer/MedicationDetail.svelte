<!-- M1-03: Medication detail — bottom sheet overlay (cache + desktop enrichment) -->
<script lang="ts">
	import type { CachedMedication, MedicationDetail as MedicationDetailType } from '$lib/types/viewer.js';

	const { medication, detail, connected, onClose, onShare }: {
		medication: CachedMedication;
		detail: MedicationDetailType | null;
		connected: boolean;
		onClose: () => void;
		onShare: (id: string) => void;
	} = $props();

	const sinceFormatted = $derived(formatDate(medication.since));

	function formatDate(iso: string): string {
		const d = new Date(iso);
		return d.toLocaleDateString('en-US', { month: 'long', year: 'numeric' });
	}
</script>

<div class="detail-overlay" role="dialog" aria-label="Medication detail: {medication.name}">
	<div class="detail-sheet">
		<div class="sheet-header">
			<h2 class="sheet-title">{medication.name} {medication.dose}</h2>
			<button class="close-btn" onclick={onClose} aria-label="Close detail">&times;</button>
		</div>

		<div class="sheet-body">
			<dl class="detail-list">
				<dt>Dose</dt>
				<dd>{medication.dose} {medication.frequency}</dd>

				<dt>Prescriber</dt>
				<dd>{medication.prescriber}</dd>

				<dt>Purpose</dt>
				<dd>{medication.purpose}</dd>

				<dt>Since</dt>
				<dd>{sinceFormatted}</dd>

				{#if medication.sourceDocumentTitle}
					<dt>Source</dt>
					<dd>{medication.sourceDocumentTitle}</dd>
				{/if}
			</dl>

			{#if detail?.history && detail.history.length > 0}
				<section class="history-section">
					<h3>History</h3>
					{#each detail.history as entry}
						<div class="history-entry">
							<span class="history-date">{entry.date}</span>
							<span class="history-event">{entry.event}</span>
						</div>
					{/each}
				</section>
			{:else if connected}
				<p class="loading-note">Loading history...</p>
			{:else}
				<p class="offline-note">Connect to your desktop for full details</p>
			{/if}

			{#if medication.notes}
				<section class="notes-section">
					<h3>Notes</h3>
					<p>{medication.notes}</p>
				</section>
			{/if}

			<button class="share-btn" onclick={() => onShare(medication.id)}>
				Share This Medication
			</button>
		</div>
	</div>
</div>

<style>
	.detail-overlay {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.4);
		display: flex;
		align-items: flex-end;
		z-index: 50;
	}

	.detail-sheet {
		width: 100%;
		max-height: 80vh;
		background: white;
		border-radius: 16px 16px 0 0;
		overflow-y: auto;
		-webkit-overflow-scrolling: touch;
	}

	.sheet-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 16px;
		border-bottom: 1px solid #E7E5E4;
		position: sticky;
		top: 0;
		background: white;
	}

	.sheet-title {
		font-size: 18px;
		font-weight: 700;
		margin: 0;
	}

	.close-btn {
		width: 36px;
		height: 36px;
		border: none;
		background: #F5F5F4;
		border-radius: 50%;
		font-size: 20px;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.sheet-body {
		padding: 16px;
	}

	.detail-list {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 8px 16px;
		margin: 0 0 16px;
	}

	.detail-list dt {
		font-size: 14px;
		font-weight: 600;
		color: var(--color-text-muted);
	}

	.detail-list dd {
		font-size: 16px;
		margin: 0;
	}

	.history-section, .notes-section {
		margin-top: 16px;
		padding-top: 16px;
		border-top: 1px solid #E7E5E4;
	}

	.history-section h3, .notes-section h3 {
		font-size: 14px;
		font-weight: 600;
		color: var(--color-text-muted);
		margin: 0 0 8px;
	}

	.history-entry {
		display: flex;
		gap: 12px;
		padding: 6px 0;
		font-size: 14px;
	}

	.history-date {
		color: var(--color-text-muted);
		white-space: nowrap;
	}

	.notes-section p {
		font-size: 14px;
		line-height: 1.5;
		margin: 0;
	}

	.loading-note, .offline-note {
		font-size: 14px;
		color: var(--color-text-muted);
		font-style: italic;
		margin-top: 16px;
		padding-top: 16px;
		border-top: 1px solid #E7E5E4;
	}

	.share-btn {
		width: 100%;
		padding: 14px;
		margin-top: 20px;
		background: var(--color-primary);
		color: white;
		border: none;
		border-radius: 12px;
		font-size: 16px;
		font-weight: 600;
		cursor: pointer;
		min-height: var(--min-touch-target);
	}
</style>
