// M1-01: Tab navigation store — bottom tab management
import { writable, derived } from 'svelte/store';
import type { TabId, TabConfig } from '$lib/types/index.js';

/** Static tab configuration — mirrors desktop nav: Home, Ask, Documents, Timeline, Settings */
export const TAB_CONFIGS: readonly TabConfig[] = [
	{
		id: 'home',
		label: 'Home',
		icon: '\uD83C\uDFE0',
		ariaLabel: 'Home screen with medications and alerts',
		offlineAvailable: true
	},
	{
		id: 'ask',
		label: 'Ask',
		icon: '\uD83D\uDCAC',
		ariaLabel: 'Ask Coheara AI about your health',
		offlineAvailable: false
	},
	{
		id: 'documents',
		label: 'Docs',
		icon: '\uD83D\uDCC4',
		ariaLabel: 'Documents and uploads',
		offlineAvailable: true
	},
	{
		id: 'timeline',
		label: 'Timeline',
		icon: '\uD83D\uDCC5',
		ariaLabel: 'Health timeline with events and trends',
		offlineAvailable: true
	},
	{
		id: 'settings',
		label: 'Settings',
		icon: '\u2699\uFE0F',
		ariaLabel: 'Connection and preferences',
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

/** Route path for each tab — mirrors desktop nav structure */
const TAB_ROUTES: Record<TabId, string> = {
	home: '/',
	ask: '/ask',
	documents: '/documents',
	timeline: '/timeline',
	settings: '/settings'
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
