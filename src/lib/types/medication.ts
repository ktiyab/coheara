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

export interface MedicationDetail {
  medication: MedicationCard;
  instructions: MedicationInstructionView[];
  compound_ingredients: CompoundIngredientView[];
  tapering_steps: TaperingStepView[];
  aliases: MedicationAliasView[];
  dose_changes: DoseChangeView[];
  document_title: string | null;
  document_date: string | null;
}

export interface MedicationInstructionView {
  id: string;
  instruction: string;
  timing: string | null;
}

export interface CompoundIngredientView {
  id: string;
  ingredient_name: string;
  ingredient_dose: string | null;
  maps_to_generic: string | null;
}

export interface TaperingStepView {
  step_number: number;
  dose: string;
  duration_days: number;
  start_date: string | null;
  instructions: string | null;
  is_current: boolean;
}

export interface DoseChangeView {
  id: string;
  old_dose: string | null;
  new_dose: string;
  old_frequency: string | null;
  new_frequency: string | null;
  change_date: string;
  changed_by_name: string | null;
  reason: string | null;
  document_title: string | null;
}

export interface MedicationAliasView {
  generic_name: string;
  brand_name: string;
  country: string;
  source: string;
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

export interface OtcMedicationInput {
  name: string;
  dose: string;
  frequency: string;
  route: string;
  reason: string | null;
  start_date: string | null;
  instructions: string | null;
}

export interface AliasSearchResult {
  generic_name: string;
  brand_names: string[];
  source: string;
}
