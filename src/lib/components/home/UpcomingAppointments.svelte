<!-- Spec 49: Surface upcoming appointments on the home screen. -->
<!-- LP-06: Tappable — navigates to timeline with appointment filter. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { StoredAppointment } from '$lib/types/appointment';
  import { CalendarIcon } from '$lib/components/icons/md';
  import { navigation } from '$lib/stores/navigation.svelte';

  interface Props {
    appointments: StoredAppointment[];
  }

  let { appointments }: Props = $props();

  let upcoming = $derived.by(() => {
    const today = new Date().toISOString().slice(0, 10);
    return appointments
      .filter((a) => a.date >= today)
      .sort((a, b) => a.date.localeCompare(b.date))
      .slice(0, 2);
  });

  function formatDate(dateStr: string): string {
    const date = new Date(dateStr + 'T00:00:00');
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const diff = Math.floor((date.getTime() - today.getTime()) / 86400000);
    if (diff === 0) return $t('home.upcoming_today');
    if (diff === 1) return $t('home.upcoming_tomorrow');
    return date.toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric' });
  }
</script>

{#if upcoming.length > 0}
  <section class="px-6 py-3" aria-label={$t('home.upcoming_heading')}>
    <h2 class="text-sm font-semibold text-[var(--color-text-secondary)] mb-2">
      {$t('home.upcoming_heading')}
    </h2>
    <div class="flex flex-col gap-2">
      {#each upcoming as appt (appt.id)}
        <button
          onclick={() => navigation.navigate('timeline', { filter: 'appointment' })}
          class="flex items-center gap-3 p-3 rounded-xl bg-white dark:bg-gray-900 border border-[var(--color-border)] w-full
                 hover:border-[var(--color-primary)] hover:shadow-sm transition-colors cursor-pointer text-left"
        >
          <div class="shrink-0 w-10 h-10 rounded-lg bg-[var(--color-primary-50)] flex items-center justify-center">
            <CalendarIcon class="w-5 h-5 text-[var(--color-primary)]" />
          </div>
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-[var(--color-text-primary)] truncate">
              {appt.professional_name}
            </p>
            <p class="text-xs text-[var(--color-text-muted)]">
              {appt.professional_specialty} · {formatDate(appt.date)}
            </p>
          </div>
          {#if !appt.prep_generated}
            <button
              class="shrink-0 text-xs px-2 py-1 rounded-full bg-[var(--color-warning-50)] text-[var(--color-warning-800)]
                     hover:bg-[var(--color-warning-100)] transition-colors"
              onclick={(e) => {
                e.stopPropagation();
                const prefill = `I have an appointment with ${appt.professional_name} (${appt.professional_specialty}) on ${appt.date}. Help me prepare.`;
                navigation.navigate('chat', { prefill });
              }}
              aria-label={$t('home.upcoming_prep_needed')}
            >
              {$t('home.upcoming_prep_needed')}
            </button>
          {/if}
        </button>
      {/each}
    </div>
  </section>
{/if}
