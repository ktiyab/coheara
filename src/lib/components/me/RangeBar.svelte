<!-- ME-REDESIGN: Horizontal colored range bar with value marker. -->
<script lang="ts">
  import type { RangeTier } from '$lib/types/me';

  let {
    tiers,
    currentValue = null,
    normalMin,
    normalMax
  }: {
    tiers: RangeTier[];
    currentValue: number | null;
    normalMin: number;
    normalMax: number;
  } = $props();

  const COLOR_MAP: Record<string, { bg: string; bgDark: string }> = {
    green: { bg: 'bg-emerald-400', bgDark: 'dark:bg-emerald-600' },
    yellow: { bg: 'bg-amber-400', bgDark: 'dark:bg-amber-500' },
    orange: { bg: 'bg-orange-400', bgDark: 'dark:bg-orange-500' },
    red: { bg: 'bg-red-400', bgDark: 'dark:bg-red-500' }
  };

  let rangeMin = $derived(tiers.length > 0 ? tiers[0].min_value : 0);
  let rangeMax = $derived(tiers.length > 0 ? tiers[tiers.length - 1].max_value : 100);
  let totalSpan = $derived(rangeMax - rangeMin || 1);

  let markerPercent = $derived(
    currentValue != null
      ? Math.max(0, Math.min(100, ((currentValue - rangeMin) / totalSpan) * 100))
      : null
  );

  function tierWidth(tier: RangeTier): number {
    return ((tier.max_value - tier.min_value) / totalSpan) * 100;
  }

  function isNormal(tier: RangeTier): boolean {
    return tier.min_value >= normalMin && tier.max_value <= normalMax;
  }
</script>

<div class="relative w-full">
  <!-- Tier bar -->
  <div class="flex h-2.5 rounded-full overflow-hidden">
    {#each tiers as tier (tier.key)}
      {@const colors = COLOR_MAP[tier.color] ?? COLOR_MAP.green}
      <div
        class="{colors.bg} {colors.bgDark} transition-opacity"
        style="width: {tierWidth(tier)}%"
        class:opacity-50={currentValue == null && !isNormal(tier)}
        title={tier.label}
      ></div>
    {/each}
  </div>

  <!-- Value marker -->
  {#if markerPercent != null}
    <div
      class="absolute -top-1 w-0 h-0"
      style="left: {markerPercent}%"
    >
      <div class="relative -left-1.5">
        <div class="w-3 h-3 rounded-full bg-stone-800 dark:bg-gray-100 border-2 border-white dark:border-gray-900 shadow-sm"></div>
      </div>
    </div>
  {/if}
</div>
