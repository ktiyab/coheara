/** L3-04 Review Screen — Tauri invoke wrappers. */

import { invoke } from '@tauri-apps/api/core';
import type {
  ReviewData,
  FieldCorrection,
  ExcludedEntity,
  ReviewConfirmResult,
  ReviewRejectResult,
} from '$lib/types/review';

export async function getReviewData(documentId: string): Promise<ReviewData> {
  return invoke<ReviewData>('get_review_data', { documentId });
}

export async function getOriginalFile(documentId: string): Promise<string> {
  return invoke<string>('get_original_file', { documentId });
}

export async function updateExtractedField(
  documentId: string,
  fieldId: string,
  newValue: string,
): Promise<void> {
  return invoke('update_extracted_field', { documentId, fieldId, newValue });
}

export async function confirmReview(
  documentId: string,
  corrections: FieldCorrection[],
  excludedEntities: ExcludedEntity[] = [],
): Promise<ReviewConfirmResult> {
  return invoke<ReviewConfirmResult>('confirm_review', { documentId, corrections, excludedEntities });
}

export async function rejectReview(
  documentId: string,
  reason: string | null,
  action: 'retry' | 'remove',
): Promise<ReviewRejectResult> {
  return invoke<ReviewRejectResult>('reject_review', { documentId, reason, action });
}
