<script lang="ts">
  import { onMount } from 'svelte';
  import { getActiveProfileName, checkAiStatus } from '$lib/api/profile';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { profile } from '$lib/stores/profile.svelte';
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

  onMount(async () => {
    try {
      profile.name = await getActiveProfileName();
    } catch {
      profile.name = 'Patient';
    }

    // Non-blocking AI status check (IMP-015)
    checkAiStatus()
      .then((status) => { profile.aiStatus = status; })
      .catch(() => { /* Silently ignore — will be null */ });
  });
</script>

<!-- AI status banner when Ollama unavailable (IMP-015) -->
{#if profile.aiStatus && !profile.isAiAvailable}
  <div class="bg-amber-50 border-b border-amber-200 px-4 py-2 text-sm text-amber-800 flex items-center gap-2">
    <span class="text-amber-500">!</span>
    <span>{profile.aiStatus.summary}</span>
  </div>
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
