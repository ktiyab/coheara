// M1-01: Tab navigation store — bottom tab management
import { writable, derived } from 'svelte/store';
import type { TabId, TabConfig } from '$lib/types/index.js';

/** Static tab configuration — order defines display order */
export const TAB_CONFIGS: readonly TabConfig[] = [
	{
		id: 'home',
		label: 'Home',
		icon: '\uD83C\uDFE0',
		ariaLabel: 'Home screen with medications and alerts',
		offlineAvailable: true
	},
	{
		id: 'chat',
		label: 'Chat',
		icon: '\uD83D\uDCAC',
		ariaLabel: 'Chat with Coheara AI',
		offlineAvailable: false
	},
	{
		id: 'meds',
		label: 'Meds',
		icon: '\uD83D\uDC8A',
		ariaLabel: 'Medication list by schedule',
		offlineAvailable: true
	},
	{
		id: 'journal',
		label: 'Log',
		icon: '\uD83D\uDCDD',
		ariaLabel: 'Symptom journal and history',
		offlineAvailable: true
	},
	{
		id: 'more',
		label: 'More',
		icon: '\u22EF',
		ariaLabel: 'More options: timeline, appointments, settings',
		offlineAvailable: true
	}
] as const;

/** Currently active tab */
export const activeTab = writable<TabId>('home');

/** Navigation history for back button support */
const history = writable<TabId[]>(['home']);

/** Current tab configuration */
export const activeTabConfig = derived(activeTab, ($tab) =>
	TAB_CONFIGS.find((t) => t.id === $tab)!
);

/** Whether the current tab works offline */
export const currentTabOffline = derived(activeTab, ($tab) =>
	TAB_CONFIGS.find((t) => t.id === $tab)?.offlineAvailable ?? false
);

/** Route path for each tab */
const TAB_ROUTES: Record<TabId, string> = {
	home: '/',
	chat: '/chat',
	meds: '/meds',
	journal: '/journal',
	more: '/more'
};

/** Navigate to a tab */
export function navigateToTab(tabId: TabId): void {
	activeTab.set(tabId);
	history.update((h) => {
		if (h[h.length - 1] !== tabId) {
			return [...h, tabId];
		}
		return h;
	});
}

/** Navigate back to previous tab; returns false if at root */
export function navigateBack(): boolean {
	let didNavigate = false;
	history.update((h) => {
		if (h.length > 1) {
			const newHistory = h.slice(0, -1);
			activeTab.set(newHistory[newHistory.length - 1]);
			didNavigate = true;
			return newHistory;
		}
		return h;
	});
	return didNavigate;
}

/** Get the route path for a tab */
export function getTabRoute(tabId: TabId): string {
	return TAB_ROUTES[tabId];
}

/** Resolve a deep link URL to a tab */
export function resolveDeepLink(path: string): TabId | null {
	for (const [tab, route] of Object.entries(TAB_ROUTES)) {
		if (path === route || path.startsWith(route + '/')) {
			return tab as TabId;
		}
	}
	return null;
}

/** Reset navigation state (for testing) */
export function resetNavigation(): void {
	activeTab.set('home');
	history.set(['home']);
}
