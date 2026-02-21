<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { t } from 'svelte-i18n';
  import { getActiveProfileName, getActiveProfileInfo, checkAiStatus, verifyAiStatus } from '$lib/api/profile';
  import type { ProfileInfo } from '$lib/types/profile';
  import { locale } from 'svelte-i18n';
  import { getUserPreference } from '$lib/api/ai';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { profile } from '$lib/stores/profile.svelte';
  import { ai } from '$lib/stores/ai.svelte';
  import { extraction } from '$lib/stores/extraction.svelte';
  import { isTauriEnv } from '$lib/utils/tauri';
  import ErrorBanner from '$lib/components/ErrorBanner.svelte';
  import AppShell from '$lib/components/navigation/AppShell.svelte';
  import HomeScreen from '$lib/components/home/HomeScreen.svelte';
  import ChatScreen from '$lib/components/chat/ChatScreen.svelte';
  import ReviewScreen from '$lib/components/review/ReviewScreen.svelte';
  import TimelineScreen from '$lib/components/timeline/TimelineScreen.svelte';
  import TransferScreen from '$lib/components/transfer/TransferScreen.svelte';
  import ImportScreen from '$lib/components/import/ImportScreen.svelte';
  import DocumentListScreen from '$lib/components/documents/DocumentListScreen.svelte';
  import DocumentDetailScreen from '$lib/components/documents/DocumentDetailScreen.svelte';
  import SettingsScreen from '$lib/components/settings/SettingsScreen.svelte';
  import PrivacyScreen from '$lib/components/settings/PrivacyScreen.svelte';
  import PairingScreen from '$lib/components/settings/PairingScreen.svelte';
  import AiSettingsScreen from '$lib/components/settings/AiSettingsScreen.svelte';
  import AiSetupWizard from '$lib/components/settings/AiSetupWizard.svelte';
  import { ArrowRightOutline } from 'flowbite-svelte-icons';

  // Spec 45 [PU-02]: Active profile info (used by sidebar avatar)
  let activeProfileInfo: ProfileInfo | null = $state(null);

  onMount(async () => {
    // I18N-04: Upgrade locale from saved user preference (i18n already initialized in +layout.svelte)
    if (isTauriEnv()) {
      const langPref = await getUserPreference('language').catch(() => null);
      if (langPref) {
        locale.set(langPref);
      }
    }

    try {
      profile.name = await getActiveProfileName();
      activeProfileInfo = await getActiveProfileInfo();
      profile.colorIndex = activeProfileInfo?.color_index ?? null;
    } catch {
      profile.name = 'Patient';
    }

    // LP-01: Listen for batch extraction progress events
    extraction.startListening();

    // S.2+S.5: One-shot AI status check (immediate baseline + 30s verify)
    if (isTauriEnv()) {
      ai.startupCheck(
        async () => {
          const status = await checkAiStatus();
          profile.aiStatus = status;
          return status;
        },
        async () => {
          const status = await verifyAiStatus();
          profile.aiStatus = status;
          return status;
        },
      );
    }
  });

  onDestroy(() => {
    ai.cleanup();
    extraction.stopListening();
  });
</script>

<!-- S.6: AI status error banner (floats above AppShell) -->
{#if ai.statusError}
  <div class="fixed top-0 left-0 right-0 z-50 px-4 pt-2">
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
  <button
    class="fixed top-0 left-0 right-0 z-50 w-full bg-[var(--color-warning-50)] border-b border-[var(--color-warning-200)] px-4 py-2 text-sm text-[var(--color-warning-800)] flex items-center gap-2 hover:bg-[var(--color-warning-200)] cursor-pointer text-left"
    onclick={() => navigation.navigate('ai-settings')}
  >
    <span class="text-[var(--color-warning)]">!</span>
    <span class="flex-1">{ai.statusSummary || $t('ai.status_banner')}</span>
    <span class="text-[var(--color-warning-200)] text-xs flex items-center gap-1">{$t('nav.settings')} <ArrowRightOutline class="w-3 h-3" /></span>
  </button>
{/if}

<!-- D6: AppShell wraps sidebar + header + content -->
<AppShell>
  {#if navigation.activeScreen === 'home'}
    <HomeScreen />
  {:else if navigation.activeScreen === 'chat'}
    <ChatScreen
      initialConversationId={navigation.screenParams.conversationId}
      prefill={navigation.screenParams.prefill}
    />
  {:else if navigation.activeScreen === 'review' && navigation.screenParams.documentId}
    <ReviewScreen
      documentId={navigation.screenParams.documentId}
    />
  {:else if navigation.activeScreen === 'timeline'}
    <TimelineScreen />
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
    <HomeScreen />
  {/if}
</AppShell>
