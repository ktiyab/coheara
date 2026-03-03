<!-- RV-UX: Expanded edit panel for one entity's fields. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { ReviewEntity, FieldCorrection } from '$lib/types/review';
  import { CheckIcon } from '$lib/components/icons/md';
  import StatusDot from '$lib/components/ui/StatusDot.svelte';
  import Button from '$lib/components/ui/Button.svelte';

  interface Props {
    entity: ReviewEntity;
    corrections: FieldCorrection[];
    onCorrection: (correction: FieldCorrection) => void;
    onCollapse: () => void;
    onDismiss: () => void;
  }
  let { entity, corrections, onCorrection, onCollapse, onDismiss }: Props = $props();

  let editValues: Record<string, string> = $state({});

  // Initialize edit values from corrections or original values
  $effect(() => {
    const values: Record<string, string> = {};
    for (const field of entity.fields) {
      const correction = corrections.find(c => c.field_id === field.id);
      values[field.id] = correction?.corrected_value ?? field.value;
    }
    editValues = values;
  });

  function handleSave(fieldId: string, originalValue: string) {
    const trimmed = (editValues[fieldId] ?? '').trim();
    if (trimmed && trimmed !== originalValue) {
      onCorrection({
        field_id: fieldId,
        original_value: originalValue,
        corrected_value: trimmed,
      });
    }
  }

  function handleKeydown(e: KeyboardEvent, fieldId: string, originalValue: string) {
    if (e.key === 'Enter') {
      e.preventDefault();
      handleSave(fieldId, originalValue);
      // Move focus to next input
      const inputs = (e.target as HTMLElement)
        .closest('.entity-edit-panel')
        ?.querySelectorAll('input');
      if (inputs) {
        const idx = Array.from(inputs).indexOf(e.target as HTMLInputElement);
        if (idx >= 0 && idx < inputs.length - 1) {
          (inputs[idx + 1] as HTMLInputElement).focus();
        }
      }
    } else if (e.key === 'Escape') {
      onCollapse();
    }
  }

  function isCorrected(fieldId: string): boolean {
    return corrections.some(c => c.field_id === fieldId);
  }
</script>

<div class="entity-edit-panel px-4 py-3 bg-stone-50 dark:bg-gray-900/50
            border-t border-stone-100 dark:border-gray-800">
  <div class="flex flex-col gap-2.5">
    {#each entity.fields as field (field.id)}
      <div class="flex items-center gap-3">
        <label
          for="edit-{field.id}"
          class="text-xs text-stone-500 dark:text-gray-400 min-w-[80px] shrink-0"
        >
          {field.display_label}
        </label>
        <input
          id="edit-{field.id}"
          type="text"
          bind:value={editValues[field.id]}
          onblur={() => handleSave(field.id, field.value)}
          onkeydown={(e) => handleKeydown(e, field.id, field.value)}
          class="flex-1 px-2 py-1.5 text-sm border rounded-lg min-h-[44px]
                 bg-white dark:bg-gray-900
                 text-stone-800 dark:text-gray-100
                 focus:border-[var(--color-primary)] focus:outline-none
                 {isCorrected(field.id)
                   ? 'border-[var(--color-info-200)] bg-[var(--color-info-50)]'
                   : 'border-stone-200 dark:border-gray-700'}"
        />
        <div class="w-5 shrink-0 flex justify-center">
          {#if field.confidence >= 0.90}
            <CheckIcon class="w-3.5 h-3.5 text-[var(--color-success)]" />
          {:else if field.is_flagged}
            <StatusDot variant={field.confidence < 0.50 ? 'danger' : 'warning'} />
          {/if}
        </div>
      </div>
    {/each}
  </div>

  <div class="mt-3 flex items-center">
    <Button variant="danger" onclick={onDismiss}>
      {$t('common.delete')}
    </Button>
    <div class="flex-1"></div>
    <Button variant="ghost" onclick={onCollapse}>
      {$t('review.entity_edit_done')}
    </Button>
  </div>
</div>
