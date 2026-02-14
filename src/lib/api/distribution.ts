/** ADS: Tauri IPC wrappers for the App Distribution Server. */

import { invoke } from '@tauri-apps/api/core';
import type { DistributionStatus, InstallQrCode } from '$lib/types/distribution';

/** Start the distribution server. Returns QR code for phone to scan. */
export async function startDistribution(): Promise<InstallQrCode> {
	return invoke<InstallQrCode>('start_distribution');
}

/** Stop the distribution server. */
export async function stopDistribution(): Promise<void> {
	return invoke('stop_distribution');
}

/** Get distribution server status, or null if not running. */
export async function getDistributionStatus(): Promise<DistributionStatus | null> {
	return invoke<DistributionStatus | null>('get_distribution_status');
}

/** Get the install QR code from the running distribution server. */
export async function getInstallQr(): Promise<InstallQrCode> {
	return invoke<InstallQrCode>('get_install_qr');
}
