<!-- RV-UX: Compact one-line entity row with status dot and tap-to-expand edit. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { ReviewEntity, FieldCorrection, EntityCategory } from '$lib/types/review';
  import StatusDot from '$lib/components/ui/StatusDot.svelte';
  import Badge from '$lib/components/ui/Badge.svelte';
  import { ChevronDownIcon } from '$lib/components/icons/md';
  import EntityEditPanel from './EntityEditPanel.svelte';

  interface Props {
    entity: ReviewEntity;
    corrections: FieldCorrection[];
    onCorrection: (correction: FieldCorrection) => void;
    onDismiss: () => void;
  }
  let { entity, corrections, onCorrection, onDismiss }: Props = $props();

  let expanded = $state(false);

  let dotVariant = $derived.by(() => {
    if (entity.confidence >= 0.70) return 'success' as const;
    if (entity.confidence >= 0.50) return 'warning' as const;
    return 'danger' as const;
  });

  let correctedCount = $derived(
    entity.fields.filter(f => corrections.some(c => c.field_id === f.id)).length
  );

  /** Get a field's display value (corrected or original). */
  function fieldValue(fieldName: string): string {
    const field = entity.fields.find(f => f.field_name === fieldName);
    if (!field) return '';
    const correction = corrections.find(c => c.field_id === field.id);
    return correction?.corrected_value ?? field.value;
  }

  /** Build entity-specific one-line summary. */
  let summary = $derived.by(() => {
    const parts: string[] = [];
    const fv = fieldValue;

    switch (entity.category as EntityCategory) {
      case 'LabResult': {
        const name = fv('test_name');
        const value = fv('value');
        const unit = fv('unit');
        const rangeLow = fv('reference_range_low');
        const rangeHigh = fv('reference_range_high');
        const rangeText = fv('reference_range');
        if (name) parts.push(name);
        const valueUnit = [value, unit].filter(Boolean).join(' ');
        if (valueUnit) parts.push(valueUnit);
        if (rangeText) {
          parts.push(`(${rangeText})`);
        } else if (rangeLow && rangeHigh) {
          parts.push(`(${rangeLow} – ${rangeHigh})`);
        }
        break;
      }
      case 'Medication': {
        const name = fv('generic_name');
        const dose = fv('dose');
        const freq = fv('frequency');
        if (name) parts.push(name);
        if (dose) parts.push(dose);
        if (freq) parts.push(freq);
        break;
      }
      case 'Diagnosis': {
        const name = fv('name');
        const status = fv('status');
        if (name) parts.push(name);
        if (status) parts.push(status);
        break;
      }
      case 'Allergy': {
        const allergen = fv('allergen');
        const reaction = fv('reaction');
        if (allergen) parts.push(allergen);
        if (reaction) parts.push(reaction);
        break;
      }
      case 'Procedure': {
        const name = fv('name');
        const date = fv('date');
        if (name) parts.push(name);
        if (date) parts.push(date);
        break;
      }
      case 'Referral': {
        const to = fv('referred_to');
        const specialty = fv('specialty');
        if (to) parts.push(to);
        if (specialty) parts.push(specialty);
        break;
      }
      case 'Professional': {
        const name = fv('name');
        const specialty = fv('specialty');
        if (name) parts.push(name);
        if (specialty) parts.push(specialty);
        break;
      }
      case 'Date': {
        const date = fv('document_date');
        if (date) parts.push(date);
        break;
      }
    }

    return parts.length > 0 ? parts.join(' \u00b7 ') : $t('review.entity_no_value');
  });

  function toggleExpand() {
    expanded = !expanded;
  }
</script>

<div>
  <button
    class="group flex items-center gap-2 w-full px-3 py-3 text-left
           hover:bg-stone-50 dark:hover:bg-gray-800 transition-colors min-h-[44px]"
    onclick={toggleExpand}
    aria-expanded={expanded}
  >
    <StatusDot variant={dotVariant} />

    <span class="flex-1 text-sm text-stone-800 dark:text-gray-100 truncate">
      {summary}
    </span>

    {#if correctedCount > 0}
      <Badge variant="info" size="sm">
        {$t('review.entity_edited_count', { values: { count: correctedCount } })}
      </Badge>
    {/if}

    <ChevronDownIcon
      class="w-4 h-4 text-stone-400 dark:text-gray-500 transition-transform shrink-0
             {expanded ? 'rotate-180' : ''}"
    />
  </button>

  {#if expanded}
    <EntityEditPanel
      {entity}
      {corrections}
      {onCorrection}
      onCollapse={() => expanded = false}
      {onDismiss}
    />
  {/if}
</div>
