<!-- L3-05: Main medication list screen with filters, status tabs, card list. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { onMount } from 'svelte';
  import { getMedications } from '$lib/api/medications';
  import type {
    MedicationListData,
    MedicationListFilter,
  } from '$lib/types/medication';
  import { navigation } from '$lib/stores/navigation.svelte';
  import MedicationCardView from './MedicationCardView.svelte';
  import MedicationSearch from './MedicationSearch.svelte';
  import EmptyState from '$lib/components/ui/EmptyState.svelte';
  import { PillIcon } from '$lib/components/icons';
  import InteractionCheckButton from './InteractionCheckButton.svelte';
  import LoadingState from '$lib/components/ui/LoadingState.svelte';
  import ErrorState from '$lib/components/ui/ErrorState.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import TabGroup from '$lib/components/ui/TabGroup.svelte';

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
      { label: $t('medications.list_tab_all'), value: null as string | null, count: totalCount },
      { label: $t('medications.list_tab_active'), value: 'active' as string | null, count: d?.total_active ?? 0 },
      { label: $t('medications.list_tab_paused'), value: 'paused' as string | null, count: d?.total_paused ?? 0 },
      { label: $t('medications.list_tab_stopped'), value: 'stopped' as string | null, count: d?.total_stopped ?? 0 },
    ];
  });
</script>

<div class="flex flex-col bg-stone-50 dark:bg-gray-950">
  <!-- Header -->
  <header class="px-6 pt-6 pb-4">
    <h1 class="text-2xl font-bold text-stone-800 dark:text-gray-100">{$t('medications.list_title')}</h1>
    {#if data}
      <p class="text-sm text-stone-500 dark:text-gray-400 mt-1">
        {data.total_active} {$t('medications.list_summary_active')}{data.total_paused > 0 ? ` \u00B7 ${data.total_paused} ${$t('medications.list_summary_paused')}` : ''}{data.total_stopped > 0 ? ` \u00B7 ${data.total_stopped} ${$t('medications.list_summary_stopped')}` : ''}
      </p>
    {/if}
  </header>

  {#if loading && !data}
    <LoadingState message={$t('medications.list_loading')} />
  {:else if error}
    <ErrorState
      message={$t('medications.list_error', { values: { error } })}
      onretry={refresh}
      retryLabel={$t('medications.list_try_again')}
    />
  {:else if data && totalCount === 0 && !searchQuery}
    <EmptyState
      icon={PillIcon}
      title={$t('medications.empty_title')}
      description={$t('medications.empty_description')}
      actionLabel={$t('medications.empty_add_otc')}
      onaction={() => navigation.navigate('otc-entry')}
    />
  {:else if data}
    <!-- Spec 49 [FE-03]: Drug interaction check -->
    <InteractionCheckButton medications={data.medications} />

    <!-- Search and filter bar -->
    <MedicationSearch
      value={searchQuery}
      onInput={handleSearchInput}
      prescribers={data.prescribers}
      selectedPrescriber={prescriberFilter}
      onPrescriberChange={(id) => { prescriberFilter = id; }}
    />

    <!-- Status tabs -->
    <div class="px-6 py-2">
      <TabGroup
        tabs={statusTabs.map(t => ({ value: t.value ?? 'all', label: t.label, count: t.count }))}
        selected={statusFilter ?? 'all'}
        onselect={(v) => { statusFilter = v === 'all' ? null : v; }}
      />
    </div>

    <!-- Medication cards -->
    <div class="px-6 py-3 flex flex-col gap-3" role="list" aria-label={$t('medications.list_title')}>
      {#each data.medications as medication (medication.id)}
        <div role="listitem">
          <MedicationCardView
            {medication}
            onTap={(med) => navigation.navigate('medication-detail', { medicationId: med.id })}
          />
        </div>
      {:else}
        <div class="text-center py-8 text-stone-500 dark:text-gray-400 text-sm">
          {$t('medications.list_no_results')}
        </div>
      {/each}
    </div>

    <!-- Add OTC button -->
    <div class="px-6 py-4">
      <Button variant="dashed" fullWidth onclick={() => navigation.navigate('otc-entry')}>
        {$t('medications.list_add_otc')}
      </Button>
    </div>
  {/if}
</div>
