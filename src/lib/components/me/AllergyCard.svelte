<!-- ALLERGY-01 B6: Allergy card with severity coding, cross-reactivity notes, and inline actions. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { AllergyInfo } from '$lib/types/me';
  import { deleteAllergy, verifyAllergy } from '$lib/api/allergy';
  import { CheckIcon, CloseIcon, EditIcon } from '$lib/components/icons/md';

  let { allergy, onedit, onrefresh }: {
    allergy: AllergyInfo;
    onedit?: (allergy: AllergyInfo) => void;
    onrefresh?: () => void;
  } = $props();

  let confirmDelete = $state(false);
  let deleting = $state(false);
  let verifying = $state(false);
  let deleteError = $state<string | null>(null);

  // Severity color classes (backend sends lowercase via str_enum!)
  let severityColor = $derived(
    allergy.severity === 'life_threatening'
      ? 'bg-red-100 dark:bg-red-900/40 text-red-700 dark:text-red-300'
      : allergy.severity === 'severe'
        ? 'bg-orange-100 dark:bg-orange-900/40 text-orange-700 dark:text-orange-300'
        : allergy.severity === 'moderate'
          ? 'bg-amber-100 dark:bg-amber-900/40 text-amber-700 dark:text-amber-300'
          : 'bg-emerald-100 dark:bg-emerald-900/40 text-emerald-700 dark:text-emerald-300'
  );

  let severityLabel = $derived(
    allergy.severity === 'life_threatening'
      ? $t('me.allergy_severity_life_threatening')
      : allergy.severity === 'severe'
        ? $t('me.allergy_severity_severe')
        : allergy.severity === 'moderate'
          ? $t('me.allergy_severity_moderate')
          : $t('me.allergy_severity_mild')
  );

  let categoryLabel = $derived(
    allergy.category
      ? $t(`me.allergy_cat_${allergy.category}`)
      : null
  );

  let sourceLabel = $derived(
    allergy.source === 'patient_reported'
      ? $t('me.allergy_source_patient')
      : $t('me.allergy_source_document')
  );

  // Icon badge color: severity-based left border
  let borderColor = $derived(
    allergy.severity === 'life_threatening'
      ? 'border-l-red-500'
      : allergy.severity === 'severe'
        ? 'border-l-orange-500'
        : allergy.severity === 'moderate'
          ? 'border-l-amber-500'
          : 'border-l-emerald-500'
  );

  function requestDelete() {
    confirmDelete = true;
    deleteError = null;
  }

  function cancelDelete() {
    confirmDelete = false;
    deleteError = null;
  }

  async function executeDelete() {
    deleting = true;
    deleteError = null;
    try {
      await deleteAllergy(allergy.id);
      confirmDelete = false;
      onrefresh?.();
    } catch (e) {
      deleteError = String(e);
      deleting = false;
    }
  }

  async function handleVerify() {
    if (allergy.verified || verifying) return;
    verifying = true;
    try {
      await verifyAllergy(allergy.id);
      onrefresh?.();
    } catch {
      verifying = false;
    }
  }
</script>

<div
  class="p-3 rounded-xl border border-l-4 bg-white dark:bg-gray-900
         border-stone-200 dark:border-gray-800 {borderColor} flex flex-col gap-2"
>
  <!-- Header row -->
  <div class="flex items-start gap-2">
    <div class="min-w-0 flex-1">
      <div class="flex items-center gap-2 flex-wrap">
        <p class="text-sm font-medium text-stone-800 dark:text-gray-100 leading-tight">
          {allergy.allergen}
        </p>
        <!-- Severity badge -->
        <span class="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium {severityColor}">
          {severityLabel}
        </span>
        <!-- Verified badge -->
        {#if allergy.verified}
          <span class="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded text-[10px] font-medium
                       bg-teal-100 dark:bg-teal-900/40 text-teal-700 dark:text-teal-300">
            <CheckIcon class="w-2.5 h-2.5" />
            {$t('me.allergy_verified')}
          </span>
        {/if}
      </div>

      <!-- Reaction -->
      {#if allergy.reaction}
        <p class="text-xs text-stone-600 dark:text-gray-300 mt-1">
          {allergy.reaction}
        </p>
      {/if}

      <!-- Details row: category + source + date -->
      <p class="text-[11px] text-stone-500 dark:text-gray-400 mt-1">
        {#if categoryLabel}{categoryLabel} · {/if}{sourceLabel}{#if allergy.date_identified} · {allergy.date_identified}{/if}
      </p>
    </div>

    <!-- Actions -->
    <div class="flex items-center gap-1 flex-shrink-0">
      {#if !allergy.verified}
        <button
          onclick={handleVerify}
          disabled={verifying}
          class="p-1 text-stone-400 dark:text-gray-500 hover:text-teal-600
                 dark:hover:text-teal-400 transition-colors"
          title={$t('me.allergy_verified')}
          aria-label={$t('me.allergy_verified')}
        >
          <CheckIcon class="w-3.5 h-3.5" />
        </button>
      {/if}
      {#if onedit}
        <button
          onclick={() => onedit?.(allergy)}
          class="p-1 text-stone-400 dark:text-gray-500 hover:text-stone-600
                 dark:hover:text-gray-300 transition-colors"
          aria-label={$t('me.allergy_edit_title')}
        >
          <EditIcon class="w-3.5 h-3.5" />
        </button>
      {/if}
      <!-- Delete: inline confirm (Signal pattern) -->
      {#if confirmDelete}
        <button
          onclick={executeDelete}
          disabled={deleting}
          class="p-1 text-red-500 hover:text-red-600"
          aria-label={$t('me.edit_save')}
        >
          <CheckIcon class="w-3.5 h-3.5" />
        </button>
        <button
          onclick={cancelDelete}
          class="p-1 text-stone-400 hover:text-stone-600 dark:hover:text-gray-300"
          aria-label={$t('me.edit_cancel')}
        >
          <CloseIcon class="w-3.5 h-3.5" />
        </button>
      {:else}
        <button
          onclick={requestDelete}
          class="p-1 text-stone-300 dark:text-gray-600 hover:text-red-400
                 dark:hover:text-red-400 transition-colors"
          aria-label={$t('me.allergy_delete_confirm')}
        >
          <CloseIcon class="w-3.5 h-3.5" />
        </button>
      {/if}
    </div>
  </div>

  <!-- Delete error -->
  {#if deleteError}
    <p class="text-[11px] text-red-500">{deleteError}</p>
  {/if}

  <!-- Cross-reactivity notes -->
  {#if allergy.cross_reactivities.length > 0}
    <div class="border-t border-stone-100 dark:border-gray-800 pt-1.5">
      <p class="text-[10px] text-stone-400 dark:text-gray-500 font-medium uppercase tracking-wide mb-1">
        {$t('me.allergy_cross_reactivity')}
      </p>
      <div class="flex flex-wrap gap-1">
        {#each allergy.cross_reactivities as note}
          <span class="px-1.5 py-0.5 rounded bg-stone-100 dark:bg-gray-800
                       text-[10px] text-stone-600 dark:text-gray-400">
            {note}
          </span>
        {/each}
      </div>
    </div>
  {/if}
</div>
