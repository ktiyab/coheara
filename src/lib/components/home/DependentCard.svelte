<!-- Spec 46 [CG-02]: Dependent profile summary card for caregiver dashboard -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { CaregiverSummary } from '$lib/api/profile';
  import { PROFILE_COLORS } from '$lib/types/profile';
  import Avatar from '$lib/components/ui/Avatar.svelte';
  import Card from '$lib/components/ui/Card.svelte';

  interface Props {
    summary: CaregiverSummary;
    onViewProfile: () => void;
  }
  let { summary, onViewProfile }: Props = $props();

  let color = $derived(PROFILE_COLORS[summary.color_index ?? 0]);

  let isStale = $derived(() => {
    if (!summary.updated_at) return false;
    const updated = new Date(summary.updated_at);
    const now = new Date();
    const diffDays = (now.getTime() - updated.getTime()) / (1000 * 60 * 60 * 24);
    return diffDays > 7;
  });
</script>

<Card>
  <div class="flex items-start gap-3 p-4">
    <Avatar name={summary.managed_profile_name} color={color} size="md" />

    <div class="flex-1 min-w-0">
      <h3 class="font-semibold text-stone-800 dark:text-gray-100 truncate">{summary.managed_profile_name}</h3>

      <div class="flex flex-wrap gap-x-3 gap-y-1 mt-1 text-sm text-stone-500 dark:text-gray-400">
        {#if summary.critical_alert_count > 0}
          <span class="text-[var(--color-danger)] font-medium">
            {$t('caregiver.critical_alerts', { values: { count: summary.critical_alert_count } })}
          </span>
        {:else if summary.alert_count > 0}
          <span class="text-[var(--color-warning)]">
            {$t('caregiver.alerts', { values: { count: summary.alert_count } })}
          </span>
        {/if}

        {#if summary.active_medication_count > 0}
          <span>{$t('caregiver.meds_active', { values: { count: summary.active_medication_count } })}</span>
        {/if}

        {#if summary.next_appointment_date}
          <span>{$t('caregiver.next_appointment', { values: { date: summary.next_appointment_date } })}</span>
        {/if}
      </div>

      {#if isStale()}
        <p class="text-xs text-stone-400 dark:text-gray-500 mt-1">
          {$t('caregiver.stale_summary')}
        </p>
      {/if}
    </div>

    <button
      class="text-sm text-[var(--color-primary)] font-medium min-h-[44px] min-w-[44px]
             flex items-center justify-center flex-shrink-0 hover:underline"
      onclick={onViewProfile}
    >
      {$t('caregiver.view_profile')}
    </button>
  </div>
</Card>
