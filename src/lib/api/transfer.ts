/** L4-03: WiFi Transfer API — Tauri IPC wrappers. */

import { invoke } from '@tauri-apps/api/core';
import type { QrCodeData, TransferStatusResponse } from '$lib/types/transfer';

export async function startWifiTransfer(): Promise<QrCodeData> {
  return invoke<QrCodeData>('start_wifi_transfer');
}

export async function stopWifiTransfer(): Promise<void> {
  return invoke('stop_wifi_transfer');
}

export async function getTransferStatus(): Promise<TransferStatusResponse | null> {
  return invoke<TransferStatusResponse | null>('get_transfer_status');
}

export async function processStagedFiles(): Promise<number> {
  return invoke<number>('process_staged_files');
}
