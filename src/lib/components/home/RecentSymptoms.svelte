<!-- LP-07: Recent symptoms widget for Home dashboard Zone D. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { navigation } from '$lib/stores/navigation.svelte';
  import type { RecentSymptomCard } from '$lib/types/home';
  import { PlusIcon } from '$lib/components/icons/md';

  interface Props {
    symptoms: RecentSymptomCard[];
  }
  let { symptoms }: Props = $props();

  const SEVERITY_COLORS: Record<number, string> = {
    1: '#4ade80',
    2: '#a3e635',
    3: '#facc15',
    4: '#fb923c',
    5: '#f87171',
  };

  const SEVERITY_LABELS: Record<number, string> = {
    1: 'Mild',
    2: 'Mild',
    3: 'Moderate',
    4: 'Severe',
    5: 'Very severe',
  };

  function handleTap(symptom: RecentSymptomCard) {
    const prefill = $t('home.symptoms_prefill', { values: { specific: symptom.specific } })
      || `I want to update my ${symptom.specific} symptom.`;
    navigation.navigate('chat', { prefill });
  }

  function formatDate(dateStr: string): string {
    const date = new Date(dateStr);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffDays = Math.floor(diffMs / 86400000);
    if (diffDays === 0) return $t('home.upcoming_today') ?? 'Today';
    if (diffDays === 1) return $t('home.upcoming_tomorrow') !== 'Tomorrow' ? $t('home.time_days_ago', { values: { count: 1 } }) : 'Yesterday';
    if (diffDays < 7) return $t('home.time_days_ago', { values: { count: diffDays } });
    return date.toLocaleDateString();
  }
</script>

{#if symptoms.length > 0}
  <section class="px-6 py-3" aria-label={$t('home.symptoms_heading') ?? 'Recent Symptoms'}>
    <h2 class="text-sm font-semibold text-[var(--color-text-secondary)] mb-2">
      {$t('home.symptoms_heading') ?? 'Recent Symptoms'}
    </h2>
    <div class="bg-white dark:bg-gray-900 border border-stone-200 dark:border-gray-700 rounded-xl overflow-hidden">
      {#each symptoms as symptom, i (symptom.id)}
        <button
          class="w-full flex items-center gap-3 px-4 py-3 text-left hover:bg-stone-50 dark:hover:bg-gray-800 transition-colors
                 {i > 0 ? 'border-t border-stone-100 dark:border-gray-800' : ''}"
          onclick={() => handleTap(symptom)}
        >
          <!-- Severity dot -->
          <span
            class="shrink-0 w-3 h-3 rounded-full"
            style="background-color: {SEVERITY_COLORS[symptom.severity] ?? '#9ca3af'}"
            title={SEVERITY_LABELS[symptom.severity] ?? ''}
          ></span>

          <!-- Name + category -->
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-[var(--color-text-primary)] truncate">
              {symptom.specific}
            </p>
            <p class="text-xs text-[var(--color-text-muted)]">
              {symptom.category}
              {#if symptom.related_medication_name}
                · {$t('home.symptoms_related_to', { values: { medication: symptom.related_medication_name } }) ?? `Related to ${symptom.related_medication_name}`}
              {/if}
            </p>
          </div>

          <!-- Date + status -->
          <div class="shrink-0 text-right">
            <p class="text-xs text-[var(--color-text-muted)]">{formatDate(symptom.onset_date)}</p>
            <span class="text-xs px-1.5 py-0.5 rounded-full
              {symptom.still_active
                ? 'bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-400'
                : 'bg-stone-100 dark:bg-gray-800 text-stone-500 dark:text-gray-400'}">
              {symptom.still_active
                ? ($t('home.symptoms_still_active') ?? 'Active')
                : ($t('home.symptoms_resolved') ?? 'Resolved')}
            </span>
          </div>
        </button>
      {/each}
    </div>

    <!-- Log new -->
    <button
      class="mt-2 flex items-center gap-1.5 text-sm text-[var(--color-primary)] hover:underline"
      onclick={() => navigation.navigate('chat', { prefill: $t('chat.prefill_symptom') ?? 'I want to log a symptom.' })}
    >
      <PlusIcon class="w-3.5 h-3.5" />
      {$t('home.symptoms_log_new') ?? 'Log new symptom'}
    </button>
  </section>
{/if}
