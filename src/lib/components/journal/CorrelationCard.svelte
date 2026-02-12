<!-- L4-01: Temporal correlation display — calm informational card. -->
<script lang="ts">
  import type { TemporalCorrelation } from '$lib/types/journal';

  interface Props {
    correlation: TemporalCorrelation;
  }
  let { correlation }: Props = $props();
</script>

<div class="mx-auto max-w-sm p-4 bg-blue-50 border border-blue-100 rounded-xl mb-3 text-left">
  <p class="text-sm text-blue-800">{correlation.message}</p>
  <p class="text-xs text-blue-600 mt-1">
    ({correlation.days_since_change} day{correlation.days_since_change === 1 ? '' : 's'} ago)
  </p>
</div>
