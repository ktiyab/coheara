<!-- V8-B5: Sequential progress block — replaces OnboardingMilestones + EmptyState -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { OnboardingProgress } from '$lib/types/home';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { CheckOutline } from 'flowbite-svelte-icons';
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
    lockedAfter: number;
  }

  const steps: Step[] = [
    { key: 'first_document_loaded', labelKey: 'home.progress_step1', action: 'import', lockedAfter: 0 },
    { key: 'first_document_reviewed', labelKey: 'home.progress_step2', action: 'documents', lockedAfter: 1 },
    { key: 'first_question_asked', labelKey: 'home.progress_step3', action: 'chat', lockedAfter: 2 },
  ];

  let completedCount = $derived(steps.filter(s => progress[s.key]).length);
  let allComplete = $derived(completedCount === steps.length);

  function stepState(index: number): StepState {
    if (progress[steps[index].key]) return 'completed';
    // Active = first incomplete step whose prerequisite is met
    if (index === 0) return 'active';
    if (progress[steps[index - 1].key]) return 'active';
    return 'locked';
  }
</script>

{#if allComplete}
  <!-- Collapsed completion message -->
  <div class="mx-6 mt-4 px-4 py-3 bg-[var(--color-success-50)] dark:bg-green-900/20 border border-[var(--color-success-200)] dark:border-green-800 rounded-xl text-center">
    <p class="text-sm text-[var(--color-success-700)] dark:text-green-300 font-medium">
      {$t('home.progress_complete')}
    </p>
  </div>
{:else}
  <div class="mx-6 mt-4 bg-white dark:bg-gray-900 border border-stone-200 dark:border-gray-700 rounded-xl shadow-sm p-5">
    <!-- Header: title + progress bar -->
    <div class="flex items-center justify-between mb-4">
      <h2 class="text-xs font-semibold uppercase tracking-wider text-stone-500 dark:text-gray-400">
        {$t('home.progress_heading')}
      </h2>
      <span class="text-xs font-medium text-stone-400 dark:text-gray-500">
        {completedCount} / {steps.length}
      </span>
    </div>

    <!-- Progress bar -->
    <div class="w-full h-1.5 bg-stone-100 dark:bg-gray-800 rounded-full mb-5">
      <div
        class="h-full bg-[var(--color-interactive)] rounded-full transition-all duration-500"
        style="width: {(completedCount / steps.length) * 100}%"
      ></div>
    </div>

    <!-- Steps -->
    <ol class="flex flex-col gap-3" aria-label={$t('home.progress_heading')}>
      {#each steps as step, i}
        {@const state = stepState(i)}
        <li class="flex items-center gap-3 min-h-[44px]">
          <!-- Step indicator -->
          {#if state === 'completed'}
            <span class="w-7 h-7 rounded-full bg-[var(--color-success)] flex items-center justify-center flex-shrink-0">
              <CheckOutline class="w-4 h-4 text-white" />
            </span>
          {:else if state === 'active'}
            <span class="w-7 h-7 rounded-full bg-[var(--color-interactive)] flex items-center justify-center flex-shrink-0 text-white text-xs font-bold">
              {i + 1}
            </span>
          {:else}
            <span class="w-7 h-7 rounded-full bg-stone-100 dark:bg-gray-800 flex items-center justify-center flex-shrink-0 text-stone-400 dark:text-gray-500 text-xs font-medium">
              {i + 1}
            </span>
          {/if}

          <!-- Step content -->
          <div class="flex-1 flex items-center justify-between gap-2">
            <span class="text-sm {state === 'completed'
              ? 'text-stone-400 dark:text-gray-500 line-through'
              : state === 'active'
                ? 'text-stone-800 dark:text-gray-100 font-medium'
                : 'text-stone-400 dark:text-gray-500'}">
              {$t(step.labelKey)}
            </span>

            {#if state === 'active'}
              <Button variant="primary" size="sm" onclick={() => navigation.navigate(step.action)}>
                {$t('home.progress_add_document')}
              </Button>
            {:else if state === 'locked'}
              <span class="text-xs text-stone-400 dark:text-gray-600">
                {$t('home.progress_unlocks_after', { values: { step: step.lockedAfter } })}
              </span>
            {/if}
          </div>
        </li>
      {/each}
    </ol>
  </div>
{/if}
