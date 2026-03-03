<!-- ME-REDESIGN + ME-06: Health center with reference ranges, screenings, and vaccines. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { t, locale } from 'svelte-i18n';
  import { getMeOverview } from '$lib/api/me';
  import type { MeOverview, ScreeningInfo } from '$lib/types/me';
  import LoadingState from '$lib/components/ui/LoadingState.svelte';
  import ErrorState from '$lib/components/ui/ErrorState.svelte';
  import ProfileCard from './ProfileCard.svelte';
  import InsightCard from './InsightCard.svelte';
  import MetricTile from './MetricTile.svelte';
  import ScreeningCard from './ScreeningCard.svelte';
  import CalibrationBanner from './CalibrationBanner.svelte';
  import RecordScreeningModal from './RecordScreeningModal.svelte';

  let data = $state<MeOverview | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(true);
  let recordTarget = $state<ScreeningInfo | null>(null);

  async function load() {
    loading = true;
    error = null;
    try {
      data = await getMeOverview($locale ?? 'en');
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => { load(); });

  let vitalRanges = $derived(data?.reference_ranges.filter(r => r.domain === 'vitals') ?? []);
  let labRanges = $derived(data?.reference_ranges.filter(r => r.domain === 'labs') ?? []);

  // ME-06: Split by category
  let vaccines = $derived(data?.screenings.filter(s => s.category === 'vaccine') ?? []);
  let cancerScreenings = $derived(data?.screenings.filter(s => s.category !== 'vaccine') ?? []);
  let eligibleScreenings = $derived(cancerScreenings.filter(s => s.eligible));
  let otherScreenings = $derived(cancerScreenings.filter(s => !s.eligible));

  function handleRecord(screening: ScreeningInfo) {
    recordTarget = screening;
  }

  function handleRecorded() {
    recordTarget = null;
    load();
  }
</script>

<div class="flex flex-col bg-stone-50 dark:bg-gray-950 min-h-full">
  <!-- Header -->
  <header class="px-[var(--spacing-page-x)] pt-6 pb-4">
    <h1 class="text-2xl font-bold text-stone-800 dark:text-gray-100">
      {$t('me.heading')}
    </h1>
    <p class="text-xs text-stone-400 dark:text-gray-500 mt-1">
      {$t('me.heading_subtitle')}
    </p>
  </header>

  {#if loading}
    <LoadingState message={$t('me.loading')} />
  {:else if error}
    <ErrorState
      message="{$t('me.error')}: {error}"
      onretry={load}
    />
  {:else if data}
    <div class="px-[var(--spacing-page-x)] pb-8 flex flex-col gap-6">
      <!-- Identity zone -->
      <ProfileCard identity={data.identity} onUpdated={load} />

      <!-- ME-04 B6: Calibration explainer -->
      <CalibrationBanner identity={data.identity} />

      <!-- Guideline Notes -->
      {#if data.alerts.length > 0}
        <section>
          <h2 class="text-sm font-semibold text-stone-600 dark:text-gray-400 uppercase tracking-wide mb-3">
            {$t('me.alerts_section')}
          </h2>
          <div class="flex flex-col gap-2">
            {#each data.alerts as insight (insight.summary_key)}
              <InsightCard {insight} />
            {/each}
          </div>
        </section>
      {/if}

      <!-- ME-06 QA: Vaccinations — always visible, grid layout (3-4 per row) -->
      <section>
        <h2 class="text-sm font-semibold text-stone-600 dark:text-gray-400 uppercase tracking-wide mb-3">
          {$t('me.vaccines_section')}
        </h2>
        {#if vaccines.length > 0}
          <div class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-3">
            {#each vaccines as screening (screening.key)}
              <ScreeningCard {screening} onrecord={handleRecord} onrefresh={load} />
            {/each}
          </div>
        {:else}
          <p class="text-sm text-stone-500 dark:text-gray-400">
            {$t('me.vaccines_empty')}
          </p>
        {/if}
      </section>

      <!-- Vital Signs -->
      {#if vitalRanges.length > 0}
        <section>
          <h2 class="text-sm font-semibold text-stone-600 dark:text-gray-400 uppercase tracking-wide mb-3">
            {$t('me.vitals_section')}
          </h2>
          <div class="grid grid-cols-2 gap-3">
            {#each vitalRanges as range (range.key)}
              <MetricTile {range} />
            {/each}
          </div>
        </section>
      {/if}

      <!-- Laboratory -->
      {#if labRanges.length > 0}
        <section>
          <h2 class="text-sm font-semibold text-stone-600 dark:text-gray-400 uppercase tracking-wide mb-3">
            {$t('me.labs_section')}
          </h2>
          <div class="grid grid-cols-2 gap-3">
            {#each labRanges as range (range.key)}
              <MetricTile {range} />
            {/each}
          </div>
        </section>
      {/if}

      <!-- Screenings (cancer/metabolic) -->
      <section>
        <h2 class="text-sm font-semibold text-stone-600 dark:text-gray-400 uppercase tracking-wide mb-3">
          {$t('me.screenings_section')}
        </h2>
        {#if eligibleScreenings.length > 0 || otherScreenings.length > 0}
          <div class="flex flex-col gap-2">
            {#each eligibleScreenings as screening (screening.key)}
              <ScreeningCard {screening} onrecord={handleRecord} onrefresh={load} />
            {/each}
            {#each otherScreenings as screening (screening.key)}
              <ScreeningCard {screening} onrecord={handleRecord} onrefresh={load} />
            {/each}
          </div>
        {:else}
          <p class="text-sm text-stone-500 dark:text-gray-400">
            {$t('me.screenings_empty')}
          </p>
        {/if}
      </section>
    </div>
  {/if}
</div>

<!-- ME-06: Record screening modal -->
{#if recordTarget}
  <RecordScreeningModal
    screening={recordTarget}
    onrecorded={handleRecorded}
    onclose={() => { recordTarget = null; }}
  />
{/if}
