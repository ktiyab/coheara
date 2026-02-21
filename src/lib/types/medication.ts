// L3-05: Medication List — TypeScript types matching Rust backend.

export interface MedicationCard {
  id: string;
  generic_name: string;
  brand_name: string | null;
  dose: string;
  frequency: string;
  frequency_type: string;
  route: string;
  prescriber_name: string | null;
  prescriber_specialty: string | null;
  start_date: string | null;
  end_date: string | null;
  status: string;
  reason_start: string | null;
  is_otc: boolean;
  is_compound: boolean;
  has_tapering: boolean;
  dose_type: string;
  administration_instructions: string | null;
  condition: string | null;
  coherence_alerts: MedicationAlert[];
}

export interface MedicationAlert {
  id: string;
  alert_type: string;
  severity: string;
  summary: string;
}

export interface MedicationListFilter {
  status: string | null;
  prescriber_id: string | null;
  search_query: string | null;
  include_otc: boolean;
}

export interface MedicationListData {
  medications: MedicationCard[];
  total_active: number;
  total_paused: number;
  total_stopped: number;
  prescribers: PrescriberOption[];
}

export interface PrescriberOption {
  id: string;
  name: string;
  specialty: string | null;
  medication_count: number;
}
