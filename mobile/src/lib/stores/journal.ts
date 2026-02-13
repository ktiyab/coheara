// M1-04: Journal store — offline-first symptom journal
import { writable, derived, get } from 'svelte/store';
import type {
	JournalEntry,
	JournalEntryDraft,
	JournalDateGroup,
	JournalCorrelation,
	SymptomChip,
	BodyRegion,
	OldcartsData,
	SaveResult
} from '$lib/types/journal.js';
import { SYMPTOM_CHIP_CATEGORIES } from '$lib/types/journal.js';

// --- Core stores ---

/** All journal entries (local) */
export const journalEntries = writable<JournalEntry[]>([]);

/** Pending correlations from last sync */
export const pendingCorrelations = writable<JournalCorrelation[]>([]);

// --- Derived stores ---

/** Unsynced entry count (badge display) */
export const unsyncedCount = derived(journalEntries, ($entries) =>
	$entries.filter((e) => !e.synced).length
);

/** Entries grouped by date (history view) */
export const entriesByDate = derived(journalEntries, ($entries) => {
	const sorted = [...$entries].sort(
		(a, b) => new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()
	);

	const groups = new Map<string, JournalEntry[]>();
	const today = new Date();
	const todayKey = formatDateKey(today);
	const yesterday = new Date(today);
	yesterday.setDate(yesterday.getDate() - 1);
	const yesterdayKey = formatDateKey(yesterday);

	for (const entry of sorted) {
		const key = formatDateKey(new Date(entry.createdAt));
		const existing = groups.get(key);
		if (existing) {
			existing.push(entry);
		} else {
			groups.set(key, [entry]);
		}
	}

	const result: JournalDateGroup[] = [];
	for (const [key, entries] of groups) {
		let label: string;
		if (key === todayKey) label = 'Today';
		else if (key === yesterdayKey) label = 'Yesterday';
		else label = formatDisplayDate(key);

		result.push({ label, date: key, entries });
	}

	return result;
});

/** Whether any entries exist */
export const hasEntries = derived(journalEntries, ($entries) => $entries.length > 0);

// --- Entry management ---

let entryCounter = 0;

/** Create a new journal entry from draft */
export function createEntry(draft: JournalEntryDraft): JournalEntry {
	const entry: JournalEntry = {
		id: `journal-${Date.now()}-${++entryCounter}`,
		severity: draft.severity,
		bodyLocations: draft.bodyLocations,
		freeText: draft.freeText,
		activityContext: draft.activityContext,
		symptomChip: draft.symptomChip,
		oldcarts: draft.oldcarts,
		createdAt: new Date().toISOString(),
		synced: false,
		syncedAt: null
	};

	journalEntries.update(($e) => [...$e, entry]);
	return entry;
}

/** Save entry (create + return save status) */
export function saveEntry(draft: JournalEntryDraft, connected: boolean): SaveResult {
	createEntry(draft);

	if (!connected) return 'saved_offline';
	// In real implementation, would attempt immediate sync
	return 'saved_offline'; // Default to offline until sync engine runs
}

/** Update an unsynced entry */
export function updateEntry(id: string, updates: Partial<JournalEntryDraft>): boolean {
	const entries = get(journalEntries);
	const entry = entries.find((e) => e.id === id);

	if (!entry || entry.synced) return false;

	journalEntries.update(($e) =>
		$e.map((e) => e.id === id ? { ...e, ...updates } : e)
	);
	return true;
}

/** Delete an entry */
export function deleteEntry(id: string): boolean {
	const entries = get(journalEntries);
	const entry = entries.find((e) => e.id === id);
	if (!entry) return false;

	if (entry.synced) {
		// Hide locally only (desktop retains)
		journalEntries.update(($e) => $e.filter((e) => e.id !== id));
	} else {
		// Permanent local delete
		journalEntries.update(($e) => $e.filter((e) => e.id !== id));
	}

	return true;
}

/** Check if entry is editable (before sync only) */
export function isEditable(id: string): boolean {
	const entries = get(journalEntries);
	const entry = entries.find((e) => e.id === id);
	return entry ? !entry.synced : false;
}

/** Get unsynced entries for sync */
export function getUnsyncedEntries(): JournalEntry[] {
	return get(journalEntries)
		.filter((e) => !e.synced)
		.sort((a, b) => new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime());
}

/** Mark entries as synced (after successful sync) */
export function markSynced(ids: string[]): void {
	const now = new Date().toISOString();
	const idSet = new Set(ids);
	journalEntries.update(($e) =>
		$e.map((e) => idSet.has(e.id) ? { ...e, synced: true, syncedAt: now } : e)
	);
}

/** Set pending correlations (shown as toast) */
export function setCorrelations(correlations: JournalCorrelation[]): void {
	pendingCorrelations.set(correlations);
}

/** Dismiss a correlation */
export function dismissCorrelation(entryId: string): void {
	pendingCorrelations.update(($c) => $c.filter((c) => c.entryId !== entryId));
}

// --- Draft management ---

/** Create empty draft */
export function emptyDraft(): JournalEntryDraft {
	return {
		severity: 5,
		bodyLocations: [],
		freeText: '',
		activityContext: '',
		symptomChip: null,
		oldcarts: null
	};
}

/** Check if draft is valid (minimum: severity set) */
export function isDraftValid(draft: JournalEntryDraft): boolean {
	return draft.severity >= 1 && draft.severity <= 10;
}

/** Get desktop category for a symptom chip */
export function getDesktopCategory(chip: SymptomChip): string {
	return SYMPTOM_CHIP_CATEGORIES[chip];
}

// --- Reset ---

/** Reset all journal state (for testing) */
export function resetJournalState(): void {
	journalEntries.set([]);
	pendingCorrelations.set([]);
}

// --- Helpers ---

function formatDateKey(date: Date): string {
	return date.toISOString().split('T')[0];
}

function formatDisplayDate(dateKey: string): string {
	const date = new Date(dateKey + 'T00:00:00');
	return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
}
