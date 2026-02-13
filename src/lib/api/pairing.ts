/** M0-02: Device pairing API layer. */

import { invoke } from '@tauri-apps/api/core';
import type { PairingStartResponse, PendingApproval } from '$lib/types/pairing';

export async function startPairing(): Promise<PairingStartResponse> {
  return invoke<PairingStartResponse>('start_pairing');
}

export async function cancelPairing(): Promise<void> {
  return invoke<void>('cancel_pairing');
}

export async function getPendingApproval(): Promise<PendingApproval | null> {
  return invoke<PendingApproval | null>('get_pending_approval');
}

export async function approvePairing(): Promise<void> {
  return invoke<void>('approve_pairing');
}

export async function denyPairing(): Promise<void> {
  return invoke<void>('deny_pairing');
}
