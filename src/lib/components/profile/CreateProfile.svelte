<script lang="ts">
  import { tick } from 'svelte';
  import { t } from 'svelte-i18n';
  import { createProfile, updateProfileDemographics } from '$lib/api/profile';
  import { recordVitalSign } from '$lib/api/me';
  import type { ProfileCreateResult, ProfileInfo, BiologicalSex, EthnicityGroup } from '$lib/types/profile';
  import PasswordStrengthBar from '$lib/components/ui/PasswordStrengthBar.svelte';
  import DateOfBirthInput from '$lib/components/ui/DateOfBirthInput.svelte';
  import { COUNTRIES } from '$lib/data/countries';

  const ALL_ETHNICITIES: { value: EthnicityGroup; labelKey: string }[] = [
    { value: 'European', labelKey: 'profile.ethnicity_european' },
    { value: 'SouthAsian', labelKey: 'profile.ethnicity_south_asian' },
    { value: 'EastAsian', labelKey: 'profile.ethnicity_east_asian' },
    { value: 'African', labelKey: 'profile.ethnicity_african' },
    { value: 'MiddleEastern', labelKey: 'profile.ethnicity_middle_eastern' },
    { value: 'Hispanic', labelKey: 'profile.ethnicity_hispanic' },
    { value: 'PacificIslander', labelKey: 'profile.ethnicity_pacific_islander' },
    { value: 'Indigenous', labelKey: 'profile.ethnicity_indigenous' },
  ];

  type SexPill = 'Male' | 'Female' | 'decline';
  const SEX_PILLS: { pill: SexPill; labelKey: string; display?: string }[] = [
    { pill: 'Male', labelKey: 'profile.sex_male' },
    { pill: 'Female', labelKey: 'profile.sex_female' },
    { pill: 'decline', labelKey: 'profile.sex_skip', display: '\u2014' },
  ];

  const MAX_ETHNICITIES = 3;
  const TOTAL_STEPS = 4;

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

  let caregiverLocked = $derived(isCaregiverPath && caregiverInfo != null);

  // ── Form state ────────────────────────────────────────────────────────
  let name = $state('');
  let password = $state('');
  let confirmPassword = $state('');
  let caregiverName = $state(caregiverInfo?.name ?? '');
  let dateOfBirth = $state('');
  let selectedEthnicities = $state<EthnicityGroup[]>([]);
  let country = $state((isCaregiverPath && caregiverInfo?.country) ? caregiverInfo.country : '');
  let address = $state((isCaregiverPath && caregiverInfo?.address) ? caregiverInfo.address : '');
  let loading = $state(false);
  let weight = $state('');
  let height = $state('');

  // ── Wizard state ──────────────────────────────────────────────────────
  let subStep = $state(0);

  // Sex: track which pill is visually active (null = nothing selected)
  let activeSexPill = $state<SexPill | null>(null);
  let sex = $derived<BiologicalSex | null>(
    activeSexPill === 'Male' ? 'Male' :
    activeSexPill === 'Female' ? 'Female' : null
  );

  // ── Ethnicity ─────────────────────────────────────────────────────────
  function toggleEthnicity(value: EthnicityGroup) {
    if (selectedEthnicities.includes(value)) {
      selectedEthnicities = selectedEthnicities.filter(e => e !== value);
    } else if (selectedEthnicities.length < MAX_ETHNICITIES) {
      selectedEthnicities = [...selectedEthnicities, value];
    }
  }

  let atEthnicityMax = $derived(selectedEthnicities.length >= MAX_ETHNICITIES);

  function toggleSexPill(pill: SexPill) {
    activeSexPill = activeSexPill === pill ? null : pill;
  }

  // ── Navigation ────────────────────────────────────────────────────────
  function nextStep() {
    if (subStep === 0 && name.trim().length === 0) return;
    if (subStep < TOTAL_STEPS - 1) subStep++;
  }

  function prevStep() {
    if (subStep > 0) subStep--;
  }

  // ── Validation ────────────────────────────────────────────────────────
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
  let nameValid = $derived(name.trim().length > 0);
  let weightNum = $derived(weight ? parseFloat(weight) : NaN);
  let heightNum = $derived(height ? parseFloat(height) : NaN);
  let weightValid = $derived(!weight || (weightNum >= 20 && weightNum <= 300));
  let heightValid = $derived(!height || (heightNum >= 50 && heightNum <= 250));

  // ── Submit ────────────────────────────────────────────────────────────
  async function handleCreate() {
    if (!formValid) return;
    loading = true;
    await tick();
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
      if (sex || selectedEthnicities.length > 0) {
        await updateProfileDemographics(result.profile.id, sex, selectedEthnicities);
      }
      // ME-04 B7: Record weight/height as vital signs if provided
      const wn = parseFloat(weight);
      const hn = parseFloat(height);
      if (!isNaN(wn) && wn >= 20 && wn <= 300) {
        await recordVitalSign('weight', wn);
      }
      if (!isNaN(hn) && hn >= 50 && hn <= 250) {
        await recordVitalSign('height', hn);
      }
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
    <!-- Progress dots -->
    <div class="flex justify-center gap-2" role="progressbar" aria-valuenow={subStep + 1} aria-valuemin={1} aria-valuemax={TOTAL_STEPS}>
      {#each Array(TOTAL_STEPS) as _, i}
        <div class="w-2 h-2 rounded-full transition-colors duration-300 {i <= subStep ? 'bg-[var(--color-primary)]' : 'bg-stone-200 dark:bg-gray-700'}"></div>
      {/each}
    </div>

    {#if subStep === 0}
      <!-- ── SUB-STEP 1: IDENTITY ───────────────────────────────────────── -->
      <div class="w-full flex flex-col gap-5">
        <div class="text-center">
          <h2 class="text-2xl font-bold text-stone-800 dark:text-gray-100">
            {$t('profile.step_identity_heading')}
          </h2>
          <p class="text-stone-500 dark:text-gray-400 text-sm mt-1">
            {isCaregiverPath ? $t('profile.step_identity_subtitle_caregiver') : $t('profile.step_identity_subtitle')}
          </p>
        </div>

        <label class="flex flex-col gap-1">
          <span class="text-stone-600 dark:text-gray-400 text-sm font-medium">
            {isCaregiverPath ? $t('profile.patient_name_label') : $t('profile.name_label')} *
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
          <p class="text-stone-400 dark:text-gray-400 text-xs mt-1">
            {$t('profile.step_identity_dob_hint')}
          </p>
        </div>

        <button
          class="mt-2 px-8 py-4 rounded-xl text-lg font-medium min-h-[44px] transition-colors
                 bg-[var(--color-interactive)] text-white
                 hover:bg-[var(--color-interactive-hover)]
                 active:bg-[var(--color-interactive-active)]
                 disabled:bg-stone-300 disabled:dark:bg-gray-700 disabled:text-stone-500 disabled:dark:text-gray-400 disabled:cursor-not-allowed"
          onclick={nextStep}
          disabled={!nameValid}
        >
          {$t('profile.wizard_next')} &rarr;
        </button>
      </div>

    {:else if subStep === 1}
      <!-- ── SUB-STEP 2: HEALTH (optional) ──────────────────────────────── -->
      <div class="w-full flex flex-col gap-5">
        <div class="text-center">
          <h2 class="text-2xl font-bold text-stone-800 dark:text-gray-100">
            {$t('profile.step_health_heading')}
          </h2>
          <p class="text-stone-500 dark:text-gray-400 text-sm mt-1">
            {$t('profile.step_health_subtitle')}
          </p>
        </div>

        <!-- Sex: Pill-Toggle Buttons -->
        <fieldset class="flex flex-col gap-2">
          <legend class="text-stone-600 dark:text-gray-400 text-sm font-medium">{$t('profile.sex_label')}</legend>
          <div class="flex gap-3">
            {#each SEX_PILLS as { pill, labelKey, display }}
              <button
                type="button"
                class="px-4 py-2.5 rounded-full text-sm font-medium min-h-[44px] transition-colors
                       {activeSexPill === pill
                         ? 'bg-[var(--color-primary)] text-white'
                         : 'bg-white dark:bg-gray-800 border border-stone-200 dark:border-gray-700 text-stone-600 dark:text-gray-400 hover:bg-stone-50 dark:hover:bg-gray-700'}"
                aria-pressed={activeSexPill === pill}
                aria-label={display ? $t(labelKey) : undefined}
                onclick={() => toggleSexPill(pill)}
              >
                {display ?? $t(labelKey)}
              </button>
            {/each}
          </div>
          <p class="text-stone-400 dark:text-gray-400 text-xs">
            {$t('profile.step_health_sex_hint')}
          </p>
        </fieldset>

        <!-- Ethnicity: Chip Tags -->
        <fieldset class="flex flex-col gap-2">
          <legend class="text-stone-600 dark:text-gray-400 text-sm font-medium">{$t('profile.ethnicity_label')}</legend>
          <div class="flex flex-wrap gap-2">
            {#each ALL_ETHNICITIES as { value, labelKey }}
              {@const selected = selectedEthnicities.includes(value)}
              {@const chipDisabled = atEthnicityMax && !selected}
              <button
                type="button"
                class="px-3 py-1.5 rounded-full text-sm transition-colors min-h-[36px]
                       {selected
                         ? 'bg-[var(--color-primary)]/10 border border-[var(--color-primary)] text-[var(--color-primary)] font-medium'
                         : 'bg-stone-100 dark:bg-gray-800 border border-stone-200 dark:border-gray-700 text-stone-600 dark:text-gray-400 hover:bg-stone-200 dark:hover:bg-gray-700'}
                       {chipDisabled ? 'opacity-40 cursor-not-allowed' : 'cursor-pointer'}"
                aria-pressed={selected}
                disabled={chipDisabled}
                onclick={() => toggleEthnicity(value)}
              >
                {$t(labelKey)}
              </button>
            {/each}
          </div>
          {#if selectedEthnicities.length > 0}
            <p class="text-stone-400 dark:text-gray-400 text-xs">
              {atEthnicityMax
                ? $t('profile.ethnicity_count_max', { values: { count: selectedEthnicities.length, max: MAX_ETHNICITIES } })
                : $t('profile.ethnicity_count', { values: { count: selectedEthnicities.length, max: MAX_ETHNICITIES } })}
            </p>
          {:else}
            <p class="text-stone-400 dark:text-gray-400 text-xs">
              {$t('profile.step_health_ethnicity_hint')}
            </p>
          {/if}
        </fieldset>

        <!-- ME-04 B7: Weight + Height (optional) -->
        <div class="grid grid-cols-2 gap-3">
          <div>
            <label class="text-stone-600 dark:text-gray-400 text-sm font-medium mb-1 block">
              {$t('profile.weight_label')}
            </label>
            <div class="relative">
              <input
                type="number"
                bind:value={weight}
                min="20" max="300" step="0.1"
                placeholder="\u2014"
                class="w-full px-3 py-2.5 pr-10 rounded-lg border text-sm min-h-[44px]
                  {weightValid
                    ? 'border-stone-300 dark:border-gray-600'
                    : 'border-red-400 dark:border-red-600'}
                  bg-white dark:bg-gray-900 text-stone-800 dark:text-gray-100"
              />
              <span class="absolute right-3 top-1/2 -translate-y-1/2 text-xs
                           text-stone-400 dark:text-gray-400">
                {$t('profile.weight_unit')}
              </span>
            </div>
            {#if !weightValid}
              <p class="text-xs text-red-500 mt-1">{$t('profile.weight_invalid')}</p>
            {/if}
          </div>
          <div>
            <label class="text-stone-600 dark:text-gray-400 text-sm font-medium mb-1 block">
              {$t('profile.height_label')}
            </label>
            <div class="relative">
              <input
                type="number"
                bind:value={height}
                min="50" max="250" step="0.1"
                placeholder="\u2014"
                class="w-full px-3 py-2.5 pr-10 rounded-lg border text-sm min-h-[44px]
                  {heightValid
                    ? 'border-stone-300 dark:border-gray-600'
                    : 'border-red-400 dark:border-red-600'}
                  bg-white dark:bg-gray-900 text-stone-800 dark:text-gray-100"
              />
              <span class="absolute right-3 top-1/2 -translate-y-1/2 text-xs
                           text-stone-400 dark:text-gray-400">
                {$t('profile.height_unit')}
              </span>
            </div>
            {#if !heightValid}
              <p class="text-xs text-red-500 mt-1">{$t('profile.height_invalid')}</p>
            {/if}
          </div>
        </div>
        <p class="text-stone-400 dark:text-gray-400 text-xs">
          {$t('profile.step_health_body_hint')}
        </p>

        <!-- Privacy note -->
        <p class="text-stone-400 dark:text-gray-400 text-xs text-center italic">
          {$t('profile.step_health_privacy')}
        </p>

        <!-- Navigation -->
        <div class="flex flex-col gap-3 mt-2">
          <div class="flex items-center justify-between">
            <button
              type="button"
              class="text-stone-500 dark:text-gray-400 text-sm hover:text-stone-700 dark:hover:text-gray-200 transition-colors min-h-[44px]"
              onclick={prevStep}
            >
              &larr; {$t('profile.wizard_back')}
            </button>
            <button
              class="px-8 py-4 rounded-xl text-lg font-medium min-h-[44px] transition-colors
                     bg-[var(--color-interactive)] text-white
                     hover:bg-[var(--color-interactive-hover)]
                     active:bg-[var(--color-interactive-active)]"
              onclick={nextStep}
            >
              {$t('profile.wizard_next')} &rarr;
            </button>
          </div>
          <button
            type="button"
            class="text-stone-400 dark:text-gray-400 text-xs text-center hover:text-stone-600 dark:hover:text-gray-300 transition-colors"
            onclick={nextStep}
          >
            {$t('profile.step_health_skip')}
          </button>
        </div>
      </div>

    {:else if subStep === 2}
      <!-- ── SUB-STEP 3: LOCATION (optional) ────────────────────────────── -->
      <div class="w-full flex flex-col gap-5">
        <div class="text-center">
          <h2 class="text-2xl font-bold text-stone-800 dark:text-gray-100">
            {$t('profile.step_location_heading')}
          </h2>
          <p class="text-stone-500 dark:text-gray-400 text-sm mt-1">
            {$t('profile.step_location_subtitle')}
          </p>
        </div>

        <label class="flex flex-col gap-1">
          <span class="text-stone-600 dark:text-gray-400 text-sm font-medium">{$t('profile.country_label')}</span>
          <select bind:value={country} class={inputClass} aria-label={$t('profile.country_label')}>
            <option value="" class="text-stone-300 dark:text-gray-600">{$t('profile.country_placeholder')}</option>
            {#each COUNTRIES as c}
              <option value={c.code}>{c.name}</option>
            {/each}
          </select>
          <p class="text-stone-400 dark:text-gray-400 text-xs">
            {$t('profile.step_location_country_hint')}
          </p>
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
          <p class="text-stone-400 dark:text-gray-400 text-xs">
            {$t('profile.step_location_address_hint')}
          </p>
        </label>

        <!-- Navigation -->
        <div class="flex flex-col gap-3 mt-2">
          <div class="flex items-center justify-between">
            <button
              type="button"
              class="text-stone-500 dark:text-gray-400 text-sm hover:text-stone-700 dark:hover:text-gray-200 transition-colors min-h-[44px]"
              onclick={prevStep}
            >
              &larr; {$t('profile.wizard_back')}
            </button>
            <button
              class="px-8 py-4 rounded-xl text-lg font-medium min-h-[44px] transition-colors
                     bg-[var(--color-interactive)] text-white
                     hover:bg-[var(--color-interactive-hover)]
                     active:bg-[var(--color-interactive-active)]"
              onclick={nextStep}
            >
              {$t('profile.wizard_next')} &rarr;
            </button>
          </div>
          <button
            type="button"
            class="text-stone-400 dark:text-gray-400 text-xs text-center hover:text-stone-600 dark:hover:text-gray-300 transition-colors"
            onclick={nextStep}
          >
            {$t('profile.step_location_skip')}
          </button>
        </div>
      </div>

    {:else}
      <!-- ── SUB-STEP 4: SECURITY ───────────────────────────────────────── -->
      <div class="w-full flex flex-col gap-5">
        <div class="text-center">
          <h2 class="text-2xl font-bold text-stone-800 dark:text-gray-100">
            {$t('profile.step_security_heading')}
          </h2>
          <p class="text-stone-500 dark:text-gray-400 text-sm mt-1">
            {$t('profile.step_security_subtitle')}
          </p>
        </div>

        <label class="flex flex-col gap-1">
          <span class="text-stone-600 dark:text-gray-400 text-sm font-medium">
            {isCaregiverPath ? $t('profile.password_label_caregiver') : $t('profile.password_label')} *
          </span>
          <input
            type="password"
            bind:value={password}
            placeholder={$t('profile.password_placeholder')}
            class={inputClass}
            autocomplete="new-password"
          />
          <PasswordStrengthBar {password} />
        </label>

        <label class="flex flex-col gap-1">
          <span class="text-stone-600 dark:text-gray-400 text-sm font-medium">{$t('profile.confirm_password_label')} *</span>
          <input
            type="password"
            bind:value={confirmPassword}
            placeholder={$t('profile.confirm_password_placeholder')}
            class={inputClass}
            autocomplete="new-password"
          />
        </label>

        <!-- Validation messages -->
        <p class="text-[var(--color-danger)] text-sm {tooShortVisible ? '' : 'invisible'}">{$t('profile.password_too_short')}</p>
        <p class="text-[var(--color-danger)] text-sm {mismatchVisible ? '' : 'invisible'}">{$t('profile.password_mismatch')}</p>

        <p class="text-stone-400 dark:text-gray-400 text-xs">
          {$t('profile.step_security_password_hint')}
        </p>

        <!-- Navigation -->
        <div class="flex items-center justify-between mt-2">
          <button
            type="button"
            class="text-stone-500 dark:text-gray-400 text-sm hover:text-stone-700 dark:hover:text-gray-200 transition-colors min-h-[44px]"
            onclick={prevStep}
          >
            &larr; {$t('profile.wizard_back')}
          </button>
          <button
            class="px-8 py-4 rounded-xl text-lg font-medium min-h-[44px] transition-colors
                   bg-[var(--color-interactive)] text-white
                   hover:bg-[var(--color-interactive-hover)]
                   active:bg-[var(--color-interactive-active)]
                   disabled:bg-stone-300 disabled:dark:bg-gray-700 disabled:text-stone-500 disabled:dark:text-gray-400 disabled:cursor-not-allowed"
            onclick={handleCreate}
            disabled={!formValid}
          >
            {$t('profile.create_button')}
          </button>
        </div>

        {#if isCaregiverPath}
          <p class="text-stone-400 dark:text-gray-400 text-xs text-center">
            {$t('profile.caregiver_note_bottom')}
          </p>
        {/if}
      </div>
    {/if}
  </div>
{/if}
