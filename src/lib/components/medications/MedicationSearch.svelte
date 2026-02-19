<!-- L3-05: Search bar + prescriber filter dropdown. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { PrescriberOption } from '$lib/types/medication';

  interface Props {
    value: string;
    onInput: (value: string) => void;
    prescribers: PrescriberOption[];
    selectedPrescriber: string | null;
    onPrescriberChange: (id: string | null) => void;
  }
  let { value, onInput, prescribers, selectedPrescriber, onPrescriberChange }: Props = $props();
</script>

<div class="px-6 py-2 flex gap-3 items-center">
  <div class="flex-1 relative">
    <input
      type="text"
      {value}
      oninput={(e) => onInput(e.currentTarget.value)}
      placeholder={$t('medications.search_placeholder')}
      class="w-full px-4 py-2.5 pl-10 rounded-lg border border-stone-200 bg-white
             text-sm min-h-[44px]
             focus:border-[var(--color-primary)] focus:outline-none"
      aria-label={$t('medications.search_aria')}
    />
    <span class="absolute left-3 top-1/2 -translate-y-1/2 text-stone-500 text-sm"
          aria-hidden="true">
      &#x1F50D;
    </span>
  </div>

  {#if prescribers.length > 0}
    <select
      class="px-3 py-2.5 rounded-lg border border-stone-200 bg-white text-sm
             min-h-[44px] text-stone-600
             focus:border-[var(--color-primary)] focus:outline-none"
      value={selectedPrescriber ?? ''}
      onchange={(e) => {
        const val = e.currentTarget.value;
        onPrescriberChange(val === '' ? null : val);
      }}
      aria-label={$t('medications.search_filter_prescriber')}
    >
      <option value="">{$t('medications.search_all_prescribers')}</option>
      {#each prescribers as prescriber}
        <option value={prescriber.id}>
          {prescriber.name} ({prescriber.medication_count})
        </option>
      {/each}
    </select>
  {/if}
</div>
