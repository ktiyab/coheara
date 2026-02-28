/** BTL-10 C5: Import queue types — mirrors Rust import_queue.rs. */

export type ImportJobState =
  | 'Queued'
  | 'Importing'
  | 'Extracting'
  | 'Structuring'
  | 'PendingReview'
  | 'Done'
  | 'Failed'
  | 'Cancelled';

export interface ImportQueueItem {
  id: string;
  file_path: string;
  filename: string;
  state: ImportJobState;
  progress_pct: number;
  document_id: string | null;
  model_used: string | null;
  error: string | null;
  queued_at: string;
  started_at: string | null;
  completed_at: string | null;
}

export interface QueueSnapshot {
  jobs: ImportQueueItem[];
  is_running: boolean;
}

/** Tauri event payload for queue updates. */
export interface ImportQueueEvent {
  job_id: string;
  state: ImportJobState;
  progress_pct: number;
  filename: string;
  document_id: string | null;
  error: string | null;
}
