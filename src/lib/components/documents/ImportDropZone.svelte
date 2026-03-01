<!--
  RV-UX: Import bar — single row with type selector + import button.
  No drag-drop. Calls importQueue.enqueue(paths).
-->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { open } from '@tauri-apps/plugin-dialog';
  import { importQueue } from '$lib/stores/importQueue.svelte';
  import { isTauriEnv } from '$lib/utils/tauri';
  import Button from '$lib/components/ui/Button.svelte';
  import { PlusIcon } from '$lib/components/icons/md';
  import DocumentTypeSelector from './DocumentTypeSelector.svelte';
  import type { UserDocumentType } from '$lib/types/import-queue';

  /** UC-01: User-selected document type — default Lab Report (most common). */
  let selectedDocType = $state<UserDocumentType>('lab_report');

  const SUPPORTED_EXTENSIONS = ['pdf', 'jpg', 'jpeg', 'png', 'tiff', 'tif', 'txt'];

  let documentFilters = $derived([
    { name: $t('import.filter_medical'), extensions: SUPPORTED_EXTENSIONS },
    { name: $t('import.filter_pdf'), extensions: ['pdf'] },
    { name: $t('import.filter_images'), extensions: ['jpg', 'jpeg', 'png', 'tiff', 'tif'] },
  ]);

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
</script>

<div class="flex items-center justify-between mx-4 mt-2 px-4 py-2
            bg-stone-100 dark:bg-gray-800 rounded-xl border border-stone-200 dark:border-gray-700">
  <DocumentTypeSelector selected={selectedDocType} onselect={(v) => selectedDocType = v} />
  <Button variant="secondary" size="sm" onclick={browseFiles}>
    <PlusIcon class="w-4 h-4" />
    {$t('documents.list_import')}
  </Button>
</div>
