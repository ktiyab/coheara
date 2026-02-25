<!--
  T7: Hardware Status Card — GPU/CPU detection with color-coded indicators.

  Shows: GPU tier (colored dot), VRAM, processor label, estimated speed.
  Includes refresh button to re-detect hardware.
-->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { getHardwareProfile } from '$lib/api/ai';
  import { formatModelSize } from '$lib/types/ai';
  import type { HardwareStatus, GpuTier } from '$lib/types/ai';

  let hardware = $state<HardwareStatus | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);

  /** Color class for the GPU tier dot. */
  function tierDotClass(tier: GpuTier): string {
    switch (tier) {
      case 'full_gpu': return 'bg-[var(--color-success)]';
      case 'partial_gpu': return 'bg-amber-500';
      case 'cpu_only': return 'bg-red-500';
    }
  }

  /** i18n key for GPU tier label. */
  function tierLabelKey(tier: GpuTier): string {
    switch (tier) {
      case 'full_gpu': return 'ai.hardware_full_gpu';
      case 'partial_gpu': return 'ai.hardware_partial_gpu';
      case 'cpu_only': return 'ai.hardware_cpu_only';
    }
  }

  async function detect() {
    loading = true;
    error = null;
    try {
      hardware = await getHardwareProfile();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  // Auto-detect on mount
  $effect(() => {
    detect();
  });
</script>

<section class="bg-white dark:bg-gray-900 rounded-xl p-5 border border-stone-100 dark:border-gray-800 shadow-sm">
  <div class="flex items-center justify-between mb-3">
    <h2 class="text-sm font-medium text-stone-500 dark:text-gray-400">{$t('ai.hardware_heading')}</h2>
    <button
      class="text-xs text-[var(--color-interactive-hover)] border border-[var(--color-interactive)] px-3 py-1.5 rounded-lg hover:bg-[var(--color-interactive-50)] disabled:opacity-50 min-h-[44px]"
      onclick={detect}
      disabled={loading}
    >
      {loading ? $t('ai.hardware_refreshing') : $t('ai.hardware_refresh')}
    </button>
  </div>

  {#if error}
    <p class="text-sm text-[var(--color-danger)] py-2">{error}</p>
  {:else if hardware}
    <div class="space-y-2 text-sm">
      <!-- GPU tier -->
      <div class="flex justify-between items-center">
        <span class="text-stone-600 dark:text-gray-300">{$t('ai.hardware_gpu')}</span>
        <span class="flex items-center gap-2 text-stone-800 dark:text-gray-100">
          <span class="inline-block w-2.5 h-2.5 rounded-full {tierDotClass(hardware.gpu_tier)}" aria-hidden="true"></span>
          {$t(tierLabelKey(hardware.gpu_tier))}
        </span>
      </div>

      <!-- VRAM -->
      <div class="flex justify-between">
        <span class="text-stone-600 dark:text-gray-300">{$t('ai.hardware_vram')}</span>
        <span class="text-stone-800 dark:text-gray-100">
          {hardware.vram_bytes > 0 ? formatModelSize(hardware.vram_bytes) : $t('ai.hardware_na')}
        </span>
      </div>

      <!-- Processor -->
      <div class="flex justify-between">
        <span class="text-stone-600 dark:text-gray-300">{$t('ai.hardware_processor')}</span>
        <span class="text-stone-800 dark:text-gray-100">{hardware.processor_label}</span>
      </div>

      <!-- Estimated speed -->
      <div class="flex justify-between">
        <span class="text-stone-600 dark:text-gray-300">{$t('ai.hardware_speed')}</span>
        <span class="text-stone-800 dark:text-gray-100">
          {$t('ai.hardware_speed_value', { values: { tokPerSec: hardware.estimated_tok_per_sec } })}
        </span>
      </div>
    </div>
  {:else if loading}
    <p class="text-sm text-stone-500 dark:text-gray-400 py-2">{$t('ai.hardware_refreshing')}</p>
  {/if}
</section>
