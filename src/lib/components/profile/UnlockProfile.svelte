<script lang="ts">
  import { tick } from 'svelte';
  import { t } from 'svelte-i18n';
  import { unlockProfile } from '$lib/api/profile';
  import type { ProfileInfo } from '$lib/types/profile';
  import BackButton from '$lib/components/ui/BackButton.svelte';
  import Avatar from '$lib/components/ui/Avatar.svelte';

  interface Props {
    profile: ProfileInfo;
    onUnlocked: () => void;
    onBack: () => void;
    onForgotPassword: () => void;
  }
  let { profile, onUnlocked, onBack, onForgotPassword }: Props = $props();

  let password = $state('');
  let error = $state('');
  let loading = $state(false);
  let attempts = $state(0);

  const inputClass = `w-full px-4 py-3 rounded-lg border border-stone-300 dark:border-gray-600 text-lg min-h-[44px]
    bg-white dark:bg-gray-900 text-stone-800 dark:text-gray-100
    placeholder:text-stone-300 dark:placeholder:text-gray-600
    focus:border-[var(--color-primary)] focus:outline-none`;

  async function handleUnlock() {
    if (!password) return;
    loading = true;
    error = '';
    await tick(); // Force DOM to render loading state before IPC
    try {
      await unlockProfile(profile.id, password);
      onUnlocked();
    } catch (e) {
      attempts += 1;
      if (attempts >= 3) {
        error = $t('profile.wrong_password_recovery');
      } else {
        error = $t('profile.wrong_password');
      }
    } finally {
      loading = false;
    }
  }
</script>

{#if loading}
  <div class="flex flex-col items-center justify-center min-h-screen px-8 gap-6 max-w-md mx-auto" role="status" aria-live="polite">
    <div class="w-12 h-12 border-4 border-[var(--color-interactive)]/30 border-t-[var(--color-interactive)] rounded-full animate-spin"></div>
    <p class="text-lg font-medium text-stone-700 dark:text-gray-200">
      {$t('common.unlocking')}
    </p>
  </div>
{:else}
  <div class="flex flex-col items-center justify-center min-h-screen px-8 gap-6 max-w-md mx-auto">
    <div class="self-start">
      <BackButton onclick={onBack} label={$t('common.back')} />
    </div>

    <Avatar name={profile.name} variant="user" size="lg" />

    <h2 class="text-2xl font-bold text-stone-800 dark:text-gray-100">{profile.name}</h2>

    {#if profile.password_hint}
      <p class="text-stone-500 dark:text-gray-400 text-sm">{$t('profile.hint_prefix')}{profile.password_hint}</p>
    {/if}

    <label class="w-full flex flex-col gap-1">
      <span class="text-stone-600 dark:text-gray-400 text-sm font-medium">{$t('profile.password_field')}</span>
      <input
        type="password"
        bind:value={password}
        placeholder={$t('profile.enter_password')}
        class={inputClass}
        autocomplete="current-password"
        onkeydown={(e) => { if (e.key === 'Enter') handleUnlock(); }}
      />
    </label>

    <!-- Error message — always holds space -->
    <p class="text-[var(--color-danger)] text-sm {error ? '' : 'invisible'}">{error || '\u00a0'}</p>

    <button
      class="w-full px-8 py-4 rounded-xl text-lg font-medium min-h-[44px] transition-colors
             bg-[var(--color-interactive)] text-white
             hover:bg-[var(--color-interactive-hover)]
             active:bg-[var(--color-interactive-active)]
             disabled:bg-stone-300 disabled:dark:bg-gray-700 disabled:text-stone-500 disabled:dark:text-gray-500 disabled:cursor-not-allowed"
      onclick={handleUnlock}
      disabled={!password}
    >
      {$t('profile.unlock_button')}
    </button>

    <!-- Forgot password link — always holds space -->
    <button
      class="text-sm min-h-[44px] transition-colors cursor-pointer
             text-stone-500 dark:text-gray-400 hover:text-stone-700 dark:hover:text-gray-200
             {attempts >= 3 ? '' : 'invisible'}"
      onclick={onForgotPassword}
      tabindex={attempts >= 3 ? 0 : -1}
    >
      {$t('profile.forgot_password')}
    </button>
  </div>
{/if}
