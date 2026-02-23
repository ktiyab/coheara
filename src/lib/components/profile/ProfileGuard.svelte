<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  import { t } from 'svelte-i18n';
  import { invoke } from '@tauri-apps/api/core';
  import { isProfileActive, listProfiles, getActiveProfileInfo } from '$lib/api/profile';
  import { isTauriEnv } from '$lib/utils/tauri';
  import { onProfileSwitch } from '$lib/utils/session-events';
  import { profile } from '$lib/stores/profile.svelte';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { ai } from '$lib/stores/ai.svelte';
  import { extraction } from '$lib/stores/extraction.svelte';
  import type { ProfileInfo, AppScreen } from '$lib/types/profile';
  import TrustScreen from './TrustScreen.svelte';
  import ProfileTypeChoice from './ProfileTypeChoice.svelte';
  import CreateProfile from './CreateProfile.svelte';
  import ProfilePicker from './ProfilePicker.svelte';
  import UnlockProfile from './UnlockProfile.svelte';
  import LockScreen from './LockScreen.svelte';
  import RecoveryPhraseDisplay from './RecoveryPhraseDisplay.svelte';
  import RecoverProfile from './RecoverProfile.svelte';
  import WelcomeTour from './WelcomeTour.svelte';
  import LoadingState from '$lib/components/ui/LoadingState.svelte';
  import OnboardingShell from '$lib/components/ui/OnboardingShell.svelte';

  interface Props {
    children: Snippet;
  }
  let { children }: Props = $props();

  let screen = $state<AppScreen>('loading');
  let profiles = $state<ProfileInfo[]>([]);
  let selectedProfile = $state<ProfileInfo | null>(null);
  let recoveryWords = $state<string[]>([]);

  // Spec 45 [ON-02]: Track caregiver path selection from ProfileTypeChoice
  let isCaregiverPath = $state(false);

  /** F7: Redirect to auth screen — ALWAYS refreshes profile list from disk.
   *  Pass targetProfileId to pre-select a specific profile in the picker. */
  async function redirectToAuth(targetProfileId?: string) {
    // F7: Reset ALL stores to prevent cross-profile data leakage
    profile.reset();      // name, colorIndex, aiStatus, activeInfo
    navigation.reset();   // screenParams (documentIds), URL hash
    ai.reset();           // model status, timers, health
    extraction.reset();   // pending review items (medical data!)

    try { profiles = await listProfiles(); } catch { profiles = []; }
    if (profiles.length === 0) { screen = 'trust'; return; }

    // Pre-select: explicit target > cached > auto (single profile)
    if (targetProfileId) {
      selectedProfile = profiles.find(p => p.id === targetProfileId) ?? null;
    } else if (selectedProfile) {
      // Re-validate cached selection against fresh list
      selectedProfile = profiles.find(p => p.id === selectedProfile!.id) ?? null;
    }
    if (profiles.length === 1) selectedProfile = profiles[0];

    screen = 'picker'; // ALWAYS picker — LockScreen handles single/multi display
  }

  onMount(async () => {
    // Frontend-only dev mode: no Tauri backend, show app shell directly
    if (!isTauriEnv()) {
      screen = 'app';
      return;
    }

    try {
      const active = await isProfileActive();
      if (active) {
        // Store active profile for auto-lock redirect
        try {
          selectedProfile = await getActiveProfileInfo();
        } catch {
          profiles = await listProfiles().catch(() => []);
          if (profiles.length === 1) selectedProfile = profiles[0];
        }
        screen = 'app';
        return;
      }

      profiles = await listProfiles();
      if (profiles.length === 0) {
        screen = 'trust';
      } else if (profiles.length === 1) {
        selectedProfile = profiles[0];
        screen = 'picker'; // F7: Always use picker — LockScreen handles single/multi
      } else {
        screen = 'picker';
      }
    } catch {
      // Backend not ready — show first-launch screen rather than infinite loading
      screen = 'trust';
    }
  });

  // F7: Instant profile switch event listener — replaces 30s polling delay
  onMount(() => {
    if (!isTauriEnv()) return;
    return onProfileSwitch(async (detail) => {
      if (screen === 'app') await redirectToAuth(detail.targetProfileId);
    });
  });

  // Periodic inactivity check — only in Tauri mode
  onMount(() => {
    if (!isTauriEnv()) return;

    const interval = setInterval(async () => {
      if (screen === 'app') {
        try {
          const locked = await invoke<boolean>('check_inactivity');
          if (locked) await redirectToAuth();
        } catch {
          // Backend unavailable — skip this cycle
        }
      }
    }, 30_000);
    return () => clearInterval(interval);
  });

  // Frontend heartbeat + tab-refocus session check
  onMount(() => {
    if (!isTauriEnv()) return;

    // Heartbeat: reset backend inactivity timer on user interaction.
    // Listens to pointerdown (clicks/taps), keydown, pointermove (mouse/pen/touch
    // movement), and wheel (scroll wheel/trackpad). Debounced to 60s — timeout is
    // 900s so this gives ~15 resets per window.
    // Sources: MDN Pointer Events (10 event types), MDN Wheel Event (passive required).
    let lastHeartbeat = 0;
    const DEBOUNCE_MS = 60_000;

    function heartbeat() {
      if (screen !== 'app') return;
      const now = Date.now();
      if (now - lastHeartbeat < DEBOUNCE_MS) return;
      lastHeartbeat = now;
      invoke('update_activity').catch(() => {});
    }

    // Visibility: check session immediately when tab regains focus
    // (handles sleep/wake, alt-tab return after long absence).
    async function onVisibilityChange() {
      if (document.visibilityState !== 'visible' || screen !== 'app') return;
      try {
        const active = await isProfileActive();
        if (!active) await redirectToAuth();
      } catch {
        // Backend unavailable — skip
      }
    }

    window.addEventListener('pointerdown', heartbeat);
    window.addEventListener('keydown', heartbeat);
    window.addEventListener('pointermove', heartbeat);
    window.addEventListener('wheel', heartbeat, { passive: true });
    document.addEventListener('visibilitychange', onVisibilityChange);

    return () => {
      window.removeEventListener('pointerdown', heartbeat);
      window.removeEventListener('keydown', heartbeat);
      window.removeEventListener('pointermove', heartbeat);
      window.removeEventListener('wheel', heartbeat);
      document.removeEventListener('visibilitychange', onVisibilityChange);
    };
  });
