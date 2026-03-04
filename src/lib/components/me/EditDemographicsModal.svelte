<!-- ME-04 B5: Edit demographics modal — sex, ethnicities, weight, height. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { MeIdentity } from '$lib/types/me';
  import type { BiologicalSex, EthnicityGroup } from '$lib/types/profile';
  import { updateProfileDemographics } from '$lib/api/profile';
  import { recordVitalSign } from '$lib/api/me';
  import { CloseIcon } from '$lib/components/icons/md';

  let { identity, onclose, onsaved }: {
    identity: MeIdentity;
    onclose: () => void;
    onsaved: () => void;
  } = $props();

  // State — pre-populated from current identity
  let sex = $state<BiologicalSex | null>(
    identity.sex === 'male' ? 'Male'
      : identity.sex === 'female' ? 'Female'
      : null
  );
  let selectedEthnicities = $state<EthnicityGroup[]>(
    identity.ethnicities as EthnicityGroup[]
  );
  let bloodType = $state<string | null>(identity.blood_type ?? null);
  let weight = $state(identity.weight_kg?.toString() ?? '');
  let height = $state(identity.height_cm?.toString() ?? '');
  let saving = $state(false);
  let saveError = $state<string | null>(null);

  const BLOOD_TYPE_OPTIONS = [
    { key: 'o_positive', display: 'O+' },
    { key: 'o_negative', display: 'O-' },
    { key: 'a_positive', display: 'A+' },
    { key: 'a_negative', display: 'A-' },
    { key: 'b_positive', display: 'B+' },
    { key: 'b_negative', display: 'B-' },
    { key: 'ab_positive', display: 'AB+' },
    { key: 'ab_negative', display: 'AB-' },
  ] as const;

  // Validation
  let weightNum = $derived(weight ? parseFloat(weight) : NaN);
  let heightNum = $derived(height ? parseFloat(height) : NaN);
  let weightValid = $derived(!weight || (weightNum >= 20 && weightNum <= 300));
  let heightValid = $derived(!height || (heightNum >= 50 && heightNum <= 250));
  let canSave = $derived(weightValid && heightValid && !saving);

  const ALL_ETHNICITIES: EthnicityGroup[] = [
    'European', 'SouthAsian', 'EastAsian', 'African',
    'MiddleEastern', 'Hispanic', 'PacificIslander', 'Indigenous',
  ];

  /** Map PascalCase ethnicity to i18n snake_case key. */
  const ETHNICITY_KEY_MAP: Record<string, string> = {
    European: 'european',
    SouthAsian: 'south_asian',
    EastAsian: 'east_asian',
    African: 'african',
    MiddleEastern: 'middle_eastern',
    Hispanic: 'hispanic',
    PacificIslander: 'pacific_islander',
    Indigenous: 'indigenous',
  };

  function toggleEthnicity(e: EthnicityGroup) {
    if (selectedEthnicities.includes(e)) {
      selectedEthnicities = selectedEthnicities.filter(x => x !== e);
    } else if (selectedEthnicities.length < 3) {
      selectedEthnicities = [...selectedEthnicities, e];
    }
  }

  async function handleSave() {
    if (!canSave) return;
    saving = true;
    saveError = null;

    try {
      await updateProfileDemographics(identity.profile_id, sex, selectedEthnicities, bloodType);

      const wn = parseFloat(weight);
      if (!isNaN(wn) && wn >= 20 && wn <= 300) {
        await recordVitalSign('weight', wn);
      }

      const hn = parseFloat(height);
      if (!isNaN(hn) && hn >= 50 && hn <= 250) {
        await recordVitalSign('height', hn);
      }

      onsaved();
    } catch (e) {
      saveError = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<!-- Backdrop -->
<div
  class="fixed inset-0 bg-black/50 z-40 flex items-center justify-center p-4"
  onclick={onclose}
  role="dialog"
  aria-modal="true"
  aria-label={$t('me.edit_title')}
>
  <!-- Modal -->
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div
    class="bg-white dark:bg-gray-900 rounded-2xl border border-stone-200
           dark:border-gray-700 w-full max-w-md max-h-[85vh] overflow-y-auto
           shadow-xl"
    onclick={(e) => e.stopPropagation()}
  >
    <!-- Header -->
    <div class="flex items-center justify-between p-4 border-b border-stone-100
                dark:border-gray-800">
      <h2 class="text-lg font-semibold text-stone-800 dark:text-gray-100">
        {$t('me.edit_title')}
      </h2>
      <button onclick={onclose}
        class="text-stone-400 dark:text-gray-400 hover:text-stone-600
               dark:hover:text-gray-300">
        <CloseIcon class="w-5 h-5" />
      </button>
    </div>

    <div class="p-4 space-y-5">
      <!-- Sex -->
      <fieldset>
        <legend class="text-sm font-medium text-stone-700 dark:text-gray-300 mb-2">
          {$t('profile.sex_label')}
        </legend>
        <div class="flex gap-2">
          {#each [
            { val: 'Male' as BiologicalSex, label: $t('profile.sex_male') },
            { val: 'Female' as BiologicalSex, label: $t('profile.sex_female') },
          ] as opt}
            <button
              onclick={() => sex = sex === opt.val ? null : opt.val}
              class="px-4 py-2 rounded-full text-sm font-medium transition-colors
                {sex === opt.val
                  ? 'bg-teal-600 text-white'
                  : 'bg-stone-100 dark:bg-gray-800 text-stone-600 dark:text-gray-300 hover:bg-stone-200 dark:hover:bg-gray-700'}"
            >
              {opt.label}
            </button>
          {/each}
          <button
            onclick={() => sex = null}
            class="px-4 py-2 rounded-full text-sm transition-colors
              {sex === null
                ? 'bg-teal-600 text-white'
                : 'bg-stone-100 dark:bg-gray-800 text-stone-500 dark:text-gray-400 hover:bg-stone-200 dark:hover:bg-gray-700'}"
          >
            {$t('profile.sex_skip')}
          </button>
        </div>
      </fieldset>

      <!-- Blood Type -->
      <fieldset>
        <legend class="text-sm font-medium text-stone-700 dark:text-gray-300 mb-2">
          {$t('me.blood_type_label')}
        </legend>
        <div class="grid grid-cols-4 gap-2">
          {#each BLOOD_TYPE_OPTIONS as opt}
            <button
              onclick={() => bloodType = bloodType === opt.key ? null : opt.key}
              class="px-3 py-2 rounded-lg text-sm font-bold transition-colors
                {bloodType === opt.key
                  ? 'bg-red-600 text-white'
                  : 'bg-stone-100 dark:bg-gray-800 text-stone-600 dark:text-gray-300 hover:bg-stone-200 dark:hover:bg-gray-700'}"
            >
              {opt.display}
            </button>
          {/each}
        </div>
        <button
          onclick={() => bloodType = null}
          class="mt-2 px-3 py-1.5 rounded-full text-xs transition-colors
            {bloodType === null
              ? 'bg-stone-600 dark:bg-gray-600 text-white'
              : 'bg-stone-100 dark:bg-gray-800 text-stone-500 dark:text-gray-400 hover:bg-stone-200 dark:hover:bg-gray-700'}"
        >
          {$t('me.blood_type_unknown')}
        </button>
      </fieldset>

      <!-- Ethnicities -->
      <fieldset>
        <legend class="text-sm font-medium text-stone-700 dark:text-gray-300 mb-2">
          {$t('profile.ethnicity_label')}
        </legend>
        <div class="flex flex-wrap gap-2">
          {#each ALL_ETHNICITIES as eth}
            <button
              onclick={() => toggleEthnicity(eth)}
              class="px-3 py-1.5 rounded-full text-xs font-medium transition-colors
                {selectedEthnicities.includes(eth)
                  ? 'bg-teal-600 text-white'
                  : 'bg-stone-100 dark:bg-gray-800 text-stone-600 dark:text-gray-300 hover:bg-stone-200 dark:hover:bg-gray-700'}
                {selectedEthnicities.length >= 3 && !selectedEthnicities.includes(eth)
                  ? 'opacity-40 cursor-not-allowed' : ''}"
              disabled={selectedEthnicities.length >= 3 && !selectedEthnicities.includes(eth)}
            >
              {$t(`profile.ethnicity_${ETHNICITY_KEY_MAP[eth] ?? eth.toLowerCase()}`)}
            </button>
          {/each}
        </div>
      </fieldset>

      <!-- Weight + Height side by side -->
      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="text-sm font-medium text-stone-700 dark:text-gray-300 mb-1 block">
            {$t('profile.weight_label')}
          </label>
          <div class="relative">
            <input
              type="number"
              bind:value={weight}
              min="20" max="300" step="0.1"
              placeholder="-"
              class="w-full px-3 py-2 pr-10 rounded-lg border text-sm
                {weightValid
                  ? 'border-stone-200 dark:border-gray-700'
                  : 'border-red-400 dark:border-red-600'}
                bg-white dark:bg-gray-800 text-stone-800 dark:text-gray-100"
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
          <label class="text-sm font-medium text-stone-700 dark:text-gray-300 mb-1 block">
            {$t('profile.height_label')}
          </label>
          <div class="relative">
            <input
              type="number"
              bind:value={height}
              min="50" max="250" step="0.1"
              placeholder="-"
              class="w-full px-3 py-2 pr-10 rounded-lg border text-sm
                {heightValid
                  ? 'border-stone-200 dark:border-gray-700'
                  : 'border-red-400 dark:border-red-600'}
                bg-white dark:bg-gray-800 text-stone-800 dark:text-gray-100"
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
      <p class="text-xs text-stone-400 dark:text-gray-400">
        {$t('profile.step_health_body_hint')}
      </p>

      <!-- Error -->
      {#if saveError}
        <p class="text-sm text-red-600 dark:text-red-400">{saveError}</p>
      {/if}
    </div>

    <!-- Footer -->
    <div class="flex justify-end gap-3 p-4 border-t border-stone-100
                dark:border-gray-800">
      <button
        onclick={onclose}
        class="px-4 py-2 rounded-lg text-sm text-stone-600 dark:text-gray-400
               hover:bg-stone-100 dark:hover:bg-gray-800"
      >
        {$t('me.edit_cancel')}
      </button>
      <button
        onclick={handleSave}
        disabled={!canSave}
        class="px-4 py-2 rounded-lg text-sm font-medium bg-teal-600 text-white
               hover:bg-teal-700 disabled:opacity-50 disabled:cursor-not-allowed"
      >
        {saving ? $t('me.loading') : $t('me.edit_save')}
      </button>
    </div>
  </div>
</div>
