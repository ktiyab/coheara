<!--
  BTL-10 C6: ImportDropZone — Drag-drop + browse area for document import.
  Extracted from ImportScreen idle state. Collapsible: full when no documents,
  compact single-row button when documents exist and no active imports.
  Auto-expands on drag-over. Calls importQueue.enqueue(paths).
-->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { open } from '@tauri-apps/plugin-dialog';
  import { importQueue } from '$lib/stores/importQueue.svelte';
  import { isTauriEnv } from '$lib/utils/tauri';
  import Button from '$lib/components/ui/Button.svelte';
  import { DocumentScannerIcon, PlusIcon } from '$lib/components/icons/md';
  import DocumentTypeSelector from './DocumentTypeSelector.svelte';
  import type { UserDocumentType } from '$lib/types/import-queue';

  interface Props {
    /** True when documents exist — renders compact mode. */
    hasDocuments?: boolean;
    /** Optional: receive dropped file paths from parent (e.g., DropZoneOverlay). */
    droppedFiles?: string[];
  }

  let { hasDocuments = false, droppedFiles }: Props = $props();

  /** UC-01: User-selected document type — default Lab Report (most common). */
  let selectedDocType = $state<UserDocumentType>('lab_report');

  const SUPPORTED_EXTENSIONS = ['pdf', 'jpg', 'jpeg', 'png', 'tiff', 'tif', 'txt'];

  let documentFilters = $derived([
    { name: $t('import.filter_medical'), extensions: SUPPORTED_EXTENSIONS },
    { name: $t('import.filter_pdf'), extensions: ['pdf'] },
    { name: $t('import.filter_images'), extensions: ['jpg', 'jpeg', 'png', 'tiff', 'tif'] },
  ]);

  function isSupportedPath(path: string): boolean {
    const ext = path.split('.').pop()?.toLowerCase() ?? '';
    return SUPPORTED_EXTENSIONS.includes(ext);
  }

  async function browseFiles() {
    if (!isTauriEnv()) return;
    const selected = await open({
      title: $t('import.dialog_title'),
      multiple: true,
      filters: documentFilters,
    });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    if (paths.length > 0) {
      await importQueue.enqueue(paths, selectedDocType);
    }
  }

  // Handle droppedFiles from parent
  $effect(() => {
    if (droppedFiles && droppedFiles.length > 0) {
      const supported = droppedFiles.filter(isSupportedPath);
      if (supported.length > 0) {
        importQueue.enqueue(supported, selectedDocType);
      }
    }
  });

  // Drag-drop is handled globally by DropZoneOverlay → navigates here with droppedFiles prop.
  // No local native listener needed (avoids double-enqueue).

  let isCompact = $derived(hasDocuments && !importQueue.hasActiveImports);
</script>

{#if isCompact}
  <!-- Compact mode: selector + import button -->
  <div class="px-4 pt-2 flex flex-col gap-2">
    <DocumentTypeSelector selected={selectedDocType} onselect={(v) => selectedDocType = v} />
    <Button variant="dashed" fullWidth onclick={browseFiles}>
      <PlusIcon class="w-5 h-5" />
      {$t('documents.list_import')}
    </Button>
  </div>
{:else}
  <!-- Full drop zone -->
  <div
    class="mx-4 mt-3 rounded-xl border-2 border-dashed transition-colors p-6 text-center
           border-stone-300 dark:border-gray-600 bg-stone-50 dark:bg-gray-800/50"
    role="region"
    aria-label={$t('import.drop_files_here')}
  >
    <div class="flex justify-center mb-4">
      <DocumentTypeSelector selected={selectedDocType} onselect={(v) => selectedDocType = v} />
    </div>
    <DocumentScannerIcon class="w-10 h-10 mx-auto mb-3 text-stone-400 dark:text-gray-500" />
    <p class="text-sm font-medium text-stone-700 dark:text-gray-200 mb-1">
      {$t('import.add_documents')}
    </p>
    <p class="text-xs text-stone-500 dark:text-gray-400 mb-4">
      {$t('import.drag_drop_hint')}
    </p>
    <Button variant="secondary" size="sm" onclick={browseFiles}>
      {$t('import.browse_files')}
    </Button>
    <p class="text-xs text-stone-400 dark:text-gray-500 mt-3">
      {$t('import.supported_formats')}
    </p>
  </div>
{/if}
