<!-- L5-01: Backup & Restore — encrypted backup creation and restore -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { createBackup, previewBackup, restoreFromBackup } from '$lib/api/trust';
  import type { BackupResult, RestorePreview } from '$lib/types/trust';
  import ErrorBanner from '$lib/components/ErrorBanner.svelte';
  import { RefreshIcon } from '$lib/components/icons/md';

  type View = 'idle' | 'backup' | 'restore-preview' | 'restore-password' | 'restore-success';

  let view = $state<View>('idle');
  let backupPath = $state('');
  let restorePath = $state('');
  let restorePassword = $state('');
  let loading = $state(false);
  let error: string | null = $state(null);
  let backupResult: BackupResult | null = $state(null);
  let restorePreview: RestorePreview | null = $state(null);
  // R.2: Replace alert() with inline restore success message
  let restoreSuccessMessage: string | null = $state(null);

  function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
  }

  function reset() {
    view = 'idle';
    backupPath = '';
    restorePath = '';
    restorePassword = '';
    loading = false;
    error = null;
    backupResult = null;
    restorePreview = null;
  }

  async function handleCreateBackup() {
    if (!backupPath.trim()) {
      error = $t('error.field_required', { values: { field: $t('backup.field_backup_path') } });
      return;
    }
    loading = true;
    error = null;
    try {
      backupResult = await createBackup(backupPath.trim());
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function handlePreviewRestore() {
    if (!restorePath.trim()) {
      error = $t('error.field_required', { values: { field: $t('backup.field_file_path') } });
      return;
    }
    loading = true;
    error = null;
    try {
      restorePreview = await previewBackup(restorePath.trim());
      view = 'restore-preview';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function handleRestore() {
    if (!restorePassword) {
      error = $t('error.field_required', { values: { field: $t('backup.field_password') } });
      return;
    }
    loading = true;
    error = null;
    try {
      const result = await restoreFromBackup(restorePath.trim(), restorePassword);
      restoreSuccessMessage = $t('backup.restored_message', {
        values: { count: result.documents_restored, size: formatBytes(result.total_size_bytes) },
      });
      view = 'restore-success';
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }
</script>

<section class="bg-white dark:bg-gray-900 rounded-[var(--radius-card)] border border-stone-100 dark:border-gray-800 shadow-sm">
  <!-- Section header row with icon -->
  <div class="flex items-center gap-4 px-4 py-3 min-h-[52px] border-b border-stone-100 dark:border-gray-800">
    <RefreshIcon class="w-9 h-9 text-[var(--color-success)] flex-shrink-0" />
    <span class="text-sm font-medium text-stone-800 dark:text-gray-200">{$t('backup.heading')}</span>
  </div>

  <div class="divide-y divide-stone-100 dark:divide-gray-800">
  {#if view === 'idle'}
    {#if backupResult}
      <div class="px-4 py-3">
        <div class="bg-[var(--color-success-50)] rounded-lg p-3 border border-[var(--color-success-50)]">
          <p class="text-sm text-[var(--color-success)]">
            {$t('backup.created_message', { values: { count: backupResult.total_documents, size: formatBytes(backupResult.total_size_bytes) } })}
          </p>
          <p class="text-xs text-[var(--color-success)] mt-1 font-mono truncate">
            {backupResult.backup_path}
          </p>
        </div>
      </div>
    {/if}

    <!-- Create Backup row -->
    <div class="flex items-center gap-4 px-4 py-3 min-h-[52px] pl-[68px]">
      <div class="flex-1 min-w-0">
        <p class="text-sm text-stone-800 dark:text-gray-200 font-medium">{$t('backup.create')}</p>
        <p class="text-xs text-stone-500 dark:text-gray-400 mt-0.5">{$t('backup.create_desc')}</p>
      </div>
      <button
        class="px-4 py-2 bg-[var(--color-interactive)] text-white rounded-lg text-sm
               font-medium min-h-[36px] flex-shrink-0"
        onclick={() => {
          view = 'backup';
          error = null;
        }}
      >
        {$t('backup.create')}
      </button>
    </div>

    <!-- Restore row -->
    <div class="flex items-center gap-4 px-4 py-3 min-h-[52px] pl-[68px]">
      <div class="flex-1 min-w-0">
        <p class="text-sm text-stone-800 dark:text-gray-200 font-medium">{$t('backup.restore')}</p>
        <p class="text-xs text-stone-500 dark:text-gray-400 mt-0.5">{$t('backup.restore_desc')}</p>
      </div>
      <button
        class="px-4 py-2 bg-white dark:bg-gray-900 border border-stone-200 dark:border-gray-700 rounded-lg
               text-sm font-medium text-stone-700 dark:text-gray-200 min-h-[36px] flex-shrink-0"
        onclick={() => {
          view = 'restore-preview';
          restorePreview = null;
          error = null;
        }}
      >
        {$t('backup.restore')}
      </button>
    </div>

  {:else if view === 'backup'}
    <div class="px-4 py-4">
      <label for="backup-path" class="block text-sm text-stone-600 dark:text-gray-300 mb-1">{$t('backup.save_to')}</label>
      <input
        id="backup-path"
        type="text"
        class="w-full px-4 py-3 rounded-lg border border-stone-200 dark:border-gray-700 text-stone-700 dark:text-gray-200
               text-sm mb-3 min-h-[44px] font-mono dark:bg-gray-900"
        bind:value={backupPath}
        placeholder="/path/to/backup.coheara-backup"
      />

      {#if error}
        <p class="text-[var(--color-danger)] text-sm mb-3">{error}</p>
      {/if}

      <div class="flex gap-3">
        <button
          class="flex-1 px-4 py-3 bg-[var(--color-interactive)] text-white rounded-xl text-sm
                 font-medium min-h-[44px] disabled:opacity-50"
          disabled={loading || !backupPath.trim()}
          onclick={handleCreateBackup}
        >
          {loading ? $t('common.creating') : $t('backup.create')}
        </button>
        <button
          class="px-4 py-3 bg-white dark:bg-gray-900 border border-stone-200 dark:border-gray-700 rounded-xl text-sm
                 text-stone-600 dark:text-gray-300 min-h-[44px]"
          onclick={reset}
        >
          {$t('common.cancel')}
        </button>
      </div>
    </div>

  {:else if view === 'restore-preview'}
    <div class="px-4 py-4">
    {#if !restorePreview}
      <label for="restore-path" class="block text-sm text-stone-600 dark:text-gray-300 mb-1">{$t('backup.file_path')}</label>
      <input
        id="restore-path"
        type="text"
        class="w-full px-4 py-3 rounded-lg border border-stone-200 dark:border-gray-700 text-stone-700 dark:text-gray-200
               text-sm mb-3 min-h-[44px] font-mono dark:bg-gray-900"
        bind:value={restorePath}
        placeholder="/path/to/backup.coheara-backup"
      />

      {#if error}
        <p class="text-[var(--color-danger)] text-sm mb-3">{error}</p>
      {/if}

      <div class="flex gap-3">
        <button
          class="flex-1 px-4 py-3 bg-[var(--color-interactive)] text-white rounded-xl text-sm
                 font-medium min-h-[44px] disabled:opacity-50"
          disabled={loading || !restorePath.trim()}
          onclick={handlePreviewRestore}
        >
          {loading ? $t('common.loading') : $t('backup.preview')}
        </button>
        <button
          class="px-4 py-3 bg-white dark:bg-gray-900 border border-stone-200 dark:border-gray-700 rounded-xl text-sm
                 text-stone-600 dark:text-gray-300 min-h-[44px]"
          onclick={reset}
        >
          {$t('common.cancel')}
        </button>
      </div>
    {:else}
      <div class="space-y-2 text-sm mb-4">
        <p class="text-stone-500 dark:text-gray-400">{$t('backup.profile_label')}: <span class="text-stone-800 dark:text-gray-100 font-medium">{restorePreview.metadata.profile_name}</span></p>
        <p class="text-stone-500 dark:text-gray-400">{$t('settings.documents')}: <span class="text-stone-800 dark:text-gray-100 font-medium">{restorePreview.metadata.document_count}</span></p>
        <p class="text-stone-500 dark:text-gray-400">{$t('backup.created_label')}: <span class="text-stone-800 dark:text-gray-100 font-medium">{new Date(restorePreview.metadata.created_at).toLocaleDateString()}</span></p>
        <p class="text-stone-500 dark:text-gray-400">{$t('settings.total_size')}: <span class="text-stone-800 dark:text-gray-100 font-medium">{formatBytes(restorePreview.total_size_bytes)}</span></p>
        <p class="text-stone-500 dark:text-gray-400">{$t('backup.version_label')}: <span class="text-stone-800 dark:text-gray-100 font-medium">{restorePreview.metadata.coheara_version}</span></p>
        {#if !restorePreview.compatible}
          <div class="bg-[var(--color-warning-50)] rounded-lg p-3 border border-[var(--color-warning-200)]">
            <p class="text-sm text-[var(--color-warning-800)]">
              {restorePreview.compatibility_message ?? $t('backup.compatibility_warning')}
            </p>
          </div>
        {/if}
      </div>

      <button
        class="w-full px-4 py-3 bg-[var(--color-interactive)] text-white rounded-xl text-sm
               font-medium min-h-[44px] mb-2"
        onclick={() => (view = 'restore-password')}
      >
        {$t('backup.restore')}
      </button>
      <button
        class="w-full px-4 py-3 bg-white dark:bg-gray-900 border border-stone-200 dark:border-gray-700 rounded-xl text-sm
               text-stone-600 dark:text-gray-300 min-h-[44px]"
        onclick={reset}
      >
        {$t('common.cancel')}
      </button>
    {/if}
    </div>

  {:else if view === 'restore-password'}
    <div class="px-4 py-4">
      <p class="text-sm text-stone-600 dark:text-gray-300 mb-3">
        {$t('backup.restore_password', { values: { name: restorePreview?.metadata.profile_name ?? '' } })}
      </p>

      <input
        type="password"
        class="w-full px-4 py-3 rounded-lg border border-stone-200 dark:border-gray-700 text-stone-700 dark:text-gray-200
               text-sm mb-3 min-h-[44px] dark:bg-gray-900"
        bind:value={restorePassword}
        placeholder={$t('backup.password_placeholder')}
        aria-label={$t('backup.password_placeholder')}
      />

      {#if error}
        <div class="mb-3">
          <ErrorBanner
            message={error}
            severity="error"
            guidance={$t('backup.password_guidance')}
            onDismiss={() => { error = null; }}
          />
        </div>
      {/if}

      <div class="flex gap-3">
        <button
          class="flex-1 px-4 py-3 bg-[var(--color-interactive)] text-white rounded-xl text-sm
                 font-medium min-h-[44px] disabled:opacity-50"
          disabled={loading || !restorePassword}
          onclick={handleRestore}
        >
          {loading ? $t('common.loading') : $t('backup.restore')}
        </button>
        <button
          class="px-4 py-3 bg-white dark:bg-gray-900 border border-stone-200 dark:border-gray-700 rounded-xl text-sm
                 text-stone-600 dark:text-gray-300 min-h-[44px]"
          onclick={reset}
        >
          {$t('common.cancel')}
        </button>
      </div>
    </div>

  {:else if view === 'restore-success'}
    <div class="px-4 py-4">
      <div class="bg-[var(--color-success-50)] rounded-lg p-3 mb-3 border border-[var(--color-success-50)]">
        <p class="text-sm text-[var(--color-success)]">{restoreSuccessMessage}</p>
      </div>
      <button
        class="w-full px-4 py-3 bg-[var(--color-interactive)] text-white rounded-xl text-sm
               font-medium min-h-[44px]"
        onclick={reset}
      >
        {$t('common.done')}
      </button>
    </div>
  {/if}
  </div>
</section>
