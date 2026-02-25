<!-- V8-B5 + AUDIT_01 §8: Compact progress banner — inline status dots. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { OnboardingProgress } from '$lib/types/home';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { CheckIcon } from '$lib/components/icons/md';
  import Button from '$lib/components/ui/Button.svelte';

  interface Props {
    progress: OnboardingProgress;
  }
  let { progress }: Props = $props();

  type StepState = 'completed' | 'active' | 'locked';

  interface Step {
    key: keyof OnboardingProgress;
    labelKey: string;
    action: string;
  }

  const steps: Step[] = [
    { key: 'first_document_loaded', labelKey: 'home.progress_step1', action: 'import' },
    { key: 'first_document_reviewed', labelKey: 'home.progress_step2', action: 'documents' },
    { key: 'first_question_asked', labelKey: 'home.progress_step3', action: 'chat' },
  ];

  let completedCount = $derived(steps.filter(s => progress[s.key]).length);
  let allComplete = $derived(completedCount === steps.length);

  function stepState(index: number): StepState {
    if (progress[steps[index].key]) return 'completed';
    if (index === 0) return 'active';
    if (progress[steps[index - 1].key]) return 'active';
    return 'locked';
  }
</script>

{#if allComplete}
  <!-- Collapsed completion message -->
  <div class="mx-[var(--spacing-page-x)] mt-4 px-4 py-3 bg-[var(--color-success-50)] border border-[var(--color-success-200)] rounded-[var(--radius-card)] text-center">
    <p class="text-sm text-[var(--color-success)] font-medium">
      {$t('home.progress_complete')}
    </p>
  </div>
{:else}
  <!-- Compact banner — inline step indicators (AUDIT_01 §8) -->
  <div class="mx-[var(--spacing-page-x)] mt-4 bg-white dark:bg-gray-900 border border-stone-200 dark:border-gray-700 rounded-[var(--radius-card)] shadow-sm px-5 py-4">
    <!-- Header: title + count -->
    <div class="flex items-center justify-between mb-3">
      <h2 class="text-sm font-medium text-stone-700 dark:text-gray-200">
        {$t('home.progress_heading')}
      </h2>
      <span class="text-xs font-medium text-stone-400 dark:text-gray-500">
        {completedCount} / {steps.length}
      </span>
    </div>

    <!-- Inline step indicators -->
    <div class="flex flex-wrap items-center gap-x-6 gap-y-2">
      {#each steps as step, i}
        {@const state = stepState(i)}
        <div class="inline-flex items-center gap-2">
          <!-- Step dot/check -->
          {#if state === 'completed'}
            <span class="w-5 h-5 rounded-full bg-[var(--color-success)] flex items-center justify-center flex-shrink-0">
              <CheckIcon class="w-3 h-3 text-white" />
            </span>
          {:else if state === 'active'}
            <span class="w-2.5 h-2.5 rounded-full bg-[var(--color-interactive)] flex-shrink-0"></span>
          {:else}
            <span class="w-2.5 h-2.5 rounded-full bg-stone-200 dark:bg-gray-700 flex-shrink-0"></span>
          {/if}

          <!-- Step label -->
          <span class="text-sm {state === 'completed'
            ? 'text-stone-400 dark:text-gray-500 line-through'
            : state === 'active'
              ? 'text-stone-800 dark:text-gray-100 font-medium'
              : 'text-stone-400 dark:text-gray-500'}">
            {$t(step.labelKey)}
          </span>

          <!-- Active step CTA -->
          {#if state === 'active'}
            <Button variant="primary" size="sm" onclick={() => navigation.navigate(step.action)}>
              {$t('home.progress_add_document')}
            </Button>
          {/if}
        </div>
      {/each}
    </div>
  </div>
{/if}
