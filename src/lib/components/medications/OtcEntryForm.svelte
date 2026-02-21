<!-- L3-05: OTC medication entry form with autocomplete. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { addOtcMedication, searchMedicationAlias } from '$lib/api/medications';
  import type { AliasSearchResult } from '$lib/types/medication';
  import BackButton from '$lib/components/ui/BackButton.svelte';
  import Button from '$lib/components/ui/Button.svelte';

  interface Props {
    onBack: () => void;
    onAdded: () => void;
  }
  let { onBack, onAdded }: Props = $props();

  let name = $state('');
  let dose = $state('');
  let frequency = $state('');
  let route = $state('oral');
  let reason = $state('');
  let startDate = $state('');
  let instructions = $state('');
  let loading = $state(false);
  let error = $state('');

  let suggestions = $state<AliasSearchResult[]>([]);
  let showSuggestions = $state(false);
  let searchTimeout: ReturnType<typeof setTimeout> | null = $state(null);

  function handleNameInput(value: string) {
    name = value;
    if (searchTimeout) clearTimeout(searchTimeout);
    if (value.trim().length >= 2) {
      searchTimeout = setTimeout(async () => {
        try {
          suggestions = await searchMedicationAlias(value.trim(), 8);
          showSuggestions = suggestions.length > 0;
        } catch {
          suggestions = [];
          showSuggestions = false;
        }
      }, 200);
    } else {
      suggestions = [];
      showSuggestions = false;
    }
  }

  function selectSuggestion(result: AliasSearchResult) {
    name = result.generic_name;
    showSuggestions = false;
    suggestions = [];
  }

  async function handleSubmit() {
    error = '';

    if (!name.trim()) { error = $t('medications.otc_error_name'); return; }
    if (!dose.trim()) { error = $t('medications.otc_error_dose'); return; }
    if (!frequency.trim()) { error = $t('medications.otc_error_frequency'); return; }

    loading = true;
    try {
      await addOtcMedication({
        name: name.trim(),
        dose: dose.trim(),
        frequency: frequency.trim(),
        route,
        reason: reason.trim() || null,
        start_date: startDate || null,
        instructions: instructions.trim() || null,
      });
      onAdded();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  let routeOptions = $derived([
    { value: 'oral', label: $t('medications.otc_route_oral') },
    { value: 'topical', label: $t('medications.otc_route_topical') },
    { value: 'other', label: $t('medications.otc_route_other') },
  ]);
</script>

<div class="flex flex-col bg-stone-50 dark:bg-gray-950">
  <header class="px-6 pt-4 pb-2">
    <BackButton onclick={onBack} label={$t('medications.otc_back')} />
  </header>

  <div class="px-6 py-4">
    <h2 class="text-xl font-bold text-stone-800 dark:text-gray-100 mb-6">
      {$t('medications.otc_title')}
    </h2>

    <div class="flex flex-col gap-5">
      <!-- Medication name with autocomplete -->
      <label class="flex flex-col gap-1 relative">
        <span class="text-stone-600 dark:text-gray-300 text-sm font-medium">
          {$t('medications.otc_name_label')} <span class="text-[var(--color-danger)]">*</span>
        </span>
        <input
          type="text"
          value={name}
          oninput={(e) => handleNameInput(e.currentTarget.value)}
          onfocus={() => { if (suggestions.length > 0) showSuggestions = true; }}
          onblur={() => { setTimeout(() => { showSuggestions = false; }, 200); }}
          placeholder={$t('medications.otc_name_placeholder')}
          class="px-4 py-3 rounded-lg border border-stone-300 dark:border-gray-600 text-base dark:text-gray-200 min-h-[44px]
                 dark:bg-gray-900 focus:border-[var(--color-primary)] focus:outline-none"
          autocomplete="off"
          aria-label={$t('medications.otc_name_label')}
          aria-autocomplete="list"
        />
        <span class="text-xs text-stone-500 dark:text-gray-400">{$t('medications.otc_name_hint')}</span>

        {#if showSuggestions}
          <div
            class="absolute top-full left-0 right-0 mt-1 bg-white dark:bg-gray-900 rounded-lg shadow-lg
                   border border-stone-200 dark:border-gray-700 max-h-[200px] overflow-y-auto z-10"
            role="listbox"
          >
            {#each suggestions as result}
              <button
                class="w-full text-left px-4 py-3 hover:bg-stone-50 dark:hover:bg-gray-800 text-sm
                       min-h-[44px] border-b border-stone-50 dark:border-gray-950 last:border-0"
                role="option"
                aria-selected="false"
                onmousedown={() => selectSuggestion(result)}
              >
                <span class="font-medium text-stone-800 dark:text-gray-100">{result.generic_name}</span>
                {#if result.brand_names.length > 0}
                  <span class="text-stone-500 dark:text-gray-400 ml-1">
                    ({result.brand_names.slice(0, 3).join(', ')})
                  </span>
                {/if}
              </button>
            {/each}
          </div>
        {/if}
      </label>

      <!-- Dose -->
      <label class="flex flex-col gap-1">
        <span class="text-stone-600 dark:text-gray-300 text-sm font-medium">
          {$t('medications.otc_dose_label')} <span class="text-[var(--color-danger)]">*</span>
        </span>
        <input
          type="text"
          bind:value={dose}
          placeholder={$t('medications.otc_dose_placeholder')}
          class="px-4 py-3 rounded-lg border border-stone-300 dark:border-gray-600 text-base dark:text-gray-200 min-h-[44px]
                 dark:bg-gray-900 focus:border-[var(--color-primary)] focus:outline-none"
        />
      </label>

      <!-- Frequency -->
      <label class="flex flex-col gap-1">
        <span class="text-stone-600 dark:text-gray-300 text-sm font-medium">
          {$t('medications.otc_frequency_label')} <span class="text-[var(--color-danger)]">*</span>
        </span>
        <input
          type="text"
          bind:value={frequency}
          placeholder={$t('medications.otc_frequency_placeholder')}
          class="px-4 py-3 rounded-lg border border-stone-300 dark:border-gray-600 text-base dark:text-gray-200 min-h-[44px]
                 dark:bg-gray-900 focus:border-[var(--color-primary)] focus:outline-none"
        />
      </label>

      <!-- Route -->
      <fieldset class="flex flex-col gap-1">
        <legend class="text-stone-600 dark:text-gray-300 text-sm font-medium">{$t('medications.otc_route_label')}</legend>
        <div class="flex gap-3 mt-1">
          {#each routeOptions as option}
            <label
              class="flex items-center justify-center px-4 py-2 rounded-lg border
                     min-h-[44px] cursor-pointer transition-colors
                     {route === option.value
                       ? 'border-[var(--color-primary)] bg-[var(--color-info-50)] text-[var(--color-primary)]'
                       : 'border-stone-200 dark:border-gray-700 bg-white dark:bg-gray-900 text-stone-600 dark:text-gray-300 hover:bg-stone-50 dark:hover:bg-gray-800'}"
            >
              <input
                type="radio"
                name="route"
                value={option.value}
                bind:group={route}
                class="sr-only"
              />
              <span class="text-sm font-medium">{option.label}</span>
            </label>
          {/each}
        </div>
      </fieldset>

      <!-- Reason -->
      <label class="flex flex-col gap-1">
        <span class="text-stone-600 dark:text-gray-300 text-sm font-medium">{$t('medications.otc_reason_label')}</span>
        <input
          type="text"
          bind:value={reason}
          placeholder={$t('medications.otc_reason_placeholder')}
          class="px-4 py-3 rounded-lg border border-stone-300 dark:border-gray-600 text-base dark:text-gray-200 min-h-[44px]
                 dark:bg-gray-900 focus:border-[var(--color-primary)] focus:outline-none"
        />
      </label>

      <!-- Start date -->
      <label class="flex flex-col gap-1">
        <span class="text-stone-600 dark:text-gray-300 text-sm font-medium">{$t('medications.otc_start_date_label')}</span>
        <input
          type="date"
          bind:value={startDate}
          max={new Date().toISOString().split('T')[0]}
          class="px-4 py-3 rounded-lg border border-stone-300 dark:border-gray-600 text-base dark:text-gray-200 min-h-[44px]
                 dark:bg-gray-900 focus:border-[var(--color-primary)] focus:outline-none"
        />
      </label>

      <!-- Instructions -->
      <label class="flex flex-col gap-1">
        <span class="text-stone-600 dark:text-gray-300 text-sm font-medium">{$t('medications.otc_instructions_label')}</span>
        <textarea
          bind:value={instructions}
          placeholder={$t('medications.otc_instructions_placeholder')}
          rows="2"
          class="px-4 py-3 rounded-lg border border-stone-300 dark:border-gray-700 text-base dark:text-gray-200 min-h-[44px]
                 dark:bg-gray-900 focus:border-[var(--color-primary)] focus:outline-none resize-none"
        ></textarea>
      </label>

      {#if error}
        <p class="text-[var(--color-danger)] text-sm" role="alert">{error}</p>
      {/if}

      <div class="mt-2">
        <Button variant="primary" size="lg" fullWidth loading={loading}
                disabled={!name.trim() || !dose.trim() || !frequency.trim()}
                onclick={handleSubmit}>
          {loading ? $t('medications.otc_adding') : $t('medications.otc_submit')}
        </Button>
      </div>
    </div>
  </div>
</div>
