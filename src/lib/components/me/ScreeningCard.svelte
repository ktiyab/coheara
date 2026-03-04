<!-- ME-06 QA: Screening/vaccine card with expired badge, delete confirm, grid-compatible. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { ScreeningInfo, CompletedDose } from '$lib/types/me';
  import { deleteScreeningRecord } from '$lib/api/me';
  import { CalendarIcon, CheckIcon, CloseIcon } from '$lib/components/icons/md';

  let { screening, onrecord, onrefresh }: {
    screening: ScreeningInfo;
    onrecord?: (screening: ScreeningInfo) => void;
    onrefresh?: () => void;
  } = $props();

  let deleting = $state<string | null>(null);
  let confirmDelete = $state<string | null>(null);
  let deleteError = $state<string | null>(null);

  let intervalText = $derived(
    screening.interval_months === 0
      ? (screening.total_doses > 0
        ? $t('me.vaccine_series', { values: { total: screening.total_doses } })
        : $t('me.screening_one_time'))
      : $t('me.screening_interval', { values: { months: screening.interval_months } })
  );

  let ageText = $derived(
    screening.max_age != null
      ? $t('me.screening_age_range', { values: { min: screening.min_age, max: screening.max_age } })
      : $t('me.screening_age_from', { values: { min: screening.min_age } })
  );

  let sexText = $derived(
    screening.sex_required === 'female'
      ? $t('me.screening_female_only')
      : screening.sex_required === 'male'
        ? $t('me.screening_male_only')
        : null
  );

  let isVaccine = $derived(screening.category === 'vaccine');
  let showRecordButton = $derived(screening.eligible && !screening.is_complete);

  // F1: Expired detection — !is_complete && next_due means validity window passed
  let isExpired = $derived(!screening.is_complete && screening.next_due != null);

  // Icon badge color: expired=amber, complete=emerald, eligible=teal, else=stone
  let iconBgClass = $derived(
    screening.is_complete
      ? 'bg-emerald-100 dark:bg-emerald-900/50 text-emerald-600 dark:text-emerald-300'
      : isExpired
        ? 'bg-amber-100 dark:bg-amber-900/50 text-amber-600 dark:text-amber-300'
        : screening.eligible
          ? 'bg-teal-100 dark:bg-teal-900/50 text-teal-600 dark:text-teal-300'
          : 'bg-stone-100 dark:bg-gray-800 text-stone-400 dark:text-gray-400'
  );

  function requestDelete(dose: CompletedDose) {
    confirmDelete = dose.record_id;
    deleteError = null;
  }

  function cancelDelete() {
    confirmDelete = null;
    deleteError = null;
  }

  async function executeDelete(dose: CompletedDose) {
    deleting = dose.record_id;
    deleteError = null;
    try {
      await deleteScreeningRecord(dose.record_id);
      confirmDelete = null;
      onrefresh?.();
    } catch (e) {
      deleteError = String(e);
      deleting = null;
    }
  }
</script>

<div
  class="p-3 rounded-xl border flex flex-col gap-2 transition-opacity
    {screening.eligible
      ? 'bg-white dark:bg-gray-900 border-stone-200 dark:border-gray-800'
      : 'bg-stone-50 dark:bg-gray-900/50 border-stone-100 dark:border-gray-800/50 opacity-60'}"
