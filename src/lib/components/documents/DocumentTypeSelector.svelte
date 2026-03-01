<!--
  UC-01: Document type selector — segmented control for import classification.
  3 options, all visible, one click. Replaces LLM auto-classification.
  Pattern: Apple segmented control (iOS Settings, macOS Finder view switcher).
-->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { UserDocumentType } from '$lib/types/import-queue';
  import { DocumentScannerIcon, ClipboardIcon, HeartIcon } from '$lib/components/icons/md';

  interface Props {
    selected: UserDocumentType;
    onselect: (value: UserDocumentType) => void;
  }

  let { selected, onselect }: Props = $props();

  const options: { value: UserDocumentType; labelKey: string; icon: typeof DocumentScannerIcon }[] = [
    { value: 'lab_report', labelKey: 'import.type_lab_report', icon: DocumentScannerIcon },
    { value: 'prescription', labelKey: 'import.type_prescription', icon: ClipboardIcon },
    { value: 'medical_image', labelKey: 'import.type_medical_image', icon: HeartIcon },
  ];

  function handleKeydown(event: KeyboardEvent, index: number) {
    let next = index;
    if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
      next = (index + 1) % options.length;
      event.preventDefault();
    } else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') {
      next = (index - 1 + options.length) % options.length;
      event.preventDefault();
    }
    if (next !== index) {
      onselect(options[next].value);
      // Focus the new button
      const container = (event.target as HTMLElement).closest('[role="radiogroup"]');
      const buttons = container?.querySelectorAll<HTMLButtonElement>('[role="radio"]');
      buttons?.[next]?.focus();
    }
  }
</script>

<div class="flex flex-col gap-1">
  <span class="text-xs font-medium text-stone-500 dark:text-gray-400">
    {$t('import.type_label')}
  </span>
  <div
    role="radiogroup"
    aria-label={$t('import.type_label')}
    class="inline-flex rounded-lg border border-stone-200 dark:border-gray-600 bg-stone-100 dark:bg-gray-800 p-0.5"
  >
    {#each options as option, i}
      <button
        role="radio"
        aria-checked={selected === option.value}
        tabindex={selected === option.value ? 0 : -1}
        class="flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium transition-all
          {selected === option.value
            ? 'bg-white dark:bg-gray-700 text-blue-600 dark:text-blue-400 shadow-sm'
            : 'text-stone-500 dark:text-gray-400 hover:text-stone-700 dark:hover:text-gray-300'}"
        onclick={() => onselect(option.value)}
        onkeydown={(e) => handleKeydown(e, i)}
      >
        <option.icon class="w-3.5 h-3.5" />
        {$t(option.labelKey)}
      </button>
    {/each}
  </div>
</div>
