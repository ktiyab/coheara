// L5-01: Trust & Safety — TypeScript interfaces

// ─── Emergency Protocol ───

export interface CriticalLabAlert {
  id: string;
  test_name: string;
  value: string;
  unit: string;
  reference_range: string;
  abnormal_flag: string;
  lab_date: string;
  document_id: string;
  detected_at: string;
  dismissed: boolean;
}

export type DismissStep =
  | 'AskConfirmation'
  | { ConfirmDismissal: { reason: string } };

export interface CriticalDismissRequest {
  alert_id: string;
  step: DismissStep;
}

// ─── Dose Plausibility ───

export type PlausibilityResult =
  | 'Plausible'
  | { HighDose: { message: string } }
  | { VeryHighDose: { message: string } }
  | { LowDose: { message: string } }
  | 'UnknownMedication';

export interface DosePlausibility {
  medication_name: string;
  extracted_dose: string;
  extracted_value: number;
  extracted_unit: string;
  typical_range_low: number;
  typical_range_high: number;
  typical_unit: string;
  plausibility: PlausibilityResult;
}

// ─── Backup & Restore ───

export interface BackupMetadata {
  version: number;
  created_at: string;
  profile_name: string;
  document_count: number;
  coheara_version: string;
  salt_b64: string;
}

export interface BackupResult {
  backup_path: string;
  total_documents: number;
  total_size_bytes: number;
  created_at: string;
  encrypted: boolean;
}

export interface RestorePreview {
  metadata: BackupMetadata;
  file_count: number;
  total_size_bytes: number;
  compatible: boolean;
  compatibility_message: string | null;
}

export interface RestoreResult {
  documents_restored: number;
  total_size_bytes: number;
  warnings: string[];
}

// ─── Cryptographic Erasure ───

export interface ErasureRequest {
  profile_id: string;
  confirmation_text: string;
  password: string;
}

export interface ErasureResult {
  profile_name: string;
  files_deleted: number;
  bytes_erased: number;
  key_zeroed: boolean;
}

// ─── Privacy Verification ───

export interface PrivacyInfo {
  data_location: string;
  total_data_size_bytes: number;
  document_count: number;
  last_backup_date: string | null;
  encryption_algorithm: string;
  key_derivation: string;
  network_permissions: string;
  telemetry: string;
}
