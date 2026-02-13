/** ME-02: Device management API layer. */

import { invoke } from '@tauri-apps/api/core';
import type { DeviceSummary, DeviceCount, InactiveWarning } from '$lib/types/devices';

export async function listPairedDevices(): Promise<DeviceSummary[]> {
  return invoke<DeviceSummary[]>('list_paired_devices');
}

export async function unpairDevice(deviceId: string): Promise<void> {
  return invoke<void>('unpair_device', { deviceId });
}

export async function getDeviceCount(): Promise<DeviceCount> {
  return invoke<DeviceCount>('get_device_count');
}

export async function getInactiveWarnings(): Promise<InactiveWarning[]> {
  return invoke<InactiveWarning[]>('get_inactive_warnings');
}
