<script lang="ts">
  import { t } from 'svelte-i18n';
  import { recoverProfile } from '$lib/api/profile';
  import type { ProfileInfo } from '$lib/types/profile';
  import Button from '$lib/components/ui/Button.svelte';
  import BackButton from '$lib/components/ui/BackButton.svelte';

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
      error = $t('profile.password_mismatch');
      return;
    }
    if (newPassword.length < 6) {
      error = $t('profile.password_too_short');
      return;
    }

    loading = true;
    error = '';
    try {
      await recoverProfile(profile.id, phrase, newPassword);
      onRecovered();
    } catch (e) {
      error = $t('profile.recovery_failed');
    } finally {
      loading = false;
    }
  }
</script>

<div class="flex flex-col items-center justify-center min-h-screen px-8 gap-6 max-w-lg mx-auto">
  <div class="self-start">
    <BackButton onclick={onBack} label={$t('common.back')} />
  </div>

  <h2 class="text-2xl font-bold text-stone-800 dark:text-gray-100">{$t('profile.recover_heading', { values: { name: profile.name } })}</h2>
  <p class="text-stone-600 dark:text-gray-300 text-center">{$t('profile.recover_instructions')}</p>

  <div class="grid grid-cols-3 gap-2 w-full">
    {#each words as _, i}
      <label class="flex items-center gap-1">
        <span class="text-stone-500 dark:text-gray-400 text-sm w-5 text-right">{i + 1}.</span>
        <input
          type="text"
          bind:value={words[i]}
          class="w-full px-2 py-2 rounded border border-stone-300 dark:border-gray-600 bg-white dark:bg-gray-900 text-stone-700 dark:text-gray-200 font-mono min-h-[44px]
                 focus:border-[var(--color-primary)] focus:outline-none"
          autocomplete="off"
          autocapitalize="off"
        />
      </label>
    {/each}
  </div>

  <label class="w-full flex flex-col gap-1 mt-4">
    <span class="text-stone-600 dark:text-gray-300 text-sm font-medium">{$t('profile.new_password_label')}</span>
    <input type="password" bind:value={newPassword}
           placeholder={$t('profile.password_placeholder')}
           class="px-4 py-3 rounded-lg border border-stone-300 dark:border-gray-600 bg-white dark:bg-gray-900 text-stone-700 dark:text-gray-200 min-h-[44px]" />
  </label>
  <label class="w-full flex flex-col gap-1">
    <span class="text-stone-600 dark:text-gray-300 text-sm font-medium">{$t('profile.confirm_new_password_label')}</span>
    <input type="password" bind:value={confirmPassword}
           placeholder={$t('profile.confirm_password_placeholder')}
           class="px-4 py-3 rounded-lg border border-stone-300 dark:border-gray-600 bg-white dark:bg-gray-900 text-stone-700 dark:text-gray-200 min-h-[44px]" />
  </label>

  {#if error}
    <p class="text-[var(--color-danger)] text-sm">{error}</p>
  {/if}

  <Button variant="primary" fullWidth loading={loading} onclick={handleRecover}>
    {loading ? $t('common.recovering') : $t('profile.recover_button')}
  </Button>
</div>
