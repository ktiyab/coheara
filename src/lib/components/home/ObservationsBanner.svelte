<!-- L2-03/GAP-M02: Display non-critical coherence observations on the home screen. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { dismissCoherenceAlert } from '$lib/api/coherence';
  import type { CoherenceAlert, AlertType } from '$lib/types/coherence';
  import { CloseOutline } from 'flowbite-svelte-icons';

  interface Props {
    alerts: CoherenceAlert[];
    onDismiss?: () => void;
  }
  let { alerts, onDismiss }: Props = $props();

  let dismissing: string | null = $state(null);

  const typeIcon: Record<AlertType, string> = {
    conflict: '!',
    duplicate: '=',
    gap: '?',
    drift: '~',
    temporal: '#',
    allergy: '!',
    dose: '!',
    critical: '!',
  };

  const typeColor: Record<AlertType, string> = {
    conflict: 'text-[var(--color-warning)]',
    duplicate: 'text-[var(--color-info)]',
    gap: 'text-stone-500 dark:text-gray-400',
    drift: 'text-[var(--color-warning)]',
    temporal: 'text-purple-600',
    allergy: 'text-[var(--color-danger)]',
    dose: 'text-[var(--color-warning)]',
    critical: 'text-[var(--color-danger)]',
  };

  async function handleDismiss(alertId: string) {
    dismissing = alertId;
    try {
      await dismissCoherenceAlert(alertId, 'Reviewed by patient');
      onDismiss?.();
    } catch (e) {
      console.error('Failed to dismiss observation:', e);
    } finally {
      dismissing = null;
    }
  }
</script>

<section class="px-6 py-2" aria-label={$t('home.observations_heading') ?? 'Health observations'}>
  <h2 class="text-sm font-semibold text-stone-500 dark:text-gray-400 uppercase tracking-wider mb-2">
    {$t('home.observations_heading') ?? 'Observations'}
  </h2>
  <div class="flex flex-col gap-2" role="list" aria-live="polite">
    {#each alerts as alert (alert.id)}
      <div
        class="bg-white dark:bg-gray-900 border border-stone-200 dark:border-gray-700 rounded-xl px-4 py-3 flex items-start gap-3"
        role="listitem"
      >
        <span
          class="flex-shrink-0 w-6 h-6 rounded-full bg-stone-100 dark:bg-gray-800 flex items-center justify-center text-xs font-bold {typeColor[alert.alert_type]}"
          aria-hidden="true"
        >
          {typeIcon[alert.alert_type]}
        </span>
        <div class="flex-1 min-w-0">
          <p class="text-sm text-stone-800 dark:text-gray-100">{alert.patient_message}</p>
          <p class="text-xs text-stone-500 dark:text-gray-400 mt-0.5">
            {new Date(alert.detected_at).toLocaleDateString()}
          </p>
        </div>
        <button
          class="flex-shrink-0 text-stone-500 dark:text-gray-400 hover:text-stone-600 dark:hover:text-gray-300 min-h-[44px] min-w-[44px]
                 flex items-center justify-center"
          onclick={() => handleDismiss(alert.id)}
          disabled={dismissing === alert.id}
          aria-label={$t('common.dismiss')}
        >
          <CloseOutline class="w-4 h-4" />
        </button>
      </div>
    {/each}
  </div>
</section>
