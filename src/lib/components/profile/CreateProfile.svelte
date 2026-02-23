<script lang="ts">
  import { tick } from 'svelte';
  import { t } from 'svelte-i18n';
  import { createProfile } from '$lib/api/profile';
  import type { ProfileCreateResult, ProfileInfo } from '$lib/types/profile';
  import PasswordStrengthBar from '$lib/components/ui/PasswordStrengthBar.svelte';
  import DateOfBirthInput from '$lib/components/ui/DateOfBirthInput.svelte';
  import { COUNTRIES } from '$lib/data/countries';

  interface Props {
    isCaregiverPath?: boolean;
    /** F7: When false, backend won't switch active session to the new profile. */
    autoOpen?: boolean;
    /** F7: Active caregiver's profile — used to pre-fill caregiver name (readonly),
     *  country and address (editable) when creating a managed profile. */
    caregiverInfo?: ProfileInfo | null;
    onCreated: (result: ProfileCreateResult) => void;
    onError: (error: string) => void;
  }
  let { isCaregiverPath = false, autoOpen, caregiverInfo, onCreated, onError }: Props = $props();

  // F7: Whether caregiver name is auto-filled and locked
  let caregiverLocked = $derived(isCaregiverPath && caregiverInfo != null);

  let name = $state('');
  let password = $state('');
  let confirmPassword = $state('');
  // F7: Pre-fill from caregiver info when creating a managed profile
  let caregiverName = $state(caregiverInfo?.name ?? '');
  let dateOfBirth = $state('');
  let country = $state((isCaregiverPath && caregiverInfo?.country) ? caregiverInfo.country : '');
  let address = $state((isCaregiverPath && caregiverInfo?.address) ? caregiverInfo.address : '');
  let loading = $state(false);

  const inputClass = `px-4 py-3 rounded-lg border border-stone-300 dark:border-gray-600 text-lg min-h-[44px]
    bg-white dark:bg-gray-900 text-stone-800 dark:text-gray-100
    placeholder:text-stone-300 dark:placeholder:text-gray-600
    focus:border-[var(--color-primary)] focus:outline-none`;

  let mismatchVisible = $derived(confirmPassword.length > 0 && password !== confirmPassword);
  let tooShortVisible = $derived(password.length > 0 && password.length < 10);
  let formValid = $derived(
    name.trim().length > 0
    && password.length >= 10
    && password === confirmPassword
  );

  async function handleCreate() {
    if (!formValid) return;

    loading = true;
    await tick(); // Force DOM to render the loading view BEFORE the heavy backend work
    try {
      const result = await createProfile(
        name.trim(),
        password,
        isCaregiverPath ? caregiverName.trim() : null,
        dateOfBirth || null,
        country || null,
        address.trim() || null,
        autoOpen ?? null,
      );
      onCreated(result);
    } catch (e) {
      onError(String(e));
    } finally {
      loading = false;
    }
  }
</script>

