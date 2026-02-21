<!-- V16: Settings hub — grouped rows with inline controls (macOS / Raycast pattern). -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { t } from 'svelte-i18n';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { profile } from '$lib/stores/profile.svelte';
  import { lockProfile } from '$lib/api/profile';
  import { getUserPreference, setUserPreference } from '$lib/api/ai';
  import { triggerExtractionBatch } from '$lib/api/extraction';
  import { extraction } from '$lib/stores/extraction.svelte';
  import { isTauriEnv } from '$lib/utils/tauri';
  import LanguageSelector from './LanguageSelector.svelte';
  import { soundManager } from '$lib/utils/sound';
  import { theme, type Theme } from '$lib/stores/theme.svelte';
  import {
    BrainSolid, UserSolid, MobilePhoneSolid, LockSolid,
    PaletteSolid, VolumeUpSolid, InfoCircleSolid, SunSolid, MoonSolid,
    GlobeSolid, ChevronRightOutline, ArrowRightToBracketOutline,
    ClipboardCheckOutline, PlayOutline,
  } from 'flowbite-svelte-icons';
  import { Toggle, Range, Select } from 'flowbite-svelte';

  const APP_VERSION = '0.2.0';

  /** Shared row button class — focus-visible outline for keyboard a11y (LR-13). */
  const rowBtn = `w-full flex items-center gap-4 px-4 py-3 min-h-[52px] text-left
    hover:bg-stone-50 dark:hover:bg-gray-800 transition-colors first:rounded-t-xl last:rounded-b-xl
    focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--color-interactive)]`;

  let soundEnabled = $state(soundManager.isEnabled());
  let soundVolume = $state(soundManager.getVolume());

  type ThemeOption = { value: Theme; labelKey: string; Icon: typeof SunSolid };

  const themes: ThemeOption[] = [
    { value: 'light', labelKey: 'settings.theme_light', Icon: SunSolid },
    { value: 'dark', labelKey: 'settings.theme_dark', Icon: MoonSolid },
    { value: 'colorful', labelKey: 'settings.theme_colorful', Icon: PaletteSolid },
  ];

  function selectTheme(value: Theme) {
    theme.set(value);
  }

  function toggleSound() {
    soundEnabled = !soundEnabled;
    soundManager.setEnabled(soundEnabled);
  }

  function updateVolume(e: Event) {
    const target = e.target as HTMLInputElement;
    soundVolume = parseFloat(target.value);
    soundManager.setVolume(soundVolume);
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
  <header class="px-6 pt-6 pb-4">
    <h1 class="text-2xl font-bold text-stone-800 dark:text-gray-100">{$t('settings.heading')}</h1>
  </header>

  <div class="px-6 pb-6 space-y-6">

    <!-- ═══ Section: AI & Profile ═══ -->
    <section>
      <h2 class="text-xs font-semibold text-stone-400 dark:text-gray-500 uppercase tracking-wider px-1 mb-2">
        {$t('settings.section_ai_profile')}
      </h2>
      <div class="bg-white dark:bg-gray-900 rounded-xl border border-stone-100 dark:border-gray-800 shadow-sm divide-y divide-stone-100 dark:divide-gray-800">
        <!-- AI Settings → navigate (SC8-02: status badge) -->
        <button class={rowBtn} onclick={() => navigation.navigate('ai-settings')}>
          <BrainSolid class="w-5 h-5 text-stone-400 dark:text-gray-500 flex-shrink-0" />
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-stone-800 dark:text-gray-200">{$t('settings.hub_ai_title')}</p>
            <p class="text-xs text-stone-500 dark:text-gray-400">{$t('settings.hub_ai_description')}</p>
          </div>
          {#if profile.isAiAvailable}
            <span class="text-xs text-[var(--color-success)] font-medium flex-shrink-0">{$t('settings.ai_ready')}</span>
          {:else}
            <span class="text-xs text-[var(--color-warning-800)] flex-shrink-0">{$t('settings.ai_not_configured')}</span>
          {/if}
          <ChevronRightOutline class="w-4 h-4 text-stone-300 dark:text-gray-600 flex-shrink-0" />
        </button>

        <!-- Switch Profile → action (ST-12: no chevron — action, not navigation) -->
        <button class={rowBtn} onclick={async () => { await lockProfile(); }}>
          <UserSolid class="w-5 h-5 text-stone-400 dark:text-gray-500 flex-shrink-0" />
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-stone-800 dark:text-gray-200">{$t('settings.hub_switch_title')}</p>
            <p class="text-xs text-stone-500 dark:text-gray-400">{$t('settings.hub_switch_description')}</p>
          </div>
          <ArrowRightToBracketOutline class="w-4 h-4 text-stone-300 dark:text-gray-600 flex-shrink-0" />
        </button>
      </div>
    </section>

    <!-- ═══ Section: Preferences ═══ -->
    <section>
      <h2 class="text-xs font-semibold text-stone-400 dark:text-gray-500 uppercase tracking-wider px-1 mb-2">
        {$t('settings.section_preferences')}
      </h2>
      <div class="bg-white dark:bg-gray-900 rounded-xl border border-stone-100 dark:border-gray-800 shadow-sm divide-y divide-stone-100 dark:divide-gray-800">

        <!-- Language — inline pills (label clicks focus pill group) -->
        <div class="flex items-center gap-4 px-4 py-3 min-h-[52px]">
          <button
            class="flex items-center gap-4 flex-shrink-0 text-left cursor-pointer"
            onclick={(e) => { const rg = (e.currentTarget as HTMLElement).closest('div')?.querySelector('[role=radiogroup] button'); if (rg instanceof HTMLElement) rg.focus(); }}
          >
            <GlobeSolid class="w-5 h-5 text-stone-400 dark:text-gray-500 flex-shrink-0" />
            <span class="text-sm font-medium text-stone-800 dark:text-gray-200">{$t('settings.language_label')}</span>
          </button>
          <div class="flex-1 flex justify-end">
            <LanguageSelector />
          </div>
        </div>

        <!-- Theme — inline pills (label clicks focus pill group) -->
        <div class="flex items-center gap-4 px-4 py-3 min-h-[52px]">
          <button
            class="flex items-center gap-4 flex-shrink-0 text-left cursor-pointer"
            onclick={(e) => { const rg = (e.currentTarget as HTMLElement).closest('div')?.querySelector('[role=radiogroup] button'); if (rg instanceof HTMLElement) rg.focus(); }}
          >
            <PaletteSolid class="w-5 h-5 text-stone-400 dark:text-gray-500 flex-shrink-0" />
            <span class="text-sm font-medium text-stone-800 dark:text-gray-200">{$t('settings.theme_heading')}</span>
          </button>
          <div class="flex-1 flex justify-end">
            <div class="flex gap-1 rounded-lg bg-stone-100 dark:bg-gray-800 p-0.5" role="radiogroup" aria-label={$t('settings.theme_heading')}>
              {#each themes as opt (opt.value)}
                <button
                  role="radio"
                  aria-checked={theme.current === opt.value}
                  class="flex items-center gap-1.5 px-3 py-2 rounded-md text-sm font-medium min-h-[44px] transition-colors
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

        <!-- Sound — inline toggle + volume (icon+label clickable to toggle) -->
        <div class="flex flex-col gap-2 px-4 py-3 min-h-[52px]">
          <div class="flex items-center gap-4">
            <button class="flex items-center gap-4 flex-1 text-left cursor-pointer" onclick={toggleSound}>
              <VolumeUpSolid class="w-5 h-5 text-stone-400 dark:text-gray-500 flex-shrink-0" />
              <span class="text-sm font-medium text-stone-800 dark:text-gray-200">{$t('settings.sound_heading')}</span>
            </button>
            <Toggle checked={soundEnabled} color="primary" onchange={toggleSound} aria-label={$t('settings.sound_enabled')} />
          </div>
          {#if soundEnabled}
            <div class="flex items-center gap-3 pl-9">
              <Range
                value={soundVolume}
                min={0}
                max={1}
                step={0.1}
                size="md"
                oninput={updateVolume}
                aria-label={$t('settings.sound_volume')}
              />
              <span class="text-xs text-stone-500 dark:text-gray-400 w-8 text-right flex-shrink-0">{Math.round(soundVolume * 100)}%</span>
            </div>
          {/if}
        </div>
      </div>
    </section>

    <!-- ═══ Section: Extraction (LP-01) ═══ -->
    <section>
      <h2 class="text-xs font-semibold text-stone-400 dark:text-gray-500 uppercase tracking-wider px-1 mb-2">
        {$t('settings.section_extraction')}
      </h2>
      <div class="bg-white dark:bg-gray-900 rounded-xl border border-stone-100 dark:border-gray-800 shadow-sm divide-y divide-stone-100 dark:divide-gray-800">

        <!-- Enable extraction — toggle -->
        <div class="flex items-center gap-4 px-4 py-3 min-h-[52px]">
          <button class="flex items-center gap-4 flex-1 text-left cursor-pointer" onclick={toggleExtraction}>
            <ClipboardCheckOutline class="w-5 h-5 text-stone-400 dark:text-gray-500 flex-shrink-0" />
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
            <div class="flex-1 min-w-0 pl-9">
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
          <PlayOutline class="w-5 h-5 text-stone-400 dark:text-gray-500 flex-shrink-0" />
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
            <div class="flex-1 min-w-0 pl-9">
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
      <h2 class="text-xs font-semibold text-stone-400 dark:text-gray-500 uppercase tracking-wider px-1 mb-2">
        {$t('settings.section_privacy')}
      </h2>
      <div class="bg-white dark:bg-gray-900 rounded-xl border border-stone-100 dark:border-gray-800 shadow-sm divide-y divide-stone-100 dark:divide-gray-800">
        <!-- Privacy & Data → navigate -->
        <button class={rowBtn} onclick={() => navigation.navigate('privacy')}>
          <LockSolid class="w-5 h-5 text-stone-400 dark:text-gray-500 flex-shrink-0" />
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-stone-800 dark:text-gray-200">{$t('settings.hub_privacy_title')}</p>
            <p class="text-xs text-stone-500 dark:text-gray-400">{$t('settings.hub_privacy_description')}</p>
          </div>
          <ChevronRightOutline class="w-4 h-4 text-stone-300 dark:text-gray-600 flex-shrink-0" />
        </button>

        <!-- Paired Devices → navigate -->
        <button class={rowBtn} onclick={() => navigation.navigate('pairing')}>
          <MobilePhoneSolid class="w-5 h-5 text-stone-400 dark:text-gray-500 flex-shrink-0" />
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-stone-800 dark:text-gray-200">{$t('settings.hub_devices_title')}</p>
            <p class="text-xs text-stone-500 dark:text-gray-400">{$t('settings.hub_devices_description')}</p>
          </div>
          <ChevronRightOutline class="w-4 h-4 text-stone-300 dark:text-gray-600 flex-shrink-0" />
        </button>
      </div>
    </section>

    <!-- ═══ Section: About ═══ -->
    <section>
      <h2 class="text-xs font-semibold text-stone-400 dark:text-gray-500 uppercase tracking-wider px-1 mb-2">
        {$t('settings.section_about')}
      </h2>
      <div class="bg-white dark:bg-gray-900 rounded-xl border border-stone-100 dark:border-gray-800 shadow-sm">
        <div class="flex items-center gap-4 px-4 py-3 min-h-[52px]">
          <InfoCircleSolid class="w-5 h-5 text-stone-400 dark:text-gray-500 flex-shrink-0" />
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-stone-800 dark:text-gray-200">{$t('settings.hub_about_title')}</p>
            <p class="text-xs text-stone-500 dark:text-gray-400">{$t('settings.hub_about_version', { values: { version: APP_VERSION } })}</p>
          </div>
        </div>
      </div>
    </section>

  </div>
</div>
