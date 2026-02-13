// M1-04: Journal types — symptom journal (offline-first)

/** Severity face icons (Mamadou: one-tap entry) */
export type SeverityFace = 'good' | 'okay' | 'not_great' | 'bad' | 'awful';

/** Map severity faces to numeric values */
export const SEVERITY_FACE_VALUES: Record<SeverityFace, number> = {
	good: 2,
	okay: 4,
	not_great: 6,
	bad: 8,
	awful: 10
} as const;

/** Severity face labels */
export const SEVERITY_FACE_LABELS: Record<SeverityFace, string> = {
	good: 'Good',
	okay: 'Okay',
	not_great: 'Not great',
	bad: 'Bad',
	awful: 'Awful'
} as const;

/** Severity face emoji characters */
export const SEVERITY_FACE_EMOJI: Record<SeverityFace, string> = {
	good: '\uD83D\uDE0A',      // 😊
	okay: '\uD83D\uDE10',      // 😐
	not_great: '\uD83D\uDE1F', // 😟
	bad: '\uD83D\uDE23',       // 😣
	awful: '\uD83D\uDE30'      // 😰
} as const;

/** Symptom chips (Dr. Diallo: "clinical short-circuits") */
export type SymptomChip = 'pain' | 'dizzy' | 'nausea' | 'tired' | 'breath' | 'mood' | 'other';

/** Symptom chip display labels */
export const SYMPTOM_CHIP_LABELS: Record<SymptomChip, string> = {
	pain: 'Pain',
	dizzy: 'Dizzy',
	nausea: 'Nausea',
	tired: 'Tired',
	breath: 'Short of breath',
	mood: 'Mood',
	other: 'Other'
} as const;

/** Symptom chip → desktop category mapping */
export const SYMPTOM_CHIP_CATEGORIES: Record<SymptomChip, string> = {
	pain: 'Pain',
	dizzy: 'Neurological/Dizziness',
	nausea: 'Digestive/Nausea',
	tired: 'General/Fatigue',
	breath: 'Respiratory/Shortness of breath',
	mood: 'Mood',
	other: 'Other'
} as const;

/** Body region identifiers (22 regions matching desktop BODY_REGIONS) */
export type BodyRegion =
	| 'head' | 'face' | 'neck'
	| 'chest_left' | 'chest_right' | 'chest_center'
	| 'abdomen_upper' | 'abdomen_lower'
	| 'back_upper' | 'back_lower'
	| 'shoulder_left' | 'shoulder_right'
	| 'arm_left' | 'arm_right'
	| 'hand_left' | 'hand_right'
	| 'hip_left' | 'hip_right'
	| 'leg_left' | 'leg_right'
	| 'knee_left' | 'knee_right'
	| 'foot_left' | 'foot_right';

/** Body region display labels */
export const BODY_REGION_LABELS: Record<BodyRegion, string> = {
	head: 'Head', face: 'Face', neck: 'Neck',
	chest_left: 'Left chest', chest_right: 'Right chest', chest_center: 'Center chest',
	abdomen_upper: 'Upper abdomen', abdomen_lower: 'Lower abdomen',
	back_upper: 'Upper back', back_lower: 'Lower back',
	shoulder_left: 'Left shoulder', shoulder_right: 'Right shoulder',
	arm_left: 'Left arm', arm_right: 'Right arm',
	hand_left: 'Left hand', hand_right: 'Right hand',
	hip_left: 'Left hip', hip_right: 'Right hip',
	leg_left: 'Left leg', leg_right: 'Right leg',
	knee_left: 'Left knee', knee_right: 'Right knee',
	foot_left: 'Left foot', foot_right: 'Right foot'
} as const;

/** OLDCARTS onset quick options */
export type OnsetQuick = 'today' | 'yesterday' | 'this_week' | 'custom';

/** OLDCARTS duration quick options */
export type DurationQuick = 'minutes' | 'hours' | 'constant' | 'custom';

/** OLDCARTS character options */
export const CHARACTER_OPTIONS = ['sharp', 'dull', 'burning', 'throbbing', 'pressure', 'other'] as const;

/** OLDCARTS aggravating factors */
export const AGGRAVATING_OPTIONS = ['movement', 'eating', 'stress', 'standing', 'lying_down', 'other'] as const;

/** OLDCARTS relieving factors */
export const RELIEVING_OPTIONS = ['rest', 'medication', 'ice_heat', 'eating', 'position_change'] as const;

/** OLDCARTS timing patterns */
export const TIMING_OPTIONS = ['constant', 'comes_and_goes', 'certain_times', 'after_meals', 'after_meds'] as const;

/** OLDCARTS data structure */
export interface OldcartsData {
	onset?: { quick: OnsetQuick; customDate?: string };
	duration?: { quick: DurationQuick; customText?: string };
	character?: string[];
	aggravating?: string[];
	relieving?: string[];
	timing?: string[];
}

/** Journal entry (stored locally) */
export interface JournalEntry {
	id: string;
	severity: number;
	bodyLocations: BodyRegion[];
	freeText: string;
	activityContext: string;
	symptomChip: SymptomChip | null;
	oldcarts: OldcartsData | null;
	createdAt: string;
	synced: boolean;
	syncedAt: string | null;
}

/** New entry draft (before saving) */
export interface JournalEntryDraft {
	severity: number;
	bodyLocations: BodyRegion[];
	freeText: string;
	activityContext: string;
	symptomChip: SymptomChip | null;
	oldcarts: OldcartsData | null;
}

/** Journal sync result from desktop */
export interface JournalSyncResult {
	syncedIds: string[];
	correlations: JournalCorrelation[];
}

/** Temporal correlation found after sync */
export interface JournalCorrelation {
	entryId: string;
	medication: string;
	daysSinceChange: number;
	message: string;
}

/** Journal entry date group (for history view) */
export interface JournalDateGroup {
	label: string;
	date: string;
	entries: JournalEntry[];
}

/** Journal sync status for display */
export type JournalSyncStatus = 'synced' | 'pending' | 'failed';

/** Save result */
export type SaveResult = 'saved_offline' | 'saved_synced' | 'saved_sync_failed';

/** All severity face keys for iteration */
export const SEVERITY_FACES: readonly SeverityFace[] = ['good', 'okay', 'not_great', 'bad', 'awful'] as const;

/** All symptom chip keys for iteration */
export const SYMPTOM_CHIPS: readonly SymptomChip[] = ['pain', 'dizzy', 'nausea', 'tired', 'breath', 'mood', 'other'] as const;
