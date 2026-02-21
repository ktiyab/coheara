<!-- LP-07: Rich display of non-critical coherence alerts, replacing ObservationsBanner. -->
<script lang="ts">
  import type { Component } from 'svelte';
  import { t } from 'svelte-i18n';
  import { dismissCoherenceAlert } from '$lib/api/coherence';
  import type { CoherenceAlert, AlertType } from '$lib/types/coherence';
  import {
    WarningIcon,
    InfoIcon,
    ClockIcon,
    CloseIcon,
  } from '$lib/components/icons/md';

  interface Props {
    alerts: CoherenceAlert[];
    onDismiss?: () => void;
  }
  let { alerts, onDismiss }: Props = $props();

  let dismissing: string | null = $state(null);
  let expanded = $state(false);

  const COLLAPSE_THRESHOLD = 3;
  let visible = $derived(expanded ? alerts : alerts.slice(0, COLLAPSE_THRESHOLD));
  let hiddenCount = $derived(Math.max(0, alerts.length - COLLAPSE_THRESHOLD));

  const typeIcon: Record<AlertType, Component<{ class?: string }>> = {
    conflict: WarningIcon,
    duplicate: InfoIcon,
    gap: InfoIcon,
    drift: WarningIcon,
    temporal: ClockIcon,
    allergy: WarningIcon,
    dose: WarningIcon,
    critical: WarningIcon,
  };

  const typeColor: Record<AlertType, string> = {
    conflict: 'text-[var(--color-warning)]',
    duplicate: 'text-blue-500 dark:text-blue-400',
    gap: 'text-stone-500 dark:text-gray-400',
    drift: 'text-[var(--color-warning)]',
    temporal: 'text-blue-500 dark:text-blue-400',
    allergy: 'text-[var(--color-danger)]',
    dose: 'text-[var(--color-warning)]',
    critical: 'text-[var(--color-danger)]',
  };

  const severityStyle: Record<string, string> = {
    Info: 'border-blue-200 dark:border-blue-800 bg-blue-50/50 dark:bg-blue-900/20',
    Standard: 'border-amber-200 dark:border-amber-800 bg-amber-50/50 dark:bg-amber-900/20',
  };

  async function handleDismiss(alertId: string) {
    dismissing = alertId;
    try {
      await dismissCoherenceAlert(alertId, 'Reviewed by patient');
      onDismiss?.();
    } catch (e) {
      console.error('Failed to dismiss insight:', e);
    } finally {
      dismissing = null;
    }
  }
</script>

<section class="px-6 py-2" aria-label={$t('home.insights_heading') ?? 'Health Insights'}>
  <h2 class="text-sm font-semibold text-stone-500 dark:text-gray-400 uppercase tracking-wider mb-2">
    {$t('home.insights_heading') ?? 'Health Insights'}
  </h2>
  <div class="flex flex-col gap-2" role="list" aria-live="polite">
    {#each visible as alert (alert.id)}
      {@const Icon = typeIcon[alert.alert_type]}
      {@const color = typeColor[alert.alert_type]}
      {@const border = severityStyle[alert.severity] ?? severityStyle.Info}
      <div
        class="border rounded-xl px-4 py-3 flex items-start gap-3 {border}"
        role="listitem"
      >
        <span class="flex-shrink-0 w-8 h-8 rounded-lg bg-white dark:bg-gray-800 flex items-center justify-center {color}">
          <Icon class="w-4 h-4" />
        </span>
        <div class="flex-1 min-w-0">
          <p class="text-sm text-stone-800 dark:text-gray-100">{alert.patient_message}</p>
          <p class="text-xs text-stone-500 dark:text-gray-400 mt-0.5">
            {new Date(alert.detected_at).toLocaleDateString()}
            <span class="ml-1 uppercase">{alert.alert_type}</span>
          </p>
        </div>
        <button
          class="flex-shrink-0 text-stone-400 dark:text-gray-500 hover:text-stone-600 dark:hover:text-gray-300
                 min-h-[44px] min-w-[44px] flex items-center justify-center"
          onclick={() => handleDismiss(alert.id)}
          disabled={dismissing === alert.id}
          aria-label={$t('common.dismiss')}
        >
          <CloseIcon class="w-4 h-4" />
        </button>
      </div>
    {/each}
  </div>

  {#if !expanded && hiddenCount > 0}
    <button
      class="mt-2 text-sm text-[var(--color-primary)] hover:underline"
      onclick={() => { expanded = true; }}
    >
      {$t('home.insights_show_more', { values: { count: hiddenCount } }) ?? `Show ${hiddenCount} more`}
    </button>
  {/if}
</section>
