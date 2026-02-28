/** E2E-B01 & B02: Document Import & Processing API — Tauri IPC wrappers. */

import { invoke } from '@tauri-apps/api/core';
import type { ImportResult, ProcessingOutcome } from '$lib/types/import';
import type { QueueSnapshot } from '$lib/types/import-queue';

/** Import a single document from a local file path (staging only). */
export async function importDocument(filePath: string): Promise<ImportResult> {
  return invoke<ImportResult>('import_document', { filePath });
}

/** Import multiple documents from local file paths (max 50, staging only). */
export async function importDocumentsBatch(filePaths: string[]): Promise<ImportResult[]> {
  return invoke<ImportResult[]>('import_documents_batch', { filePaths });
}

/** Process a document end-to-end: import → extract → structure → pending review. */
export async function processDocument(filePath: string): Promise<ProcessingOutcome> {
  return invoke<ProcessingOutcome>('process_document', { filePath });
}

/** Process multiple documents end-to-end (max 20). */
export async function processDocumentsBatch(filePaths: string[]): Promise<ProcessingOutcome[]> {
  return invoke<ProcessingOutcome[]>('process_documents_batch', { filePaths });
}

// ---------------------------------------------------------------------------
// BTL-10: Import queue IPC wrappers
// ---------------------------------------------------------------------------

/** Enqueue files for import. Returns job IDs. */
export async function enqueueImports(filePaths: string[]): Promise<string[]> {
  return invoke<string[]>('enqueue_imports', { filePaths });
}

/** Get the full import queue snapshot. */
export async function getImportQueue(): Promise<QueueSnapshot> {
  return invoke<QueueSnapshot>('get_import_queue');
}

/** Cancel an active import job. */
export async function cancelImport(jobId: string): Promise<void> {
  return invoke<void>('cancel_import', { jobId });
}

/** Retry a failed import job. Returns new job ID. */
export async function retryImport(jobId: string): Promise<string> {
  return invoke<string>('retry_import', { jobId });
}

/** Delete a terminal import job from the queue. */
export async function deleteImport(jobId: string): Promise<void> {
  return invoke<void>('delete_import', { jobId });
}

/** Delete a document from the database. */
export async function deleteDocument(documentId: string): Promise<void> {
  return invoke<void>('delete_document', { documentId });
}

/** Reprocess a failed/imported document. */
export async function reprocessDocument(documentId: string): Promise<ProcessingOutcome> {
  return invoke<ProcessingOutcome>('reprocess_document', { documentId });
}
