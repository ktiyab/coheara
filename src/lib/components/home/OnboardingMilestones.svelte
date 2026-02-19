<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { OnboardingProgress } from '$lib/types/home';

  import { navigation } from '$lib/stores/navigation.svelte';

  interface Props {
    progress: OnboardingProgress;
  }
  let { progress }: Props = $props();

  // Spec 47 [OB-04]: Reduced from 5 to 3 core milestones
  const milestones = [
    { key: 'first_document_loaded' as const, labelKey: 'home.onboarding_first_document', action: 'import' },
    { key: 'first_document_reviewed' as const, labelKey: 'home.onboarding_first_review', action: 'documents' },
    { key: 'first_question_asked' as const, labelKey: 'home.onboarding_first_question', action: 'chat' },
  ];

  const allComplete = $derived(milestones.every(m => progress[m.key]));
</script>

<div class="px-6 py-4">
  <h2 class="text-sm font-medium text-stone-500 mb-3">{$t('home.onboarding_heading')}</h2>
  <div class="flex flex-col gap-2">
    {#each milestones as milestone}
      {@const completed = progress[milestone.key]}
      <button
        class="flex items-center gap-3 text-left w-full py-2 min-h-[44px]"
        onclick={() => { if (!completed) navigation.navigate(milestone.action); }}
        disabled={completed}
      >
        <span class="w-5 h-5 rounded-full flex items-center justify-center flex-shrink-0
                     {completed ? 'bg-[var(--color-success)] text-white' : 'border-2 border-stone-300'}">
          {#if completed}
            <span class="text-xs">&#x2713;</span>
          {/if}
        </span>
        <span class="text-sm {completed ? 'text-stone-500 line-through' : 'text-stone-700'}">
          {$t(milestone.labelKey)}
        </span>
      </button>
    {/each}
  </div>

  <!-- Spec 47 [OB-04]: Post-milestone explore prompt -->
  {#if allComplete}
    <p class="text-sm text-stone-400 mt-3 text-center">
      {$t('home.onboarding_explore_more')}
    </p>
  {/if}
</div>
