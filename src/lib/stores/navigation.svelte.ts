/**
 * E2E-F06 + E2E-F03: Global navigation store with URL hash state.
 *
 * Replaces prop-drilled `onNavigate` callbacks with a singleton
 * reactive store. Syncs activeScreen with `window.location.hash`
 * for URL state, deep linking, and browser back/forward support.
 */

import { browser } from '$app/environment';

/** Screens that display the tab bar. */
const TAB_SCREENS = new Set([
	'home',
	'chat',
	'journal',
	'medications',
	'documents',
	'timeline',
	'appointments',
	'settings',
	'ai-settings'
]);

/** Main tab IDs (non-"more" screens). */
const MAIN_TABS = new Set(['home', 'chat', 'journal', 'medications']);

class NavigationStore {
	activeScreen = $state('home');
	previousScreen = $state('home');
	screenParams = $state<Record<string, string>>({});

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

	get activeTab(): string {
		if (MAIN_TABS.has(this.activeScreen)) {
			return this.activeScreen;
		}
		if (TAB_SCREENS.has(this.activeScreen)) {
			return 'more';
		}
		return '';
	}

	get showTabBar(): boolean {
		return TAB_SCREENS.has(this.activeScreen);
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
		const screen = qIdx >= 0 ? hash.slice(0, qIdx) : hash;
		this.previousScreen = this.activeScreen;
		this.activeScreen = screen || 'home';
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
