<!-- L4-01: Multi-step symptom recording flow. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { recordSymptom } from '$lib/api/journal';
  import type { SymptomEntry, TemporalCorrelation } from '$lib/types/journal';
  import CategorySelector from './CategorySelector.svelte';
  import SeverityScale from './SeverityScale.svelte';
  import DateSelector from './DateSelector.svelte';
  import ExpandedDetails from './ExpandedDetails.svelte';
  import CorrelationCard from './CorrelationCard.svelte';
  import BackButton from '$lib/components/ui/BackButton.svelte';
  import Button from '$lib/components/ui/Button.svelte';

  interface Props {
    onComplete: () => void;
    onCancel: () => void;
  }
  let { onComplete, onCancel }: Props = $props();

  type Step = 'category' | 'severity' | 'when' | 'expanded' | 'notes' | 'done';

  let step: Step = $state('category');
  let category = $state('');
  let specific = $state('');
  let severity = $state(0);
  let onsetDate = $state(new Date().toISOString().split('T')[0]);
  let onsetTime: string | null = $state(null);

  // Expanded fields
  let bodyRegion: string | null = $state(null);
  let duration: string | null = $state(null);
  let character: string | null = $state(null);
  let aggravating: string[] = $state([]);
  let relieving: string[] = $state([]);
  let timingPattern: string | null = $state(null);
  let notes: string | null = $state(null);

  let correlations: TemporalCorrelation[] = $state([]);
  let saving = $state(false);
  let error: string | null = $state(null);

  function canSave(): boolean {
    return category !== '' && specific !== '' && severity >= 1 && severity <= 5;
  }

  async function save() {
    if (!canSave()) return;
    saving = true;
    error = null;
    try {
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
        notes,
      };
      const result = await recordSymptom(entry);
      correlations = result.correlations;
      step = 'done';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      saving = false;
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="px-6 py-4" onkeydown={(e) => { if (e.key === 'Escape' && step !== 'done') onCancel(); }}>
  <!-- ACC: Step progress announcement -->
  <div role="status" aria-live="polite" class="sr-only">
    {step === 'category' ? $t('journal.recording_step_category')
      : step === 'severity' ? $t('journal.recording_step_severity')
      : step === 'when' ? $t('journal.recording_step_when')
      : step === 'expanded' ? $t('journal.recording_step_expanded')
      : step === 'notes' ? $t('journal.recording_step_notes')
      : $t('journal.recording_done_heading')}
  </div>

  <!-- Cancel button -->
  {#if step !== 'done'}
    <BackButton onclick={onCancel} label={$t('journal.recording_cancel')} />
  {/if}

  {#if step === 'category'}
    <h2 class="text-xl font-semibold text-stone-800 mb-4">{$t('journal.recording_step_category')}</h2>
    <CategorySelector
      onSelect={(cat, spec) => {
        category = cat;
        specific = spec;
        step = 'severity';
      }}
    />

  {:else if step === 'severity'}
    <h2 class="text-xl font-semibold text-stone-800 mb-4">{$t('journal.recording_step_severity')}</h2>
    <SeverityScale
      value={severity}
      onChange={(v) => { severity = v; }}
      onNext={() => { step = 'when'; }}
    />

  {:else if step === 'when'}
    <h2 class="text-xl font-semibold text-stone-800 mb-4">{$t('journal.recording_step_when')}</h2>
    <DateSelector
      date={onsetDate}
      time={onsetTime}
      onDateChange={(d) => { onsetDate = d; }}
      onTimeChange={(t) => { onsetTime = t; }}
    />
    <div class="flex gap-3 mt-6">
      <Button variant="primary" loading={saving} disabled={!canSave()} onclick={save}>
        {saving ? $t('journal.recording_saving') : $t('journal.recording_save')}
      </Button>
      <Button variant="secondary" onclick={() => { step = 'expanded'; }}>
        {$t('journal.recording_tell_more')}
      </Button>
    </div>

  {:else if step === 'expanded'}
    <h2 class="text-xl font-semibold text-stone-800 mb-4">{$t('journal.recording_step_expanded')}</h2>
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
      onNext={() => { step = 'notes'; }}
    />

  {:else if step === 'notes'}
    <h2 class="text-xl font-semibold text-stone-800 mb-4">{$t('journal.recording_step_notes')}</h2>
    <textarea
      class="w-full h-32 p-4 rounded-xl border border-stone-200 text-stone-700
             resize-none focus:outline-none focus:border-[var(--color-primary)]"
      placeholder={$t('journal.recording_notes_placeholder')}
      maxlength={500}
      value={notes ?? ''}
      oninput={(e) => { notes = e.currentTarget.value || null; }}
    ></textarea>
    <p class="text-xs text-stone-500 mt-1 text-right">{$t('journal.recording_char_count', { values: { count: notes?.length ?? 0 } })}</p>
    <div class="mt-4">
      <Button variant="primary" fullWidth loading={saving} disabled={!canSave()} onclick={save}>
        {saving ? $t('journal.recording_saving') : $t('journal.recording_save')}
      </Button>
    </div>

  {:else if step === 'done'}
    <div class="text-center py-8">
      <div class="w-16 h-16 bg-[var(--color-success-50)] rounded-full flex items-center justify-center mx-auto mb-4">
        <span class="text-[var(--color-success)] text-2xl">&#x2713;</span>
      </div>
      <h2 class="text-xl font-semibold text-stone-800 mb-2">{$t('journal.recording_done_title')}</h2>
      <p class="text-stone-500 text-sm mb-6">{$t('journal.recording_done_message')}</p>

      {#each correlations as correlation}
        <CorrelationCard {correlation} />
      {/each}

      <div class="mt-6">
        <Button variant="secondary" onclick={onComplete}>
          {$t('journal.recording_done_button')}
        </Button>
      </div>
    </div>
  {/if}

  {#if error}
    <p class="text-[var(--color-danger)] text-sm mt-4" role="alert">{error}</p>
  {/if}
</div>
