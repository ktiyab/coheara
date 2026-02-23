// M1-01: Navigation store tests — mirrors desktop nav (Home, Ask, Documents, Timeline, Settings)
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
		const tabIds = ['home', 'ask', 'documents', 'timeline', 'settings'] as const;

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

		// Ask is online-only, others are offline-available
		const askTab = TAB_CONFIGS.find((t) => t.id === 'ask');
		expect(askTab?.offlineAvailable).toBe(false);

		const homeTab = TAB_CONFIGS.find((t) => t.id === 'home');
		expect(homeTab?.offlineAvailable).toBe(true);

		const docsTab = TAB_CONFIGS.find((t) => t.id === 'documents');
		expect(docsTab?.offlineAvailable).toBe(true);

		const timelineTab = TAB_CONFIGS.find((t) => t.id === 'timeline');
		expect(timelineTab?.offlineAvailable).toBe(true);

		const settingsTab = TAB_CONFIGS.find((t) => t.id === 'settings');
		expect(settingsTab?.offlineAvailable).toBe(true);
	});

	it('supports back navigation through history', () => {
		navigateToTab('ask');
		navigateToTab('documents');
		navigateToTab('timeline');
		expect(get(activeTab)).toBe('timeline');

		const didNav1 = navigateBack();
		expect(didNav1).toBe(true);
		expect(get(activeTab)).toBe('documents');

		const didNav2 = navigateBack();
		expect(didNav2).toBe(true);
		expect(get(activeTab)).toBe('ask');

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
		expect(resolveDeepLink('/ask')).toBe('ask');
		expect(resolveDeepLink('/ask/history')).toBe('ask');
		expect(resolveDeepLink('/documents')).toBe('documents');
		expect(resolveDeepLink('/documents/upload')).toBe('documents');
		expect(resolveDeepLink('/timeline')).toBe('timeline');
		expect(resolveDeepLink('/timeline/event-123')).toBe('timeline');
		expect(resolveDeepLink('/settings')).toBe('settings');
		expect(resolveDeepLink('/unknown')).toBeNull();

		// Route mapping
		expect(getTabRoute('home')).toBe('/');
		expect(getTabRoute('ask')).toBe('/ask');
		expect(getTabRoute('documents')).toBe('/documents');
		expect(getTabRoute('timeline')).toBe('/timeline');
		expect(getTabRoute('settings')).toBe('/settings');
	});
});
