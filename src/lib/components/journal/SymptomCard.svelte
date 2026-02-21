<!-- V14: Enriched expandable symptom card replacing inline HTML in SymptomHistory. -->
<script lang="ts">
  import { t, locale } from 'svelte-i18n';
  import type { StoredSymptom } from '$lib/types/journal';
  import { SEVERITY_COLORS } from '$lib/types/journal';
  import Badge from '$lib/components/ui/Badge.svelte';

  interface Props {
    symptom: StoredSymptom;
    onResolve: (id: string) => void;
    onDelete: (id: string) => void;
  }
  let { symptom, onResolve, onDelete }: Props = $props();

  let expanded = $state(false);

  let severityLabel = $derived(
    symptom.severity > 0 ? $t(`journal.severity_${symptom.severity - 1}`) : ''
  );

  let onsetDisplay = $derived.by(() => {
    const today = new Date().toISOString().slice(0, 10);
    const yesterday = new Date(Date.now() - 86400000).toISOString().slice(0, 10);
    const time = symptom.onset_time ?? '';

    if (symptom.onset_date === today) {
      return $t('journal.card_since_today', { values: { time } });
    } else if (symptom.onset_date === yesterday) {
      return $t('journal.card_since_yesterday', { values: { time } });
    } else {
      const dateStr = new Date(symptom.onset_date).toLocaleDateString($locale ?? 'en', {
        month: 'short', day: 'numeric',
      });
      return $t('journal.card_since_date', { values: { date: dateStr, time } });
    }
  });

  let categoryLabel = $derived($t(`journal.category_${symptom.category.toLowerCase()}`));
</script>

<div class="bg-white dark:bg-gray-900 rounded-xl p-4 border border-stone-100 dark:border-gray-800 shadow-sm">
  <!-- Header row -->
  <div class="flex items-start justify-between">
    <div class="flex items-center gap-2 min-w-0">
      <span
        class="w-2.5 h-2.5 rounded-full flex-shrink-0"
        style="background-color: {SEVERITY_COLORS[symptom.severity] || '#d6d3d1'}"
        aria-hidden="true"
      ></span>
      <span class="font-medium text-stone-800 dark:text-gray-100 truncate">{symptom.specific}</span>
      <span class="text-sm text-stone-500 dark:text-gray-400 flex-shrink-0">{severityLabel}</span>
    </div>
    <Badge variant={symptom.still_active ? 'success' : 'neutral'} size="sm">
      {symptom.still_active ? $t('journal.history_status_active') : $t('journal.history_status_resolved')}
    </Badge>
  </div>

  <!-- Subtitle: category + onset -->
  <p class="text-xs text-stone-500 dark:text-gray-400 mt-1 ml-[18px]">
    {categoryLabel} &middot; {onsetDisplay}
  </p>

  <!-- Medication link banner -->
  {#if symptom.related_medication_name}
    <div class="mt-2 ml-[18px] px-3 py-1.5 rounded-lg bg-[var(--color-info-50)] text-xs text-[var(--color-info)]">
      {$t('journal.card_linked_med', { values: { name: symptom.related_medication_name } })}
    </div>
  {/if}

  <!-- Actions row -->
  <div class="flex items-center justify-between mt-3 ml-[18px]">
    <div>
      {#if symptom.still_active}
        <button
          class="text-xs text-stone-600 dark:text-gray-400 underline min-h-[44px] px-1"
          onclick={() => onResolve(symptom.id)}
        >
          {$t('journal.history_mark_resolved')}
        </button>
      {/if}
    </div>
    <button
      class="text-xs text-stone-500 dark:text-gray-400 underline min-h-[44px] px-1 flex items-center gap-1"
      onclick={() => { expanded = !expanded; }}
    >
      {$t('journal.card_details')}
      <svg class="w-3 h-3 transition-transform {expanded ? 'rotate-180' : ''}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
      </svg>
    </button>
  </div>

  <!-- Expanded details -->
  {#if expanded}
    <div class="mt-3 ml-[18px] space-y-1 text-xs text-stone-500 dark:text-gray-400 border-t border-stone-100 dark:border-gray-800 pt-3">
      {#if symptom.body_region}
        <p>{$t('journal.history_region_label')} {symptom.body_region}</p>
      {/if}
      {#if symptom.duration}
        <p>{$t('journal.history_duration_label')} {symptom.duration}</p>
      {/if}
      {#if symptom.character}
        <p>{$t('journal.history_character_label')} {symptom.character}</p>
      {/if}
      {#if symptom.aggravating}
        <p>{$t('journal.expanded_aggravating_title')} {symptom.aggravating}</p>
      {/if}
      {#if symptom.relieving}
        <p>{$t('journal.expanded_relieving_title')} {symptom.relieving}</p>
      {/if}
      {#if symptom.notes}
        <p class="italic">{symptom.notes}</p>
      {/if}

      <!-- Remove (muted, in expanded only) -->
      <button
        class="text-xs text-stone-500 dark:text-gray-400 underline min-h-[44px] px-1 mt-2"
        onclick={() => onDelete(symptom.id)}
      >
        {$t('journal.history_remove')}
      </button>
    </div>
  {/if}
</div>
