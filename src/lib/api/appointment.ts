// L4-02: Appointment — Tauri invoke wrappers.

import { invoke } from '@tauri-apps/api/core';
import type { StoredAppointment } from '$lib/types/appointment';

export async function listAppointments(): Promise<StoredAppointment[]> {
  return invoke<StoredAppointment[]>('list_appointments');
}
