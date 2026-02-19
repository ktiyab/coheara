<!-- L4-01: Temporal correlation display — calm informational card. -->
<script lang="ts">
  import type { TemporalCorrelation } from '$lib/types/journal';

  interface Props {
    correlation: TemporalCorrelation;
  }
  let { correlation }: Props = $props();
</script>

<div class="mx-auto max-w-sm p-4 bg-[var(--color-info-50)] border border-[var(--color-info-200)] rounded-xl mb-3 text-left">
  <p class="text-sm text-[var(--color-info-800)]">{correlation.message}</p>
  <p class="text-xs text-[var(--color-info)] mt-1">
    ({correlation.days_since_change} day{correlation.days_since_change === 1 ? '' : 's'} ago)
  </p>
</div>
