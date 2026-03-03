<!-- ME-06: Modal for recording a screening/vaccination date. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { ScreeningInfo } from '$lib/types/me';
  import { recordScreening } from '$lib/api/me';
  import { CloseIcon } from '$lib/components/icons/md';

  let { screening, onrecorded, onclose }: {
    screening: ScreeningInfo;
    onrecorded: () => void;
    onclose: () => void;
  } = $props();

  let completedAt = $state('');
  let provider = $state('');
  let notes = $state('');
  let saving = $state(false);
  let saveError = $state<string | null>(null);

  let isVaccine = $derived(screening.category === 'vaccine');
  let nextDose = $derived(
    screening.total_doses > 0
      ? screening.completed_doses.length + 1
      : 1
  );

  // Date validation: not in future
  let today = new Date().toISOString().slice(0, 10);
  let dateValid = $derived(completedAt !== '' && completedAt <= today);
  let canSave = $derived(dateValid && !saving);

  async function handleSave() {
    if (!canSave) return;
    saving = true;
    saveError = null;

    try {
      await recordScreening(
        screening.key,
        nextDose,
        completedAt,
        provider || null,
        notes || null,
      );
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
  aria-label={isVaccine ? $t('me.vaccine_record_title') : $t('me.screening_record_btn')}
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
        {isVaccine ? $t('me.vaccine_record_title') : $t('me.screening_record_btn')}
      </h2>
      <button onclick={onclose}
        class="text-stone-400 dark:text-gray-500 hover:text-stone-600
               dark:hover:text-gray-300">
        <CloseIcon class="w-5 h-5" />
      </button>
    </div>

    <div class="p-4 space-y-4">
      <!-- Screening info -->
      <div>
        <p class="text-sm font-medium text-stone-800 dark:text-gray-100">
          {screening.label}
        </p>
        <p class="text-xs text-stone-400 dark:text-gray-500 mt-0.5">
          {screening.source}
        </p>
        {#if screening.total_doses > 0}
          <p class="text-xs text-teal-600 dark:text-teal-400 mt-1">
            {$t('me.vaccine_dose_of', { values: { current: nextDose, total: screening.total_doses } })}
          </p>
        {/if}
      </div>

      <!-- Date -->
      <div>
        <label class="text-sm font-medium text-stone-700 dark:text-gray-300 mb-1 block">
          {$t('me.vaccine_date_label')}
        </label>
        <input
          type="date"
          bind:value={completedAt}
          max={today}
          autofocus
          class="w-full px-3 py-2 rounded-lg border text-sm
            {dateValid || completedAt === ''
              ? 'border-stone-200 dark:border-gray-700'
              : 'border-red-400 dark:border-red-600'}
            bg-white dark:bg-gray-800 text-stone-800 dark:text-gray-100"
        />
        {#if completedAt !== '' && !dateValid}
          <p class="text-xs text-red-500 mt-1">{$t('me.vaccine_date_future')}</p>
        {/if}
      </div>

      <!-- Provider (optional) -->
      <div>
        <label class="text-sm font-medium text-stone-700 dark:text-gray-300 mb-1 block">
          {$t('me.vaccine_provider_label')}
        </label>
        <input
          type="text"
          bind:value={provider}
          maxlength={200}
          placeholder="-"
          class="w-full px-3 py-2 rounded-lg border text-sm
            border-stone-200 dark:border-gray-700
            bg-white dark:bg-gray-800 text-stone-800 dark:text-gray-100"
        />
      </div>

      <!-- Notes (optional) -->
      <div>
        <label class="text-sm font-medium text-stone-700 dark:text-gray-300 mb-1 block">
          {$t('me.vaccine_notes_label')}
        </label>
        <textarea
          bind:value={notes}
          maxlength={500}
          rows={2}
          placeholder="-"
          class="w-full px-3 py-2 rounded-lg border text-sm resize-none
            border-stone-200 dark:border-gray-700
            bg-white dark:bg-gray-800 text-stone-800 dark:text-gray-100"
        ></textarea>
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
