<script lang="ts">
  import { t } from 'svelte-i18n';
  import { unlockProfile } from '$lib/api/profile';
  import type { ProfileInfo } from '$lib/types/profile';
  import Button from '$lib/components/ui/Button.svelte';
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
  <div class="self-start">
    <BackButton onclick={onBack} label={$t('common.back')} />
  </div>

  <Avatar name={profile.name} variant="user" size="lg" />

  <h2 class="text-2xl font-bold text-stone-800">{profile.name}</h2>

  {#if profile.password_hint}
    <p class="text-stone-500 text-sm">{$t('profile.hint_prefix')}{profile.password_hint}</p>
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
    <p class="text-[var(--color-danger)] text-sm">{error}</p>
  {/if}

  <Button variant="primary" fullWidth loading={loading} disabled={!password} onclick={handleUnlock}>
    {loading ? $t('common.unlocking') : $t('profile.unlock_button')}
  </Button>

  {#if attempts >= 3}
    <Button variant="ghost" onclick={onForgotPassword}>
      {$t('profile.forgot_password')}
    </Button>
  {/if}
</div>
