<!-- L4-01: Multi-step symptom recording flow. -->
<script lang="ts">
  import { recordSymptom } from '$lib/api/journal';
  import type { SymptomEntry, TemporalCorrelation } from '$lib/types/journal';
  import CategorySelector from './CategorySelector.svelte';
  import SeverityScale from './SeverityScale.svelte';
  import DateSelector from './DateSelector.svelte';
  import ExpandedDetails from './ExpandedDetails.svelte';
  import CorrelationCard from './CorrelationCard.svelte';

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

<div class="px-6 py-4">
  <!-- Cancel button -->
  {#if step !== 'done'}
    <button
      class="text-stone-500 text-sm mb-4 min-h-[44px]"
      onclick={onCancel}
    >
      &larr; Cancel
    </button>
  {/if}

  {#if step === 'category'}
    <h2 class="text-xl font-semibold text-stone-800 mb-4">What's bothering you?</h2>
    <CategorySelector
      onSelect={(cat, spec) => {
        category = cat;
        specific = spec;
        step = 'severity';
      }}
    />

  {:else if step === 'severity'}
    <h2 class="text-xl font-semibold text-stone-800 mb-4">How bad is it?</h2>
    <SeverityScale
      value={severity}
      onChange={(v) => { severity = v; }}
      onNext={() => { step = 'when'; }}
    />

  {:else if step === 'when'}
    <h2 class="text-xl font-semibold text-stone-800 mb-4">When did this start?</h2>
    <DateSelector
      date={onsetDate}
      time={onsetTime}
      onDateChange={(d) => { onsetDate = d; }}
      onTimeChange={(t) => { onsetTime = t; }}
    />
    <div class="flex gap-3 mt-6">
      <button
        class="flex-1 px-4 py-3 bg-[var(--color-primary)] text-white rounded-xl
               font-medium min-h-[44px] disabled:opacity-50"
        disabled={!canSave() || saving}
        onclick={save}
      >
        {saving ? 'Saving...' : 'Save'}
      </button>
      <button
        class="px-4 py-3 bg-stone-100 text-stone-700 rounded-xl
               font-medium min-h-[44px]"
        onclick={() => { step = 'expanded'; }}
      >
        Tell me more
      </button>
    </div>

  {:else if step === 'expanded'}
    <h2 class="text-xl font-semibold text-stone-800 mb-4">Tell me more</h2>
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
    <h2 class="text-xl font-semibold text-stone-800 mb-4">Anything else?</h2>
    <textarea
      class="w-full h-32 p-4 rounded-xl border border-stone-200 text-stone-700
             resize-none focus:outline-none focus:border-[var(--color-primary)]"
      placeholder="Optional notes..."
      maxlength={500}
      value={notes ?? ''}
      oninput={(e) => { notes = e.currentTarget.value || null; }}
    ></textarea>
    <p class="text-xs text-stone-400 mt-1 text-right">{(notes?.length ?? 0)}/500</p>
    <button
      class="w-full mt-4 px-4 py-3 bg-[var(--color-primary)] text-white rounded-xl
             font-medium min-h-[44px] disabled:opacity-50"
      disabled={!canSave() || saving}
      onclick={save}
    >
      {saving ? 'Saving...' : 'Save'}
    </button>

  {:else if step === 'done'}
    <div class="text-center py-8">
      <div class="w-16 h-16 bg-green-100 rounded-full flex items-center justify-center mx-auto mb-4">
        <span class="text-green-600 text-2xl">&#x2713;</span>
      </div>
      <h2 class="text-xl font-semibold text-stone-800 mb-2">Recorded</h2>
      <p class="text-stone-500 text-sm mb-6">Your symptom has been saved.</p>

      {#each correlations as correlation}
        <CorrelationCard {correlation} />
      {/each}

      <button
        class="mt-6 px-6 py-3 bg-stone-100 text-stone-700 rounded-xl
               font-medium min-h-[44px]"
        onclick={onComplete}
      >
        Done
      </button>
    </div>
  {/if}

  {#if error}
    <p class="text-red-600 text-sm mt-4" role="alert">{error}</p>
  {/if}
</div>
