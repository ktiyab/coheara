// L4-02: Appointment — TypeScript interfaces matching Rust backend types.

export interface StoredAppointment {
  id: string;
  professional_name: string;
  professional_specialty: string;
  date: string;
  appointment_type: string;
  prep_generated: boolean;
  has_post_notes: boolean;
}
