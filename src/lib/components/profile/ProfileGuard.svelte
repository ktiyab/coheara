<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  import { t } from 'svelte-i18n';
  import { invoke } from '@tauri-apps/api/core';
  import { isProfileActive, listProfiles } from '$lib/api/profile';
  import type { ProfileInfo, AppScreen } from '$lib/types/profile';
  import TrustScreen from './TrustScreen.svelte';
  import ProfileTypeChoice from './ProfileTypeChoice.svelte';
  import CreateProfile from './CreateProfile.svelte';
  import ProfilePicker from './ProfilePicker.svelte';
  import UnlockProfile from './UnlockProfile.svelte';
  import RecoveryPhraseDisplay from './RecoveryPhraseDisplay.svelte';
  import RecoverProfile from './RecoverProfile.svelte';
  import WelcomeTour from './WelcomeTour.svelte';
  import LoadingState from '$lib/components/ui/LoadingState.svelte';

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

  onMount(async () => {
    const active = await isProfileActive();
    if (active) {
      screen = 'app';
      return;
    }

    profiles = await listProfiles();
    if (profiles.length === 0) {
      screen = 'trust';
    } else if (profiles.length === 1) {
      selectedProfile = profiles[0];
      screen = 'unlock';
    } else {
      screen = 'picker';
    }
  });

  // Periodic inactivity check
  onMount(() => {
    const interval = setInterval(async () => {
      if (screen === 'app') {
        const locked = await invoke<boolean>('check_inactivity');
        if (locked) {
          screen = 'unlock';
        }
      }
    }, 30_000);
    return () => clearInterval(interval);
  });
</script>

{#if screen === 'loading'}
  <LoadingState message={$t('common.loading')} />
{:else if screen === 'trust'}
  <TrustScreen onContinue={() => screen = 'profile_type_choice'} />
{:else if screen === 'profile_type_choice'}
  <!-- Spec 45 [ON-02]: "For myself" vs "For someone I care for" -->
  <ProfileTypeChoice
    onSelect={(isCaregiver) => {
      isCaregiverPath = isCaregiver;
      screen = 'create';
    }}
  />
{:else if screen === 'create'}
  <CreateProfile
    {isCaregiverPath}
    onCreated={(result) => {
      recoveryWords = result.recovery_phrase;
      selectedProfile = result.profile;
      screen = 'recovery_display';
    }}
    onError={(err) => console.error(err)}
  />
{:else if screen === 'recovery_display'}
  <RecoveryPhraseDisplay
    words={recoveryWords}
    profileName={selectedProfile?.name}
    onConfirmed={() => { recoveryWords = []; screen = 'welcome_tour'; }}
  />
{:else if screen === 'welcome_tour'}
  <!-- Spec 45 [ON-02]: 3-slide welcome tour before home -->
  <WelcomeTour onComplete={() => screen = 'app'} />
{:else if screen === 'picker'}
  <ProfilePicker
    {profiles}
    onSelect={(p) => { selectedProfile = p; screen = 'unlock'; }}
    onCreateNew={() => screen = 'profile_type_choice'}
  />
{:else if screen === 'unlock' && selectedProfile}
  <UnlockProfile
    profile={selectedProfile}
    onUnlocked={() => screen = 'app'}
    onBack={() => { selectedProfile = null; screen = 'picker'; }}
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
