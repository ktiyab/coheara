<!-- Spec 49 [FE-02]: Quick symptom log — 2-tap entry (category + severity). -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { recordSymptom } from '$lib/api/journal';

  interface Props {
    onLogged: () => void;
    onDetailedEntry: () => void;
  }

  let { onLogged, onDetailedEntry }: Props = $props();

  const QUICK_SYMPTOMS = [
    { category: 'head', specific: 'headache', icon: '\u{1F915}', labelKey: 'journal.quick_headache' },
    { category: 'respiratory', specific: 'cold', icon: '\u{1F927}', labelKey: 'journal.quick_cold' },
    { category: 'general', specific: 'fatigue', icon: '\u{1F634}', labelKey: 'journal.quick_fatigue' },
    { category: 'general', specific: 'fever', icon: '\u{1F912}', labelKey: 'journal.quick_fever' },
    { category: 'digestive', specific: 'nausea', icon: '\u{1F922}', labelKey: 'journal.quick_nausea' },
    { category: 'pain', specific: 'pain', icon: '\u{1FA79}', labelKey: 'journal.quick_pain' },
  ] as const;

  let selected: (typeof QUICK_SYMPTOMS)[number] | null = $state(null);
  let severity: number = $state(0);
  let note: string = $state('');
  let saving = $state(false);
  let saved = $state(false);

  async function logSymptom() {
    if (!selected || severity === 0) return;
    saving = true;
    try {
      const now = new Date();
      await recordSymptom({
        category: selected.category,
        specific: selected.specific,
        severity,
        onset_date: now.toISOString().slice(0, 10),
        onset_time: now.toTimeString().slice(0, 5),
        body_region: null,
        duration: null,
        character: null,
        aggravating: [],
        relieving: [],
        timing_pattern: null,
        notes: note.trim() || null,
      });
      saved = true;
      setTimeout(() => onLogged(), 800);
    } catch {
      // Silently fail — user can retry
      saving = false;
    }
  }

  function reset() {
    selected = null;
    severity = 0;
    note = '';
    saved = false;
  }
</script>

<div class="flex flex-col gap-4">
  <h2 class="text-sm font-semibold text-[var(--color-text-secondary)]">
    {$t('journal.quick_heading')}
  </h2>

  {#if saved}
    <div class="flex items-center justify-center py-6 text-center">
      <p class="text-sm text-[var(--color-success)]">{$t('journal.quick_saved')}</p>
    </div>
  {:else}
    <!-- Step 1: Symptom selection -->
    <div class="grid grid-cols-3 gap-2" role="radiogroup" aria-label={$t('journal.quick_heading')}>
      {#each QUICK_SYMPTOMS as symptom}
        <button
          class="flex flex-col items-center gap-1 p-3 rounded-xl border transition-colors min-h-[64px]
                 {selected === symptom
                   ? 'border-[var(--color-primary)] bg-[var(--color-primary-50)] text-[var(--color-primary)]'
                   : 'border-[var(--color-border)] bg-white text-[var(--color-text-primary)] hover:bg-[var(--color-surface-hover)]'}"
          role="radio"
          aria-checked={selected === symptom}
          onclick={() => { selected = symptom; }}
        >
          <span class="text-xl" aria-hidden="true">{symptom.icon}</span>
          <span class="text-xs font-medium">{$t(symptom.labelKey)}</span>
        </button>
      {/each}
    </div>

    <!-- "Other" link to detailed entry -->
    <button
      class="text-xs text-[var(--color-text-muted)] hover:text-[var(--color-primary)] transition-colors"
      onclick={onDetailedEntry}
    >
      {$t('journal.quick_other')}
    </button>

    {#if selected}
      <!-- Step 2: Severity -->
      <div>
        <p class="text-xs text-[var(--color-text-secondary)] mb-2">{$t('journal.quick_severity')}</p>
        <div class="flex gap-2" role="radiogroup" aria-label={$t('journal.quick_severity')}>
          {#each [1, 2, 3, 4, 5] as level}
            <button
              class="flex-1 py-2.5 rounded-lg text-sm font-medium transition-colors min-h-[44px]
                     {severity === level
                       ? 'bg-[var(--color-primary)] text-white'
                       : 'bg-[var(--color-surface)] border border-[var(--color-border)] text-[var(--color-text-primary)] hover:bg-[var(--color-surface-hover)]'}"
              role="radio"
              aria-checked={severity === level}
              aria-label={String(level)}
              onclick={() => { severity = level; }}
            >
              {level}
            </button>
          {/each}
        </div>
        <div class="flex justify-between text-[10px] text-[var(--color-text-muted)] mt-1 px-1">
          <span>{$t('journal.quick_mild')}</span>
          <span>{$t('journal.quick_severe')}</span>
        </div>
      </div>

      <!-- Step 3: Optional note -->
      <div>
        <textarea
          class="w-full px-3 py-2 rounded-lg border border-[var(--color-border)]
                 bg-[var(--color-surface)] text-sm text-[var(--color-text-primary)]
                 placeholder:text-[var(--color-text-muted)] resize-none
                 focus:outline-none focus:ring-2 focus:ring-[var(--color-primary)]"
          rows="2"
          maxlength="500"
          placeholder={$t('journal.quick_note_placeholder')}
          bind:value={note}
        ></textarea>
      </div>

      <!-- Log button -->
      <button
        class="w-full py-3 rounded-lg bg-[var(--color-primary)] text-white text-sm font-medium
               disabled:opacity-50 disabled:cursor-not-allowed min-h-[48px]
               hover:bg-[var(--color-primary-hover)] active:bg-[var(--color-primary-active)] transition-colors"
        disabled={severity === 0 || saving}
        onclick={logSymptom}
      >
        {saving ? $t('journal.quick_saving') : $t('journal.quick_log_it')}
      </button>
    {/if}
  {/if}
</div>
