<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { PromptSuggestion } from '$lib/types/chat';
  import { profile } from '$lib/stores/profile.svelte';
  import { ai } from '$lib/stores/ai.svelte';
  import Avatar from '$lib/components/ui/Avatar.svelte';
  import Button from '$lib/components/ui/Button.svelte';

  interface Props {
    suggestions: PromptSuggestion[];
    onSuggestionTap: (suggestion: PromptSuggestion) => void;
    onNavigate?: (screen: string) => void;
  }
  let { suggestions, onSuggestionTap, onNavigate }: Props = $props();
</script>

<div class="flex flex-col items-center justify-center h-full px-6 text-center max-w-md mx-auto">
  <div class="mb-4">
    <Avatar name={$t('chat.avatar_initial')} variant="ai" size="lg" />
  </div>

  <h2 class="text-xl font-bold text-stone-800 mb-2">
    {$t('chat.greeting', { values: { name: profile.name } })}
  </h2>
  <p class="text-sm text-stone-500 mb-2 leading-relaxed">
    {$t('chat.description_1')}
  </p>
  <p class="text-sm text-stone-500 mb-8 leading-relaxed">
    {$t('chat.description_2')}
  </p>

  <!-- Spec 47 [OB-06]: AI setup offer when AI not configured -->
  {#if !ai.isAiAvailable && onNavigate}
    <div class="w-full p-4 bg-[var(--color-primary-50)] border border-[var(--color-primary-200)] rounded-xl mb-4">
      <p class="text-sm font-medium text-[var(--color-text-primary)] mb-2">
        {$t('chat.ai_setup_prompt')}
      </p>
      <Button variant="primary" size="sm" onclick={() => onNavigate('ai-setup')}>
        {$t('settings.ai_setup')}
      </Button>
    </div>
  {/if}

  {#if suggestions.length > 0}
    <div class="w-full">
      <p class="text-xs text-stone-500 uppercase font-medium mb-3">{$t('chat.suggestions_header')}</p>
      <div class="grid grid-cols-1 gap-2">
        {#each suggestions as suggestion}
          <button
            class="w-full text-left px-4 py-3 rounded-xl bg-white border border-stone-200
                   text-sm text-stone-700 hover:border-[var(--color-interactive)]
                   hover:shadow-sm transition-all min-h-[44px]"
            onclick={() => onSuggestionTap(suggestion)}
          >
            {suggestion.text}
          </button>
        {/each}
      </div>
    </div>
  {/if}
</div>
