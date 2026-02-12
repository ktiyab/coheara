<!-- L3-05: OTC medication entry form with autocomplete. -->
<script lang="ts">
  import { addOtcMedication, searchMedicationAlias } from '$lib/api/medications';
  import type { AliasSearchResult } from '$lib/types/medication';

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

    if (!name.trim()) { error = 'Please enter a medication name.'; return; }
    if (!dose.trim()) { error = 'Please enter a dose.'; return; }
    if (!frequency.trim()) { error = 'Please enter how often you take it.'; return; }

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

  const routeOptions = [
    { value: 'oral', label: 'Oral' },
    { value: 'topical', label: 'Topical' },
    { value: 'other', label: 'Other' },
  ];
</script>

<div class="flex flex-col min-h-screen pb-20 bg-stone-50">
  <header class="px-6 pt-4 pb-2">
    <button
      class="text-stone-400 hover:text-stone-600 min-h-[44px] min-w-[44px]"
      onclick={onBack}
      aria-label="Back to medication list"
    >
      &larr; Back
    </button>
  </header>

  <div class="px-6 py-4">
    <h2 class="text-xl font-bold text-stone-800 mb-6">
      Add an over-the-counter medication
    </h2>

    <div class="flex flex-col gap-5">
      <!-- Medication name with autocomplete -->
      <label class="flex flex-col gap-1 relative">
        <span class="text-stone-600 text-sm font-medium">
          Medication name <span class="text-red-500">*</span>
        </span>
        <input
          type="text"
          value={name}
          oninput={(e) => handleNameInput(e.currentTarget.value)}
          onfocus={() => { if (suggestions.length > 0) showSuggestions = true; }}
          onblur={() => { setTimeout(() => { showSuggestions = false; }, 200); }}
          placeholder="Ibuprofen"
          class="px-4 py-3 rounded-lg border border-stone-300 text-base min-h-[44px]
                 focus:border-[var(--color-primary)] focus:outline-none"
          autocomplete="off"
          aria-label="Medication name"
          aria-autocomplete="list"
        />
        <span class="text-xs text-stone-400">Start typing to search known medications</span>

        {#if showSuggestions}
          <div
            class="absolute top-full left-0 right-0 mt-1 bg-white rounded-lg shadow-lg
                   border border-stone-200 max-h-[200px] overflow-y-auto z-10"
            role="listbox"
          >
            {#each suggestions as result}
              <button
                class="w-full text-left px-4 py-3 hover:bg-stone-50 text-sm
                       min-h-[44px] border-b border-stone-50 last:border-0"
                role="option"
                onmousedown={() => selectSuggestion(result)}
              >
                <span class="font-medium text-stone-800">{result.generic_name}</span>
                {#if result.brand_names.length > 0}
                  <span class="text-stone-400 ml-1">
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
        <span class="text-stone-600 text-sm font-medium">
          Dose <span class="text-red-500">*</span>
        </span>
        <input
          type="text"
          bind:value={dose}
          placeholder="400mg"
          class="px-4 py-3 rounded-lg border border-stone-300 text-base min-h-[44px]
                 focus:border-[var(--color-primary)] focus:outline-none"
        />
      </label>

      <!-- Frequency -->
      <label class="flex flex-col gap-1">
        <span class="text-stone-600 text-sm font-medium">
          How often? <span class="text-red-500">*</span>
        </span>
        <input
          type="text"
          bind:value={frequency}
          placeholder="As needed for pain"
          class="px-4 py-3 rounded-lg border border-stone-300 text-base min-h-[44px]
                 focus:border-[var(--color-primary)] focus:outline-none"
        />
      </label>

      <!-- Route -->
      <fieldset class="flex flex-col gap-1">
        <legend class="text-stone-600 text-sm font-medium">How do you take it?</legend>
        <div class="flex gap-3 mt-1">
          {#each routeOptions as option}
            <label
              class="flex items-center justify-center px-4 py-2 rounded-lg border
                     min-h-[44px] cursor-pointer transition-colors
                     {route === option.value
                       ? 'border-[var(--color-primary)] bg-blue-50 text-[var(--color-primary)]'
                       : 'border-stone-200 bg-white text-stone-600 hover:bg-stone-50'}"
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
        <span class="text-stone-600 text-sm font-medium">Why are you taking it?</span>
        <input
          type="text"
          bind:value={reason}
          placeholder="Headaches and muscle pain"
          class="px-4 py-3 rounded-lg border border-stone-300 text-base min-h-[44px]
                 focus:border-[var(--color-primary)] focus:outline-none"
        />
      </label>

      <!-- Start date -->
      <label class="flex flex-col gap-1">
        <span class="text-stone-600 text-sm font-medium">When did you start?</span>
        <input
          type="date"
          bind:value={startDate}
          max={new Date().toISOString().split('T')[0]}
          class="px-4 py-3 rounded-lg border border-stone-300 text-base min-h-[44px]
                 focus:border-[var(--color-primary)] focus:outline-none"
        />
      </label>

      <!-- Instructions -->
      <label class="flex flex-col gap-1">
        <span class="text-stone-600 text-sm font-medium">Special instructions</span>
        <textarea
          bind:value={instructions}
          placeholder="Take with food"
          rows="2"
          class="px-4 py-3 rounded-lg border border-stone-300 text-base min-h-[44px]
                 focus:border-[var(--color-primary)] focus:outline-none resize-none"
        ></textarea>
      </label>

      {#if error}
        <p class="text-red-600 text-sm" role="alert">{error}</p>
      {/if}

      <button
        class="mt-2 px-8 py-4 bg-[var(--color-primary)] text-white rounded-xl text-lg
               font-medium hover:brightness-110 disabled:opacity-50 min-h-[44px]"
        onclick={handleSubmit}
        disabled={loading || !name.trim() || !dose.trim() || !frequency.trim()}
      >
        {loading ? 'Adding...' : 'Add medication'}
      </button>
    </div>
  </div>
</div>
