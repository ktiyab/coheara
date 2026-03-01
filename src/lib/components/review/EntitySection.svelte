<!-- RV-UX: Section wrapper with colored header, confidence banner, and entity rows. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type {
    EntityCategory,
    ReviewEntity,
    PlausibilityWarning,
    FieldCorrection,
  } from '$lib/types/review';
  import { WarningIcon } from '$lib/components/icons/md';
  import EntityRow from './EntityRow.svelte';

  interface Props {
    category: EntityCategory;
    entities: ReviewEntity[];
    warnings: PlausibilityWarning[];
    corrections: FieldCorrection[];
    onCorrection: (correction: FieldCorrection) => void;
  }
  let { category, entities, warnings, corrections, onCorrection }: Props = $props();

  const categoryStyles: Record<EntityCategory, {
    i18nKey: string;
    headerClass: string;
    borderClass: string;
  }> = {
    Medication: {
      i18nKey: 'review.category_medications',
      headerClass: 'bg-blue-50 text-blue-800 dark:bg-blue-950 dark:text-blue-200',
      borderClass: 'border-blue-200 dark:border-blue-800',
    },
    LabResult: {
      i18nKey: 'review.category_lab_results',
      headerClass: 'bg-[var(--color-success-50)] text-[var(--color-success-800)]',
      borderClass: 'border-[var(--color-success-200)]',
    },
    Diagnosis: {
      i18nKey: 'review.category_diagnoses',
      headerClass: 'bg-indigo-50 text-indigo-800 dark:bg-indigo-950 dark:text-indigo-200',
      borderClass: 'border-indigo-200 dark:border-indigo-800',
    },
    Allergy: {
      i18nKey: 'review.category_allergies',
      headerClass: 'bg-red-50 text-red-800 dark:bg-red-950 dark:text-red-200',
      borderClass: 'border-red-200 dark:border-red-800',
    },
    Procedure: {
      i18nKey: 'review.category_procedures',
      headerClass: 'bg-teal-50 text-teal-800 dark:bg-teal-950 dark:text-teal-200',
      borderClass: 'border-teal-200 dark:border-teal-800',
    },
    Referral: {
      i18nKey: 'review.category_referrals',
      headerClass: 'bg-violet-50 text-violet-800 dark:bg-violet-950 dark:text-violet-200',
      borderClass: 'border-violet-200 dark:border-violet-800',
    },
    Professional: {
      i18nKey: 'review.category_professional',
      headerClass: 'bg-purple-50 text-purple-800 dark:bg-purple-950 dark:text-purple-200',
      borderClass: 'border-purple-200 dark:border-purple-800',
    },
    Date: {
      i18nKey: 'review.category_date',
      headerClass: 'bg-amber-50 text-amber-800 dark:bg-amber-950 dark:text-amber-200',
      borderClass: 'border-amber-200 dark:border-amber-800',
    },
  };

  let config = $derived(categoryStyles[category]);
  let hasFlagged = $derived(entities.some(e => e.isFlagged));
</script>

<section>
  <!-- Section header -->
  <h2 class="text-sm font-semibold px-3 py-2 rounded-t-lg {config.headerClass}">
    {$t(config.i18nKey)}
    <span class="font-normal opacity-70">
      ({$t('review.entity_count', { values: { count: entities.length } })})
    </span>
  </h2>

  <div class="border border-t-0 rounded-b-lg {config.borderClass}
              bg-white dark:bg-gray-900">
    <!-- Confidence banner — single per section -->
    {#if hasFlagged}
      <div class="flex items-center gap-2 px-3 py-2.5
                  bg-[var(--color-warning-50)] border-b border-[var(--color-warning-200)]
                  text-[var(--color-warning-800)]">
        <WarningIcon class="w-4 h-4 shrink-0" />
        <p class="text-xs">{$t('review.section_confidence_banner')}</p>
      </div>
    {/if}

    <!-- Entity rows -->
    <div class="divide-y divide-stone-100 dark:divide-gray-800">
      {#each entities as entity (`${entity.category}:${entity.entityIndex}`)}
        <EntityRow
          {entity}
          {corrections}
          {onCorrection}
        />
      {/each}
    </div>
  </div>
</section>
