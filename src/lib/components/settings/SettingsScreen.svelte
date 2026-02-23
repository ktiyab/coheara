<!-- V16 + AUDIT_01 §7: Settings hub — card sections with icon+title+desc headers. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { t } from 'svelte-i18n';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { profile } from '$lib/stores/profile.svelte';
  import { getUserPreference, setUserPreference } from '$lib/api/ai';
  import { triggerExtractionBatch } from '$lib/api/extraction';
  import { extraction } from '$lib/stores/extraction.svelte';
  import { isTauriEnv } from '$lib/utils/tauri';
  import LanguageSelector from './LanguageSelector.svelte';
  import { soundManager } from '$lib/utils/sound';
  import { theme, type Theme } from '$lib/stores/theme.svelte';
  import {
    BrainIcon, GroupIcon, PhoneIcon, LockIcon,
    PaletteIcon, VolumeIcon, InfoIcon, SunIcon, MoonIcon,
    GlobeIcon, ChevronRightIcon, ArrowForwardIcon,
    ClipboardIcon, PlayIcon, SettingsIcon,
  } from '$lib/components/icons/md';
  import { Toggle, Select } from 'flowbite-svelte';
  import type { Component } from 'svelte';

  const APP_VERSION = '0.2.0';

  /** Shared row button class — focus-visible outline for keyboard a11y (LR-13). */
  const rowBtn = `w-full flex items-center gap-4 px-4 py-3 min-h-[52px] text-left
    hover:bg-stone-50 dark:hover:bg-gray-800 transition-colors first:rounded-t-xl last:rounded-b-xl
    focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--color-interactive)]`;

  let soundEnabled = $state(soundManager.isEnabled());

  type ThemeOption = { value: Theme; labelKey: string; Icon: Component<{ class?: string }> };

  const themes: ThemeOption[] = [
    { value: 'light', labelKey: 'settings.theme_light', Icon: SunIcon },
    { value: 'dark', labelKey: 'settings.theme_dark', Icon: MoonIcon },
    { value: 'colorful', labelKey: 'settings.theme_colorful', Icon: PaletteIcon },
  ];

  function selectTheme(value: Theme) {
    theme.set(value);
  }

  function toggleSound() {
    soundEnabled = !soundEnabled;
    soundManager.setEnabled(soundEnabled);
  }

  // ── LP-01: Extraction settings ──
  let extractionEnabled = $state(true);
  let batchHour = $state('2');
  let batchRunning = $state(false);
  let batchResult = $state<string | null>(null);

  const hourOptions = Array.from({ length: 24 }, (_, i) => ({
    value: String(i),
    name: `${i === 0 ? '12' : i > 12 ? String(i - 12) : String(i)}:00 ${i < 12 ? 'AM' : 'PM'}`,
  }));

  onMount(async () => {
    if (!isTauriEnv()) return;
    const enabledPref = await getUserPreference('extraction_enabled').catch(() => null);
    if (enabledPref !== null) extractionEnabled = enabledPref !== 'false';
    const hourPref = await getUserPreference('batch_start_hour').catch(() => null);
    if (hourPref !== null) batchHour = hourPref;
  });

  async function toggleExtraction() {
    extractionEnabled = !extractionEnabled;
    await setUserPreference('extraction_enabled', String(extractionEnabled)).catch(() => {});
  }

  async function updateBatchHour(e: Event) {
    const target = e.target as HTMLSelectElement;
    batchHour = target.value;
    await setUserPreference('batch_start_hour', batchHour).catch(() => {});
  }

  async function runBatchNow() {
    batchRunning = true;
    batchResult = null;
    try {
      const result = await triggerExtractionBatch();
      batchResult = $t('settings.extraction_batch_result', {
        values: { processed: result.conversations_processed, extracted: result.items_extracted },
      });
      extraction.refresh().catch(() => {});
    } catch (e) {
      batchResult = e instanceof Error ? e.message : String(e);
    } finally {
      batchRunning = false;
    }
  }
</script>

