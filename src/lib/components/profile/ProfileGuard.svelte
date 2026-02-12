<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { isProfileActive, listProfiles } from '$lib/api/profile';
  import type { ProfileInfo, AppScreen } from '$lib/types/profile';
  import TrustScreen from './TrustScreen.svelte';
  import CreateProfile from './CreateProfile.svelte';
  import ProfilePicker from './ProfilePicker.svelte';
  import UnlockProfile from './UnlockProfile.svelte';
  import RecoveryPhraseDisplay from './RecoveryPhraseDisplay.svelte';
  import RecoverProfile from './RecoverProfile.svelte';

  interface Props {
    children: Snippet;
  }
  let { children }: Props = $props();

  let screen = $state<AppScreen>('loading');
  let profiles = $state<ProfileInfo[]>([]);
  let selectedProfile = $state<ProfileInfo | null>(null);
  let recoveryWords = $state<string[]>([]);

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
  <div class="flex items-center justify-center min-h-screen">
    <p class="text-stone-400">Loading...</p>
  </div>
{:else if screen === 'trust'}
  <TrustScreen onContinue={() => screen = 'create'} />
{:else if screen === 'create'}
  <CreateProfile
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
    onConfirmed={() => { recoveryWords = []; screen = 'app'; }}
  />
{:else if screen === 'picker'}
  <ProfilePicker
    {profiles}
    onSelect={(p) => { selectedProfile = p; screen = 'unlock'; }}
    onCreateNew={() => screen = 'create'}
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
