<!-- ME-04 B5: Enhanced identity card with demographic completeness + edit. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { MeIdentity } from '$lib/types/me';
  import { EditIcon } from '$lib/components/icons/md';
  import EditDemographicsModal from './EditDemographicsModal.svelte';

  let { identity, onUpdated }: {
    identity: MeIdentity;
    onUpdated: () => void;
  } = $props();

  let showEditModal = $state(false);

  let initials = $derived(
    identity.name
      .split(/\s+/)
      .slice(0, 2)
      .map(w => w[0]?.toUpperCase() ?? '')
      .join('')
  );

  let sexLabel = $derived(
    identity.sex === 'male'
      ? $t('me.sex_male')
      : identity.sex === 'female'
        ? $t('me.sex_female')
        : null
  );

  let ageText = $derived(
    identity.age != null
      ? $t('me.age_label', { values: { age: identity.age } })
      : null
  );

  /** Map PascalCase ethnicity variant to i18n snake_case key. */
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

  let ethnicityText = $derived(
    identity.ethnicities.length > 0
      ? identity.ethnicities
          .map(e => $t(`profile.ethnicity_${ETHNICITY_KEY_MAP[e] ?? e.toLowerCase()}`))
          .join(', ')
      : null
  );

  let weightText = $derived(
    identity.weight_kg != null ? `${identity.weight_kg} kg` : null
  );

  let heightText = $derived(
    identity.height_cm != null ? `${identity.height_cm} cm` : null
  );

  let bloodTypeText = $derived(identity.blood_type_display ?? null);

  let bmiText = $derived(
    identity.bmi != null
      ? `${identity.bmi.toFixed(1)} kg/m\u00b2`
      : (identity.weight_kg == null || identity.height_cm == null)
        ? $t('me.field_needs_wh')
        : null
  );

  type DemoField = { label: string; value: string | null; auto?: boolean };
  let fields = $derived<DemoField[]>([
    { label: $t('me.field_sex'), value: sexLabel },
    { label: $t('me.field_age'), value: ageText, auto: true },
    { label: $t('me.field_blood_type'), value: bloodTypeText },
    { label: $t('me.field_ethnicity'), value: ethnicityText },
    { label: $t('me.field_weight'), value: weightText },
    { label: $t('me.field_height'), value: heightText },
    { label: $t('me.field_bmi'), value: bmiText },
  ]);
</script>

<div class="p-4 rounded-xl bg-white dark:bg-gray-900 border border-stone-200
            dark:border-gray-800">
  <!-- Header: avatar + name + edit -->
  <div class="flex items-start gap-3">
    <div class="w-11 h-11 rounded-full bg-teal-100 dark:bg-teal-900/50
                flex items-center justify-center flex-shrink-0">
      <span class="text-sm font-bold text-teal-700 dark:text-teal-300">
        {initials}
      </span>
    </div>
    <div class="flex-1 min-w-0">
      <p class="text-lg font-semibold text-stone-800 dark:text-gray-100
                truncate">{identity.name}</p>
      <div class="flex items-center gap-1.5 flex-wrap">
        <p class="text-sm text-stone-500 dark:text-gray-400">
          {#if ageText}{ageText}{/if}
          {#if ageText && sexLabel}
            <span class="text-stone-300 dark:text-gray-600"> &middot; </span>
          {/if}
          {#if sexLabel}{sexLabel}{/if}
        </p>
        {#if bloodTypeText}
          <span class="inline-flex items-center px-1.5 py-0.5 rounded text-[11px]
                       font-bold bg-red-50 dark:bg-red-900/30 text-red-700
                       dark:text-red-300 border border-red-200 dark:border-red-800">
            {bloodTypeText}
          </span>
        {/if}
      </div>
    </div>
    <button
      onclick={() => showEditModal = true}
      class="text-xs text-teal-600 dark:text-teal-400 hover:text-teal-700
             dark:hover:text-teal-300 flex items-center gap-1 flex-shrink-0
             mt-1"
    >
      <EditIcon class="w-3 h-3" />
      {$t('me.edit_profile')}
    </button>
  </div>

  <!-- Demographics table -->
  <div class="mt-3 pt-3 border-t border-stone-100 dark:border-gray-800">
    <div class="space-y-1.5">
      {#each fields as field}
        <div class="flex items-center text-sm">
          <span class="w-4 flex-shrink-0 {field.value
            ? 'text-teal-500 dark:text-teal-400'
            : 'text-stone-300 dark:text-gray-600'}">
            {field.value ? '\u2713' : '\u25cb'}
          </span>
          <span class="w-24 text-stone-500 dark:text-gray-400 flex-shrink-0">
            {field.label}
          </span>
          <span class="flex-1 {field.value
            ? 'text-stone-700 dark:text-gray-200'
            : 'text-stone-400 dark:text-gray-400 italic'}">
            {field.value ?? $t('me.field_not_set')}
            {#if field.auto && field.value}
              <span class="text-[10px] text-stone-400 dark:text-gray-400 ml-1">
                ({$t('me.field_auto')})
              </span>
            {/if}
          </span>
        </div>
      {/each}
    </div>
  </div>

  <!-- Entity counts -->
  {#if identity.medication_count > 0 || identity.allergy_count > 0}
    <div class="mt-3 pt-3 border-t border-stone-100 dark:border-gray-800
                flex flex-wrap gap-2 text-sm text-stone-500 dark:text-gray-400">
      {#if identity.medication_count > 0}
        <span>{$t('me.medications_label', {
          values: { count: identity.medication_count }
        })}</span>
      {/if}
      {#if identity.medication_count > 0 && identity.allergy_count > 0}
        <span class="text-stone-300 dark:text-gray-600">&middot;</span>
      {/if}
      {#if identity.allergy_count > 0}
        <span>{$t('me.allergies_label', {
          values: { count: identity.allergy_count }
        })}</span>
      {/if}
    </div>
  {/if}
</div>

{#if showEditModal}
  <EditDemographicsModal
    {identity}
    onclose={() => showEditModal = false}
    onsaved={() => {
      showEditModal = false;
      onUpdated();
    }}
  />
{/if}
