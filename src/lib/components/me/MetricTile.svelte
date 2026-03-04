<!-- ME-REDESIGN: Per-metric card with icon, value, range bar, classification. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { t } from 'svelte-i18n';
  import type { ReferenceRange } from '$lib/types/me';
  import { getVitalTrend } from '$lib/api/me';
  import {
    HeartIcon,
    PersonIcon,
    SunIcon,
    BrainIcon,
    ClipboardIcon
  } from '$lib/components/icons/md';
  import RangeBar from './RangeBar.svelte';
  import Sparkline from './Sparkline.svelte';

  let { range }: { range: ReferenceRange } = $props();

  let hasValue = $derived(range.current_value != null);

  /** REVIEW-01: Map metric key to backend vital_type for trend queries. */
  const VITAL_TYPE_MAP: Record<string, string> = {
    blood_pressure: 'blood_pressure',
    heart_rate: 'heart_rate',
    spo2: 'oxygen_saturation',
    glucose: 'blood_glucose',
    temperature: 'temperature',
  };

  let trendPoints: number[] = $state([]);

  onMount(async () => {
    const vitalType = VITAL_TYPE_MAP[range.key];
    if (!vitalType) return;
    try {
      const trend = await getVitalTrend(vitalType, 30);
      trendPoints = trend.map(p => p.value);
    } catch {
      // Silently fail — sparkline is decorative
    }
  });

  const METRIC_STYLES: Record<string, { icon: typeof HeartIcon; bg: string }> = {
    blood_pressure: { icon: HeartIcon, bg: 'bg-rose-100 dark:bg-rose-900/50 text-rose-600 dark:text-rose-300' },
    heart_rate: { icon: HeartIcon, bg: 'bg-pink-100 dark:bg-pink-900/50 text-pink-600 dark:text-pink-300' },
    spo2: { icon: HeartIcon, bg: 'bg-sky-100 dark:bg-sky-900/50 text-sky-600 dark:text-sky-300' },
    bmi: { icon: PersonIcon, bg: 'bg-emerald-100 dark:bg-emerald-900/50 text-emerald-600 dark:text-emerald-300' },
    glucose: { icon: ClipboardIcon, bg: 'bg-amber-100 dark:bg-amber-900/50 text-amber-600 dark:text-amber-300' },
    temperature: { icon: SunIcon, bg: 'bg-orange-100 dark:bg-orange-900/50 text-orange-600 dark:text-orange-300' },
    egfr: { icon: ClipboardIcon, bg: 'bg-teal-100 dark:bg-teal-900/50 text-teal-600 dark:text-teal-300' },
    hba1c: { icon: ClipboardIcon, bg: 'bg-violet-100 dark:bg-violet-900/50 text-violet-600 dark:text-violet-300' },
    ldl_cholesterol: { icon: HeartIcon, bg: 'bg-yellow-100 dark:bg-yellow-900/50 text-yellow-600 dark:text-yellow-300' },
    potassium: { icon: ClipboardIcon, bg: 'bg-lime-100 dark:bg-lime-900/50 text-lime-600 dark:text-lime-300' },
    sodium: { icon: ClipboardIcon, bg: 'bg-blue-100 dark:bg-blue-900/50 text-blue-600 dark:text-blue-300' },
    alt: { icon: ClipboardIcon, bg: 'bg-orange-100 dark:bg-orange-900/50 text-orange-600 dark:text-orange-300' },
    hemoglobin: { icon: HeartIcon, bg: 'bg-red-100 dark:bg-red-900/50 text-red-600 dark:text-red-300' },
    tsh: { icon: BrainIcon, bg: 'bg-purple-100 dark:bg-purple-900/50 text-purple-600 dark:text-purple-300' },
    uacr: { icon: ClipboardIcon, bg: 'bg-cyan-100 dark:bg-cyan-900/50 text-cyan-600 dark:text-cyan-300' },
    vitamin_d: { icon: SunIcon, bg: 'bg-yellow-100 dark:bg-yellow-900/50 text-yellow-600 dark:text-yellow-300' }
  };

  let style = $derived(METRIC_STYLES[range.key] ?? { icon: ClipboardIcon, bg: 'bg-stone-100 dark:bg-gray-800 text-stone-600 dark:text-gray-300' });
  let IconComponent = $derived(style.icon);
</script>

<div
  class="p-3 rounded-xl border transition-opacity
    {hasValue
      ? 'bg-white dark:bg-gray-900 border-stone-200 dark:border-gray-800'
      : 'bg-stone-50 dark:bg-gray-900/50 border-stone-100 dark:border-gray-800/50 opacity-75'}"
>
  <!-- Header: icon + name + unit -->
  <div class="flex items-center gap-2.5 mb-2">
    <span class="w-7 h-7 rounded-full {style.bg} flex items-center justify-center flex-shrink-0">
      <IconComponent class="w-3.5 h-3.5" />
    </span>
    <div class="min-w-0 flex-1">
      <p class="text-xs font-medium text-stone-700 dark:text-gray-200 truncate">{range.label}</p>
    </div>
    <span class="text-[10px] text-stone-400 dark:text-gray-400 flex-shrink-0">{range.unit}</span>
  </div>

  <!-- Value + sparkline -->
  <div class="flex items-end justify-between mb-2">
    <p class="text-lg font-semibold {hasValue ? 'text-stone-800 dark:text-gray-100' : 'text-stone-300 dark:text-gray-600'}">
      {range.current_display ?? $t('me.no_value')}
    </p>
    {#if trendPoints.length >= 2}
      <Sparkline points={trendPoints} width={64} height={20} />
    {/if}
  </div>

  <!-- Range bar -->
  <RangeBar
    tiers={range.tiers}
    currentValue={range.current_value}
    normalMin={range.normal_min}
    normalMax={range.normal_max}
  />

  <!-- Classification or normal hint -->
  <div class="mt-1.5 flex items-baseline justify-between">
    <p class="text-[11px] {hasValue ? 'text-stone-600 dark:text-gray-300' : 'text-stone-400 dark:text-gray-400'}">
      {#if range.current_tier_label}
        {range.current_tier_label}
      {:else}
        {$t('me.normal_range_hint', { values: { range: `${range.normal_min}\u2013${range.normal_max}` } })}
      {/if}
    </p>
    <p class="text-[10px] text-stone-400 dark:text-gray-400">{range.source}</p>
  </div>
</div>
