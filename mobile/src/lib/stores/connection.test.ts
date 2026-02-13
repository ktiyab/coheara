// M1-01: Connection state store tests — 6 tests
import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
	connection,
	hasData,
	isConnected,
	needsAction,
	statusText,
	statusLabel,
	setConnected,
	setOffline,
	setConnecting,
	setLocked,
	setDesktopLocked,
	setUnpaired,
	setError
} from './connection.js';

describe('connection store', () => {
	beforeEach(() => {
		setUnpaired();
	});

	it('initial state is unpaired', () => {
		expect(get(connection)).toEqual({ status: 'unpaired' });
		expect(get(hasData)).toBe(false);
		expect(get(isConnected)).toBe(false);
		expect(get(needsAction)).toBe(true);
	});

	it('transitions through all 7 connection states correctly', () => {
		// unpaired → connecting
		setConnecting();
		expect(get(connection).status).toBe('connecting');
		expect(get(hasData)).toBe(false);

		// connecting → connected
		setConnected('Mamadou', '2026-02-12T10:00:00Z');
		expect(get(connection).status).toBe('connected');
		expect(get(hasData)).toBe(true);
		expect(get(isConnected)).toBe(true);
		expect(get(needsAction)).toBe(false);

		// connected → offline
		setOffline('Mamadou', '2026-02-12T10:00:00Z', '2h ago');
		expect(get(connection).status).toBe('offline');
		expect(get(hasData)).toBe(true);
		expect(get(isConnected)).toBe(false);

		// offline → locked
		setLocked();
		expect(get(connection).status).toBe('locked');
		expect(get(hasData)).toBe(false);
		expect(get(needsAction)).toBe(true);

		// locked → desktop_locked
		setDesktopLocked();
		expect(get(connection).status).toBe('desktop_locked');
		expect(get(needsAction)).toBe(true);

		// desktop_locked → error
		setError('Connection refused');
		expect(get(connection).status).toBe('error');
		expect(get(needsAction)).toBe(true);

		// error → unpaired
		setUnpaired();
		expect(get(connection).status).toBe('unpaired');
	});

	it('derives correct status text for each state', () => {
		setConnected('Mamadou', '2026-02-12T10:00:00Z');
		expect(get(statusText)).toBe('Connected \u00b7 Mamadou');

		setOffline('Mamadou', '2026-02-12T10:00:00Z', '2h ago');
		expect(get(statusText)).toBe('Offline \u00b7 Mamadou \u00b7 Updated 2h ago');

		setDesktopLocked();
		expect(get(statusText)).toBe('Desktop needs to be unlocked');

		setLocked();
		expect(get(statusText)).toBe('Unlock to continue');

		setUnpaired();
		expect(get(statusText)).toBe('Not connected');

		setConnecting();
		expect(get(statusText)).toBe('Connecting\u2026');

		setError('WiFi lost');
		expect(get(statusText)).toBe('WiFi lost');
	});

	it('derives accessible labels that are not color-only', () => {
		setConnected('Mamadou', '2026-02-12T10:00:00Z');
		expect(get(statusLabel)).toBe('Connected');

		setOffline('Mamadou', '2026-02-12T10:00:00Z', '2h ago');
		expect(get(statusLabel)).toBe('Offline');

		setDesktopLocked();
		expect(get(statusLabel)).toBe('Desktop locked');

		setLocked();
		expect(get(statusLabel)).toBe('Locked');

		setError('Timeout');
		expect(get(statusLabel)).toBe('Error');

		setUnpaired();
		expect(get(statusLabel)).toBe('Not paired');

		setConnecting();
		expect(get(statusLabel)).toBe('Connecting');
	});

	it('connected state carries profile name and sync time', () => {
		setConnected('L\u00e9a', '2026-02-12T15:30:00Z');
		const state = get(connection);
		expect(state.status).toBe('connected');
		if (state.status === 'connected') {
			expect(state.profileName).toBe('L\u00e9a');
			expect(state.lastSync).toBe('2026-02-12T15:30:00Z');
		}
	});

	it('offline state carries cached-at timestamp', () => {
		setOffline('Thomas', '2026-02-12T08:00:00Z', '30 minutes ago');
		const state = get(connection);
		expect(state.status).toBe('offline');
		if (state.status === 'offline') {
			expect(state.profileName).toBe('Thomas');
			expect(state.cachedAt).toBe('30 minutes ago');
		}
	});
});
