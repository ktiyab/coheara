<!-- L3-05: Tapering schedule steps with current-step highlight. -->
<script lang="ts">
  import type { TaperingStepView } from '$lib/types/medication';

  interface Props {
    steps: TaperingStepView[];
  }
  let { steps }: Props = $props();

  function formatDateRange(step: TaperingStepView): string {
    if (!step.start_date) return `${step.duration_days} days`;
    const start = new Date(step.start_date);
    const end = new Date(start.getTime() + step.duration_days * 86400000);
    const fmt = (d: Date) => d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
    return `${fmt(start)} - ${fmt(end)}`;
  }
</script>

<div>
  <h3 class="text-sm font-medium text-stone-500 mb-3">Tapering schedule</h3>
  <div class="flex flex-col gap-2">
    {#each steps as step}
      <div
        class="flex items-center gap-3 px-3 py-2 rounded-lg
               {step.is_current
                 ? 'bg-blue-50 border border-blue-200'
                 : 'bg-white border border-stone-100'}"
      >
        <span class="text-xs text-stone-400 w-12 flex-shrink-0">
          Step {step.step_number}
        </span>
        <div class="flex-1">
          <p class="text-sm font-medium text-stone-800">
            {step.dose}
            <span class="text-stone-500 font-normal">for {step.duration_days} days</span>
          </p>
          <p class="text-xs text-stone-400">{formatDateRange(step)}</p>
        </div>
        {#if step.is_current}
          <span class="text-xs text-blue-600 font-medium flex-shrink-0">Current</span>
        {/if}
      </div>
    {/each}
  </div>
</div>
