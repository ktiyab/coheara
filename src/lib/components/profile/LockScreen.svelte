<!-- UA02-03: Unified lock screen — Windows/iOS-style profile picker + password.
     No back arrow. All profiles shown. Selected profile expands with password field.
     Pattern: Windows 11 lock screen / iOS user switch. -->
<script lang="ts">
  import { tick } from 'svelte';
  import { t } from 'svelte-i18n';
  import { unlockProfile } from '$lib/api/profile';
  import { PROFILE_COLORS, type ProfileInfo } from '$lib/types/profile';
  import Avatar from '$lib/components/ui/Avatar.svelte';

  interface Props {
    profiles: ProfileInfo[];
    /** Pre-selected profile (e.g. from auto-lock — remembers who was active). */
    initialProfile?: ProfileInfo | null;
    onUnlocked: () => void;
    onForgotPassword: () => void;
  }
  let { profiles, initialProfile = null, onUnlocked, onForgotPassword }: Props = $props();

  let selectedProfile = $state<ProfileInfo | null>(initialProfile ?? (profiles.length === 1 ? profiles[0] : null));
  let password = $state('');
  let error = $state('');
  let loading = $state(false);
  let attempts = $state(0);
  let passwordInput: HTMLInputElement | undefined = $state(undefined);

  function getColor(profile: ProfileInfo): string {
    return profile.color_index != null
      ? PROFILE_COLORS[profile.color_index % PROFILE_COLORS.length]
      : PROFILE_COLORS[0];
  }

  async function selectProfile(profile: ProfileInfo) {
    selectedProfile = profile;
    password = '';
    error = '';
    attempts = 0;
    await tick();
    passwordInput?.focus();
  }

  async function handleUnlock() {
    if (!password || !selectedProfile) return;
    loading = true;
    error = '';
    await tick();
    try {
      await unlockProfile(selectedProfile.id, password);
      onUnlocked();
    } catch {
      attempts += 1;
      error = attempts >= 3
        ? $t('profile.wrong_password_recovery')
        : $t('profile.wrong_password');
    } finally {
      loading = false;
    }
  }

  const inputClass = `w-full px-4 py-3 rounded-lg border border-stone-300 dark:border-gray-600 text-base min-h-[44px]
    bg-white dark:bg-gray-900 text-stone-800 dark:text-gray-100
    placeholder:text-stone-300 dark:placeholder:text-gray-600
    focus:border-[var(--color-primary)] focus:outline-none`;
</script>

{#if loading}
  <div class="flex flex-col items-center justify-center min-h-screen px-8 gap-6" role="status" aria-live="polite">
    <div class="w-12 h-12 border-4 border-[var(--color-interactive)]/30 border-t-[var(--color-interactive)] rounded-full animate-spin"></div>
    <p class="text-lg font-medium text-stone-700 dark:text-gray-200">
      {$t('common.unlocking')}
    </p>
  </div>
{:else}
  <div class="flex flex-col items-center justify-center min-h-screen px-8 bg-stone-50 dark:bg-gray-950">
    <!-- App branding -->
    <div class="mb-8">
      <h1 class="text-2xl font-bold text-[var(--color-primary)] tracking-tight">Coheara</h1>
    </div>

    <!-- Profile avatars row (all profiles) -->
    {#if profiles.length > 1}
      <div class="flex items-center gap-4 mb-8" role="radiogroup" aria-label={$t('profile.picker_heading')}>
        {#each profiles as profile (profile.id)}
          {@const isSelected = selectedProfile?.id === profile.id}
          <button
            role="radio"
            aria-checked={isSelected}
            class="flex flex-col items-center gap-2 p-3 rounded-2xl transition-all min-w-[80px]
                   {isSelected
                     ? 'bg-white dark:bg-gray-900 shadow-md ring-2 ring-[var(--color-interactive)] scale-105'
                     : 'hover:bg-white/60 dark:hover:bg-gray-800/60'}"
            onclick={() => selectProfile(profile)}
          >
            <div class="w-14 h-14 rounded-full flex items-center justify-center text-white text-xl font-bold"
                 style:background-color={getColor(profile)}>
              {profile.name.charAt(0).toUpperCase()}
            </div>
            <span class="text-xs font-medium truncate max-w-[80px]
                         {isSelected ? 'text-stone-800 dark:text-gray-100' : 'text-stone-500 dark:text-gray-400'}">
              {profile.name}
            </span>
            {#if profile.managed_by}
              <span class="text-[10px] text-stone-400 dark:text-gray-500 truncate max-w-[80px]">
                {$t('profile.managed_by_label', { values: { managedBy: profile.managed_by } })}
              </span>
            {/if}
          </button>
        {/each}
      </div>
    {/if}

    <!-- Selected profile + password form -->
    {#if selectedProfile}
      <div class="flex flex-col items-center gap-5 w-full max-w-sm">
        <!-- Large avatar (for single-profile or confirmation) -->
        {#if profiles.length === 1}
          <div class="w-20 h-20 rounded-full flex items-center justify-center text-white text-3xl font-bold shadow-lg"
               style:background-color={getColor(selectedProfile)}>
            {selectedProfile.name.charAt(0).toUpperCase()}
          </div>
        {/if}

        <h2 class="text-xl font-bold text-stone-800 dark:text-gray-100">{selectedProfile.name}</h2>

        {#if selectedProfile.password_hint}
          <p class="text-stone-500 dark:text-gray-400 text-sm text-center">
            {$t('profile.hint_prefix')}{selectedProfile.password_hint}
          </p>
        {/if}

        <label class="w-full flex flex-col gap-1.5">
          <span class="text-stone-600 dark:text-gray-400 text-sm font-medium">{$t('profile.password_field')}</span>
          <input
            bind:this={passwordInput}
            type="password"
            bind:value={password}
            placeholder={$t('profile.enter_password')}
            class={inputClass}
            autocomplete="current-password"
            onkeydown={(e) => { if (e.key === 'Enter') handleUnlock(); }}
          />
        </label>

        <!-- Error message -->
        <p class="text-[var(--color-danger)] text-sm min-h-[20px] {error ? '' : 'invisible'}">{error || '\u00a0'}</p>

        <button
          class="w-full px-8 py-3.5 rounded-xl text-base font-medium min-h-[44px] transition-colors
                 bg-[var(--color-interactive)] text-white
                 hover:bg-[var(--color-interactive-hover)]
                 active:bg-[var(--color-interactive-active)]
                 disabled:bg-stone-300 disabled:dark:bg-gray-700 disabled:text-stone-500 disabled:dark:text-gray-500 disabled:cursor-not-allowed"
          onclick={handleUnlock}
          disabled={!password}
        >
          {$t('profile.unlock_button')}
        </button>

        <!-- Forgot password — appears after 3 failed attempts -->
        {#if attempts >= 3}
          <button
            class="text-sm min-h-[44px] transition-colors cursor-pointer
                   text-[var(--color-interactive)] hover:underline"
            onclick={onForgotPassword}
          >
            {$t('profile.forgot_password')}
          </button>
        {/if}
      </div>
    {:else}
      <!-- No profile selected yet (multi-profile, none tapped) -->
      <p class="text-stone-500 dark:text-gray-400 text-base">{$t('profile.picker_heading')}</p>
    {/if}
  </div>
{/if}
