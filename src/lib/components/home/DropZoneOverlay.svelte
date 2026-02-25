<!-- Spec 49 [FE-04]: Global drop zone overlay for home screen file import. -->
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { t } from 'svelte-i18n';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { ArrowDownIcon } from '$lib/components/icons/md';

  const SUPPORTED_EXTENSIONS = ['pdf', 'jpg', 'jpeg', 'png', 'tiff', 'tif', 'txt'];

  let isDragging = $state(false);
  let unlistenDragDrop: (() => void) | null = null;

  function isSupported(path: string): boolean {
    const ext = path.split('.').pop()?.toLowerCase() ?? '';
    return SUPPORTED_EXTENSIONS.includes(ext);
  }

  onMount(async () => {
    try {
      const webview = getCurrentWebview();
      unlistenDragDrop = await webview.onDragDropEvent((event) => {
        const payload = event.payload;
        if (payload.type === 'enter') {
          isDragging = true;
        } else if (payload.type === 'leave') {
          isDragging = false;
        } else if (payload.type === 'drop') {
          isDragging = false;
          const supported = payload.paths.filter(isSupported);
          if (supported.length > 0) {
            navigation.navigate('import', { droppedFiles: supported.join('|') });
          }
        }
      });
    } catch {
      // Drag-drop not available in this environment
    }
  });

  onDestroy(() => {
    unlistenDragDrop?.();
  });
</script>

{#if isDragging}
  <div
    class="fixed inset-0 z-50 bg-[var(--color-success-50)]/80 backdrop-blur-sm
           flex flex-col items-center justify-center pointer-events-none"
    role="status"
    aria-live="assertive"
  >
    <div class="w-20 h-20 bg-white dark:bg-gray-900 rounded-2xl shadow-lg flex items-center justify-center mb-4">
      <ArrowDownIcon class="w-10 h-10 text-[var(--color-success)]" />
    </div>
    <p class="text-lg font-semibold text-[var(--color-success)]">
      {$t('import.drop_files_here')}
    </p>
    <p class="text-sm text-[var(--color-text-secondary)] mt-1">
      {$t('import.supported_formats')}
    </p>
  </div>
{/if}