<div class="flex flex-col bg-stone-50 dark:bg-gray-950 min-h-full">
  <header class="px-[var(--spacing-page-x)] pt-6 pb-4">
    <h1 class="text-2xl font-bold text-stone-800 dark:text-gray-100">{$t('settings.heading')}</h1>
  </header>

  <div class="px-[var(--spacing-page-x)] pb-6 space-y-4">

    <!-- ═══ Section: AI & Profile ═══ -->
    <section>
      <div class="bg-white dark:bg-gray-900 rounded-[var(--radius-card)] border border-stone-100 dark:border-gray-800 shadow-sm divide-y divide-stone-100 dark:divide-gray-800">
        <button class={rowBtn} onclick={() => navigation.navigate('ai-settings')}>
          <BrainIcon class="w-6 h-6 text-stone-400 dark:text-gray-500 flex-shrink-0" />
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-stone-800 dark:text-gray-200">{$t('settings.hub_ai_title')}</p>
            <p class="text-xs text-stone-500 dark:text-gray-400">{$t('settings.hub_ai_description')}</p>
          </div>
          {#if profile.isAiAvailable}
            <span class="text-xs text-[var(--color-success)] font-medium flex-shrink-0">{$t('settings.ai_ready')}</span>
          {:else}
            <span class="text-xs text-[var(--color-warning-800)] flex-shrink-0">{$t('settings.ai_not_configured')}</span>
          {/if}
          <ChevronRightIcon class="w-5 h-5 text-stone-300 dark:text-gray-600 flex-shrink-0" />
        </button>

        <button class={rowBtn} onclick={() => navigation.navigate('profiles')}>
          <GroupIcon class="w-6 h-6 text-stone-400 dark:text-gray-500 flex-shrink-0" />
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-stone-800 dark:text-gray-200">{$t('settings.hub_switch_title')}</p>
            <p class="text-xs text-stone-500 dark:text-gray-400">{$t('settings.hub_switch_description')}</p>
          </div>
          <ChevronRightIcon class="w-5 h-5 text-stone-300 dark:text-gray-600 flex-shrink-0" />
        </button>
      </div>
    </section>

    <!-- ═══ Section: Preferences ═══ -->
    <section>
      <div class="bg-white dark:bg-gray-900 rounded-[var(--radius-card)] border border-stone-100 dark:border-gray-800 shadow-sm divide-y divide-stone-100 dark:divide-gray-800">
        <!-- Language — inline pills -->
        <div class="flex items-center gap-4 px-4 py-3 min-h-[52px]">
          <button
            class="flex items-center gap-4 flex-shrink-0 text-left cursor-pointer"
            onclick={(e) => { const rg = (e.currentTarget as HTMLElement).closest('div')?.querySelector('[role=radiogroup] button'); if (rg instanceof HTMLElement) rg.focus(); }}
          >
            <GlobeIcon class="w-6 h-6 text-stone-400 dark:text-gray-500 flex-shrink-0" />
            <span class="text-sm font-medium text-stone-800 dark:text-gray-200">{$t('settings.language_label')}</span>
          </button>
          <div class="flex-1 flex justify-end">
            <LanguageSelector />
          </div>
        </div>

        <!-- Theme — inline pills (matched width with Language via flex-1) -->
        <div class="flex items-center gap-4 px-4 py-3 min-h-[52px]">
          <button
            class="flex items-center gap-4 flex-shrink-0 text-left cursor-pointer"
            onclick={(e) => { const rg = (e.currentTarget as HTMLElement).closest('div')?.querySelector('[role=radiogroup] button'); if (rg instanceof HTMLElement) rg.focus(); }}
          >
            <PaletteIcon class="w-6 h-6 text-stone-400 dark:text-gray-500 flex-shrink-0" />
            <span class="text-sm font-medium text-stone-800 dark:text-gray-200">{$t('settings.theme_heading')}</span>
          </button>
          <div class="flex-1 flex justify-end">
            <div class="flex gap-1 rounded-lg bg-stone-100 dark:bg-gray-800 p-0.5" role="radiogroup" aria-label={$t('settings.theme_heading')}>
              {#each themes as opt (opt.value)}
                <button
                  role="radio"
                  aria-checked={theme.current === opt.value}
                  class="flex-1 flex items-center justify-center gap-1.5 px-3 py-2 rounded-md text-sm font-medium min-h-[44px] transition-colors
                         focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-[var(--color-interactive)]
                         {theme.current === opt.value
                           ? 'bg-white dark:bg-gray-700 text-stone-800 dark:text-gray-100 shadow-sm'
                           : 'text-stone-500 dark:text-gray-400 hover:text-stone-700 dark:hover:text-gray-200'}"
                  onclick={() => selectTheme(opt.value)}
                >
                  <opt.Icon class="w-4 h-4" />
                  <span>{$t(opt.labelKey)}</span>
                </button>
              {/each}
            </div>
          </div>
        </div>

        <!-- Sound — toggle only (UA02-08) -->
        <div class="flex items-center gap-4 px-4 py-3 min-h-[52px]">
          <button class="flex items-center gap-4 flex-1 text-left cursor-pointer" onclick={toggleSound}>
            <VolumeIcon class="w-6 h-6 text-stone-400 dark:text-gray-500 flex-shrink-0" />
            <span class="text-sm font-medium text-stone-800 dark:text-gray-200">{$t('settings.sound_heading')}</span>
          </button>
          <Toggle checked={soundEnabled} color="primary" onchange={toggleSound} aria-label={$t('settings.sound_enabled')} />
        </div>
      </div>
    </section>

    <!-- ═══ Section: Extraction (LP-01) ═══ -->
    <section>
      <div class="bg-white dark:bg-gray-900 rounded-[var(--radius-card)] border border-stone-100 dark:border-gray-800 shadow-sm divide-y divide-stone-100 dark:divide-gray-800">
        <!-- Enable extraction — toggle -->
        <div class="flex items-center gap-4 px-4 py-3 min-h-[52px]">
          <button class="flex items-center gap-4 flex-1 text-left cursor-pointer" onclick={toggleExtraction}>
            <ClipboardIcon class="w-6 h-6 text-stone-400 dark:text-gray-500 flex-shrink-0" />
            <div class="flex-1 min-w-0">
              <span class="text-sm font-medium text-stone-800 dark:text-gray-200">{$t('settings.extraction_enabled')}</span>
              <p class="text-xs text-stone-500 dark:text-gray-400">{$t('settings.extraction_enabled_desc')}</p>
            </div>
          </button>
          <Toggle checked={extractionEnabled} color="primary" onchange={toggleExtraction} aria-label={$t('settings.extraction_enabled')} />
        </div>

        <!-- Batch hour — dropdown -->
        {#if extractionEnabled}
          <div class="flex items-center gap-4 px-4 py-3 min-h-[52px]">
            <div class="flex-1 min-w-0 pl-10">
              <span class="text-sm font-medium text-stone-800 dark:text-gray-200">{$t('settings.extraction_hour')}</span>
            </div>
            <Select
              items={hourOptions}
              value={batchHour}
              size="sm"
              class="w-32"
              onchange={updateBatchHour}
              aria-label={$t('settings.extraction_hour')}
            />
          </div>
        {/if}

        <!-- Manual trigger — action button -->
        <div class="flex items-center gap-4 px-4 py-3 min-h-[52px]">
          <PlayIcon class="w-6 h-6 text-stone-400 dark:text-gray-500 flex-shrink-0" />
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-stone-800 dark:text-gray-200">{$t('settings.extraction_run_now')}</p>
            {#if batchResult}
              <p class="text-xs text-stone-500 dark:text-gray-400">{batchResult}</p>
            {:else}
              <p class="text-xs text-stone-500 dark:text-gray-400">{$t('settings.extraction_run_now_desc')}</p>
            {/if}
          </div>
          <button
            class="px-3 py-1.5 text-xs font-medium rounded-lg bg-[var(--color-primary)] text-white
                   hover:opacity-90 transition-opacity disabled:opacity-50"
            onclick={runBatchNow}
            disabled={batchRunning}
          >
            {batchRunning ? '...' : $t('settings.extraction_run_btn')}
          </button>
        </div>

        <!-- Pending count (read-only) -->
        {#if extraction.count > 0}
          <div class="flex items-center gap-4 px-4 py-3 min-h-[52px]">
            <div class="flex-1 min-w-0 pl-10">
              <span class="text-sm text-stone-600 dark:text-gray-300">
                {$t('settings.extraction_pending', { values: { count: extraction.count } })}
              </span>
            </div>
            <button
              class="text-xs text-[var(--color-primary)] font-medium hover:underline"
              onclick={() => navigation.navigate('home')}
            >
              {$t('settings.extraction_view')}
            </button>
          </div>
        {/if}
      </div>
    </section>

    <!-- ═══ Section: Privacy & Devices ═══ -->
    <section>
      <div class="bg-white dark:bg-gray-900 rounded-[var(--radius-card)] border border-stone-100 dark:border-gray-800 shadow-sm divide-y divide-stone-100 dark:divide-gray-800">
        <button class={rowBtn} onclick={() => navigation.navigate('privacy')}>
          <LockIcon class="w-6 h-6 text-stone-400 dark:text-gray-500 flex-shrink-0" />
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-stone-800 dark:text-gray-200">{$t('settings.hub_privacy_title')}</p>
            <p class="text-xs text-stone-500 dark:text-gray-400">{$t('settings.hub_privacy_description')}</p>
          </div>
          <ChevronRightIcon class="w-5 h-5 text-stone-300 dark:text-gray-600 flex-shrink-0" />
        </button>

        <button class={rowBtn} onclick={() => navigation.navigate('pairing')}>
          <PhoneIcon class="w-6 h-6 text-stone-400 dark:text-gray-500 flex-shrink-0" />
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-stone-800 dark:text-gray-200">{$t('settings.hub_devices_title')}</p>
            <p class="text-xs text-stone-500 dark:text-gray-400">{$t('settings.hub_devices_description')}</p>
          </div>
          <ChevronRightIcon class="w-5 h-5 text-stone-300 dark:text-gray-600 flex-shrink-0" />
        </button>
      </div>
    </section>

    <!-- ═══ Section: About ═══ -->
    <section>
      <div class="bg-white dark:bg-gray-900 rounded-[var(--radius-card)] border border-stone-100 dark:border-gray-800 shadow-sm">
        <div class="flex items-center gap-4 px-4 py-3 min-h-[52px]">
          <InfoIcon class="w-6 h-6 text-stone-400 dark:text-gray-500 flex-shrink-0" />
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-stone-800 dark:text-gray-200">{$t('settings.hub_about_title')}</p>
            <p class="text-xs text-stone-500 dark:text-gray-400">{$t('settings.hub_about_version', { values: { version: APP_VERSION } })}</p>
          </div>
        </div>
      </div>
    </section>

  </div>
</div>
