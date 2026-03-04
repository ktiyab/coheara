<!-- ALLERGY-01 B6: Modal for adding/editing allergies with autocomplete from canonical references. -->
<script lang="ts">
  import { t, locale } from 'svelte-i18n';
  import type { AllergyInfo } from '$lib/types/me';
  import type { AllergenReference } from '$lib/api/allergy';
  import { addAllergy, updateAllergy, getAllergenReferences } from '$lib/api/allergy';
  import { CloseIcon } from '$lib/components/icons/md';
  import { onMount } from 'svelte';

  let { allergy, onrecorded, onclose }: {
    allergy?: AllergyInfo | null;
    onrecorded: () => void;
    onclose: () => void;
  } = $props();

  let isEdit = $derived(allergy != null);

  // Form state
  let allergen = $state(allergy?.allergen ?? '');
  let reaction = $state(allergy?.reaction ?? '');
  let severity = $state(allergy?.severity ?? 'mild');
  let category = $state(allergy?.category ?? '');
  let dateIdentified = $state(allergy?.date_identified ?? '');
  let saving = $state(false);
  let saveError = $state<string | null>(null);

  // Autocomplete state
  let references = $state<AllergenReference[]>([]);
  let filterCategory = $state<string | null>(null);
  let showSuggestions = $state(false);

  let filteredRefs = $derived(
    references.filter(r => {
      if (filterCategory && r.category !== filterCategory) return false;
      if (!allergen.trim()) return true;
      return r.label.toLowerCase().includes(allergen.toLowerCase())
        || r.key.toLowerCase().includes(allergen.toLowerCase());
    }).slice(0, 12)
  );

  let canSave = $derived(allergen.trim().length > 0 && !saving);

  const categories = [
    { key: null, label: 'allergy_cat_all' },
    { key: 'food', label: 'allergy_cat_food' },
    { key: 'drug', label: 'allergy_cat_drug' },
    { key: 'environmental', label: 'allergy_cat_environmental' },
    { key: 'insect', label: 'allergy_cat_insect' },
    { key: 'other', label: 'allergy_cat_other' },
  ];

  const severityOptions = [
    { key: 'mild', label: 'allergy_severity_mild', color: 'bg-emerald-500' },
    { key: 'moderate', label: 'allergy_severity_moderate', color: 'bg-amber-500' },
    { key: 'severe', label: 'allergy_severity_severe', color: 'bg-orange-500' },
    { key: 'life_threatening', label: 'allergy_severity_life_threatening', color: 'bg-red-500' },
  ];

  onMount(async () => {
    try {
      references = await getAllergenReferences($locale ?? 'en');
    } catch {
      // Autocomplete unavailable — user can still type freely
    }
  });

  function selectRef(ref: AllergenReference) {
    allergen = ref.label;
    category = ref.category;
    showSuggestions = false;
  }

  async function handleSave() {
    if (!canSave) return;
    saving = true;
    saveError = null;

    try {
      if (isEdit && allergy) {
        await updateAllergy(
          allergy.id,
          allergen,
          reaction || null,
          severity,
          category || null,
          dateIdentified || null,
        );
      } else {
        await addAllergy(
          allergen,
          severity,
          reaction || null,
          category || null,
          dateIdentified || null,
        );
      }
      onrecorded();
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
  onkeydown={(e) => { if (e.key === 'Escape') onclose(); }}
  role="dialog"
  aria-modal="true"
  aria-label={isEdit ? $t('me.allergy_edit_title') : $t('me.allergy_add_title')}
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
        {isEdit ? $t('me.allergy_edit_title') : $t('me.allergy_add_title')}
      </h2>
      <button onclick={onclose}
        class="text-stone-400 dark:text-gray-400 hover:text-stone-600
               dark:hover:text-gray-300">
        <CloseIcon class="w-5 h-5" />
      </button>
    </div>

    <div class="p-4 space-y-4">
      <!-- Category filter tabs -->
      <div class="flex flex-wrap gap-1.5">
        {#each categories as cat}
          <button
            onclick={() => { filterCategory = cat.key; }}
            class="px-2.5 py-1 rounded-full text-xs font-medium transition-colors
              {filterCategory === cat.key
                ? 'bg-teal-600 text-white'
                : 'bg-stone-100 dark:bg-gray-800 text-stone-600 dark:text-gray-400 hover:bg-stone-200 dark:hover:bg-gray-700'}"
          >
            {$t(`me.${cat.label}`)}
          </button>
        {/each}
      </div>

      <!-- Allergen input with autocomplete -->
      <div class="relative">
        <label class="text-sm font-medium text-stone-700 dark:text-gray-300 mb-1 block">
          {$t('me.allergy_allergen_label')}
        </label>
        <input
          type="text"
          bind:value={allergen}
          onfocus={() => { showSuggestions = true; }}
          onblur={() => { setTimeout(() => { showSuggestions = false; }, 200); }}
          placeholder={$t('me.allergy_allergen_placeholder')}
          autofocus
          class="w-full px-3 py-2 rounded-lg border text-sm
            border-stone-200 dark:border-gray-700
            bg-white dark:bg-gray-800 text-stone-800 dark:text-gray-100"
        />

        <!-- Autocomplete dropdown -->
        {#if showSuggestions && filteredRefs.length > 0}
          <div class="absolute left-0 right-0 top-full mt-1 z-50 bg-white dark:bg-gray-900
                      border border-stone-200 dark:border-gray-700 rounded-lg shadow-lg
                      max-h-48 overflow-y-auto">
            {#each filteredRefs as ref (ref.key)}
              <button
                class="w-full text-left px-3 py-2 text-sm hover:bg-stone-50
                       dark:hover:bg-gray-800 flex items-center gap-2"
                onmousedown={() => selectRef(ref)}
              >
                <span class="text-stone-800 dark:text-gray-100">{ref.label}</span>
                <span class="text-[10px] text-stone-400 dark:text-gray-500 ml-auto">
                  {ref.category}
                </span>
              </button>
            {/each}
          </div>
        {/if}
      </div>

      <!-- Severity selector -->
      <div>
        <label class="text-sm font-medium text-stone-700 dark:text-gray-300 mb-1.5 block">
          {$t('me.allergy_severity_label')}
        </label>
        <div class="grid grid-cols-4 gap-1.5">
          {#each severityOptions as sev}
            <button
              onclick={() => { severity = sev.key; }}
              class="flex flex-col items-center gap-1 px-2 py-2 rounded-lg text-xs font-medium
                     transition-colors border
                {severity === sev.key
                  ? 'border-teal-500 bg-teal-50 dark:bg-teal-900/30 text-teal-700 dark:text-teal-300'
                  : 'border-stone-200 dark:border-gray-700 text-stone-600 dark:text-gray-400 hover:border-stone-300 dark:hover:border-gray-600'}"
            >
              <span class="w-2.5 h-2.5 rounded-full {sev.color}"></span>
              <span class="text-center leading-tight">{$t(`me.${sev.label}`)}</span>
            </button>
          {/each}
        </div>
      </div>

      <!-- Reaction (optional) -->
      <div>
        <label class="text-sm font-medium text-stone-700 dark:text-gray-300 mb-1 block">
          {$t('me.allergy_reaction_label')}
        </label>
        <textarea
          bind:value={reaction}
          maxlength={500}
          rows={2}
          placeholder={$t('me.allergy_reaction_placeholder')}
          class="w-full px-3 py-2 rounded-lg border text-sm resize-none
            border-stone-200 dark:border-gray-700
            bg-white dark:bg-gray-800 text-stone-800 dark:text-gray-100"
        ></textarea>
      </div>

      <!-- Date identified (optional) -->
      <div>
        <label class="text-sm font-medium text-stone-700 dark:text-gray-300 mb-1 block">
          {$t('me.allergy_date_label')}
        </label>
        <input
          type="date"
          bind:value={dateIdentified}
          max={new Date().toISOString().slice(0, 10)}
          class="w-full px-3 py-2 rounded-lg border text-sm
            border-stone-200 dark:border-gray-700
            bg-white dark:bg-gray-800 text-stone-800 dark:text-gray-100"
        />
      </div>

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
