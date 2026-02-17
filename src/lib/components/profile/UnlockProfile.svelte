<script lang="ts">
  import { t } from 'svelte-i18n';
  import { unlockProfile } from '$lib/api/profile';
  import type { ProfileInfo } from '$lib/types/profile';

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

  async function handleUnlock() {
    if (!password) return;
    loading = true;
    error = '';

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

<div class="flex flex-col items-center justify-center min-h-screen px-8 gap-6 max-w-md mx-auto">
  <button
    class="self-start text-stone-400 hover:text-stone-600 min-h-[44px] min-w-[44px]"
    onclick={onBack}
    aria-label={$t('profile.back_to_list')}
  >
    &larr; {$t('common.back')}
  </button>

  <div class="w-16 h-16 rounded-full bg-stone-200 flex items-center justify-center
              text-stone-600 text-2xl font-bold">
    {profile.name.charAt(0).toUpperCase()}
  </div>

  <h2 class="text-2xl font-bold text-stone-800">{profile.name}</h2>

  {#if profile.password_hint}
    <p class="text-stone-400 text-sm">{$t('profile.hint_prefix')}{profile.password_hint}</p>
  {/if}

  <label class="w-full flex flex-col gap-1">
    <span class="text-stone-600 text-sm font-medium">{$t('profile.password_field')}</span>
    <input
      type="password"
      bind:value={password}
      placeholder={$t('profile.enter_password')}
      class="px-4 py-3 rounded-lg border border-stone-300 text-lg min-h-[44px]
             focus:border-[var(--color-primary)] focus:outline-none"
      autocomplete="current-password"
      onkeydown={(e) => { if (e.key === 'Enter') handleUnlock(); }}
    />
  </label>

  {#if error}
    <p class="text-red-600 text-sm">{error}</p>
  {/if}

  <button
    class="w-full px-8 py-4 bg-[var(--color-primary)] text-white rounded-xl text-lg
           font-medium hover:brightness-110 disabled:opacity-50 min-h-[44px]"
    onclick={handleUnlock}
    disabled={loading || !password}
  >
    {loading ? $t('common.unlocking') : $t('profile.unlock_button')}
  </button>

  {#if attempts >= 3}
    <button
      class="text-[var(--color-primary)] text-sm underline min-h-[44px]"
      onclick={onForgotPassword}
    >
      {$t('profile.forgot_password')}
    </button>
  {/if}
</div>
