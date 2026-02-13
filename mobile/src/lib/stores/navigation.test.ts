// M1-01: Navigation store tests — 5 tests
import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
	activeTab,
	activeTabConfig,
	currentTabOffline,
	TAB_CONFIGS,
	navigateToTab,
	navigateBack,
	getTabRoute,
	resolveDeepLink,
	resetNavigation
} from './navigation.js';

describe('navigation store', () => {
	beforeEach(() => {
		resetNavigation();
	});

	it('starts on the home tab', () => {
		expect(get(activeTab)).toBe('home');
		expect(get(activeTabConfig).id).toBe('home');
		expect(get(currentTabOffline)).toBe(true);
	});

	it('switches between all 5 tabs', () => {
		const tabIds = ['home', 'chat', 'meds', 'journal', 'more'] as const;

		for (const tabId of tabIds) {
			navigateToTab(tabId);
			expect(get(activeTab)).toBe(tabId);
			expect(get(activeTabConfig).id).toBe(tabId);
		}
	});

	it('provides correct tab configurations', () => {
		expect(TAB_CONFIGS).toHaveLength(5);

		// All tabs have required fields
		for (const tab of TAB_CONFIGS) {
			expect(tab.id).toBeTruthy();
			expect(tab.label).toBeTruthy();
			expect(tab.icon).toBeTruthy();
			expect(tab.ariaLabel).toBeTruthy();
			expect(typeof tab.offlineAvailable).toBe('boolean');
		}

		// Chat is online-only, others are offline-available
		const chatTab = TAB_CONFIGS.find((t) => t.id === 'chat');
		expect(chatTab?.offlineAvailable).toBe(false);

		const homeTab = TAB_CONFIGS.find((t) => t.id === 'home');
		expect(homeTab?.offlineAvailable).toBe(true);

		const medsTab = TAB_CONFIGS.find((t) => t.id === 'meds');
		expect(medsTab?.offlineAvailable).toBe(true);
	});

	it('supports back navigation through history', () => {
		navigateToTab('chat');
		navigateToTab('meds');
		navigateToTab('journal');
		expect(get(activeTab)).toBe('journal');

		const didNav1 = navigateBack();
		expect(didNav1).toBe(true);
		expect(get(activeTab)).toBe('meds');

		const didNav2 = navigateBack();
		expect(didNav2).toBe(true);
		expect(get(activeTab)).toBe('chat');

		const didNav3 = navigateBack();
		expect(didNav3).toBe(true);
		expect(get(activeTab)).toBe('home');

		// Can't go further back
		const didNav4 = navigateBack();
		expect(didNav4).toBe(false);
		expect(get(activeTab)).toBe('home');
	});

	it('resolves deep links to correct tabs', () => {
		expect(resolveDeepLink('/')).toBe('home');
		expect(resolveDeepLink('/chat')).toBe('chat');
		expect(resolveDeepLink('/meds')).toBe('meds');
		expect(resolveDeepLink('/meds/med-123')).toBe('meds');
		expect(resolveDeepLink('/journal')).toBe('journal');
		expect(resolveDeepLink('/more')).toBe('more');
		expect(resolveDeepLink('/more/timeline')).toBe('more');
		expect(resolveDeepLink('/unknown')).toBeNull();

		// Route mapping
		expect(getTabRoute('home')).toBe('/');
		expect(getTabRoute('chat')).toBe('/chat');
		expect(getTabRoute('meds')).toBe('/meds');
		expect(getTabRoute('journal')).toBe('/journal');
		expect(getTabRoute('more')).toBe('/more');
	});
});
