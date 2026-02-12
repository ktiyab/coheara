<!-- L3-04: Confidence summary bar — field count + progress bar. -->
<script lang="ts">
  interface Props {
    totalFields: number;
    confidentFields: number;
    flaggedFields: number;
    overallConfidence: number;
  }
  let { totalFields, confidentFields, flaggedFields, overallConfidence }: Props = $props();

  let summaryText = $derived.by(() => {
    if (totalFields === 0) return 'No fields extracted';
    if (flaggedFields === 0) return `${totalFields} fields extracted, all look good`;
    return `${totalFields} fields extracted \u00B7 ${confidentFields} confident \u00B7 ${flaggedFields} need checking`;
  });

  let barColor = $derived(
    flaggedFields === 0 ? 'bg-green-500' :
    flaggedFields <= 2 ? 'bg-amber-500' :
    'bg-red-500'
  );

  let fillPercent = $derived(
    totalFields > 0 ? Math.round((confidentFields / totalFields) * 100) : 0
  );
</script>

<div class="px-4 py-3 bg-white border-t border-stone-200 shrink-0">
  <div class="flex items-center gap-3">
    <div class="flex-1">
      <p class="text-sm text-stone-600">{summaryText}</p>
      <div class="mt-1 h-1.5 bg-stone-100 rounded-full overflow-hidden">
        <div class="h-full rounded-full transition-all duration-500 {barColor}"
             style="width: {fillPercent}%"></div>
      </div>
    </div>
    <span class="text-xs text-stone-400">
      Overall: {Math.round(overallConfidence * 100)}%
    </span>
  </div>
</div>
