/** SEC-HTTPS-01: Tauri IPC wrappers for the HTTPS Mobile API server. */

import { invoke } from '@tauri-apps/api/core';
import type { MobileApiSession, MobileApiStatus } from '$lib/types/mobile_api';

/** Start the HTTPS Mobile API server. Generates CA + server cert if needed. */
export async function startMobileApi(): Promise<MobileApiSession> {
	return invoke<MobileApiSession>('start_mobile_api');
}

/** Stop the HTTPS Mobile API server (graceful 5s shutdown). */
export async function stopMobileApi(): Promise<void> {
	return invoke('stop_mobile_api');
}

/** Get the current Mobile API server status. */
export async function getMobileApiStatus(): Promise<MobileApiStatus> {
	return invoke<MobileApiStatus>('get_mobile_api_status');
}
