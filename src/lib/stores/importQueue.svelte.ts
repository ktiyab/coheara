// BTL-10 C5: Import queue reactive store — persists across navigation.

import {
  enqueueImports,
  getImportQueue,
  cancelImport,
  retryImport,
  deleteImport,
} from '$lib/api/import';
import type { ImportQueueItem, ImportQueueEvent } from '$lib/types/import-queue';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { isTauriEnv } from '$lib/utils/tauri';

class ImportQueueStore {
  items = $state<ImportQueueItem[]>([]);
  isRunning = $state(false);
  loading = $state(false);
  error = $state<string | null>(null);

  private _unlisten: UnlistenFn | null = null;

  // -- Derived state --

  get activeItems(): ImportQueueItem[] {
    return this.items.filter(
      (j) => !['Done', 'Failed', 'Cancelled'].includes(j.state),
    );
  }

  get hasActiveImports(): boolean {
    return this.activeItems.length > 0;
  }

  get completedItems(): ImportQueueItem[] {
    return this.items.filter((j) => j.state === 'Done');
  }

  get failedItems(): ImportQueueItem[] {
    return this.items.filter((j) => j.state === 'Failed');
  }

  get activeCount(): number {
    return this.activeItems.length;
  }

  // -- Event subscription --

  /** Start listening to import-queue-update Tauri events. Call once at app startup. */
  async startListening(): Promise<void> {
    if (!isTauriEnv() || this._unlisten) return;
    this._unlisten = await listen<ImportQueueEvent>(
      'import-queue-update',
      (event) => {
        this.handleEvent(event.payload);
      },
    );
  }

  /** Stop listening. Call on cleanup. */
  stopListening(): void {
    this._unlisten?.();
    this._unlisten = null;
  }

  /** F7: Clear all state on lock/switch to prevent cross-profile data leakage. */
  reset(): void {
    this.items = [];
    this.isRunning = false;
    this.loading = false;
    this.error = null;
  }

  // -- Event handling --

  private handleEvent(event: ImportQueueEvent): void {
    const idx = this.items.findIndex((j) => j.id === event.job_id);
    if (idx >= 0) {
      // Update existing job
      const updated = { ...this.items[idx] };
      updated.state = event.state;
      updated.progress_pct = event.progress_pct;
      if (event.document_id) updated.document_id = event.document_id;
      if (event.error) updated.error = event.error;
      this.items = [
        ...this.items.slice(0, idx),
        updated,
        ...this.items.slice(idx + 1),
      ];
    }
    // If job not found locally, do a full refresh
    else {
      this.refresh().catch(() => {});
    }
  }

  // -- Actions --

  /** Full refresh from backend. */
  async refresh(): Promise<void> {
    try {
      this.loading = true;
      this.error = null;
      const snapshot = await getImportQueue();
      this.items = snapshot.jobs;
      this.isRunning = snapshot.is_running;
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
    } finally {
      this.loading = false;
    }
  }

  /** Enqueue files for import.
   * UC-01: `documentType` bypasses LLM classification when provided. */
  async enqueue(filePaths: string[], documentType?: string): Promise<string[]> {
    try {
      const jobIds = await enqueueImports(filePaths, documentType);
      await this.refresh();
      return jobIds;
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
      return [];
    }
  }

  /** Cancel an active job. */
  async cancel(jobId: string): Promise<void> {
    await cancelImport(jobId);
    await this.refresh();
  }

  /** Retry a failed job. Returns new job ID. */
  async retry(jobId: string): Promise<string | null> {
    try {
      const newId = await retryImport(jobId);
      await this.refresh();
      return newId;
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
      return null;
    }
  }

  /** Delete a terminal job. */
  async delete(jobId: string): Promise<void> {
    await deleteImport(jobId);
    this.items = this.items.filter((j) => j.id !== jobId);
  }
}

export const importQueue = new ImportQueueStore();