>
  <!-- Header row -->
  <div class="flex items-start gap-2">
    <span class="w-6 h-6 rounded-full flex items-center justify-center flex-shrink-0 {iconBgClass}">
      {#if screening.is_complete}
        <CheckIcon class="w-3 h-3" />
      {:else}
        <CalendarIcon class="w-3 h-3" />
      {/if}
    </span>

    <div class="min-w-0 flex-1">
      <p class="text-sm font-medium text-stone-800 dark:text-gray-100 leading-tight">
        {screening.label}
      </p>

      <!-- F1: 5-state status badge -->
      {#if screening.is_complete && screening.next_due}
        <!-- Recurring valid: show expiry date -->
        <p class="text-xs text-emerald-600 dark:text-emerald-400 mt-0.5">
          {$t('me.vaccine_valid_until', { values: { date: screening.next_due } })}
        </p>
      {:else if screening.is_complete}
        <!-- Multi-dose series complete -->
        <p class="text-xs text-emerald-600 dark:text-emerald-400 mt-0.5">
          {$t('me.vaccine_complete')}
        </p>
      {:else if isExpired}
        <!-- Expired recurring — amber (Apple Health: "Overdue") -->
        <p class="text-xs text-amber-600 dark:text-amber-400 mt-0.5">
          {$t('me.vaccine_expired')}
        </p>
      {:else if screening.eligible}
        <p class="text-xs text-emerald-600 dark:text-emerald-400 mt-0.5">
          {$t('me.screenings_eligible')}
        </p>
      {:else}
        <p class="text-xs text-stone-400 dark:text-gray-400 mt-0.5">
          {$t('me.screenings_not_eligible')}
        </p>
      {/if}

      <!-- Details -->
      <p class="text-[11px] text-stone-500 dark:text-gray-400 mt-1">
        {intervalText} · {ageText}{#if sexText} · {sexText}{/if}
      </p>
      <p class="text-[10px] text-stone-400 dark:text-gray-400 mt-0.5">{screening.source}</p>
    </div>
  </div>

  <!-- Dose history (vaccines with records) -->
  {#if screening.completed_doses.length > 0}
    <div class="flex flex-col gap-1">
      {#each screening.completed_doses as dose (dose.record_id)}
        <div class="flex items-center gap-1.5 text-xs text-stone-600 dark:text-gray-300">
          <CheckIcon class="w-3 h-3 text-emerald-500 flex-shrink-0" />
          <span class="truncate flex-1">
            {#if screening.total_doses > 0}
              {$t('me.vaccine_dose_of', { values: { current: dose.dose_number, total: screening.total_doses } })}
              -
            {/if}
            {dose.completed_at}
            {#if dose.provider}
              - {dose.provider}
            {/if}
          </span>
          <!-- F2: Inline delete confirmation (Signal pattern) -->
          {#if confirmDelete === dose.record_id}
            <span class="flex items-center gap-1 ml-auto flex-shrink-0">
              <button
                onclick={() => executeDelete(dose)}
                disabled={deleting === dose.record_id}
                class="text-red-500 hover:text-red-600 p-0.5"
                aria-label={$t('me.edit_save')}
              >
                <CheckIcon class="w-3 h-3" />
              </button>
              <button
                onclick={cancelDelete}
                class="text-stone-400 hover:text-stone-600 dark:hover:text-gray-300 p-0.5"
                aria-label={$t('me.edit_cancel')}
              >
                <CloseIcon class="w-3 h-3" />
              </button>
            </span>
          {:else}
            <button
              class="ml-auto text-stone-300 dark:text-gray-600 hover:text-red-400
                     dark:hover:text-red-400 transition-colors p-0.5 flex-shrink-0"
              onclick={() => requestDelete(dose)}
              disabled={deleting === dose.record_id}
              aria-label={$t('me.vaccine_delete_confirm')}
            >
              <CloseIcon class="w-3 h-3" />
            </button>
          {/if}
        </div>
      {/each}
      <!-- F2: Delete error feedback -->
      {#if deleteError}
        <p class="text-[11px] text-red-500 mt-0.5">{deleteError}</p>
      {/if}
    </div>
  {/if}

  <!-- Multi-dose: show remaining empty slots -->
  {#if isVaccine && screening.total_doses > 0 && screening.completed_doses.length < screening.total_doses}
    <div class="flex flex-col gap-1">
      {#each Array(screening.total_doses - screening.completed_doses.length) as _, i}
        <div class="flex items-center gap-1.5 text-xs text-stone-400 dark:text-gray-400">
          <span class="w-3 h-3 rounded-full border border-stone-300 dark:border-gray-600 flex-shrink-0"></span>
          <span class="truncate">
            {$t('me.vaccine_dose_of', { values: { current: screening.completed_doses.length + i + 1, total: screening.total_doses } })}
            - {$t('me.vaccine_dose_not_recorded')}
          </span>
        </div>
      {/each}
    </div>
  {/if}

  <!-- Record action -->
  {#if showRecordButton && onrecord}
    <button
      onclick={() => onrecord?.(screening)}
      class="w-full px-3 py-1.5 rounded-lg text-xs font-medium bg-teal-50 dark:bg-teal-900/30
             text-teal-700 dark:text-teal-300 hover:bg-teal-100 dark:hover:bg-teal-900/50
             transition-colors mt-auto"
    >
      {isVaccine
        ? $t('me.vaccine_record_btn')
        : $t('me.screening_record_btn')}
    </button>
  {/if}
</div>
