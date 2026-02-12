<!-- L5-01: Backup & Restore — encrypted backup creation and restore -->
<script lang="ts">
  import { createBackup, previewBackup, restoreFromBackup } from '$lib/api/trust';
  import type { BackupResult, RestorePreview } from '$lib/types/trust';

  type View = 'idle' | 'backup' | 'restore-preview' | 'restore-password';

  let view = $state<View>('idle');
  let backupPath = $state('');
  let restorePath = $state('');
  let restorePassword = $state('');
  let loading = $state(false);
  let error: string | null = $state(null);
  let backupResult: BackupResult | null = $state(null);
  let restorePreview: RestorePreview | null = $state(null);

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
      error = 'Please enter a backup file path';
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
      error = 'Please enter the backup file path';
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
      error = 'Password required';
      return;
    }
    loading = true;
    error = null;
    try {
      const result = await restoreFromBackup(restorePath.trim(), restorePassword);
      const warnings = result.warnings.length > 0 ? ` (${result.warnings.length} warnings)` : '';
      alert(
        `Restored ${result.documents_restored} documents (${formatBytes(result.total_size_bytes)})${warnings}`,
      );
      reset();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }
</script>

<section class="bg-white rounded-xl p-5 border border-stone-100 shadow-sm">
  <h2 class="text-sm font-medium text-stone-500 mb-3">BACKUP & RESTORE</h2>

  {#if view === 'idle'}
    {#if backupResult}
      <div class="bg-green-50 rounded-lg p-3 mb-3 border border-green-200">
        <p class="text-sm text-green-800">
          Backup created: {backupResult.total_documents} documents,
          {formatBytes(backupResult.total_size_bytes)}
        </p>
        <p class="text-xs text-green-600 mt-1 font-mono truncate">
          {backupResult.backup_path}
        </p>
      </div>
    {/if}

    <div class="flex gap-3">
      <button
        class="flex-1 px-4 py-3 bg-teal-600 text-white rounded-xl text-sm
               font-medium min-h-[44px]"
        onclick={() => {
          view = 'backup';
          error = null;
        }}
      >
        Create backup
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
        Restore from backup
      </button>
    </div>

  {:else if view === 'backup'}
    <label for="backup-path" class="block text-sm text-stone-600 mb-1">Save backup to:</label>
    <input
      id="backup-path"
      type="text"
      class="w-full px-4 py-3 rounded-lg border border-stone-200 text-stone-700
             text-sm mb-3 min-h-[44px] font-mono"
      bind:value={backupPath}
      placeholder="/path/to/backup.coheara-backup"
    />

    {#if error}
      <p class="text-red-600 text-sm mb-3">{error}</p>
    {/if}

    <div class="flex gap-3">
      <button
        class="flex-1 px-4 py-3 bg-teal-600 text-white rounded-xl text-sm
               font-medium min-h-[44px] disabled:opacity-50"
        disabled={loading || !backupPath.trim()}
        onclick={handleCreateBackup}
      >
        {loading ? 'Creating...' : 'Create backup'}
      </button>
      <button
        class="px-4 py-3 bg-white border border-stone-200 rounded-xl text-sm
               text-stone-600 min-h-[44px]"
        onclick={reset}
      >
        Cancel
      </button>
    </div>

  {:else if view === 'restore-preview'}
    {#if !restorePreview}
      <label for="restore-path" class="block text-sm text-stone-600 mb-1">Backup file path:</label>
      <input
        id="restore-path"
        type="text"
        class="w-full px-4 py-3 rounded-lg border border-stone-200 text-stone-700
               text-sm mb-3 min-h-[44px] font-mono"
        bind:value={restorePath}
        placeholder="/path/to/backup.coheara-backup"
      />

      {#if error}
        <p class="text-red-600 text-sm mb-3">{error}</p>
      {/if}

      <div class="flex gap-3">
        <button
          class="flex-1 px-4 py-3 bg-teal-600 text-white rounded-xl text-sm
                 font-medium min-h-[44px] disabled:opacity-50"
          disabled={loading || !restorePath.trim()}
          onclick={handlePreviewRestore}
        >
          {loading ? 'Reading...' : 'Preview backup'}
        </button>
        <button
          class="px-4 py-3 bg-white border border-stone-200 rounded-xl text-sm
                 text-stone-600 min-h-[44px]"
          onclick={reset}
        >
          Cancel
        </button>
      </div>
    {:else}
      <div class="space-y-2 text-sm mb-4">
        <div class="flex justify-between">
          <span class="text-stone-600">Profile</span>
          <span class="text-stone-800">{restorePreview.metadata.profile_name}</span>
        </div>
        <div class="flex justify-between">
          <span class="text-stone-600">Documents</span>
          <span class="text-stone-800">{restorePreview.metadata.document_count}</span>
        </div>
        <div class="flex justify-between">
          <span class="text-stone-600">Created</span>
          <span class="text-stone-800">
            {new Date(restorePreview.metadata.created_at).toLocaleDateString()}
          </span>
        </div>
        <div class="flex justify-between">
          <span class="text-stone-600">Size</span>
          <span class="text-stone-800">{formatBytes(restorePreview.total_size_bytes)}</span>
        </div>
        <div class="flex justify-between">
          <span class="text-stone-600">Version</span>
          <span class="text-stone-800">{restorePreview.metadata.coheara_version}</span>
        </div>
        {#if !restorePreview.compatible}
          <div class="bg-amber-50 rounded-lg p-3 border border-amber-200">
            <p class="text-sm text-amber-800">
              {restorePreview.compatibility_message ?? 'This backup may not be fully compatible.'}
            </p>
          </div>
        {/if}
      </div>

      <button
        class="w-full px-4 py-3 bg-teal-600 text-white rounded-xl text-sm
               font-medium min-h-[44px] mb-2"
        onclick={() => (view = 'restore-password')}
      >
        Restore this backup
      </button>
      <button
        class="w-full px-4 py-3 bg-white border border-stone-200 rounded-xl text-sm
               text-stone-600 min-h-[44px]"
        onclick={reset}
      >
        Cancel
      </button>
    {/if}

  {:else if view === 'restore-password'}
    <p class="text-sm text-stone-600 mb-3">
      Enter the password for <strong>{restorePreview?.metadata.profile_name}</strong> to decrypt the backup.
    </p>

    <input
      type="password"
      class="w-full px-4 py-3 rounded-lg border border-stone-200 text-stone-700
             text-sm mb-3 min-h-[44px]"
      bind:value={restorePassword}
      placeholder="Profile password"
    />

    {#if error}
      <p class="text-red-600 text-sm mb-3">{error}</p>
    {/if}

    <div class="flex gap-3">
      <button
        class="flex-1 px-4 py-3 bg-teal-600 text-white rounded-xl text-sm
               font-medium min-h-[44px] disabled:opacity-50"
        disabled={loading || !restorePassword}
        onclick={handleRestore}
      >
        {loading ? 'Restoring...' : 'Restore'}
      </button>
      <button
        class="px-4 py-3 bg-white border border-stone-200 rounded-xl text-sm
               text-stone-600 min-h-[44px]"
        onclick={reset}
      >
        Cancel
      </button>
    </div>
  {/if}
</section>
