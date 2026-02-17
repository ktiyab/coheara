<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { t } from 'svelte-i18n';
  import { getActiveProfileName, checkAiStatus, verifyAiStatus } from '$lib/api/profile';
  import { getActiveModel, getUserPreference } from '$lib/api/ai';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { profile } from '$lib/stores/profile.svelte';
  import { ai } from '$lib/stores/ai.svelte';
  import { initI18n } from '$lib/i18n';
  import ErrorBanner from '$lib/components/ErrorBanner.svelte';
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
  import PrivacyScreen from '$lib/components/settings/PrivacyScreen.svelte';
  import PairingScreen from '$lib/components/settings/PairingScreen.svelte';
  import AiSettingsScreen from '$lib/components/settings/AiSettingsScreen.svelte';
  import AiSetupWizard from '$lib/components/settings/AiSetupWizard.svelte';

  onMount(async () => {
    // I18N-04: Initialize language from user preference, then system locale, then 'en'
    const langPref = await getUserPreference('language').catch(() => null);
    initI18n(langPref);

    try {
      profile.name = await getActiveProfileName();
    } catch {
      profile.name = 'Patient';
    }

    // S.2+S.5: Start unified AI status polling (immediate check + 30s verify + 60s poll)
    ai.startPolling(
      async () => {
        const status = await checkAiStatus();
        // S.5: Keep profile.aiStatus in sync for backward compatibility
        profile.aiStatus = status;

        // L6-03: First-run AI setup wizard detection
        if (!status.ollama_available || !status.active_model) {
          const [activeModel, dismissed] = await Promise.all([
            getActiveModel().catch(() => null),
            getUserPreference('dismissed_ai_setup').catch(() => null),
          ]);
          if (!activeModel && dismissed !== 'true' && navigation.activeScreen === 'home') {
            navigation.navigate('ai-setup');
          }
        }
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
    class="w-full bg-amber-50 border-b border-amber-200 px-4 py-2 text-sm text-amber-800 flex items-center gap-2 hover:bg-amber-100 cursor-pointer text-left"
    onclick={() => navigation.navigate('ai-settings')}
  >
    <span class="text-amber-500">!</span>
    <span class="flex-1">{ai.statusSummary || $t('ai.status_banner')}</span>
    <span class="text-amber-400 text-xs">{$t('nav.settings')} &rarr;</span>
  </button>
{/if}

<!-- Screen content -->
<div class="pb-16">
  {#if navigation.activeScreen === 'home'}
    <HomeScreen />
  {:else if navigation.activeScreen === 'chat'}
    <ChatScreen
      initialConversationId={navigation.screenParams.conversationId}
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
    <PrivacyScreen />
  {:else if navigation.activeScreen === 'ai-settings'}
    <AiSettingsScreen />
  {:else if navigation.activeScreen === 'ai-setup'}
    <AiSetupWizard />
  {:else if navigation.activeScreen === 'pairing'}
    <PairingScreen />
  {:else if navigation.activeScreen === 'import'}
    <ImportScreen />
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
</div>

<!-- Tab bar (only shown on main screens) -->
{#if navigation.showTabBar}
  <TabBar />
{/if}
