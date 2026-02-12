<!-- L3-05: Dose change history timeline. -->
<script lang="ts">
  import type { DoseChangeView } from '$lib/types/medication';

  interface Props {
    changes: DoseChangeView[];
    medicationName: string;
    onClose: () => void;
  }
  let { changes, medicationName, onClose }: Props = $props();

  function formatDate(dateStr: string): string {
    return new Date(dateStr).toLocaleDateString('en-US', {
      month: 'short', day: 'numeric', year: 'numeric',
    });
  }
</script>

<div>
  <div class="flex items-center justify-between mb-3">
    <h3 class="text-sm font-medium text-stone-500">
      Dose history for {medicationName}
    </h3>
    <button
      class="text-xs text-stone-400 hover:text-stone-600 min-h-[44px] min-w-[44px]
             flex items-center justify-center"
      onclick={onClose}
      aria-label="Close dose history"
    >
      Hide
    </button>
  </div>

  <div class="relative pl-6">
    <div class="absolute left-[7px] top-2 bottom-2 w-0.5 bg-stone-200" aria-hidden="true"></div>

    {#each changes as change, i}
      <div class="relative pb-6 last:pb-0">
        <div
          class="absolute left-[-17px] top-1 w-3 h-3 rounded-full border-2
                 {i === changes.length - 1
                   ? 'bg-[var(--color-primary)] border-[var(--color-primary)]'
                   : 'bg-white border-stone-400'}"
          aria-hidden="true"
        ></div>

        <div>
          <p class="text-sm font-medium text-stone-700">
            {formatDate(change.change_date)}
          </p>
          <p class="text-sm text-stone-800 mt-0.5">
            {#if change.old_dose}
              {change.old_dose} &rarr; {change.new_dose}
            {:else}
              Started at {change.new_dose}
            {/if}
          </p>
          {#if change.old_frequency && change.new_frequency}
            <p class="text-xs text-stone-600 mt-0.5">
              {change.old_frequency} &rarr; {change.new_frequency}
            </p>
          {/if}
          {#if change.changed_by_name}
            <p class="text-xs text-stone-400 mt-0.5">
              {change.changed_by_name}
            </p>
          {/if}
          {#if change.reason}
            <p class="text-xs text-stone-500 italic mt-0.5">
              "{change.reason}"
            </p>
          {/if}
          {#if change.document_title}
            <p class="text-xs text-stone-400 mt-0.5">
              Source: {change.document_title}
            </p>
          {/if}
        </div>
      </div>
    {/each}
  </div>
</div>