{#if loading}
  <div class="flex flex-col items-center justify-center px-8 py-24 gap-6 max-w-md mx-auto w-full" role="status" aria-live="polite">
    <div class="w-12 h-12 border-4 border-[var(--color-interactive)]/30 border-t-[var(--color-interactive)] rounded-full animate-spin"></div>
    <p class="text-lg font-medium text-stone-700 dark:text-gray-200">
      {$t('common.creating')}
    </p>
    <p class="text-sm text-stone-500 dark:text-gray-400 animate-pulse">
      {$t('profile.creating_message')}
    </p>
  </div>
{:else}
  <div class="flex flex-col items-center px-8 gap-6 max-w-md mx-auto w-full">
    <h2 class="text-2xl font-bold text-stone-800 dark:text-gray-100">
      {isCaregiverPath ? $t('profile.create_heading_caregiver') : $t('profile.create_heading')}
    </h2>

    <div class="w-full flex flex-col gap-4">
      <label class="flex flex-col gap-1">
        <span class="text-stone-600 dark:text-gray-400 text-sm font-medium">
          {isCaregiverPath ? $t('profile.patient_name_label') : $t('profile.name_label')}
        </span>
        <input
          type="text"
          bind:value={name}
          placeholder={isCaregiverPath ? $t('profile.patient_name_placeholder') : $t('profile.name_placeholder')}
          class={inputClass}
          autocomplete="off"
        />
      </label>

      {#if isCaregiverPath}
        <label class="flex flex-col gap-1">
          <span class="text-stone-600 dark:text-gray-400 text-sm font-medium">{$t('profile.caregiver_name_label')}</span>
          {#if caregiverLocked}
            <input
              type="text"
              value={caregiverName}
              readonly
              class="{inputClass} bg-stone-100 dark:bg-gray-800 cursor-not-allowed opacity-75"
            />
          {:else}
            <input
              type="text"
              bind:value={caregiverName}
              placeholder={$t('profile.caregiver_placeholder')}
              class={inputClass}
            />
          {/if}
        </label>
      {/if}

      <div class="flex flex-col gap-1">
        <span class="text-stone-600 dark:text-gray-400 text-sm font-medium">{$t('profile.dob_label')}</span>
        <DateOfBirthInput value={dateOfBirth} onchange={(v) => dateOfBirth = v} />
        <span class="text-stone-400 dark:text-gray-500 text-xs">
          {isCaregiverPath ? $t('profile.dob_hint_managed') : $t('profile.dob_hint')}
        </span>
      </div>

      <label class="flex flex-col gap-1">
        <span class="text-stone-600 dark:text-gray-400 text-sm font-medium">{$t('profile.country_label')}</span>
        <select
          bind:value={country}
          class={inputClass}
          aria-label={$t('profile.country_label')}
        >
          <option value="" class="text-stone-300 dark:text-gray-600">{$t('profile.country_placeholder')}</option>
          {#each COUNTRIES as c}
            <option value={c.code}>{c.name}</option>
          {/each}
        </select>
      </label>

      <label class="flex flex-col gap-1">
        <span class="text-stone-600 dark:text-gray-400 text-sm font-medium">{$t('profile.address_label')}</span>
        <textarea
          bind:value={address}
          placeholder={$t('profile.address_placeholder')}
          rows="2"
          class="px-4 py-3 rounded-lg border border-stone-300 dark:border-gray-600 text-base min-h-[44px]
                 bg-white dark:bg-gray-900 text-stone-800 dark:text-gray-100
                 placeholder:text-stone-300 dark:placeholder:text-gray-600
                 focus:border-[var(--color-primary)] focus:outline-none resize-none"
          autocomplete="street-address"
        ></textarea>
        <span class="text-stone-400 dark:text-gray-500 text-xs">{$t('profile.address_hint')}</span>
      </label>

      <label class="flex flex-col gap-1">
        <span class="text-stone-600 dark:text-gray-400 text-sm font-medium">
          {isCaregiverPath ? $t('profile.password_label_caregiver') : $t('profile.password_label')}
        </span>
        <input
          type="password"
          bind:value={password}
          placeholder={$t('profile.password_placeholder')}
          class={inputClass}
          autocomplete="new-password"
        />
        <PasswordStrengthBar {password} />
        <span class="text-stone-400 dark:text-gray-500 text-xs">
          {isCaregiverPath
            ? $t('profile.password_hint_caregiver', { values: { name: name || '...' } })
            : $t('profile.password_hint_self')}
        </span>
      </label>

      <label class="flex flex-col gap-1">
        <span class="text-stone-600 dark:text-gray-400 text-sm font-medium">{$t('profile.confirm_password_label')}</span>
        <input
          type="password"
          bind:value={confirmPassword}
          placeholder={$t('profile.confirm_password_placeholder')}
          class={inputClass}
          autocomplete="new-password"
        />
      </label>

      <!-- Validation messages — always hold space, visible only when relevant -->
      <p class="text-[var(--color-danger)] text-sm {tooShortVisible ? '' : 'invisible'}">{$t('profile.password_too_short')}</p>
      <p class="text-[var(--color-danger)] text-sm {mismatchVisible ? '' : 'invisible'}">{$t('profile.password_mismatch')}</p>

      <button
        class="mt-2 px-8 py-4 rounded-xl text-lg font-medium min-h-[44px] transition-colors
               bg-[var(--color-interactive)] text-white
               hover:bg-[var(--color-interactive-hover)]
               active:bg-[var(--color-interactive-active)]
               disabled:bg-stone-300 disabled:dark:bg-gray-700 disabled:text-stone-500 disabled:dark:text-gray-500 disabled:cursor-not-allowed"
        onclick={handleCreate}
        disabled={!formValid}
      >
        {$t('profile.create_button')}
      </button>

      {#if isCaregiverPath}
        <p class="text-stone-400 dark:text-gray-500 text-xs text-center">
          {$t('profile.caregiver_note_bottom')}
        </p>
      {/if}
    </div>
  </div>
{/if}
