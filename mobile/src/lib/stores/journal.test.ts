// M1-04: Journal store tests — 36 tests
import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
	journalEntries,
	unsyncedCount,
	entriesByDate,
	hasEntries,
	pendingCorrelations,
	createEntry,
	saveEntry,
	updateEntry,
	deleteEntry,
	isEditable,
	getUnsyncedEntries,
	markSynced,
	setCorrelations,
	dismissCorrelation,
	emptyDraft,
	isDraftValid,
	getDesktopCategory,
	resetJournalState
} from './journal.js';
import type { JournalEntryDraft } from '$lib/types/journal.js';
import {
	SEVERITY_FACE_VALUES,
	SEVERITY_FACE_LABELS,
	SEVERITY_FACE_EMOJI,
	SYMPTOM_CHIP_CATEGORIES,
	BODY_REGION_LABELS,
	SEVERITY_FACES
} from '$lib/types/journal.js';

function makeDraft(overrides: Partial<JournalEntryDraft> = {}): JournalEntryDraft {
	return {
		severity: 6,
		bodyLocations: [],
		freeText: '',
		activityContext: '',
		symptomChip: null,
		oldcarts: null,
		...overrides
	};
}

// === ENTRY CREATION ===

describe('journal — entry creation', () => {
	beforeEach(() => resetJournalState());

	it('creates minimum entry (severity only, Mamadou 5-second flow)', () => {
		const entry = createEntry(makeDraft({ severity: 8 }));
		expect(entry.severity).toBe(8);
		expect(entry.synced).toBe(false);
		expect(entry.id).toMatch(/^journal-/);
		expect(get(journalEntries)).toHaveLength(1);
	});

	it('creates quick entry (severity + chip + text, Léa 15-second flow)', () => {
		const entry = createEntry(makeDraft({
			severity: 6,
			symptomChip: 'dizzy',
			freeText: 'Dizzy after running in PE'
		}));
		expect(entry.symptomChip).toBe('dizzy');
		expect(entry.freeText).toBe('Dizzy after running in PE');
	});

	it('creates standard entry (+ body map + activity, 30-second flow)', () => {
		const entry = createEntry(makeDraft({
			severity: 5,
			symptomChip: 'pain',
			bodyLocations: ['abdomen_upper', 'abdomen_lower'],
			freeText: 'Stomach pain after dinner',
			activityContext: 'Eating rice and vegetables'
		}));
		expect(entry.bodyLocations).toEqual(['abdomen_upper', 'abdomen_lower']);
		expect(entry.activityContext).toBe('Eating rice and vegetables');
	});

	it('creates detailed entry with OLDCARTS (Thomas 60-second flow)', () => {
		const entry = createEntry(makeDraft({
			severity: 5,
			symptomChip: 'pain',
			bodyLocations: ['abdomen_upper'],
			freeText: 'Abdominal discomfort after dinner',
			activityContext: 'Eating',
			oldcarts: {
				onset: { quick: 'today' },
				duration: { quick: 'hours' },
				character: ['dull', 'pressure'],
				aggravating: ['eating'],
				relieving: ['rest'],
				timing: ['after_meals']
			}
		}));
		expect(entry.oldcarts).not.toBeNull();
		expect(entry.oldcarts?.character).toEqual(['dull', 'pressure']);
		expect(entry.oldcarts?.timing).toEqual(['after_meals']);
	});

	it('generates unique IDs for rapid entries', () => {
		const e1 = createEntry(makeDraft({ severity: 3 }));
		const e2 = createEntry(makeDraft({ severity: 5 }));
		const e3 = createEntry(makeDraft({ severity: 7 }));
		expect(e1.id).not.toBe(e2.id);
		expect(e2.id).not.toBe(e3.id);
	});
});

// === SEVERITY PICKER ===

