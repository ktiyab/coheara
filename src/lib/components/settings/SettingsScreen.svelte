<!-- ST2: Settings hub — card navigation to sub-screens, ordered by trust gradient (CP6). -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { profile } from '$lib/stores/profile.svelte';
  import { lockProfile } from '$lib/api/profile';
  import Card from '$lib/components/ui/Card.svelte';
  import LanguageSelector from './LanguageSelector.svelte';
  import { soundManager } from '$lib/utils/sound';

  const APP_VERSION = '0.2.0';

  let soundEnabled = $state(soundManager.isEnabled());
  let soundVolume = $state(soundManager.getVolume());

  function toggleSound() {
    soundEnabled = !soundEnabled;
    soundManager.setEnabled(soundEnabled);
  }

  function updateVolume(e: Event) {
    const target = e.target as HTMLInputElement;
    soundVolume = parseFloat(target.value);
    soundManager.setVolume(soundVolume);
  }
</script>

<div class="flex flex-col min-h-screen pb-20 bg-stone-50">
  <header class="px-6 pt-6 pb-4">
    <h1 class="text-2xl font-bold text-stone-800">{$t('settings.heading')}</h1>
  </header>

  <div class="px-6 space-y-3">
    <!-- AI & Model Settings -->
    <Card onclick={() => navigation.navigate('ai-settings')}>
      <div class="flex items-center gap-4">
        <span class="text-2xl" aria-hidden="true">&#x1F916;</span>
        <div class="flex-1 min-w-0">
          <p class="text-sm font-semibold text-stone-800">{$t('settings.hub_ai_title')}</p>
          <p class="text-xs text-stone-500">{$t('settings.hub_ai_description')}</p>
        </div>
        {#if profile.isAiAvailable}
          <span class="text-xs text-[var(--color-success)] font-medium">{$t('settings.ai_ready')}</span>
        {:else}
          <span class="text-xs text-[var(--color-warning-800)]">{$t('settings.ai_not_configured')}</span>
        {/if}
      </div>
    </Card>

    <!-- Spec 45 [PU-09]: Switch Profile card -->
    <Card onclick={async () => { await lockProfile(); }}>
      <div class="flex items-center gap-4">
        <span class="text-2xl" aria-hidden="true">&#x1F464;</span>
        <div class="flex-1 min-w-0">
          <p class="text-sm font-semibold text-stone-800">{$t('settings.hub_switch_title')}</p>
          <p class="text-xs text-stone-500">{$t('settings.hub_switch_description')}</p>
        </div>
      </div>
    </Card>

    <!-- Paired Devices -->
    <Card onclick={() => navigation.navigate('pairing')}>
      <div class="flex items-center gap-4">
        <span class="text-2xl" aria-hidden="true">&#x1F4F1;</span>
        <div class="flex-1 min-w-0">
          <p class="text-sm font-semibold text-stone-800">{$t('settings.hub_devices_title')}</p>
          <p class="text-xs text-stone-500">{$t('settings.hub_devices_description')}</p>
        </div>
      </div>
    </Card>

    <!-- Privacy & Data -->
    <Card onclick={() => navigation.navigate('privacy')}>
      <div class="flex items-center gap-4">
        <span class="text-2xl" aria-hidden="true">&#x1F512;</span>
        <div class="flex-1 min-w-0">
          <p class="text-sm font-semibold text-stone-800">{$t('settings.hub_privacy_title')}</p>
          <p class="text-xs text-stone-500">{$t('settings.hub_privacy_description')}</p>
        </div>
      </div>
    </Card>

    <!-- Language (inline) -->
    <section class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
      <LanguageSelector />
    </section>

    <!-- Spec 50 [NF-02]: Sound & Notifications (inline) -->
    <section class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
      <div class="flex items-center gap-4 mb-3">
        <span class="text-2xl" aria-hidden="true">&#x1F50A;</span>
        <p class="text-sm font-semibold text-stone-800">{$t('settings.sound_heading')}</p>
      </div>

      <div class="space-y-3">
        <!-- Mute toggle -->
        <label class="flex items-center justify-between cursor-pointer">
          <span class="text-sm text-stone-700">{$t('settings.sound_enabled')}</span>
          <button
            role="switch"
            aria-checked={soundEnabled}
            class="relative w-11 h-6 rounded-full transition-colors
                   {soundEnabled ? 'bg-[var(--color-primary)]' : 'bg-stone-300'}"
            onclick={toggleSound}
          >
            <span
              class="absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full shadow transition-transform
                     {soundEnabled ? 'translate-x-5' : ''}"
            ></span>
          </button>
        </label>

        <!-- Volume slider -->
        {#if soundEnabled}
          <div class="flex items-center gap-3">
            <span class="text-xs text-stone-500 w-12">{$t('settings.sound_volume')}</span>
            <input
              type="range"
              min="0"
              max="1"
              step="0.1"
              value={soundVolume}
              oninput={updateVolume}
              class="flex-1 accent-[var(--color-primary)]"
              aria-label={$t('settings.sound_volume')}
            />
            <span class="text-xs text-stone-500 w-8 text-right">{Math.round(soundVolume * 100)}%</span>
          </div>
        {/if}
      </div>
    </section>

    <!-- About Coheara -->
    <section class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
      <div class="flex items-center gap-4">
        <span class="text-2xl" aria-hidden="true">&#x2139;&#xFE0F;</span>
        <div>
          <p class="text-sm font-semibold text-stone-800">{$t('settings.hub_about_title')}</p>
          <p class="text-xs text-stone-500">{$t('settings.hub_about_version', { values: { version: APP_VERSION } })}</p>
        </div>
      </div>
    </section>
  </div>
</div>
