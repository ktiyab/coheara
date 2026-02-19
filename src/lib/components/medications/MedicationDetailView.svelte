<!-- L3-05: Full medication detail — expanded view with all clinical info. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { t } from 'svelte-i18n';
  import { getMedicationDetail } from '$lib/api/medications';
  import type { MedicationDetail } from '$lib/types/medication';
  import DoseHistory from './DoseHistory.svelte';
  import TaperingSchedule from './TaperingSchedule.svelte';
  import CompoundIngredients from './CompoundIngredients.svelte';

  import { navigation } from '$lib/stores/navigation.svelte';
  import BackButton from '$lib/components/ui/BackButton.svelte';
  import LoadingState from '$lib/components/ui/LoadingState.svelte';
  import ErrorState from '$lib/components/ui/ErrorState.svelte';
  import Badge from '$lib/components/ui/Badge.svelte';

  interface Props {
    medicationId: string;
  }
  let { medicationId }: Props = $props();

  let detail: MedicationDetail | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);
  let showDoseHistory = $state(false);

  onMount(async () => {
    try {
      detail = await getMedicationDetail(medicationId);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  });

  function formatDate(dateStr: string | null): string {
    if (!dateStr) return '';
    return new Date(dateStr).toLocaleDateString('en-US', {
      month: 'short', day: 'numeric', year: 'numeric',
    });
  }

  function formatRoute(route: string): string {
    if (!route) return '';
    return route.charAt(0).toUpperCase() + route.slice(1).toLowerCase();
  }
</script>

<div class="flex flex-col min-h-screen pb-20 bg-stone-50">
  <header class="px-6 pt-4 pb-2">
    <BackButton label={$t('medications.detail_back')} />
  </header>

  {#if loading}
    <LoadingState message={$t('medications.detail_loading')} />
  {:else if error}
    <ErrorState
      message={error}
      onretry={() => navigation.goBack()}
      retryLabel={$t('medications.detail_go_back')}
    />
  {:else if detail}
    <!-- Medication header -->
    <section class="px-6 py-4">
      <h2 class="text-2xl font-bold text-stone-800">
        {detail.medication.generic_name}
        <span class="text-xl font-semibold text-stone-600">{detail.medication.dose}</span>
      </h2>
      {#if detail.medication.brand_name}
        <p class="text-sm text-stone-500 mt-0.5">
          ({detail.medication.brand_name})
        </p>
      {/if}

      <div class="flex items-center gap-2 mt-3 text-sm text-stone-600">
        <Badge
          variant={detail.medication.status === 'active' ? 'success'
            : detail.medication.status === 'paused' ? 'warning'
            : 'neutral'}
          size="sm"
        >
          {detail.medication.status.charAt(0).toUpperCase() + detail.medication.status.slice(1)}
        </Badge>
        <span aria-hidden="true">&middot;</span>
        <span>{formatRoute(detail.medication.route)}</span>
        <span aria-hidden="true">&middot;</span>
        <span>{detail.medication.frequency}</span>
      </div>

      {#if detail.medication.prescriber_name}
        <p class="text-sm text-stone-500 mt-2">
          {$t('medications.detail_prescribed_by', { values: { name: detail.medication.prescriber_name } })}
          {#if detail.medication.prescriber_specialty}
            ({detail.medication.prescriber_specialty})
          {/if}
        </p>
      {:else if detail.medication.is_otc}
        <p class="text-sm text-stone-500 mt-2">{$t('medications.detail_otc_self')}</p>
      {/if}

      {#if detail.medication.start_date}
        <p class="text-sm text-stone-500 mt-1">
          {$t('medications.detail_started', { values: { date: formatDate(detail.medication.start_date) } })}
          {#if detail.medication.end_date}
            &middot; {$t('medications.detail_ended', { values: { date: formatDate(detail.medication.end_date) } })}
          {/if}
        </p>
      {/if}

      {#if detail.medication.reason_start}
        <p class="text-sm text-stone-600 mt-2 italic">
          {$t('medications.detail_reason', { values: { reason: detail.medication.reason_start } })}
        </p>
      {/if}

      {#if detail.medication.condition}
        <p class="text-sm text-stone-600 mt-1 italic">
          {detail.medication.condition}
        </p>
      {/if}
    </section>

    <!-- Coherence alerts -->
    {#if detail.medication.coherence_alerts.length > 0}
      <section class="px-6 py-2">
        {#each detail.medication.coherence_alerts as alert}
          <div
            class="px-4 py-3 rounded-xl mb-2 text-sm
                   {alert.severity === 'Critical'
                     ? 'bg-[var(--color-warning-50)] text-[var(--color-warning-800)] border border-[var(--color-warning-200)]'
                     : alert.severity === 'Warning'
                       ? 'bg-[var(--color-info-50)] text-[var(--color-info-800)] border border-[var(--color-info-200)]'
                       : 'bg-stone-50 text-stone-600 border border-stone-100'}"
            role="status"
          >
            {alert.summary}
          </div>
        {/each}
      </section>
    {/if}

    <!-- Instructions -->
    {#if detail.instructions.length > 0 || detail.medication.administration_instructions}
      <section class="px-6 py-4 border-t border-stone-100">
        <h3 class="text-sm font-medium text-stone-500 mb-2">{$t('medications.detail_instructions')}</h3>
        <ul class="flex flex-col gap-2">
          {#if detail.medication.administration_instructions}
            <li class="flex items-start gap-2 text-sm text-stone-700">
              <span class="text-stone-500 mt-0.5" aria-hidden="true">&#x2022;</span>
              <span>{detail.medication.administration_instructions}</span>
            </li>
          {/if}
          {#each detail.instructions as instr}
            <li class="flex items-start gap-2 text-sm text-stone-700">
              <span class="text-stone-500 mt-0.5" aria-hidden="true">&#x2022;</span>
              <span>
                {instr.instruction}
                {#if instr.timing}
                  <span class="text-stone-500">({instr.timing})</span>
                {/if}
              </span>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    <!-- Brand/Generic aliases -->
    {#if detail.aliases.length > 0}
      <section class="px-6 py-4 border-t border-stone-100">
        <h3 class="text-sm font-medium text-stone-500 mb-2">{$t('medications.detail_known_names')}</h3>
        <p class="text-sm text-stone-600">
          <span class="font-medium">{$t('medications.detail_generic')}</span> {detail.medication.generic_name}
        </p>
        <p class="text-sm text-stone-600 mt-1">
          <span class="font-medium">{$t('medications.detail_brand_names')}</span>
          {detail.aliases.map(a => a.brand_name).join(', ')}
        </p>
      </section>
    {/if}

    <!-- Compound ingredients -->
    {#if detail.compound_ingredients.length > 0}
      <section class="px-6 py-4 border-t border-stone-100">
        <CompoundIngredients ingredients={detail.compound_ingredients} />
      </section>
    {/if}

    <!-- Tapering schedule -->
    {#if detail.tapering_steps.length > 0}
      <section class="px-6 py-4 border-t border-stone-100">
        <TaperingSchedule steps={detail.tapering_steps} />
      </section>
    {/if}

    <!-- Dose change history -->
    {#if detail.dose_changes.length > 0}
      <section class="px-6 py-4 border-t border-stone-100">
        {#if showDoseHistory}
          <DoseHistory
            changes={detail.dose_changes}
            medicationName={detail.medication.generic_name}
            onClose={() => { showDoseHistory = false; }}
          />
        {:else}
          <button
            class="w-full text-left text-sm text-[var(--color-primary)] font-medium
                   min-h-[44px] flex items-center gap-2"
            onclick={() => { showDoseHistory = true; }}
          >
            {$t('medications.detail_dose_history', { values: { count: detail.dose_changes.length } })}
          </button>
        {/if}
      </section>
    {/if}

    <!-- Source document -->
    {#if detail.document_title}
      <section class="px-6 py-4 border-t border-stone-100">
        <h3 class="text-sm font-medium text-stone-500 mb-2">{$t('medications.detail_source_document')}</h3>
        <p class="text-sm text-stone-600">
          {detail.document_title}
          {#if detail.document_date}
            &middot; {formatDate(detail.document_date)}
          {/if}
        </p>
      </section>
    {/if}
  {/if}
</div>
