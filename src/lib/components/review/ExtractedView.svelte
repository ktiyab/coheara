<!-- L3-04: Extracted content view — fields grouped by entity type with color-coding. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type {
    ExtractedField,
    PlausibilityWarning,
    FieldCorrection,
    EntityCategory,
  } from '$lib/types/review';
  import ConfidenceFlag from './ConfidenceFlag.svelte';
  import FieldEditor from './FieldEditor.svelte';

  interface Props {
    fields: ExtractedField[];
    warnings: PlausibilityWarning[];
    corrections: FieldCorrection[];
    onCorrection: (correction: FieldCorrection) => void;
  }
  let { fields, warnings, corrections, onCorrection }: Props = $props();

  type FieldGroup = {
    category: EntityCategory;
    label: string;
    fields: ExtractedField[];
    headerClass: string;
    borderClass: string;
  };

  const categoryStyles: Record<EntityCategory, { i18nKey: string; headerClass: string; borderClass: string }> = {
    Medication: { i18nKey: 'review.category_medications', headerClass: 'bg-blue-50 text-blue-800', borderClass: 'border-blue-200' },
    LabResult: { i18nKey: 'review.category_lab_results', headerClass: 'bg-green-50 text-green-800', borderClass: 'border-green-200' },
    Diagnosis: { i18nKey: 'review.category_diagnoses', headerClass: 'bg-indigo-50 text-indigo-800', borderClass: 'border-indigo-200' },
    Allergy: { i18nKey: 'review.category_allergies', headerClass: 'bg-red-50 text-red-800', borderClass: 'border-red-200' },
    Procedure: { i18nKey: 'review.category_procedures', headerClass: 'bg-teal-50 text-teal-800', borderClass: 'border-teal-200' },
    Referral: { i18nKey: 'review.category_referrals', headerClass: 'bg-violet-50 text-violet-800', borderClass: 'border-violet-200' },
    Professional: { i18nKey: 'review.category_professional', headerClass: 'bg-purple-50 text-purple-800', borderClass: 'border-purple-200' },
    Date: { i18nKey: 'review.category_date', headerClass: 'bg-amber-50 text-amber-800', borderClass: 'border-amber-200' },
  };

  let groupedFields = $derived.by(() => {
    const groups: FieldGroup[] = [];
    const categoryOrder: EntityCategory[] = [
      'Medication', 'LabResult', 'Diagnosis', 'Allergy',
      'Procedure', 'Referral', 'Professional', 'Date',
    ];

    for (const category of categoryOrder) {
      const categoryFields = fields.filter(f => f.entity_type === category);
      if (categoryFields.length > 0) {
        const config = categoryStyles[category];
        const sorted = [...categoryFields].sort((a, b) => {
          if (a.is_flagged && !b.is_flagged) return -1;
          if (!a.is_flagged && b.is_flagged) return 1;
          return a.entity_index - b.entity_index;
        });
        groups.push({
          category,
          label: $t(config.i18nKey),
          fields: sorted,
          headerClass: config.headerClass,
          borderClass: config.borderClass,
        });
      }
    }
    return groups;
  });

  function getWarningsForField(fieldId: string): PlausibilityWarning[] {
    return warnings.filter(w => w.field_id === fieldId);
  }

  function getCorrectedValue(fieldId: string): string | null {
    const correction = corrections.find(c => c.field_id === fieldId);
    return correction?.corrected_value ?? null;
  }
</script>

<div class="flex flex-col gap-4 p-4">
  {#each groupedFields as group}
    <section>
      <h2 class="text-sm font-semibold px-3 py-2 rounded-t-lg {group.headerClass}">
        {group.label}
        <span class="font-normal opacity-70">
          ({$t('review.field_count', { values: { count: group.fields.length } })})
        </span>
      </h2>

      <div class="flex flex-col border border-t-0 rounded-b-lg {group.borderClass}
                  divide-y divide-stone-100">
        {#each group.fields as field (field.id)}
          {@const fieldWarnings = getWarningsForField(field.id)}
          {@const correctedValue = getCorrectedValue(field.id)}

          <div class="px-3 py-3">
            <div class="flex items-start gap-2">
              <span class="text-xs text-stone-500 min-w-[100px] mt-1 shrink-0">
                {field.display_label}
              </span>
              <div class="flex-1">
                <FieldEditor
                  {field}
                  {correctedValue}
                  onSave={(newValue) => {
                    onCorrection({
                      field_id: field.id,
                      original_value: field.value,
                      corrected_value: newValue,
                    });
                  }}
                />
              </div>
            </div>

            {#if field.is_flagged}
              <div class="mt-2">
                <ConfidenceFlag
                  confidence={field.confidence}
                  fieldLabel={field.display_label}
                />
              </div>
            {/if}

            {#each fieldWarnings as warning}
              <div class="mt-2 px-3 py-2 rounded-lg text-sm
                          {warning.severity === 'Critical'
                            ? 'bg-[var(--color-danger-50)] text-[var(--color-danger-800)] border border-[var(--color-danger-200)]'
                            : 'bg-[var(--color-warning-50)] text-[var(--color-warning-800)] border border-[var(--color-warning-200)]'}">
                {warning.message}
              </div>
            {/each}
          </div>
        {/each}
      </div>
    </section>
  {/each}

  {#if fields.length === 0}
    <div class="text-center py-12 text-stone-500">
      <p>{$t('review.no_fields_title')}</p>
      <p class="text-sm mt-2">{$t('review.no_fields_description')}</p>
    </div>
  {/if}
</div>
