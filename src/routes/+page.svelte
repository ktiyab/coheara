<script lang="ts">
  import { onMount } from 'svelte';
  import { getActiveProfileName, checkAiStatus, type AiStatus } from '$lib/api/profile';
  import TabBar from '$lib/components/navigation/TabBar.svelte';
  import HomeScreen from '$lib/components/home/HomeScreen.svelte';
  import ChatScreen from '$lib/components/chat/ChatScreen.svelte';
  import JournalScreen from '$lib/components/journal/JournalScreen.svelte';
  import MedicationListScreen from '$lib/components/medications/MedicationListScreen.svelte';
  import ReviewScreen from '$lib/components/review/ReviewScreen.svelte';
  import TimelineScreen from '$lib/components/timeline/TimelineScreen.svelte';
  import AppointmentScreen from '$lib/components/appointment/AppointmentScreen.svelte';
  import TransferScreen from '$lib/components/transfer/TransferScreen.svelte';
  import PrivacyScreen from '$lib/components/settings/PrivacyScreen.svelte';
  import PairingScreen from '$lib/components/settings/PairingScreen.svelte';

  // ── State ──────────────────────────────────────────────
  let activeScreen = $state('home');
  let profileName = $state('');
  let aiStatus = $state<AiStatus | null>(null);

  // Screen-specific params (e.g., documentId for review)
  let screenParams = $state<Record<string, string>>({});

  // Navigation history for back buttons
  let previousScreen = $state('home');

  onMount(async () => {
    try {
      profileName = await getActiveProfileName();
    } catch {
      profileName = 'Patient';
    }

    // Non-blocking AI status check (IMP-015)
    checkAiStatus()
      .then((status) => { aiStatus = status; })
      .catch(() => { /* Silently ignore — will be null */ });
  });

  // ── Navigation handler ─────────────────────────────────
  function navigate(screen: string, params?: Record<string, string>) {
    previousScreen = activeScreen;
    screenParams = params ?? {};
    activeScreen = screen;
  }

  // ── Tab mapping ────────────────────────────────────────
  // Screens that show the tab bar
  const TAB_SCREENS = new Set([
    'home', 'chat', 'journal', 'medications',
    'documents', 'timeline', 'appointments', 'settings',
  ]);

  // Map screen to tab id
  function activeTab(): string {
    if (['home', 'chat', 'journal', 'medications'].includes(activeScreen)) {
      return activeScreen;
    }
    if (['documents', 'timeline', 'appointments', 'settings'].includes(activeScreen)) {
      return 'more';
    }
    return '';
  }
</script>

<!-- AI status banner when Ollama unavailable (IMP-015) -->
{#if aiStatus && !aiStatus.ollama_available}
  <div class="bg-amber-50 border-b border-amber-200 px-4 py-2 text-sm text-amber-800 flex items-center gap-2">
    <span class="text-amber-500">!</span>
    <span>{aiStatus.summary}</span>
  </div>
{/if}

<!-- Screen content -->
<div class="pb-16">
  {#if activeScreen === 'home'}
    <HomeScreen
      {profileName}
      onNavigate={navigate}
    />
  {:else if activeScreen === 'chat'}
    <ChatScreen
      {profileName}
      onNavigate={navigate}
      initialConversationId={screenParams.conversationId}
    />
  {:else if activeScreen === 'journal'}
    <JournalScreen onNavigate={navigate} />
  {:else if activeScreen === 'medications'}
    <MedicationListScreen onNavigate={navigate} />
  {:else if activeScreen === 'review' && screenParams.documentId}
    <ReviewScreen
      documentId={screenParams.documentId}
      onBack={() => navigate(previousScreen)}
      onNavigate={navigate}
    />
  {:else if activeScreen === 'timeline'}
    <TimelineScreen onNavigate={navigate} />
  {:else if activeScreen === 'appointments'}
    <AppointmentScreen onNavigate={navigate} />
  {:else if activeScreen === 'transfer'}
    <TransferScreen
      onComplete={() => navigate('home')}
      onCancel={() => navigate(previousScreen)}
    />
  {:else if activeScreen === 'settings'}
    <PrivacyScreen {profileName} onNavigate={navigate} />
  {:else if activeScreen === 'pairing'}
    <PairingScreen />
  {:else if activeScreen === 'import'}
    <!-- Import redirects to transfer screen (WiFi document transfer) -->
    <TransferScreen
      onComplete={() => navigate('home')}
      onCancel={() => navigate(previousScreen)}
    />
  {:else}
    <!-- Fallback: return home -->
    <HomeScreen
      {profileName}
      onNavigate={navigate}
    />
  {/if}
</div>

<!-- Tab bar (only shown on main screens) -->
{#if TAB_SCREENS.has(activeScreen)}
  <TabBar
    activeTab={activeTab()}
    onNavigate={(tab) => navigate(tab)}
  />
{/if}
