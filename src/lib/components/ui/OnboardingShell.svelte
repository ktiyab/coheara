<!-- Onboarding shell — persistent top bar with stepper + back nav, wraps setup screens -->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import { t } from 'svelte-i18n';
  import { ChevronLeftIcon, CheckIcon } from '$lib/components/icons/md';

  interface Props {
    currentStep: number;
    totalSteps: number;
    onBack?: () => void;
    children: Snippet;
  }
  let { currentStep, totalSteps, onBack, children }: Props = $props();
</script>

<div class="flex flex-col min-h-screen">
  <!-- Top bar: back button + stepper -->
  <div class="flex items-center px-6 py-4 border-b border-stone-100 dark:border-gray-800">
    <!-- Back button -->
    <div class="w-20">
      {#if onBack}
        <button
          class="min-h-[44px] min-w-[44px] flex items-center gap-1
                 text-[var(--color-success)] hover:text-[var(--color-success-800)]
                 transition-colors cursor-pointer
                 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--color-interactive)]"
          onclick={onBack}
          aria-label={$t('common.go_back')}
        >
          <ChevronLeftIcon class="w-8 h-8" />
        </button>
      {/if}
    </div>

    <!-- Stepper -->
    <div class="flex-1 flex items-center justify-center gap-0">
      {#each Array(totalSteps) as _, i}
        {@const step = i + 1}
        {@const isCompleted = step < currentStep}
        {@const isCurrent = step === currentStep}

        {#if i > 0}
          <!-- Connecting line -->
          <div
            class="w-8 h-0.5 {isCompleted ? 'bg-[var(--color-interactive)]' : 'bg-stone-200 dark:bg-gray-700'}"
          ></div>
        {/if}

        <!-- Step circle -->
        <div
          class="w-8 h-8 rounded-full flex items-center justify-center text-sm font-medium transition-colors
                 {isCompleted
                   ? 'bg-[var(--color-interactive)] text-white'
                   : isCurrent
                     ? 'bg-[var(--color-interactive)] text-white'
                     : 'border-2 border-stone-300 dark:border-gray-600 text-stone-400 dark:text-gray-500'}"
          aria-current={isCurrent ? 'step' : undefined}
          aria-label={$t('profile.setup_step', { values: { current: step, total: totalSteps } })}
        >
          {#if isCompleted}
            <CheckIcon class="w-4 h-4" />
          {:else}
            {step}
          {/if}
        </div>
      {/each}
    </div>

    <!-- Spacer to balance the back button -->
    <div class="w-20"></div>
  </div>

  <!-- Content area: centered -->
  <div class="flex-1 flex items-center justify-center">
    {@render children()}
  </div>
</div>
