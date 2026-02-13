<!-- M1-03: Medications tab — schedule-grouped list with search, detail, share -->
<script lang="ts">
	import { isConnected } from '$lib/stores/connection.js';
	import {
		medications,
		medicationsBySchedule,
		activeMedicationCount,
		discontinuedMedicationCount,
		lastSyncTimestamp,
		profile
	} from '$lib/stores/cache.js';
	import { searchMedications, shareMedicationList, emptyStateMessage } from '$lib/utils/viewer.js';
	import type { CachedMedication, MedicationDetail as MedDetailType, SharePayload } from '$lib/types/viewer.js';
	import FreshnessIndicator from '$lib/components/viewer/FreshnessIndicator.svelte';
	import MedicationScheduleGroup from '$lib/components/viewer/MedicationScheduleGroup.svelte';
	import MedicationCard from '$lib/components/viewer/MedicationCard.svelte';
	import MedicationSearch from '$lib/components/viewer/MedicationSearch.svelte';
	import MedicationDetail from '$lib/components/viewer/MedicationDetail.svelte';
	import ShareSheet from '$lib/components/viewer/ShareSheet.svelte';

	let searchVisible = $state(false);
	let searchQuery = $state('');
	let selectedMedId = $state<string | null>(null);
	let showDiscontinued = $state(false);
	let sharePayload = $state<SharePayload | null>(null);

	const filteredMeds = $derived(
		searchQuery ? searchMedications($medications, searchQuery) : $medications
	);

	const filteredActive = $derived(filteredMeds.filter((m) => m.isActive));
	const filteredDiscontinued = $derived(filteredMeds.filter((m) => !m.isActive));

	const selectedMed = $derived(
		selectedMedId ? $medications.find((m) => m.id === selectedMedId) ?? null : null
	);

	const grouped = $derived(
		searchQuery
			? null // When searching, show flat list
			: $medicationsBySchedule
	);

	function toggleSearch(): void {
		searchVisible = !searchVisible;
		if (!searchVisible) searchQuery = '';
	}

	function handleTapMedication(id: string): void {
		selectedMedId = id;
	}

	function handleCloseDetail(): void {
		selectedMedId = null;
	}

	function handleShare(): void {
		sharePayload = shareMedicationList(
			$medications,
			$profile?.name ?? 'Patient',
			$lastSyncTimestamp
		);
	}

	function handleShareMedication(_id: string): void {
		handleShare();
	}

	function handleCloseShare(): void {
		sharePayload = null;
	}
</script>

<div class="meds-screen">
	<div class="meds-header">
		<FreshnessIndicator
			syncTimestamp={$lastSyncTimestamp}
			profileName={$profile?.name}
		/>
	</div>

	<div class="title-row">
		<h1>Medications</h1>
		<button
			class="search-toggle"
			onclick={toggleSearch}
			aria-label={searchVisible ? 'Close search' : 'Search medications'}
			aria-expanded={searchVisible}
		>
			{searchVisible ? '\u2715' : '\uD83D\uDD0D'}
		</button>
	</div>

	<MedicationSearch
		query={searchQuery}
		onQueryChange={(q) => searchQuery = q}
		visible={searchVisible}
	/>

	{#if $medications.length === 0}
		<div class="empty-state">
			<p>{emptyStateMessage('medications')}</p>
		</div>
	{:else if filteredActive.length === 0 && filteredDiscontinued.length === 0}
		<div class="empty-state">
			<p>No medications match "{searchQuery}"</p>
		</div>
	{:else}
		<div class="meds-list" role="list" aria-label="Medication list">
			{#if searchQuery}
				<!-- Flat search results -->
				{#each filteredActive as med (med.id)}
					<MedicationCard medication={med} onTap={handleTapMedication} />
				{/each}
			{:else if grouped}
				<!-- Schedule-grouped view -->
				<MedicationScheduleGroup
					label="Morning"
					medications={grouped.morning}
					onTapMedication={handleTapMedication}
				/>
				<MedicationScheduleGroup
					label="Evening"
					medications={grouped.evening}
					onTapMedication={handleTapMedication}
				/>
				<MedicationScheduleGroup
					label="Multiple Times Daily"
					medications={grouped.multiple}
					onTapMedication={handleTapMedication}
				/>
				<MedicationScheduleGroup
					label="As Needed"
					medications={grouped.as_needed}
					onTapMedication={handleTapMedication}
				/>
			{/if}

			<!-- Discontinued section -->
			{#if $discontinuedMedicationCount > 0}
				<button
					class="discontinued-toggle"
					onclick={() => showDiscontinued = !showDiscontinued}
					aria-expanded={showDiscontinued}
				>
					Discontinued ({$discontinuedMedicationCount})
					<span class="toggle-arrow">{showDiscontinued ? '\u25B2' : '\u25BC'}</span>
				</button>

				{#if showDiscontinued}
					<div class="discontinued-list">
						{#each (searchQuery ? filteredDiscontinued : $medicationsBySchedule.discontinued) as med (med.id)}
							<MedicationCard medication={med} onTap={handleTapMedication} />
						{/each}
					</div>
				{/if}
			{/if}
		</div>

		<div class="share-area">
			<button class="share-list-btn" onclick={handleShare}>
				Share Medication List
			</button>
		</div>
	{/if}
</div>

<!-- Medication Detail Sheet -->
{#if selectedMed}
	<MedicationDetail
		medication={selectedMed}
		detail={null}
		connected={$isConnected}
		onClose={handleCloseDetail}
		onShare={handleShareMedication}
	/>
{/if}

<!-- Share Sheet -->
{#if sharePayload}
	<ShareSheet payload={sharePayload} onClose={handleCloseShare} />
{/if}

<style>
	.meds-screen {
		padding: 16px;
	}

	.meds-header {
		margin-bottom: 8px;
	}

	.title-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 12px;
	}

	.title-row h1 {
		font-size: 20px;
		font-weight: 700;
		margin: 0;
	}

	.search-toggle {
		width: 40px;
		height: 40px;
		border: none;
		background: #F5F5F4;
		border-radius: 50%;
		font-size: 18px;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.meds-list {
		margin-bottom: 16px;
	}

	.discontinued-toggle {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		padding: 12px 16px;
		margin-top: 12px;
		background: #F5F5F4;
		border: 1px solid #E7E5E4;
		border-radius: 12px;
		font-size: 14px;
		font-weight: 600;
		color: var(--color-text-muted);
		cursor: pointer;
		font-family: inherit;
		min-height: var(--min-touch-target);
	}

	.toggle-arrow {
		font-size: 12px;
	}

	.discontinued-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
		margin-top: 8px;
	}

	.share-area {
		padding: 16px 0;
	}

	.share-list-btn {
		width: 100%;
		padding: 14px;
		background: white;
		color: var(--color-primary);
		border: 2px solid var(--color-primary);
		border-radius: 12px;
		font-size: 16px;
		font-weight: 600;
		cursor: pointer;
		font-family: inherit;
		min-height: var(--min-touch-target);
	}

	.empty-state {
		display: flex;
		align-items: center;
		justify-content: center;
		min-height: 200px;
		text-align: center;
		padding: 24px;
	}

	.empty-state p {
		color: var(--color-text-muted);
		font-size: 16px;
		line-height: 1.5;
	}
</style>
