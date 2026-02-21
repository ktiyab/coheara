<!-- V14: Single-screen progressive symptom entry replacing QuickLogPanel + RecordingFlow. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { recordSymptom } from '$lib/api/journal';
  import { getMedications } from '$lib/api/medications';
  import type { SymptomEntry, TemporalCorrelation } from '$lib/types/journal';
  import type { MedicationCard } from '$lib/types/medication';
  import { COMMON_SYMPTOMS } from '$lib/types/journal';
  import SeverityStrip from './SeverityStrip.svelte';
  import DateSelector from './DateSelector.svelte';
  import ExpandedDetails from './ExpandedDetails.svelte';
  import CategorySelector from './CategorySelector.svelte';
  import CorrelationCard from './CorrelationCard.svelte';
  import BackButton from '$lib/components/ui/BackButton.svelte';
  import Button from '$lib/components/ui/Button.svelte';
  import { onMount } from 'svelte';

  interface Props {
    onComplete: () => void;
    onCancel: () => void;
  }
  let { onComplete, onCancel }: Props = $props();

  // Symptom selection
  let category = $state('');
  let specific = $state('');
  let showMoreSymptoms = $state(false);

  // Severity
  let severity = $state(0);

  // Date/time
  let onsetDate = $state(new Date().toISOString().split('T')[0]);
  let onsetTime = $state<string | null>(null);

  // Notes
  let notes = $state('');

  // OLDCARTS expanded details
  let showDetails = $state(false);
  let bodyRegion = $state<string | null>(null);
  let duration = $state<string | null>(null);
  let character = $state<string | null>(null);
  let aggravating = $state<string[]>([]);
  let relieving = $state<string[]>([]);
  let timingPattern = $state<string | null>(null);

  // Active medications context
  let activeMeds = $state<MedicationCard[]>([]);

  // Save state
  let saving = $state(false);
  let saved = $state(false);
  let correlations = $state<TemporalCorrelation[]>([]);

  let canSave = $derived(category !== '' && specific !== '' && severity >= 1);

  onMount(async () => {
    try {
      const data = await getMedications({
        status: 'active',
        prescriber_id: null,
        search_query: null,
        include_otc: true,
      });
      activeMeds = data.medications;
    } catch {
      // Non-critical: medication context is informational only
    }
  });

  function selectCommonSymptom(sym: typeof COMMON_SYMPTOMS[number]) {
    category = sym.category;
    specific = sym.specific;
    showMoreSymptoms = false;
  }

  function selectFromCategory(cat: string, spec: string) {
    category = cat;
    specific = spec;
    showMoreSymptoms = false;
  }

  async function save() {
    if (!canSave || saving) return;
    saving = true;

    const entry: SymptomEntry = {
      category,
      specific,
      severity,
      onset_date: onsetDate,
      onset_time: onsetTime,
      body_region: bodyRegion,
      duration,
      character,
      aggravating,
      relieving,
      timing_pattern: timingPattern,
      notes: notes.trim() || null,
    };

    try {
      const result = await recordSymptom(entry);
      correlations = result.correlations;
      saved = true;
      setTimeout(() => onComplete(), 3000);
    } catch (e) {
      console.error('Failed to record symptom:', e);
      saving = false;
    }
  }
</script>

