/** E2E-B01 & B02: Document Import & Processing API — Tauri IPC wrappers. */

import { invoke } from '@tauri-apps/api/core';
import type { ImportResult, ProcessingOutcome } from '$lib/types/import';

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
