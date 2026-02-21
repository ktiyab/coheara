<!-- Spec 49: Surface active medication summary on the home screen. -->
<!-- LP-06: Tappable — navigates to timeline with medication filter. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { MedicationCard } from '$lib/types/medication';
  import { PillIcon } from '$lib/components/icons';
  import { navigation } from '$lib/stores/navigation.svelte';

  interface Props {
    medications: MedicationCard[];
  }

  let { medications }: Props = $props();

  let active = $derived(medications.filter((m) => m.status === 'Active'));

  let recentlyChangedCount = $derived.by(() => {
    const sevenDaysAgo = new Date();
    sevenDaysAgo.setDate(sevenDaysAgo.getDate() - 7);
    const cutoff = sevenDaysAgo.toISOString().slice(0, 10);
    return active.filter((m) => m.start_date && m.start_date >= cutoff).length;
  });
</script>

{#if active.length > 0}
  <section class="px-6 py-3" aria-label={$t('home.meds_heading')}>
    <h2 class="text-sm font-semibold text-[var(--color-text-secondary)] mb-2">
      {$t('home.meds_heading')}
    </h2>
    <button
      onclick={() => navigation.navigate('timeline', { filter: 'medication' })}
      class="w-full flex items-center gap-3 p-3 rounded-xl bg-white dark:bg-gray-900 border border-[var(--color-border)]
             hover:border-[var(--color-primary)] hover:shadow-sm transition-colors cursor-pointer text-left"
    >
      <div class="shrink-0 w-10 h-10 rounded-lg bg-[var(--color-success-50)] flex items-center justify-center">
        <PillIcon class="w-5 h-5 text-[var(--color-success)]" />
      </div>
      <div class="flex-1 min-w-0">
        <p class="text-sm font-medium text-[var(--color-text-primary)]">
          {$t('home.meds_active_count', { values: { count: active.length } })}
          {#if recentlyChangedCount > 0}
            <span class="ml-1 text-xs font-normal text-amber-600 dark:text-amber-400">
              · {$t('home.meds_recently_changed', { values: { count: recentlyChangedCount } }) ?? `${recentlyChangedCount} recently changed`}
            </span>
          {/if}
        </p>
        <p class="text-xs text-[var(--color-text-muted)] truncate">
          {active.slice(0, 3).map((m) => m.generic_name).join(', ')}
          {#if active.length > 3}
            {$t('home.meds_and_more', { values: { count: active.length - 3 } })}
          {/if}
        </p>
      </div>
    </button>
  </section>
{/if}
