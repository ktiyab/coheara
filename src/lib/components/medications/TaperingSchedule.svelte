<!-- L3-05: Tapering schedule steps with current-step highlight. -->
<script lang="ts">
  import { t, locale } from 'svelte-i18n';
  import type { TaperingStepView } from '$lib/types/medication';

  interface Props {
    steps: TaperingStepView[];
  }
  let { steps }: Props = $props();

  function formatDateRange(step: TaperingStepView): string {
    if (!step.start_date) return $t('medications.tapering_days_count', { values: { days: step.duration_days } });
    const start = new Date(step.start_date);
    const end = new Date(start.getTime() + step.duration_days * 86400000);
    const fmt = (d: Date) => d.toLocaleDateString($locale ?? 'en-US', { month: 'short', day: 'numeric' });
    return `${fmt(start)} - ${fmt(end)}`;
  }
</script>

<div>
  <h3 class="text-sm font-medium text-stone-500 mb-3">{$t('medications.tapering_heading')}</h3>
  <div class="flex flex-col gap-2">
    {#each steps as step}
      <div
        class="flex items-center gap-3 px-3 py-2 rounded-lg
               {step.is_current
                 ? 'bg-[var(--color-info-50)] border border-[var(--color-info-200)]'
                 : 'bg-white border border-stone-100'}"
        aria-current={step.is_current ? 'step' : undefined}
      >
        <span class="text-xs text-stone-500 w-12 flex-shrink-0">
          {$t('medications.tapering_step', { values: { number: step.step_number } })}
        </span>
        <div class="flex-1">
          <p class="text-sm font-medium text-stone-800">
            {step.dose}
            <span class="text-stone-500 font-normal">{$t('medications.tapering_duration', { values: { days: step.duration_days } })}</span>
          </p>
          <p class="text-xs text-stone-500">{formatDateRange(step)}</p>
        </div>
        {#if step.is_current}
          <span class="text-xs text-[var(--color-info)] font-medium flex-shrink-0">{$t('medications.tapering_current')}</span>
        {/if}
      </div>
    {/each}
  </div>
</div>