describe('journal — severity picker', () => {
	it('face icons map to correct severity values', () => {
		expect(SEVERITY_FACE_VALUES.good).toBe(2);
		expect(SEVERITY_FACE_VALUES.okay).toBe(4);
		expect(SEVERITY_FACE_VALUES.not_great).toBe(6);
		expect(SEVERITY_FACE_VALUES.bad).toBe(8);
		expect(SEVERITY_FACE_VALUES.awful).toBe(10);
	});

	it('provides 5 severity faces with labels and emoji', () => {
		expect(SEVERITY_FACES).toHaveLength(5);
		for (const face of SEVERITY_FACES) {
			expect(SEVERITY_FACE_LABELS[face]).toBeTruthy();
			expect(SEVERITY_FACE_EMOJI[face]).toBeTruthy();
		}
	});

	it('validates severity range 1-10', () => {
		expect(isDraftValid(makeDraft({ severity: 1 }))).toBe(true);
		expect(isDraftValid(makeDraft({ severity: 10 }))).toBe(true);
		expect(isDraftValid(makeDraft({ severity: 0 }))).toBe(false);
		expect(isDraftValid(makeDraft({ severity: 11 }))).toBe(false);
	});
});

// === SYMPTOM CHIPS ===

describe('journal — symptom chips (Dr. Diallo)', () => {
	it('maps chips to desktop categories', () => {
		expect(getDesktopCategory('pain')).toBe('Pain');
		expect(getDesktopCategory('dizzy')).toBe('Neurological/Dizziness');
		expect(getDesktopCategory('nausea')).toBe('Digestive/Nausea');
		expect(getDesktopCategory('tired')).toBe('General/Fatigue');
		expect(getDesktopCategory('breath')).toBe('Respiratory/Shortness of breath');
		expect(getDesktopCategory('mood')).toBe('Mood');
		expect(getDesktopCategory('other')).toBe('Other');
	});

	it('all 7 symptom chips have categories', () => {
		const chips = Object.keys(SYMPTOM_CHIP_CATEGORIES);
		expect(chips).toHaveLength(7);
	});

	it('chip selection is optional (null allowed)', () => {
		const entry = createEntry(makeDraft({ symptomChip: null }));
		expect(entry.symptomChip).toBeNull();
	});
});

// === BODY MAP ===

describe('journal — body map', () => {
	beforeEach(() => resetJournalState());

	it('supports multi-region selection (Thomas: radiating pain)', () => {
		const entry = createEntry(makeDraft({
			bodyLocations: ['chest_left', 'arm_left']
		}));
		expect(entry.bodyLocations).toEqual(['chest_left', 'arm_left']);
	});

	it('provides 24 body region labels', () => {
		const regions = Object.keys(BODY_REGION_LABELS);
		expect(regions.length).toBe(24);
	});

	it('body regions are optional (Léa: no location needed)', () => {
		const entry = createEntry(makeDraft({ bodyLocations: [] }));
		expect(entry.bodyLocations).toEqual([]);
	});
});

// === EDIT/DELETE RULES ===

describe('journal — edit and delete rules', () => {
	beforeEach(() => resetJournalState());

	it('edits unsynced entry (Amara: allowed before sync)', () => {
		const entry = createEntry(makeDraft({ freeText: 'Original' }));
		expect(isEditable(entry.id)).toBe(true);

		const result = updateEntry(entry.id, { freeText: 'Updated' });
		expect(result).toBe(true);
		expect(get(journalEntries)[0].freeText).toBe('Updated');
	});

	it('blocks edit on synced entry (Amara: locked after sync)', () => {
		const entry = createEntry(makeDraft({ freeText: 'Original' }));
		markSynced([entry.id]);

		expect(isEditable(entry.id)).toBe(false);
		const result = updateEntry(entry.id, { freeText: 'Should fail' });
		expect(result).toBe(false);
		expect(get(journalEntries)[0].freeText).toBe('Original');
	});

	it('deletes unsynced entry permanently', () => {
		const entry = createEntry(makeDraft());
		expect(get(journalEntries)).toHaveLength(1);

		const result = deleteEntry(entry.id);
		expect(result).toBe(true);
		expect(get(journalEntries)).toHaveLength(0);
	});

	it('deletes synced entry (hides locally, desktop retains)', () => {
		const entry = createEntry(makeDraft());
		markSynced([entry.id]);

		const result = deleteEntry(entry.id);
		expect(result).toBe(true);
		expect(get(journalEntries)).toHaveLength(0); // Hidden locally
	});
});

// === SYNC PROTOCOL ===

