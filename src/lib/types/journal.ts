// L4-01: Symptom Journal — TypeScript types matching Rust backend.

export interface SymptomEntry {
  category: string;
  specific: string;
  severity: number; // 1-5
  onset_date: string; // YYYY-MM-DD
  onset_time: string | null; // HH:MM or null
  body_region: string | null;
  duration: string | null;
  character: string | null;
  aggravating: string[];
  relieving: string[];
  timing_pattern: string | null;
  notes: string | null;
}

export interface StoredSymptom {
  id: string;
  category: string;
  specific: string;
  severity: number;
  body_region: string | null;
  duration: string | null;
  character: string | null;
  aggravating: string | null;
  relieving: string | null;
  timing_pattern: string | null;
  onset_date: string;
  onset_time: string | null;
  recorded_date: string;
  still_active: boolean;
  resolved_date: string | null;
  related_medication_name: string | null;
  related_diagnosis_name: string | null;
  notes: string | null;
  source: string;
}

export interface TemporalCorrelation {
  medication_name: string;
  medication_change_date: string;
  days_since_change: number;
  message: string;
}

export interface RecordResult {
  symptom_id: string;
  correlations: TemporalCorrelation[];
}

export interface NudgeDecision {
  should_nudge: boolean;
  nudge_type: string | null; // "DailyCheckIn" or "PostMedicationChange"
  message: string | null;
  related_medication: string | null;
}

export interface SymptomFilter {
  category: string | null;
  severity_min: number | null;
  severity_max: number | null;
  date_from: string | null;
  date_to: string | null;
  still_active: boolean | null;
}

export interface CategoryInfo {
  name: string;
  subcategories: string[];
}

export const CATEGORIES = [
  'Pain',
  'Digestive',
  'Respiratory',
  'Neurological',
  'General',
  'Mood',
  'Skin',
  'Other',
] as const;

export const SUBCATEGORIES: Record<string, string[]> = {
  Pain: [
    'Headache', 'Back pain', 'Joint pain', 'Chest pain',
    'Abdominal pain', 'Muscle pain', 'Neck pain', 'Other',
  ],
  Digestive: [
    'Nausea', 'Vomiting', 'Diarrhea', 'Constipation',
    'Bloating', 'Heartburn', 'Loss of appetite', 'Other',
  ],
  Respiratory: [
    'Shortness of breath', 'Cough', 'Wheezing',
    'Chest tightness', 'Sore throat', 'Congestion', 'Other',
  ],
  Neurological: [
    'Dizziness', 'Numbness', 'Tingling', 'Tremor',
    'Memory issues', 'Confusion', 'Other',
  ],
  General: [
    'Fatigue', 'Fever', 'Chills', 'Weight change',
    'Night sweats', 'Swelling', 'Other',
  ],
  Mood: [
    'Anxiety', 'Low mood', 'Irritability', 'Sleep difficulty',
    'Difficulty concentrating', 'Other',
  ],
  Skin: [
    'Rash', 'Itching', 'Bruising', 'Dryness',
    'Swelling', 'Color change', 'Other',
  ],
  Other: ['Other'],
};

export const SEVERITY_LABELS = [
  '',
  'Barely noticeable',
  'Mild',
  'Moderate',
  'Severe',
  'Very severe',
];

export const SEVERITY_COLORS = [
  '',
  '#4ade80',
  '#a3e635',
  '#facc15',
  '#fb923c',
  '#f87171',
];

export const COMMON_SYMPTOMS = [
  { category: 'Pain', specific: 'Headache', labelKey: 'journal.symptom_headache' },
  { category: 'General', specific: 'Fatigue', labelKey: 'journal.symptom_fatigue' },
  { category: 'Digestive', specific: 'Nausea', labelKey: 'journal.symptom_nausea' },
  { category: 'General', specific: 'Fever', labelKey: 'journal.symptom_fever' },
  { category: 'Pain', specific: 'Back pain', labelKey: 'journal.symptom_back_pain' },
  { category: 'Respiratory', specific: 'Cough', labelKey: 'journal.symptom_cough' },
  { category: 'Mood', specific: 'Anxiety', labelKey: 'journal.symptom_anxiety' },
  { category: 'Mood', specific: 'Sleep difficulty', labelKey: 'journal.symptom_sleep' },
] as const;