<div class="px-6 py-4 flex flex-col gap-5">
  <BackButton label={$t('recording_cancel')} onclick={onCancel} />

  {#if saved}
    <!-- Success state -->
    <div class="text-center py-8">
      <p class="text-lg font-medium text-[var(--color-success)]">{$t('journal.entry_success')}</p>
      {#each correlations as corr}
        <CorrelationCard correlation={corr} />
      {/each}
    </div>
  {:else}
    <!-- Section 1: Symptom selection -->
    <div>
      <h2 class="text-lg font-medium text-stone-800 dark:text-gray-100 mb-3">{$t('journal.entry_heading')}</h2>
      <div class="flex flex-wrap gap-2">
        {#each COMMON_SYMPTOMS as sym}
          {@const selected = category === sym.category && specific === sym.specific}
          <button
            class="px-4 py-2 rounded-lg border text-sm min-h-[44px] transition-colors
                   {selected
                     ? 'border-[var(--color-primary)] bg-[var(--color-primary-50)] text-[var(--color-primary)] font-medium'
                     : 'border-stone-200 dark:border-gray-700 bg-white dark:bg-gray-900 text-stone-600 dark:text-gray-300 hover:bg-stone-50 dark:hover:bg-gray-800'}"
            onclick={() => selectCommonSymptom(sym)}
          >
            {$t(sym.labelKey)}
          </button>
        {/each}
      </div>

      <!-- More symptoms toggle -->
      <button
        class="text-xs text-[var(--color-primary)] mt-2 min-h-[44px] px-1"
        onclick={() => { showMoreSymptoms = !showMoreSymptoms; }}
      >
        {showMoreSymptoms ? $t('journal.entry_less_symptoms') : $t('journal.entry_more_symptoms')}
      </button>

      {#if showMoreSymptoms}
        <div class="mt-2">
          <CategorySelector inline onSelect={selectFromCategory} />
        </div>
      {/if}
    </div>

    <!-- Section 2: Severity (visible once symptom selected) -->
    {#if category && specific}
      <div>
        <h3 class="text-sm font-medium text-stone-600 dark:text-gray-300 mb-2">{$t('journal.entry_severity_label')}</h3>
        <SeverityStrip value={severity} onChange={(v) => { severity = v; }} />
      </div>
    {/if}

    <!-- Section 3: When (visible once severity set) -->
    {#if severity >= 1}
      <div>
        <h3 class="text-sm font-medium text-stone-600 dark:text-gray-300 mb-2">{$t('journal.entry_when_label')}</h3>
        <DateSelector
          date={onsetDate}
          time={onsetTime}
          onDateChange={(d) => { onsetDate = d; }}
          onTimeChange={(t) => { onsetTime = t; }}
        />
      </div>

      <!-- Notes -->
      <div>
        <h3 class="text-sm font-medium text-stone-600 dark:text-gray-300 mb-2">{$t('journal.entry_note_label')}</h3>
        <textarea
          class="w-full px-4 py-3 rounded-xl border border-stone-200 dark:border-gray-700
                 bg-white dark:bg-gray-900 text-stone-700 dark:text-gray-200 text-sm
                 focus:border-[var(--color-primary)] focus:outline-none resize-none"
          rows="2"
          maxlength={500}
          placeholder={$t('journal.entry_note_placeholder')}
          aria-label={$t('journal.entry_note_label')}
          bind:value={notes}
        ></textarea>
      </div>

      <!-- Active medications context -->
      {#if activeMeds.length > 0}
        <div>
          <h3 class="text-xs font-medium text-stone-500 dark:text-gray-400 mb-1">{$t('journal.entry_meds_context')}</h3>
          <p class="text-xs text-stone-500 dark:text-gray-400">
            {activeMeds.map(m => `${m.generic_name} ${m.dose}`).join(' \u00B7 ')}
          </p>
        </div>
      {/if}

      <!-- More details toggle (OLDCARTS) -->
      <button
        class="text-xs text-[var(--color-primary)] min-h-[44px] px-1"
        onclick={() => { showDetails = !showDetails; }}
      >
        {showDetails ? $t('journal.entry_details_hide') : $t('journal.entry_details_show')}
      </button>

      {#if showDetails}
        <ExpandedDetails
          {bodyRegion}
          {duration}
          {character}
          {aggravating}
          {relieving}
          {timingPattern}
          onBodyRegionChange={(v) => { bodyRegion = v; }}
          onDurationChange={(v) => { duration = v; }}
          onCharacterChange={(v) => { character = v; }}
          onAggravatingChange={(v) => { aggravating = v; }}
          onRelievingChange={(v) => { relieving = v; }}
          onTimingChange={(v) => { timingPattern = v; }}
        />
      {/if}

      <!-- Save CTA -->
      <Button variant="primary" fullWidth disabled={!canSave || saving} onclick={save}>
        {saving ? $t('journal.entry_saving') : $t('journal.entry_save')}
      </Button>
    {/if}
  {/if}
</div>