</script>

{#if screen === 'loading'}
  <LoadingState message={$t('common.loading')} />
{:else if screen === 'trust'}
  <TrustScreen onContinue={() => screen = 'profile_type_choice'} />
{:else if screen === 'profile_type_choice'}
  <OnboardingShell currentStep={1} totalSteps={3} onBack={() => screen = 'trust'}>
    <ProfileTypeChoice
      onSelect={(isCaregiver) => {
        isCaregiverPath = isCaregiver;
        screen = 'create';
      }}
    />
  </OnboardingShell>
{:else if screen === 'create'}
  <OnboardingShell currentStep={2} totalSteps={3} onBack={() => screen = 'profile_type_choice'}>
    <CreateProfile
      {isCaregiverPath}
      onCreated={(result) => {
        recoveryWords = result.recovery_phrase;
        selectedProfile = result.profile;
        screen = 'recovery_display';
      }}
      onError={(err) => console.error(err)}
    />
  </OnboardingShell>
{:else if screen === 'recovery_display'}
  <OnboardingShell currentStep={3} totalSteps={3}>
    <RecoveryPhraseDisplay
      words={recoveryWords}
      profileName={selectedProfile?.name}
      onConfirmed={() => { recoveryWords = []; screen = 'welcome_tour'; }}
    />
  </OnboardingShell>
{:else if screen === 'welcome_tour'}
  <!-- Spec 45 [ON-02]: 3-slide welcome tour before home -->
  <WelcomeTour onComplete={() => screen = 'app'} />
{:else if screen === 'picker' || (screen === 'unlock' && selectedProfile)}
  <LockScreen
    {profiles}
    initialProfile={selectedProfile}
    onUnlocked={() => screen = 'app'}
    onForgotPassword={() => screen = 'recover'}
  />
{:else if screen === 'recover' && selectedProfile}
  <RecoverProfile
    profile={selectedProfile}
    onRecovered={() => screen = 'app'}
    onBack={() => screen = 'unlock'}
  />
{:else if screen === 'app'}
  {@render children()}
{/if}
