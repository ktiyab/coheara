<!-- Spec 49: Surface active medication summary on the home screen. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { navigation } from '$lib/stores/navigation.svelte';
  import type { MedicationCard } from '$lib/types/medication';

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
      class="w-full flex items-center gap-3 p-3 rounded-xl bg-white border border-[var(--color-border)]
             hover:bg-[var(--color-surface-hover)] transition-colors text-left"
      onclick={() => navigation.navigate('medications')}
    >
      <div class="shrink-0 w-10 h-10 rounded-lg bg-[var(--color-success-50)] flex items-center justify-center text-lg">
        &#128138;
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
      <span class="shrink-0 text-[var(--color-text-muted)]" aria-hidden="true">&#8250;</span>
    </button>
  </section>
{/if}
