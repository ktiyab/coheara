<script lang="ts">
  import type { OnboardingProgress } from '$lib/types/home';

  import { navigation } from '$lib/stores/navigation.svelte';

  interface Props {
    progress: OnboardingProgress;
  }
  let { progress }: Props = $props();

  const milestones = [
    { key: 'first_document_loaded' as const, label: 'Load your first document', action: 'import' },
    { key: 'first_document_reviewed' as const, label: 'Review your first extraction', action: 'documents' },
    { key: 'first_question_asked' as const, label: 'Ask your first question', action: 'chat' },
    { key: 'three_documents_loaded' as const, label: 'Load 3 documents', action: 'import' },
    { key: 'first_symptom_recorded' as const, label: 'Record your first symptom', action: 'journal' },
  ];
</script>

<div class="px-6 py-4">
  <h3 class="text-sm font-medium text-stone-500 mb-3">Getting started</h3>
  <div class="flex flex-col gap-2">
    {#each milestones as milestone}
      {@const completed = progress[milestone.key]}
      <button
        class="flex items-center gap-3 text-left w-full py-2 min-h-[44px]"
        onclick={() => { if (!completed) navigation.navigate(milestone.action); }}
        disabled={completed}
      >
        <span class="w-5 h-5 rounded-full flex items-center justify-center flex-shrink-0
                     {completed ? 'bg-green-500 text-white' : 'border-2 border-stone-300'}">
          {#if completed}
            <span class="text-xs">&#x2713;</span>
          {/if}
        </span>
        <span class="text-sm {completed ? 'text-stone-400 line-through' : 'text-stone-700'}">
          {milestone.label}
        </span>
      </button>
    {/each}
  </div>
</div>
