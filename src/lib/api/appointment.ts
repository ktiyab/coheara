// L4-02: Appointment Prep — Tauri invoke wrappers.

import { invoke } from '@tauri-apps/api/core';
import type {
  AppointmentRequest,
  AppointmentPrep,
  PostAppointmentNotes,
  StoredAppointment,
  ProfessionalInfo,
} from '$lib/types/appointment';

export async function listProfessionals(): Promise<ProfessionalInfo[]> {
  return invoke<ProfessionalInfo[]>('list_professionals');
}

export async function prepareAppointment(
  request: AppointmentRequest
): Promise<AppointmentPrep> {
  return invoke<AppointmentPrep>('prepare_appointment', { request });
}

export async function exportPrepPdf(
  prep: AppointmentPrep,
  copyType: 'patient' | 'professional' | 'both'
): Promise<string[]> {
  return invoke<string[]>('export_prep_pdf', { prep, copyType });
}

export async function saveAppointmentNotes(
  notes: PostAppointmentNotes
): Promise<void> {
  return invoke('save_appointment_notes', { notes });
}

export async function listAppointments(): Promise<StoredAppointment[]> {
  return invoke<StoredAppointment[]>('list_appointments');
}
