/**
 * E2E-F06 + E2E-F03 + D6: Global navigation store with URL hash state.
 *
 * Replaces prop-drilled `onNavigate` callbacks with a singleton
 * reactive store. Syncs activeScreen with `window.location.hash`
 * for URL state, deep linking, and browser back/forward support.
 *
 * D6 migration: TabBar → Sidebar. Navigation sections for grouping.
 */

import { browser } from '$app/environment';

/** Screens that display the sidebar (all main screens). */
const SIDEBAR_SCREENS = new Set([
	'home',
	'chat',
	'documents',
	'timeline',
	'settings',
	'ai-settings'
]);

/** Navigation sections for sidebar grouping. */
export const NAV_SECTIONS = {
	main: ['home', 'chat'],
	library: ['documents', 'timeline'],
	system: ['settings'],
} as const;

/** Screens removed in LP-06 that redirect to home. */
const REDIRECT_MAP: Record<string, string> = {
	journal: 'home',
	medications: 'home',
	appointments: 'home',
};

class NavigationStore {
	activeScreen = $state('home');
	previousScreen = $state('home');
	screenParams = $state<Record<string, string>>({});
	sidebarCollapsed = $state(false);

	constructor() {
		if (browser) {
			const hash = window.location.hash.slice(1);
			if (hash) {
				this.readFromHash(hash);
			}
			window.addEventListener('popstate', () => {
				const h = window.location.hash.slice(1);
				this.readFromHash(h || 'home');
			});
		}
	}

	get activeSection(): string {
		for (const [section, screens] of Object.entries(NAV_SECTIONS)) {
			if ((screens as readonly string[]).includes(this.activeScreen)) return section;
		}
		return '';
	}

	get showSidebar(): boolean {
		return SIDEBAR_SCREENS.has(this.activeScreen);
	}

	toggleSidebar() {
		this.sidebarCollapsed = !this.sidebarCollapsed;
	}

	navigate(screen: string, params?: Record<string, string>) {
		this.previousScreen = this.activeScreen;
		this.screenParams = params ?? {};
		this.activeScreen = screen;
		if (browser) {
			this.writeToHash();
		}
	}

	goBack() {
		if (browser) {
			history.back();
		} else {
			this.navigate(this.previousScreen);
		}
	}

	private readFromHash(hash: string) {
		const qIdx = hash.indexOf('?');
		let screen = qIdx >= 0 ? hash.slice(0, qIdx) : hash;
		screen = REDIRECT_MAP[screen] ?? screen ?? 'home';
		this.previousScreen = this.activeScreen;
		this.activeScreen = screen;
		if (qIdx >= 0) {
			const params = new URLSearchParams(hash.slice(qIdx + 1));
			this.screenParams = Object.fromEntries(params);
		} else {
			this.screenParams = {};
		}
	}

	private writeToHash() {
		let hash = this.activeScreen;
		const entries = Object.entries(this.screenParams).filter(([, v]) => v);
		if (entries.length > 0) {
			hash += '?' + new URLSearchParams(entries).toString();
		}
		history.pushState(null, '', '#' + hash);
	}
}

export const navigation = new NavigationStore();
