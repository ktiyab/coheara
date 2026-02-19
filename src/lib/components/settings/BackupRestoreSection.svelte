<!-- L5-01: Backup & Restore — encrypted backup creation and restore -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { createBackup, previewBackup, restoreFromBackup } from '$lib/api/trust';
  import type { BackupResult, RestorePreview } from '$lib/types/trust';
  import ErrorBanner from '$lib/components/ErrorBanner.svelte';

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
      error = $t('error.field_required', { values: { field: 'backup path' } });
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
      error = $t('error.field_required', { values: { field: 'backup file path' } });
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
      error = $t('error.field_required', { values: { field: 'password' } });
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

<section class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
  <h2 class="text-sm font-medium text-stone-500 mb-3">{$t('backup.heading')}</h2>

  {#if view === 'idle'}
    {#if backupResult}
      <div class="bg-[var(--color-success-50)] rounded-lg p-3 mb-3 border border-[var(--color-success-50)]">
        <p class="text-sm text-[var(--color-success)]">
          Backup created: {backupResult.total_documents} documents,
          {formatBytes(backupResult.total_size_bytes)}
        </p>
        <p class="text-xs text-[var(--color-success)] mt-1 font-mono truncate">
          {backupResult.backup_path}
        </p>
      </div>
    {/if}

    <div class="flex gap-3">
      <button
        class="flex-1 px-4 py-3 bg-[var(--color-interactive)] text-white rounded-xl text-sm
               font-medium min-h-[44px]"
        onclick={() => {
          view = 'backup';
          error = null;
        }}
      >
        {$t('backup.create')}
      </button>
      <button
        class="flex-1 px-4 py-3 bg-white border border-stone-200 rounded-xl
               text-sm font-medium text-stone-700 min-h-[44px]"
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
    <label for="backup-path" class="block text-sm text-stone-600 mb-1">{$t('backup.save_to')}</label>
    <input
      id="backup-path"
      type="text"
      class="w-full px-4 py-3 rounded-lg border border-stone-200 text-stone-700
             text-sm mb-3 min-h-[44px] font-mono"
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
        class="px-4 py-3 bg-white border border-stone-200 rounded-xl text-sm
               text-stone-600 min-h-[44px]"
        onclick={reset}
      >
        {$t('common.cancel')}
      </button>
    </div>

  {:else if view === 'restore-preview'}
    {#if !restorePreview}
      <label for="restore-path" class="block text-sm text-stone-600 mb-1">{$t('backup.file_path')}</label>
      <input
        id="restore-path"
        type="text"
        class="w-full px-4 py-3 rounded-lg border border-stone-200 text-stone-700
               text-sm mb-3 min-h-[44px] font-mono"
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
          class="px-4 py-3 bg-white border border-stone-200 rounded-xl text-sm
                 text-stone-600 min-h-[44px]"
          onclick={reset}
        >
          {$t('common.cancel')}
        </button>
      </div>
    {:else}
      <div class="space-y-2 text-sm mb-4">
        <div class="flex justify-between">
          <span class="text-stone-600">Profile</span>
          <span class="text-stone-800">{restorePreview.metadata.profile_name}</span>
        </div>
        <div class="flex justify-between">
          <span class="text-stone-600">{$t('settings.documents')}</span>
          <span class="text-stone-800">{restorePreview.metadata.document_count}</span>
        </div>
        <div class="flex justify-between">
          <span class="text-stone-600">Created</span>
          <span class="text-stone-800">
            {new Date(restorePreview.metadata.created_at).toLocaleDateString()}
          </span>
        </div>
        <div class="flex justify-between">
          <span class="text-stone-600">{$t('settings.total_size')}</span>
          <span class="text-stone-800">{formatBytes(restorePreview.total_size_bytes)}</span>
        </div>
        <div class="flex justify-between">
          <span class="text-stone-600">Version</span>
          <span class="text-stone-800">{restorePreview.metadata.coheara_version}</span>
        </div>
        {#if !restorePreview.compatible}
          <div class="bg-[var(--color-warning-50)] rounded-lg p-3 border border-[var(--color-warning-200)]">
            <p class="text-sm text-[var(--color-warning-800)]">
              {restorePreview.compatibility_message ?? 'This backup may not be fully compatible.'}
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
        class="w-full px-4 py-3 bg-white border border-stone-200 rounded-xl text-sm
               text-stone-600 min-h-[44px]"
        onclick={reset}
      >
        {$t('common.cancel')}
      </button>
    {/if}

  {:else if view === 'restore-password'}
    <p class="text-sm text-stone-600 mb-3">
      {$t('backup.restore_password', { values: { name: restorePreview?.metadata.profile_name ?? '' } })}
    </p>

    <input
      type="password"
      class="w-full px-4 py-3 rounded-lg border border-stone-200 text-stone-700
             text-sm mb-3 min-h-[44px]"
      bind:value={restorePassword}
      placeholder={$t('backup.password_placeholder')}
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
        class="px-4 py-3 bg-white border border-stone-200 rounded-xl text-sm
               text-stone-600 min-h-[44px]"
        onclick={reset}
      >
        {$t('common.cancel')}
      </button>
    </div>

  {:else if view === 'restore-success'}
    <!-- R.2: Inline success message replacing alert() -->
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
  {/if}
</section>
