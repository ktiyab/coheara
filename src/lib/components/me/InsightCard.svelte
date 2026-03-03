<!-- L3-06: Single clinical insight card with severity color coding. -->
<script lang="ts">
  import type { MeInsight } from '$lib/types/me';

  let { insight }: { insight: MeInsight } = $props();

  let severityClasses = $derived(
    insight.severity === 'critical'
      ? 'bg-red-50 dark:bg-red-950 border-red-200 dark:border-red-800 text-red-800 dark:text-red-200'
      : insight.severity === 'warning'
        ? 'bg-amber-50 dark:bg-amber-950 border-amber-200 dark:border-amber-800 text-amber-800 dark:text-amber-200'
        : 'bg-blue-50 dark:bg-blue-950 border-blue-200 dark:border-blue-800 text-blue-800 dark:text-blue-200'
  );

  let severityIcon = $derived(
    insight.severity === 'critical'
      ? '●'
      : insight.severity === 'warning'
        ? '▲'
        : 'ℹ'
  );
</script>

<div class="p-3 rounded-lg border {severityClasses}">
  <div class="flex items-start gap-2">
    <span class="text-xs mt-0.5 flex-shrink-0">{severityIcon}</span>
    <div class="min-w-0">
      <p class="text-sm font-medium leading-snug">{insight.description}</p>
      <p class="text-xs opacity-70 mt-1">{insight.source}</p>
    </div>
  </div>
</div>
