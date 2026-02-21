<!-- L3-04: Confidence summary bar — field count + progress bar. -->
<script lang="ts">
  import { t } from 'svelte-i18n';

  interface Props {
    totalFields: number;
    confidentFields: number;
    flaggedFields: number;
    overallConfidence: number;
  }
  let { totalFields, confidentFields, flaggedFields, overallConfidence }: Props = $props();

  let summaryText = $derived.by(() => {
    if (totalFields === 0) return $t('review.summary_no_fields');
    if (flaggedFields === 0) return $t('review.summary_all_good', { values: { count: totalFields } });
    return $t('review.summary_mixed', { values: { total: totalFields, confident: confidentFields, flagged: flaggedFields } });
  });

  let barColor = $derived(
    flaggedFields === 0 ? 'bg-[var(--color-success)]' :
    flaggedFields <= 2 ? 'bg-[var(--color-warning)]' :
    'bg-[var(--color-danger)]'
  );

  let fillPercent = $derived(
    totalFields > 0 ? Math.round((confidentFields / totalFields) * 100) : 0
  );
</script>

<div class="px-4 py-3 bg-white dark:bg-gray-900 border-t border-stone-200 dark:border-gray-700 shrink-0">
  <div class="flex items-center gap-3">
    <div class="flex-1">
      <p class="text-sm text-stone-600 dark:text-gray-300">{summaryText}</p>
      <div class="mt-1 h-1.5 bg-stone-100 dark:bg-gray-800 rounded-full overflow-hidden">
        <div class="h-full rounded-full transition-all duration-500 {barColor}"
             style="width: {fillPercent}%"></div>
      </div>
    </div>
    <span class="text-xs text-stone-500 dark:text-gray-400">
      {$t('review.summary_overall')} {Math.round(overallConfidence * 100)}%
    </span>
  </div>
</div>
