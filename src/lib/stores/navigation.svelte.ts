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
	'me',
	'chat',
	'history',
	'documents',
	'review',
	'document-detail',
	'timeline',
	'companion',
	'settings',
	'ai-settings',
	'privacy',
	'profiles',
	'profiles-create'
]);

/** Maps sub-screens to their parent nav item for sidebar highlighting. */
export const PARENT_SCREEN: Record<string, string> = {
	'privacy': 'settings',
	'ai-settings': 'settings',
	'profiles': 'settings',
	'profiles-create': 'settings',
};

/** Navigation sections for sidebar grouping. */
export const NAV_SECTIONS = {
	main: ['home', 'me', 'chat', 'history'],
	library: ['documents', 'timeline'],
	system: ['companion', 'settings'],
} as const;

/** Screens removed in LP-06 that redirect to home. */
const REDIRECT_MAP: Record<string, string> = {
	journal: 'home',
	medications: 'home',
	appointments: 'home',
	pairing: 'companion',
	import: 'documents',
};

class NavigationStore {
	activeScreen = $state('home');
	previousScreen = $state('home');
	screenParams = $state<Record<string, string>>({});
	sidebarCollapsed = $state(false);

	// CHAT-NAV-01: Active conversation persists across screen switches.
	// Set by ChatScreen on create/load, cleared by "New Session" and F7 reset.
	lastChatConversationId = $state<string | null>(null);

	/** CHAT-NAV-01: Track active chat conversation for navigation persistence. */
	setLastChat(id: string | null): void {
		this.lastChatConversationId = id;
	}

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

	/** F7: Reset to home on lock/switch — clears stale screenParams (e.g. documentIds)
	 *  and URL hash to prevent cross-profile data leakage via URL or screen state. */
	reset(): void {
		this.previousScreen = 'home';
		this.activeScreen = 'home';
		this.screenParams = {};
		this.lastChatConversationId = null;  // CHAT-NAV-01: F7 security
		if (browser) {
			history.replaceState(null, '', '#home');
		}
	}

	navigate(screen: string, params?: Record<string, string>, options?: { replace?: boolean }) {
		this.previousScreen = this.activeScreen;
		this.screenParams = params ?? {};
		this.activeScreen = screen;
		if (browser) {
			this.writeToHash(options?.replace);
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

	private writeToHash(replace = false) {
		let hash = this.activeScreen;
		const entries = Object.entries(this.screenParams).filter(([, v]) => v);
		if (entries.length > 0) {
			hash += '?' + new URLSearchParams(entries).toString();
		}
		const url = '#' + hash;
		if (replace) {
			history.replaceState(null, '', url);
		} else {
			history.pushState(null, '', url);
		}
	}
}

export const navigation = new NavigationStore();
