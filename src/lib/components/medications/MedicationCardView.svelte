<!-- L3-05: Single medication card in the list view. Safety-critical display. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { MedicationCard } from '$lib/types/medication';
  import Badge from '$lib/components/ui/Badge.svelte';

  interface Props {
    medication: MedicationCard;
    onTap: (medication: MedicationCard) => void;
  }
  let { medication, onTap }: Props = $props();

  let statusBadge = $derived.by(() => {
    switch (medication.status) {
      case 'active': return { text: $t('medications.status_active'), variant: 'success' as const };
      case 'paused': return { text: $t('medications.status_paused'), variant: 'warning' as const };
      case 'stopped': return { text: $t('medications.status_stopped'), variant: 'neutral' as const };
      default: return { text: medication.status, variant: 'neutral' as const };
    }
  });

  let frequencyDisplay = $derived.by(() => {
    if (medication.frequency_type === 'as_needed') return $t('medications.frequency_as_needed');
    if (medication.frequency_type === 'tapering') return $t('medications.frequency_tapering');
    return medication.frequency;
  });

  let prescriberDisplay = $derived.by(() => {
    if (medication.is_otc) return $t('medications.prescriber_otc');
    if (medication.prescriber_name) return medication.prescriber_name;
    return $t('medications.prescriber_unknown');
  });

  function formatRoute(route: string): string {
    if (!route) return '';
    return route.charAt(0).toUpperCase() + route.slice(1).toLowerCase();
  }
</script>

<button
  class="w-full text-left bg-white dark:bg-gray-900 rounded-xl p-4 shadow-sm border border-stone-100 dark:border-gray-800
         hover:shadow-md transition-shadow min-h-[44px]"
  onclick={() => onTap(medication)}
  aria-label={$t('medications.card_aria', { values: { name: medication.generic_name, dose: medication.dose } })}
>
  <!-- Row 1: Generic name + Dose -->
  <div class="flex items-baseline justify-between gap-3">
    <span class="text-lg font-semibold text-stone-800 dark:text-gray-100 truncate">
      {medication.generic_name}
    </span>
    <span class="text-lg font-semibold text-stone-800 dark:text-gray-100 flex-shrink-0">
      {medication.dose}
    </span>
  </div>

  <!-- Row 2: Brand name + Frequency -->
  <div class="flex items-baseline justify-between gap-3 mt-0.5">
    {#if medication.brand_name}
      <span class="text-sm text-stone-500 dark:text-gray-400 truncate">
        ({medication.brand_name})
      </span>
    {:else}
      <span></span>
    {/if}
    <span class="text-sm text-stone-600 dark:text-gray-300 flex-shrink-0">
      {frequencyDisplay}
    </span>
  </div>

  <!-- Row 3: Prescriber + Route + Status badge -->
  <div class="flex items-center justify-between gap-2 mt-2">
    <div class="flex items-center gap-1 text-xs text-stone-500 dark:text-gray-400 truncate">
      <span>{prescriberDisplay}</span>
      <span aria-hidden="true">&middot;</span>
      <span>{formatRoute(medication.route)}</span>
      {#if medication.is_compound}
        <span aria-hidden="true">&middot;</span>
        <span class="text-indigo-500">{$t('medications.card_compound')}</span>
      {/if}
      {#if medication.has_tapering}
        <span aria-hidden="true">&middot;</span>
        <span class="text-[var(--color-info)]">{$t('medications.card_tapering')}</span>
      {/if}
    </div>
    <Badge variant={statusBadge.variant} size="sm">
      {statusBadge.text}
    </Badge>
  </div>

  <!-- Row 4: Condition -->
  {#if medication.condition}
    <p class="text-xs text-stone-500 dark:text-gray-400 italic mt-1">
      {medication.condition}
    </p>
  {/if}

  <!-- Row 5: Coherence alerts -->
  {#if medication.coherence_alerts.length > 0}
    {#each medication.coherence_alerts as alert}
      <div
        class="mt-2 px-3 py-2 rounded-lg text-xs
               {alert.severity === 'Critical'
                 ? 'bg-[var(--color-warning-50)] text-[var(--color-warning-800)] border border-[var(--color-warning-200)]'
                 : alert.severity === 'Warning'
                   ? 'bg-[var(--color-info-50)] text-[var(--color-info-800)] border border-[var(--color-info-200)]'
                   : 'bg-stone-50 dark:bg-gray-950 text-stone-600 dark:text-gray-300 border border-stone-100 dark:border-gray-800'}"
        role="status"
      >
        {alert.summary}
      </div>
    {/each}
  {/if}
</button>
