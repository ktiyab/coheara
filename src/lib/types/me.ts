/** ME-REDESIGN: Me Screen types — mirrors Rust MeOverview. */

export interface MeOverview {
	identity: MeIdentity;
	alerts: MeInsight[];
	allergies: AllergyInfo[];
	reference_ranges: ReferenceRange[];
	screenings: ScreeningInfo[];
}

/** ALLERGY-01 B6: Allergy info from MeOverview. */
export interface AllergyInfo {
	id: string;
	allergen: string;
	reaction: string | null;
	severity: string;
	category: string | null;
	date_identified: string | null;
	source: string;
	verified: boolean;
	cross_reactivities: string[];
}

export interface MeIdentity {
	profile_id: string;
	name: string;
	age: number | null;
	sex: string | null;
	ethnicities: string[];
	/** BT-01: Blood type key (e.g. "o_positive"). */
	blood_type: string | null;
	/** BT-01: Human-readable blood type (e.g. "O+"). */
	blood_type_display: string | null;
	weight_kg: number | null;
	height_cm: number | null;
	bmi: number | null;
	medication_count: number;
	allergy_count: number;
}

export interface MeInsight {
	kind: string;
	severity: string;
	summary_key: string;
	description: string;
	source: string;
}

export interface ReferenceRange {
	key: string;
	label: string;
	domain: string;
	unit: string;
	source: string;
	tiers: RangeTier[];
	normal_min: number;
	normal_max: number;
	current_value: number | null;
	current_display: string | null;
	current_tier_label: string | null;
}

export interface RangeTier {
	key: string;
	label: string;
	min_value: number;
	max_value: number;
	color: string;
}

export interface ScreeningInfo {
	key: string;
	label: string;
	source: string;
	interval_months: number;
	eligible: boolean;
	min_age: number;
	max_age: number | null;
	sex_required: string | null;
	// ME-06: Vaccine/screening record fields
	category: 'cancer' | 'metabolic' | 'vaccine';
	total_doses: number;
	validity_months: number | null;
	completed_doses: CompletedDose[];
	next_due: string | null;
	is_complete: boolean;
}

export interface CompletedDose {
	record_id: string;
	dose_number: number;
	completed_at: string;
	provider: string | null;
}

/** REVIEW-01: Lightweight trend data for sparkline charts. */
export interface VitalTrendPoint {
	value: number;
	recorded_at: string;
}
