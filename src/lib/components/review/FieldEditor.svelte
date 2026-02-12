<!-- L3-04: Inline field editor — click to edit, Enter to save. -->
<script lang="ts">
  import type { ExtractedField } from '$lib/types/review';

  interface Props {
    field: ExtractedField;
    correctedValue: string | null;
    onSave: (newValue: string) => void;
  }
  let { field, correctedValue, onSave }: Props = $props();

  let editing = $state(false);
  let editValue = $state('');

  let displayValue = $derived(correctedValue ?? field.value);
  let isCorrected = $derived(correctedValue !== null);

  function startEdit() {
    editValue = displayValue;
    editing = true;
  }

  function saveEdit() {
    const trimmed = editValue.trim();
    if (trimmed && trimmed !== field.value) {
      onSave(trimmed);
    }
    editing = false;
  }

  function cancelEdit() {
    editing = false;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      saveEdit();
    } else if (e.key === 'Escape') {
      cancelEdit();
    }
  }
</script>

{#if editing}
  <input
    type="text"
    bind:value={editValue}
    onkeydown={handleKeydown}
    onblur={saveEdit}
    class="w-full px-2 py-1 text-sm border-2 border-[var(--color-primary)] rounded
           focus:outline-none min-h-[44px]"
    autofocus
  />
{:else}
  <button
    class="group flex items-center gap-1.5 text-left w-full px-2 py-1 rounded
           hover:bg-stone-50 transition-colors min-h-[44px]
           {isCorrected ? 'border border-blue-300 bg-blue-50' : ''}
           {field.is_flagged ? 'border border-amber-300' : ''}"
    onclick={startEdit}
    aria-label="Edit {field.display_label}: {displayValue}"
  >
    <span class="text-sm text-stone-800 {isCorrected ? 'font-medium text-blue-800' : ''}">
      {displayValue}
    </span>

    {#if isCorrected}
      <span
        class="text-blue-500 text-xs shrink-0"
        title="Original: {field.value}"
      >
        &#9998;
      </span>
    {:else}
      <span class="text-stone-300 text-xs opacity-0 group-hover:opacity-100 transition-opacity shrink-0">
        &#9998;
      </span>
    {/if}

    {#if field.confidence >= 0.90}
      <span class="text-green-500 text-xs shrink-0" aria-label="High confidence">&#x2713;</span>
    {/if}
  </button>
{/if}
