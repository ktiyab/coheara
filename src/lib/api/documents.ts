/** E2E-F04: Document list/detail API — Tauri IPC wrappers. */

import { invoke } from '@tauri-apps/api/core';
import type { DocumentDetail, EntityConnection, ProcessingLogEntry } from '$lib/types/documents';
import type { DocumentCard } from '$lib/types/home';

/** Fetch full document detail with all linked entities. */
export async function getDocumentDetail(documentId: string): Promise<DocumentDetail> {
  return invoke<DocumentDetail>('get_document_detail', { documentId });
}

/** Fetch all documents (paginated). Re-uses home's get_more_documents. */
export async function getDocuments(offset: number, limit: number): Promise<DocumentCard[]> {
  return invoke<DocumentCard[]>('get_more_documents', { offset, limit });
}

/** BTL-10 UX GAP-01: Get entity connections for a document. */
export async function getDocumentConnections(documentId: string): Promise<EntityConnection[]> {
  return invoke<EntityConnection[]>('get_document_connections', { documentId });
}

/** BTL-10 UX GAP-04: Get processing log for a document. */
export async function getProcessingLog(documentId: string): Promise<ProcessingLogEntry[]> {
  return invoke<ProcessingLogEntry[]>('get_processing_log', { documentId });
}

/** Spec 46 [CG-06] + Spec 49: Full-text document search. */
export interface DocumentSearchResult {
  document_id: string;
  title: string;
  professional_name: string | null;
  snippet: string;
  rank: number;
}

export async function searchDocuments(
  query: string,
  docTypeFilter?: string
): Promise<DocumentSearchResult[]> {
  return invoke<DocumentSearchResult[]>('search_documents', {
    query,
    docTypeFilter: docTypeFilter ?? null,
  });
}
