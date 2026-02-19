<script lang="ts">
  import { t } from 'svelte-i18n';
  import { createProfile } from '$lib/api/profile';
  import type { ProfileCreateResult } from '$lib/types/profile';

  interface Props {
    isCaregiverPath?: boolean;
    onCreated: (result: ProfileCreateResult) => void;
    onError: (error: string) => void;
  }
  let { isCaregiverPath = false, onCreated, onError }: Props = $props();

  let name = $state('');
  let password = $state('');
  let confirmPassword = $state('');
  let caregiverMode = $state(isCaregiverPath as boolean);
  let caregiverName = $state('');
  let dateOfBirth = $state('');
  let loading = $state(false);
  let passwordError = $state('');

  function validatePassword(): boolean {
    if (password.length < 6) {
      passwordError = $t('profile.password_too_short');
      return false;
    }
    if (password !== confirmPassword) {
      passwordError = $t('profile.password_mismatch');
      return false;
    }
    passwordError = '';
    return true;
  }

  async function handleCreate() {
    if (!name.trim()) return;
    if (!validatePassword()) return;

    loading = true;
    try {
      const result = await createProfile(
        name.trim(),
        password,
        caregiverMode ? caregiverName.trim() : null,
        dateOfBirth || null,
      );
      onCreated(result);
    } catch (e) {
      onError(String(e));
    } finally {
      loading = false;
    }
  }
</script>

<div class="flex flex-col items-center justify-center min-h-screen px-8 gap-6 max-w-md mx-auto">
  <h2 class="text-2xl font-bold text-stone-800">
    {isCaregiverPath ? $t('profile.create_heading_caregiver') : $t('profile.create_heading')}
  </h2>
  {#if isCaregiverPath}
    <p class="text-stone-500 text-sm text-center">{$t('profile.caregiver_guidance')}</p>
  {/if}

  <div class="w-full flex flex-col gap-4">
    <label class="flex flex-col gap-1">
      <span class="text-stone-600 text-sm font-medium">
        {caregiverMode ? $t('profile.patient_name_label') : $t('profile.name_label')}
      </span>
      <input
        type="text"
        bind:value={name}
        placeholder={caregiverMode ? $t('profile.patient_name_placeholder') : $t('profile.name_placeholder')}
        class="px-4 py-3 rounded-lg border border-stone-300 text-lg min-h-[44px]
               focus:border-[var(--color-primary)] focus:outline-none"
        autocomplete="off"
      />
    </label>

    <!-- Show checkbox only if NOT pre-selected via ProfileTypeChoice -->
    {#if !isCaregiverPath}
      <div class="flex items-center gap-2">
        <input type="checkbox" id="caregiver" bind:checked={caregiverMode}
               class="min-h-[44px] min-w-[44px]" />
        <label for="caregiver" class="text-stone-600 text-sm">
          {$t('profile.caregiver_mode')}
        </label>
      </div>
    {/if}

    {#if caregiverMode}
      <label class="flex flex-col gap-1">
        <span class="text-stone-600 text-sm font-medium">{$t('profile.caregiver_name_label')}</span>
        <input
          type="text"
          bind:value={caregiverName}
          placeholder={$t('profile.caregiver_placeholder')}
          class="px-4 py-3 rounded-lg border border-stone-300 text-lg min-h-[44px]
                 focus:border-[var(--color-primary)] focus:outline-none"
        />
      </label>
    {/if}

    <label class="flex flex-col gap-1">
      <span class="text-stone-600 text-sm font-medium">{$t('profile.dob_label')}</span>
      <input
        type="date"
        bind:value={dateOfBirth}
        class="px-4 py-3 rounded-lg border border-stone-300 text-lg min-h-[44px]
               focus:border-[var(--color-primary)] focus:outline-none"
      />
      <span class="text-stone-400 text-xs">
        {caregiverMode ? $t('profile.dob_hint_managed') : $t('profile.dob_hint')}
      </span>
    </label>

    <label class="flex flex-col gap-1">
      <span class="text-stone-600 text-sm font-medium">{$t('profile.password_label')}</span>
      <input
        type="password"
        bind:value={password}
        placeholder={$t('profile.password_placeholder')}
        class="px-4 py-3 rounded-lg border border-stone-300 text-lg min-h-[44px]
               focus:border-[var(--color-primary)] focus:outline-none"
        autocomplete="new-password"
      />
    </label>

    <label class="flex flex-col gap-1">
      <span class="text-stone-600 text-sm font-medium">{$t('profile.confirm_password_label')}</span>
      <input
        type="password"
        bind:value={confirmPassword}
        placeholder={$t('profile.confirm_password_placeholder')}
        class="px-4 py-3 rounded-lg border border-stone-300 text-lg min-h-[44px]
               focus:border-[var(--color-primary)] focus:outline-none"
        autocomplete="new-password"
      />
    </label>

    {#if passwordError}
      <p class="text-[var(--color-danger)] text-sm">{passwordError}</p>
    {/if}

    <button
      class="mt-2 px-8 py-4 bg-[var(--color-primary)] text-white rounded-xl text-lg
             font-medium hover:brightness-110 disabled:opacity-50 min-h-[44px]"
      onclick={handleCreate}
      disabled={loading || !name.trim() || !password}
    >
      {loading ? $t('common.creating') : $t('profile.create_button')}
    </button>
  </div>
</div>
