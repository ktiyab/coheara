<!-- Spec 49: Surface active medication summary on the home screen. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { navigation } from '$lib/stores/navigation.svelte';
  import type { MedicationCard } from '$lib/types/medication';
  import { ChevronRightOutline } from 'flowbite-svelte-icons';
  import { PillIcon } from '$lib/components/icons';

  interface Props {
    medications: MedicationCard[];
  }

  let { medications }: Props = $props();

  let active = $derived(medications.filter((m) => m.status === 'Active'));
</script>

{#if active.length > 0}
  <section class="px-6 py-3" aria-label={$t('home.meds_heading')}>
    <h2 class="text-sm font-semibold text-[var(--color-text-secondary)] mb-2">
      {$t('home.meds_heading')}
    </h2>
    <button
      class="w-full flex items-center gap-3 p-3 rounded-xl bg-white dark:bg-gray-900 border border-[var(--color-border)]
             hover:bg-[var(--color-surface-hover)] transition-colors text-left"
      onclick={() => navigation.navigate('medications')}
    >
      <div class="shrink-0 w-10 h-10 rounded-lg bg-[var(--color-success-50)] flex items-center justify-center">
        <PillIcon class="w-5 h-5 text-[var(--color-success)]" />
      </div>
      <div class="flex-1 min-w-0">
        <p class="text-sm font-medium text-[var(--color-text-primary)]">
          {$t('home.meds_active_count', { values: { count: active.length } })}
        </p>
        <p class="text-xs text-[var(--color-text-muted)] truncate">
          {active.slice(0, 3).map((m) => m.generic_name).join(', ')}
          {#if active.length > 3}
            {$t('home.meds_and_more', { values: { count: active.length - 3 } })}
          {/if}
        </p>
      </div>
      <ChevronRightOutline class="w-4 h-4 shrink-0 text-[var(--color-text-muted)]" />
    </button>
  </section>
{/if}
