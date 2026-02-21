<!-- LP-07: Proactive extraction suggestions — rule-based prompts to track health data. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { navigation } from '$lib/stores/navigation.svelte';
  import type { ExtractionSuggestion } from '$lib/types/home';
  import {
    CalendarMonthOutline,
    ExclamationCircleOutline,
    InfoCircleOutline,
    HeartOutline,
    CloseOutline,
  } from 'flowbite-svelte-icons';

  interface Props {
    suggestions: ExtractionSuggestion[];
    onDismiss: (suggestionType: string, entityId: string) => void;
  }
  let { suggestions, onDismiss }: Props = $props();

  const MAX_VISIBLE = 3;
  let visible = $derived(suggestions.slice(0, MAX_VISIBLE));

  const typeIcon: Record<string, typeof HeartOutline> = {
    appointment_prep: CalendarMonthOutline,
    medication_update: ExclamationCircleOutline,
    lab_follow_up: InfoCircleOutline,
    symptom_tracking: HeartOutline,
  };

  const typeColor: Record<string, string> = {
    appointment_prep: 'text-[var(--color-primary)] bg-[var(--color-primary-50)] dark:bg-blue-900/20',
    medication_update: 'text-amber-600 dark:text-amber-400 bg-amber-50 dark:bg-amber-900/20',
    lab_follow_up: 'text-purple-600 dark:text-purple-400 bg-purple-50 dark:bg-purple-900/20',
    symptom_tracking: 'text-green-600 dark:text-green-400 bg-green-50 dark:bg-green-900/20',
  };

  function handleAction(suggestion: ExtractionSuggestion) {
    navigation.navigate('chat', { prefill: suggestion.chat_prefill });
  }

  function extractEntityId(suggestion: ExtractionSuggestion): string {
    // ID format: "suggestion-{type}-{entityId}"
    const parts = suggestion.id.split('-');
    return parts.slice(2).join('-');
  }
</script>

{#if visible.length > 0}
  <section class="px-6 py-2" aria-label={$t('home.suggestions_heading') ?? 'Suggestions'}>
    <h2 class="text-sm font-semibold text-stone-500 dark:text-gray-400 uppercase tracking-wider mb-2">
      {$t('home.suggestions_heading') ?? 'Suggestions'}
    </h2>
    <div class="flex flex-col gap-2">
      {#each visible as suggestion (suggestion.id)}
        {@const Icon = typeIcon[suggestion.suggestion_type] ?? InfoCircleOutline}
        {@const colors = typeColor[suggestion.suggestion_type] ?? typeColor.symptom_tracking}
        <div class="bg-white dark:bg-gray-900 border border-stone-200 dark:border-gray-700 rounded-xl px-4 py-3 flex items-start gap-3">
          <span class="shrink-0 w-8 h-8 rounded-lg flex items-center justify-center {colors}">
            <Icon class="w-4 h-4" />
          </span>
          <div class="flex-1 min-w-0">
            <p class="text-sm text-stone-800 dark:text-gray-100">{suggestion.message}</p>
            <button
              class="mt-1.5 text-xs font-medium text-[var(--color-primary)] hover:underline"
              onclick={() => handleAction(suggestion)}
            >
              {suggestion.action_label}
            </button>
          </div>
          <button
            class="shrink-0 text-stone-400 dark:text-gray-500 hover:text-stone-600 dark:hover:text-gray-300
                   min-h-[44px] min-w-[44px] flex items-center justify-center"
            onclick={() => onDismiss(suggestion.suggestion_type, extractEntityId(suggestion))}
            aria-label={$t('common.dismiss')}
          >
            <CloseOutline class="w-4 h-4" />
          </button>
        </div>
      {/each}
    </div>
  </section>
{/if}