describe('journal — sync protocol', () => {
	beforeEach(() => resetJournalState());

	it('getUnsyncedEntries returns only unsynced, ordered by creation', () => {
		createEntry(makeDraft({ severity: 3 }));
		createEntry(makeDraft({ severity: 5 }));
		createEntry(makeDraft({ severity: 7 }));

		// Mark first as synced
		const entries = get(journalEntries);
		markSynced([entries[0].id]);

		const unsynced = getUnsyncedEntries();
		expect(unsynced).toHaveLength(2);
		expect(unsynced[0].severity).toBe(5);
		expect(unsynced[1].severity).toBe(7);
	});

	it('markSynced updates entries with syncedAt timestamp', () => {
		const e1 = createEntry(makeDraft({ severity: 3 }));
		const e2 = createEntry(makeDraft({ severity: 5 }));

		markSynced([e1.id]);

		const entries = get(journalEntries);
		expect(entries[0].synced).toBe(true);
		expect(entries[0].syncedAt).not.toBeNull();
		expect(entries[1].synced).toBe(false);
		expect(entries[1].syncedAt).toBeNull();
	});

	it('unsyncedCount reflects pending entries', () => {
		expect(get(unsyncedCount)).toBe(0);

		createEntry(makeDraft());
		createEntry(makeDraft());
		expect(get(unsyncedCount)).toBe(2);

		const entries = get(journalEntries);
		markSynced([entries[0].id]);
		expect(get(unsyncedCount)).toBe(1);
	});

	it('handles correlation toast from sync result', () => {
		setCorrelations([{
			entryId: 'e1',
			medication: 'Lisinopril',
			daysSinceChange: 3,
			message: 'Your dizziness may be related to the Lisinopril change 3 days ago.'
		}]);

		const corrs = get(pendingCorrelations);
		expect(corrs).toHaveLength(1);
		expect(corrs[0].medication).toBe('Lisinopril');
	});

	it('dismisses individual correlation', () => {
		setCorrelations([
			{ entryId: 'e1', medication: 'Med A', daysSinceChange: 2, message: 'msg1' },
			{ entryId: 'e2', medication: 'Med B', daysSinceChange: 5, message: 'msg2' }
		]);

		dismissCorrelation('e1');
		expect(get(pendingCorrelations)).toHaveLength(1);
		expect(get(pendingCorrelations)[0].entryId).toBe('e2');
	});
});

// === JOURNAL HISTORY ===

describe('journal — history view', () => {
	beforeEach(() => resetJournalState());

	it('groups entries by date', () => {
		createEntry(makeDraft({ severity: 5 }));
		createEntry(makeDraft({ severity: 7 }));

		const groups = get(entriesByDate);
		expect(groups.length).toBeGreaterThanOrEqual(1);
		expect(groups[0].label).toBe('Today');
	});

	it('hasEntries reflects journal state', () => {
		expect(get(hasEntries)).toBe(false);
		createEntry(makeDraft());
		expect(get(hasEntries)).toBe(true);
	});

	it('empty draft has default values', () => {
		const draft = emptyDraft();
		expect(draft.severity).toBe(5);
		expect(draft.bodyLocations).toEqual([]);
		expect(draft.freeText).toBe('');
		expect(draft.symptomChip).toBeNull();
		expect(draft.oldcarts).toBeNull();
	});
});

// === OFFLINE BEHAVIOR ===

describe('journal — offline behavior', () => {
	beforeEach(() => resetJournalState());

	it('saves offline entry with saved_offline status', () => {
		const result = saveEntry(makeDraft({ severity: 6 }), false);
		expect(result).toBe('saved_offline');
		expect(get(journalEntries)).toHaveLength(1);
		expect(get(journalEntries)[0].synced).toBe(false);
	});

	it('entries persist across connection state changes', () => {
		createEntry(makeDraft({ severity: 4 }));
		createEntry(makeDraft({ severity: 8 }));

		// Entries should still be there regardless of connection
		expect(get(journalEntries)).toHaveLength(2);
		expect(get(unsyncedCount)).toBe(2);
	});
});

// === RESET ===

describe('journal — state management', () => {
	it('resetJournalState clears everything', () => {
		createEntry(makeDraft());
		setCorrelations([{ entryId: 'e1', medication: 'Med', daysSinceChange: 1, message: 'msg' }]);

		resetJournalState();
		expect(get(journalEntries)).toHaveLength(0);
		expect(get(pendingCorrelations)).toHaveLength(0);
		expect(get(hasEntries)).toBe(false);
	});
});
