<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { t } from 'svelte-i18n';
  import { getActiveProfileName, getActiveProfileInfo, lockProfile, checkAiStatus, verifyAiStatus } from '$lib/api/profile';
  import type { ProfileInfo } from '$lib/types/profile';
  import { getUserPreference } from '$lib/api/ai';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { profile } from '$lib/stores/profile.svelte';
  import { ai } from '$lib/stores/ai.svelte';
  import { initI18n } from '$lib/i18n';
  import ErrorBanner from '$lib/components/ErrorBanner.svelte';
  import ProfileBar from '$lib/components/profile/ProfileBar.svelte';
  import TabBar from '$lib/components/navigation/TabBar.svelte';
  import HomeScreen from '$lib/components/home/HomeScreen.svelte';
  import ChatScreen from '$lib/components/chat/ChatScreen.svelte';
  import JournalScreen from '$lib/components/journal/JournalScreen.svelte';
  import MedicationListScreen from '$lib/components/medications/MedicationListScreen.svelte';
  import ReviewScreen from '$lib/components/review/ReviewScreen.svelte';
  import TimelineScreen from '$lib/components/timeline/TimelineScreen.svelte';
  import AppointmentScreen from '$lib/components/appointment/AppointmentScreen.svelte';
  import TransferScreen from '$lib/components/transfer/TransferScreen.svelte';
  import ImportScreen from '$lib/components/import/ImportScreen.svelte';
  import DocumentListScreen from '$lib/components/documents/DocumentListScreen.svelte';
  import DocumentDetailScreen from '$lib/components/documents/DocumentDetailScreen.svelte';
  import SettingsScreen from '$lib/components/settings/SettingsScreen.svelte';
  import PrivacyScreen from '$lib/components/settings/PrivacyScreen.svelte';
  import PairingScreen from '$lib/components/settings/PairingScreen.svelte';
  import AiSettingsScreen from '$lib/components/settings/AiSettingsScreen.svelte';
  import AiSetupWizard from '$lib/components/settings/AiSetupWizard.svelte';

  // Spec 45 [PU-02]: Active profile info for ProfileBar
  let activeProfileInfo: ProfileInfo | null = $state(null);

  // UX-L6-26 + ACC-L6-20: Fallback toast state
  let showFallbackToast = $state(false);
  let fallbackDismissTimer: ReturnType<typeof setTimeout> | null = null;

  // Detect when model source changes to Fallback
  $effect(() => {
    if (ai.activeModel?.source === 'Fallback' && !showFallbackToast) {
      showFallbackToast = true;
      fallbackDismissTimer = setTimeout(() => {
        showFallbackToast = false;
      }, 8000);
    }
    if (ai.activeModel?.source !== 'Fallback') {
      showFallbackToast = false;
    }
  });

  onMount(async () => {
    // I18N-04: Initialize language from user preference, then system locale, then 'en'
    const langPref = await getUserPreference('language').catch(() => null);
    initI18n(langPref);

    try {
      profile.name = await getActiveProfileName();
      activeProfileInfo = await getActiveProfileInfo();
    } catch {
      profile.name = 'Patient';
    }

    // S.2+S.5: Start unified AI status polling (immediate check + 30s verify + 60s poll)
    ai.startPolling(
      async () => {
        const status = await checkAiStatus();
        // S.5: Keep profile.aiStatus in sync for backward compatibility
        profile.aiStatus = status;

        // Spec 47 [OB-03]: AI setup auto-redirect REMOVED.
        // AI setup is now surfaced as a dismissible banner on HomeScreen
        // only after the user has imported their first document.
        return status;
      },
      async () => {
        const status = await verifyAiStatus();
        profile.aiStatus = status;
        return status;
      },
    );
  });

  onDestroy(() => {
    ai.stopPolling();
    if (fallbackDismissTimer) clearTimeout(fallbackDismissTimer);
  });
</script>

