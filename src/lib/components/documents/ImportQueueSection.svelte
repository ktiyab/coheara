<!--
  BTL-10 C6: ImportQueueSection — Renders importQueue.activeItems as a list.
  Inline section within DocumentListScreen, visible only when active imports exist.
  iOS App Store "downloading" section pattern.
-->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { importQueue } from '$lib/stores/importQueue.svelte';
  import ImportQueueCard from './ImportQueueCard.svelte';
  import Divider from '$lib/components/ui/Divider.svelte';
  import ErrorBanner from '$lib/components/ErrorBanner.svelte';

  async function handleCancel(jobId: string) {
    await importQueue.cancel(jobId);
  }

  async function handleRetry(jobId: string) {
    await importQueue.retry(jobId);
  }

  async function handleDelete(jobId: string) {
    await importQueue.delete(jobId);
  }

  let activeItems = $derived(importQueue.activeItems);
  let failedItems = $derived(importQueue.failedItems);
  let visibleItems = $derived([...activeItems, ...failedItems]);
</script>

{#if visibleItems.length > 0}
  <div class="px-4">
    <Divider spacing="sm" label={$t('documents.queue_section_title')} />
  </div>

  {@const queuedItems = visibleItems.filter(j => j.state === 'Queued')}
  <div class="flex flex-col gap-2 px-4" role="list" aria-label={$t('documents.queue_section_title')}>
    {#each visibleItems as job (job.id)}
      <ImportQueueCard
        {job}
        queuePosition={job.state === 'Queued' ? queuedItems.findIndex(q => q.id === job.id) + 1 : undefined}
        queueTotal={job.state === 'Queued' ? queuedItems.length : undefined}
        onCancel={handleCancel}
        onRetry={handleRetry}
        onDelete={handleDelete}
      />
    {/each}
  </div>
{/if}

{#if importQueue.error}
  <div class="px-4 mt-2">
    <ErrorBanner
      severity="warning"
      message={importQueue.error}
      guidance={$t('documents.queue_error_guidance')}
      actionLabel={$t('common.retry')}
      onAction={() => importQueue.refresh()}
      onDismiss={() => { importQueue.error = null; }}
    />
  </div>
{/if}
