<!-- L3-05: Main medication list screen with filters, status tabs, card list. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { getMedications } from '$lib/api/medications';
  import type {
    MedicationListData,
    MedicationListFilter,
  } from '$lib/types/medication';
  import { navigation } from '$lib/stores/navigation.svelte';
  import MedicationCardView from './MedicationCardView.svelte';
  import MedicationSearch from './MedicationSearch.svelte';
  import EmptyMedicationState from './EmptyMedicationState.svelte';

  let data: MedicationListData | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);

  // Filter state
  let statusFilter = $state<string | null>(null);
  let prescriberFilter = $state<string | null>(null);
  let searchQuery = $state('');
  let includeOtc = $state(true);

  let currentFilter = $derived<MedicationListFilter>({
    status: statusFilter,
    prescriber_id: prescriberFilter,
    search_query: searchQuery.trim() || null,
    include_otc: includeOtc,
  });

  let searchTimeout: ReturnType<typeof setTimeout> | null = $state(null);

  function handleSearchInput(value: string) {
    searchQuery = value;
    if (searchTimeout) clearTimeout(searchTimeout);
    searchTimeout = setTimeout(() => refresh(), 300);
  }

  async function refresh() {
    try {
      loading = data === null;
      error = null;
      data = await getMedications(currentFilter);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    refresh();
  });

  // Re-fetch when filter changes (except search, which is debounced)
  $effect(() => {
    statusFilter;
    prescriberFilter;
    includeOtc;
    refresh();
  });

  let totalCount = $derived.by(() => {
    const d: MedicationListData | null = data;
    return d ? d.total_active + d.total_paused + d.total_stopped : 0;
  });

  let statusTabs = $derived.by(() => {
    const d: MedicationListData | null = data;
    return [
      { label: 'All', value: null as string | null, count: totalCount },
      { label: 'Active', value: 'active' as string | null, count: d?.total_active ?? 0 },
      { label: 'Paused', value: 'paused' as string | null, count: d?.total_paused ?? 0 },
      { label: 'Stopped', value: 'stopped' as string | null, count: d?.total_stopped ?? 0 },
    ];
  });
</script>

<div class="flex flex-col min-h-screen pb-20 bg-stone-50">
  <!-- Header -->
  <header class="px-6 pt-6 pb-4">
    <h1 class="text-2xl font-bold text-stone-800">Medications</h1>
    {#if data}
      <p class="text-sm text-stone-500 mt-1">
        {data.total_active} active{data.total_paused > 0 ? ` \u00B7 ${data.total_paused} paused` : ''}{data.total_stopped > 0 ? ` \u00B7 ${data.total_stopped} stopped` : ''}
      </p>
    {/if}
  </header>

  {#if loading && !data}
    <div class="flex items-center justify-center flex-1">
      <div class="animate-pulse text-stone-400">Loading medications...</div>
    </div>
  {:else if error}
    <div class="px-6 py-8 text-center">
      <p class="text-red-600 mb-4">Something went wrong: {error}</p>
      <button
        class="px-6 py-3 bg-stone-200 rounded-xl text-stone-700 min-h-[44px]"
        onclick={refresh}
      >
        Try again
      </button>
    </div>
  {:else if data && totalCount === 0 && !searchQuery}
    <EmptyMedicationState
      onAddOtc={() => navigation.navigate('otc-entry')}
    />
  {:else if data}
    <!-- Search and filter bar -->
    <MedicationSearch
      value={searchQuery}
      onInput={handleSearchInput}
      prescribers={data.prescribers}
      selectedPrescriber={prescriberFilter}
      onPrescriberChange={(id) => { prescriberFilter = id; }}
    />

    <!-- Status tabs -->
    <div class="px-6 py-2 flex gap-2 overflow-x-auto">
      {#each statusTabs as tab}
        <button
          class="px-4 py-2 rounded-full text-sm font-medium whitespace-nowrap
                 min-h-[44px] transition-colors
                 {statusFilter === tab.value
                   ? 'bg-[var(--color-primary)] text-white'
                   : 'bg-white text-stone-600 border border-stone-200 hover:bg-stone-50'}"
          onclick={() => { statusFilter = tab.value; }}
          aria-pressed={statusFilter === tab.value}
        >
          {tab.label} ({tab.count})
        </button>
      {/each}
    </div>

    <!-- Medication cards -->
    <div class="px-6 py-3 flex flex-col gap-3">
      {#each data.medications as medication (medication.id)}
        <MedicationCardView
          {medication}
          onTap={(med) => navigation.navigate('medication-detail', { medicationId: med.id })}
        />
      {:else}
        <div class="text-center py-8 text-stone-400 text-sm">
          No medications match your filters.
        </div>
      {/each}
    </div>

    <!-- Add OTC button -->
    <div class="px-6 py-4">
      <button
        class="w-full px-6 py-4 border border-dashed border-stone-300 rounded-xl
               text-stone-500 hover:border-[var(--color-primary)]
               hover:text-[var(--color-primary)] transition-all min-h-[44px]"
        onclick={() => navigation.navigate('otc-entry')}
      >
        + Add an over-the-counter medication
      </button>
    </div>
  {/if}
</div>
