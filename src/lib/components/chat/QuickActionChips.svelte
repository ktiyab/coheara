<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { PromptSuggestion } from '$lib/types/chat';

  interface Props {
    suggestions: PromptSuggestion[];
    isStreaming: boolean;
    onChipTap: (suggestion: PromptSuggestion) => void;
  }
  let { suggestions, isStreaming, onChipTap }: Props = $props();

  let chips = $derived(
    suggestions.filter(s => s.intent === 'expression').slice(0, 3)
  );

  function chipLabel(s: PromptSuggestion): string {
    const text = $t(s.template_key, { values: s.params });
    return text.length > 30 ? text.slice(0, 27) + '...' : text;
  }
</script>

{#if chips.length > 0 && !isStreaming}
  <div
    class="flex gap-2 px-4 py-2 overflow-x-auto min-h-[32px]"
    role="toolbar"
    aria-label={$t('chat.quick_actions_aria')}
  >
    {#each chips as chip}
      <button
        class="whitespace-nowrap px-3 py-1.5 rounded-full text-xs font-medium
               bg-[var(--color-interactive-subtle)] text-[var(--color-interactive)]
               border border-[var(--color-interactive-border)]
               hover:bg-[var(--color-interactive)] hover:text-white
               transition-colors min-h-[32px]"
        onclick={() => onChipTap(chip)}
      >
        {chipLabel(chip)}
      </button>
    {/each}
  </div>
{/if}
