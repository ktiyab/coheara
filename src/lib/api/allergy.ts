/** ALLERGY-01 B5: Allergy CRUD + reference data API. */

import { invoke } from '@tauri-apps/api/core';

export interface AllergenReference {
	key: string;
	label: string;
	category: string;
	mechanism: string;
	source: string;
}

/** Add a new allergy (manual patient entry). Returns the allergy UUID. */
export async function addAllergy(
	allergen: string,
	severity: string,
	reaction?: string | null,
	allergenCategory?: string | null,
	dateIdentified?: string | null,
): Promise<string> {
	return invoke('add_allergy', {
		allergen,
		reaction: reaction ?? null,
		severity,
		allergenCategory: allergenCategory ?? null,
		dateIdentified: dateIdentified ?? null,
	});
}

/** Update an existing allergy (partial update). */
export async function updateAllergy(
	allergyId: string,
	allergen?: string | null,
	reaction?: string | null,
	severity?: string | null,
	allergenCategory?: string | null,
	dateIdentified?: string | null,
): Promise<void> {
	return invoke('update_allergy', {
		allergyId,
		allergen: allergen ?? null,
		reaction: reaction ?? null,
		severity: severity ?? null,
		allergenCategory: allergenCategory ?? null,
		dateIdentified: dateIdentified ?? null,
	});
}

/** Delete an allergy by ID. Returns true if deleted. */
export async function deleteAllergy(allergyId: string): Promise<boolean> {
	return invoke('delete_allergy', { allergyId });
}

/** Mark an allergy as verified. */
export async function verifyAllergy(allergyId: string): Promise<void> {
	return invoke('verify_allergy', { allergyId });
}

/** Get canonical allergen references for autocomplete. */
export async function getAllergenReferences(
	lang: string,
	category?: string | null,
): Promise<AllergenReference[]> {
	return invoke('get_allergen_references', {
		category: category ?? null,
		lang,
	});
}
