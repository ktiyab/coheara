<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { CriticalLabAlert } from '$lib/types/trust';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { WarningIcon } from '$lib/components/icons/md';

  interface Props {
    alerts: CriticalLabAlert[];
  }
  let { alerts }: Props = $props();
</script>

<div class="px-6 py-2">
  {#each alerts as alert (alert.id)}
    <div class="bg-[var(--color-danger-50)] border border-[var(--color-danger-200)] rounded-xl p-4 mb-2"
         role="alert"
         aria-label={$t('home.alert_critical_aria', { values: { testName: alert.test_name } })}>
      <div class="flex items-start gap-3">
        <WarningIcon class="w-5 h-5 text-[var(--color-danger)] mt-0.5 flex-shrink-0" />
        <div class="flex-1">
          <p class="text-sm font-semibold text-[var(--color-danger-800)]">
            {$t('home.alert_critical_label', { values: { testName: alert.test_name } })}
          </p>
          <p class="text-sm text-[var(--color-danger-800)] mt-1">
            {alert.value} {alert.unit}
            {#if alert.reference_range}
              <span class="text-[var(--color-danger)]">{$t('home.alert_reference_range', { values: { range: alert.reference_range } })}</span>
            {/if}
          </p>
          <button
            class="text-sm text-[var(--color-danger-800)] font-medium mt-2 underline
                   min-h-[44px] min-w-[44px] -ml-1 px-1"
            onclick={() => navigation.navigate('document-detail', { documentId: alert.document_id })}
          >
            {$t('home.alert_view_source')}
          </button>
        </div>
      </div>
    </div>
  {/each}
</div>
