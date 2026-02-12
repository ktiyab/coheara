<script lang="ts">
  import { recoverProfile } from '$lib/api/profile';
  import type { ProfileInfo } from '$lib/types/profile';

  interface Props {
    profile: ProfileInfo;
    onRecovered: () => void;
    onBack: () => void;
  }
  let { profile, onRecovered, onBack }: Props = $props();

  let words = $state(Array(12).fill(''));
  let newPassword = $state('');
  let confirmPassword = $state('');
  let error = $state('');
  let loading = $state(false);

  async function handleRecover() {
    const phrase = words.map((w: string) => w.trim().toLowerCase()).join(' ');
    if (newPassword !== confirmPassword) {
      error = 'Passwords do not match';
      return;
    }
    if (newPassword.length < 6) {
      error = 'Password must be at least 6 characters';
      return;
    }

    loading = true;
    error = '';
    try {
      await recoverProfile(profile.id, phrase, newPassword);
      onRecovered();
    } catch (e) {
      error = 'Recovery failed. Please check your words and try again.';
    } finally {
      loading = false;
    }
  }
</script>

<div class="flex flex-col items-center justify-center min-h-screen px-8 gap-6 max-w-lg mx-auto">
  <button class="self-start text-stone-400 hover:text-stone-600 min-h-[44px]" onclick={onBack}>
    &larr; Back
  </button>

  <h2 class="text-2xl font-bold text-stone-800">Recover {profile.name}'s profile</h2>
  <p class="text-stone-600 text-center">Enter your 12 recovery words in order.</p>

  <div class="grid grid-cols-3 gap-2 w-full">
    {#each words as _, i}
      <label class="flex items-center gap-1">
        <span class="text-stone-400 text-sm w-5 text-right">{i + 1}.</span>
        <input
          type="text"
          bind:value={words[i]}
          class="w-full px-2 py-2 rounded border border-stone-300 font-mono min-h-[44px]
                 focus:border-[var(--color-primary)] focus:outline-none"
          autocomplete="off"
          autocapitalize="off"
        />
      </label>
    {/each}
  </div>

  <label class="w-full flex flex-col gap-1 mt-4">
    <span class="text-stone-600 text-sm font-medium">New password</span>
    <input type="password" bind:value={newPassword}
           class="px-4 py-3 rounded-lg border border-stone-300 min-h-[44px]" />
  </label>
  <label class="w-full flex flex-col gap-1">
    <span class="text-stone-600 text-sm font-medium">Confirm new password</span>
    <input type="password" bind:value={confirmPassword}
           class="px-4 py-3 rounded-lg border border-stone-300 min-h-[44px]" />
  </label>

  {#if error}
    <p class="text-red-600 text-sm">{error}</p>
  {/if}

  <button
    class="w-full px-8 py-4 bg-[var(--color-primary)] text-white rounded-xl text-lg
           font-medium hover:brightness-110 disabled:opacity-50 min-h-[44px]"
    onclick={handleRecover}
    disabled={loading}
  >
    {loading ? 'Recovering...' : 'Recover and set new password'}
  </button>
</div>
