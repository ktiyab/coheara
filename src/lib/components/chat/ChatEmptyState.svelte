<script lang="ts">
  import type { PromptSuggestion } from '$lib/types/chat';

  interface Props {
    profileName: string;
    suggestions: PromptSuggestion[];
    onSuggestionTap: (suggestion: PromptSuggestion) => void;
  }
  let { profileName, suggestions, onSuggestionTap }: Props = $props();
</script>

<div class="flex flex-col items-center justify-center h-full px-6 text-center max-w-md mx-auto">
  <div class="w-16 h-16 rounded-full bg-teal-600 flex items-center justify-center
              text-white text-2xl font-bold mb-4">
    C
  </div>

  <h2 class="text-xl font-bold text-stone-800 mb-2">
    Hello {profileName}!
  </h2>
  <p class="text-sm text-stone-500 mb-8 leading-relaxed">
    I can help you understand your medical documents. Ask me anything about your prescriptions,
    lab results, or health records.
  </p>

  {#if suggestions.length > 0}
    <div class="w-full">
      <p class="text-xs text-stone-400 uppercase font-medium mb-3">Try asking</p>
      <div class="grid grid-cols-1 gap-2">
        {#each suggestions as suggestion}
          <button
            class="w-full text-left px-4 py-3 rounded-xl bg-white border border-stone-200
                   text-sm text-stone-700 hover:border-teal-600
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