<!-- S.6: AI status error banner -->
{#if ai.statusError}
  <div class="px-4 pt-2">
    <ErrorBanner
      message={$t('ai.status_error')}
      severity="warning"
      guidance={ai.statusError}
      actionLabel={$t('settings.ai_settings')}
      onAction={() => navigation.navigate('ai-settings')}
      onDismiss={() => { ai.statusError = null; }}
    />
  </div>
{:else if ai.statusLevel !== 'unknown' && !ai.isAiAvailable}
  <!-- AI status banner when Ollama unavailable (IMP-015 + S.5) -->
  <button
    class="w-full bg-[var(--color-warning-50)] border-b border-[var(--color-warning-200)] px-4 py-2 text-sm text-[var(--color-warning-800)] flex items-center gap-2 hover:bg-[var(--color-warning-200)] cursor-pointer text-left"
    onclick={() => navigation.navigate('ai-settings')}
  >
    <span class="text-[var(--color-warning)]">!</span>
    <span class="flex-1">{ai.statusSummary || $t('ai.status_banner')}</span>
    <span class="text-[var(--color-warning-200)] text-xs">{$t('nav.settings')} &rarr;</span>
  </button>
{/if}

<!-- UX-L6-26 + ACC-L6-20: Fallback model toast notification -->
{#if showFallbackToast && ai.activeModel}
  <div
    class="fixed top-4 left-1/2 -translate-x-1/2 z-40 bg-[var(--color-warning-50)] border border-[var(--color-warning-200)] rounded-xl px-5 py-3 shadow-lg max-w-sm w-full mx-4 flex items-center gap-3 animate-slide-down"
    role="status"
    aria-live="polite"
  >
    <span class="text-[var(--color-warning)] flex-shrink-0" aria-hidden="true">!</span>
    <p class="text-sm text-[var(--color-warning-800)] flex-1">
      {$t('ai.fallback_toast', { values: { name: ai.activeModel.name } })}
    </p>
    <button
      class="text-[var(--color-warning-200)] hover:text-[var(--color-warning)] min-h-[44px] min-w-[44px] flex items-center justify-center flex-shrink-0"
      onclick={() => { showFallbackToast = false; }}
      aria-label={$t('common.dismiss')}
    >
      &times;
    </button>
  </div>
{/if}

<!-- Spec 45 [PU-02]: Profile indicator bar -->
{#if activeProfileInfo}
  <ProfileBar
    profile={activeProfileInfo}
    onSwitch={async () => { await lockProfile(); }}
    onLock={async () => { await lockProfile(); }}
  />
{/if}

<!-- ACC-00-12: Skip link for keyboard users -->
<a
  href="#main-content"
  class="sr-only focus:not-sr-only focus:absolute focus:top-2 focus:left-2 focus:z-50
         focus:bg-white focus:px-4 focus:py-2 focus:rounded-lg focus:shadow-lg
         focus:text-[var(--color-primary)] focus:font-medium"
  onclick={(e) => { e.preventDefault(); document.getElementById('main-content')?.focus(); }}
>
  {$t('nav.skip_to_content') ?? 'Skip to main content'}
</a>

<!-- ACC-00-07: Main landmark -->
<main id="main-content" tabindex="-1" class="pb-16 outline-none" aria-label={$t('nav.' + navigation.activeScreen) ?? navigation.activeScreen}>
  {#if navigation.activeScreen === 'home'}
    <HomeScreen />
  {:else if navigation.activeScreen === 'chat'}
    <ChatScreen
      initialConversationId={navigation.screenParams.conversationId}
      prefill={navigation.screenParams.prefill}
    />
  {:else if navigation.activeScreen === 'journal'}
    <JournalScreen />
  {:else if navigation.activeScreen === 'medications'}
    <MedicationListScreen />
  {:else if navigation.activeScreen === 'review' && navigation.screenParams.documentId}
    <ReviewScreen
      documentId={navigation.screenParams.documentId}
    />
  {:else if navigation.activeScreen === 'timeline'}
    <TimelineScreen />
  {:else if navigation.activeScreen === 'appointments'}
    <AppointmentScreen />
  {:else if navigation.activeScreen === 'transfer'}
    <TransferScreen />
  {:else if navigation.activeScreen === 'settings'}
    <SettingsScreen />
  {:else if navigation.activeScreen === 'privacy'}
    <PrivacyScreen />
  {:else if navigation.activeScreen === 'ai-settings'}
    <AiSettingsScreen />
  {:else if navigation.activeScreen === 'ai-setup'}
    <AiSetupWizard />
  {:else if navigation.activeScreen === 'pairing'}
    <PairingScreen />
  {:else if navigation.activeScreen === 'import'}
    <ImportScreen droppedFiles={navigation.screenParams.droppedFiles} />
  {:else if navigation.activeScreen === 'documents'}
    <DocumentListScreen />
  {:else if navigation.activeScreen === 'document-detail' && navigation.screenParams.documentId}
    <DocumentDetailScreen
      documentId={navigation.screenParams.documentId}
    />
  {:else}
    <!-- Fallback: return home -->
    <HomeScreen />
  {/if}
</main>

<!-- Tab bar (only shown on main screens) -->
{#if navigation.showTabBar}
  <TabBar />
{/if}
