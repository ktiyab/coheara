<script lang="ts">
  import { t } from 'svelte-i18n';
  import { updateProfileDemographics } from '$lib/api/profile';
  import type { ProfileInfo, BiologicalSex, EthnicityGroup } from '$lib/types/profile';

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

  interface Props {
    profile: ProfileInfo;
    onSaved: (updated: ProfileInfo) => void;
    onCancel: () => void;
  }
  let { profile, onSaved, onCancel }: Props = $props();

  let sex = $state<BiologicalSex | null>(profile.sex);
  let selectedEthnicities = $state<EthnicityGroup[]>([...profile.ethnicities]);
  let saving = $state(false);
  let error = $state('');

  function toggleEthnicity(value: EthnicityGroup) {
    if (selectedEthnicities.includes(value)) {
      selectedEthnicities = selectedEthnicities.filter(e => e !== value);
    } else if (selectedEthnicities.length < 3) {
      selectedEthnicities = [...selectedEthnicities, value];
    }
  }

  async function handleSave() {
    saving = true;
    error = '';
    try {
      const updated = await updateProfileDemographics(profile.id, sex, selectedEthnicities);
      onSaved(updated);
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<div class="flex flex-col gap-6 max-w-md mx-auto w-full px-4">
  <h3 class="text-xl font-bold text-stone-800 dark:text-gray-100">{$t('profile.demographics_edit')}</h3>

  <!-- Biological sex -->
  <fieldset class="flex flex-col gap-2">
    <legend class="text-stone-600 dark:text-gray-400 text-sm font-medium">{$t('profile.sex_label')}</legend>
    <div class="flex gap-4">
      <label class="flex items-center gap-2 cursor-pointer">
        <input type="radio" name="edit-sex" value="Male" checked={sex === 'Male'} onchange={() => sex = 'Male'}
          class="w-4 h-4 accent-[var(--color-primary)]" />
        <span class="text-stone-700 dark:text-gray-200">{$t('profile.sex_male')}</span>
      </label>
      <label class="flex items-center gap-2 cursor-pointer">
        <input type="radio" name="edit-sex" value="Female" checked={sex === 'Female'} onchange={() => sex = 'Female'}
          class="w-4 h-4 accent-[var(--color-primary)]" />
        <span class="text-stone-700 dark:text-gray-200">{$t('profile.sex_female')}</span>
      </label>
      <label class="flex items-center gap-2 cursor-pointer">
        <input type="radio" name="edit-sex" value="" checked={sex === null} onchange={() => sex = null}
          class="w-4 h-4 accent-[var(--color-primary)]" />
        <span class="text-stone-500 dark:text-gray-400">{$t('profile.sex_skip')}</span>
      </label>
    </div>
    <span class="text-stone-400 dark:text-gray-400 text-xs">{$t('profile.sex_hint')}</span>
  </fieldset>

  <!-- Ethnicity blend -->
  <fieldset class="flex flex-col gap-2">
    <legend class="text-stone-600 dark:text-gray-400 text-sm font-medium">{$t('profile.ethnicity_label')}</legend>
    <div class="grid grid-cols-2 gap-2">
      {#each ALL_ETHNICITIES as { value, labelKey }}
        <label class="flex items-center gap-2 cursor-pointer {selectedEthnicities.length >= 3 && !selectedEthnicities.includes(value) ? 'opacity-40 cursor-not-allowed' : ''}">
          <input type="checkbox"
            checked={selectedEthnicities.includes(value)}
            disabled={selectedEthnicities.length >= 3 && !selectedEthnicities.includes(value)}
            onchange={() => toggleEthnicity(value)}
            class="w-4 h-4 accent-[var(--color-primary)]" />
          <span class="text-stone-700 dark:text-gray-200 text-sm">{$t(labelKey)}</span>
        </label>
      {/each}
    </div>
    {#if selectedEthnicities.length >= 3}
      <span class="text-amber-600 dark:text-amber-400 text-xs">{$t('profile.ethnicity_max_reached')}</span>
    {:else}
      <span class="text-stone-400 dark:text-gray-400 text-xs">{$t('profile.ethnicity_hint')}</span>
    {/if}
  </fieldset>

  {#if error}
    <p class="text-[var(--color-danger)] text-sm">{error}</p>
  {/if}

  <div class="flex gap-3">
    <button
      class="flex-1 px-6 py-3 rounded-lg text-base font-medium min-h-[44px] transition-colors
             border border-stone-300 dark:border-gray-600 text-stone-700 dark:text-gray-200
             hover:bg-stone-100 dark:hover:bg-gray-800"
      onclick={onCancel}
      disabled={saving}
    >
      {$t('common.cancel')}
    </button>
    <button
      class="flex-1 px-6 py-3 rounded-lg text-base font-medium min-h-[44px] transition-colors
             bg-[var(--color-interactive)] text-white
             hover:bg-[var(--color-interactive-hover)]
             disabled:bg-stone-300 disabled:dark:bg-gray-700 disabled:cursor-not-allowed"
      onclick={handleSave}
      disabled={saving}
    >
      {saving ? $t('common.saving') : $t('common.save')}
    </button>
  </div>
</div>
