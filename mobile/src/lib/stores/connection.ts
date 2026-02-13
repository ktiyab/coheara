// M1-01: Connection state store — tracks phone↔desktop connectivity
import { writable, derived } from 'svelte/store';
import type { ConnectionState } from '$lib/types/index.js';

/** Core connection state store */
export const connection = writable<ConnectionState>({ status: 'unpaired' });

/** Whether any data is available (connected OR cached offline) */
export const hasData = derived(connection, ($c) =>
	$c.status === 'connected' || $c.status === 'offline'
);

/** Whether the phone can make live API requests */
export const isConnected = derived(connection, ($c) =>
	$c.status === 'connected'
);

/** Whether the user needs to take action (locked, desktop locked, error) */
export const needsAction = derived(connection, ($c) =>
	$c.status === 'locked' ||
	$c.status === 'desktop_locked' ||
	$c.status === 'error' ||
	$c.status === 'unpaired'
);

/** Human-readable status text for display in status bar */
export const statusText = derived(connection, ($c): string => {
	switch ($c.status) {
		case 'connected':
			return `Connected \u00b7 ${$c.profileName}`;
		case 'offline':
			return `Offline \u00b7 ${$c.profileName} \u00b7 Updated ${$c.cachedAt}`;
		case 'desktop_locked':
			return 'Desktop needs to be unlocked';
		case 'locked':
			return 'Unlock to continue';
		case 'unpaired':
			return 'Not connected';
		case 'connecting':
			return 'Connecting\u2026';
		case 'error':
			return $c.message;
	}
});

/** Status indicator color token */
export const statusColor = derived(connection, ($c): string => {
	switch ($c.status) {
		case 'connected':
			return 'var(--status-connected)';
		case 'offline':
		case 'unpaired':
			return 'var(--status-offline)';
		case 'locked':
		case 'desktop_locked':
		case 'connecting':
			return 'var(--status-locked)';
		case 'error':
			return 'var(--status-error)';
	}
});

/** Status indicator label for accessibility (not just color) */
export const statusLabel = derived(connection, ($c): string => {
	switch ($c.status) {
		case 'connected':
			return 'Connected';
		case 'offline':
			return 'Offline';
		case 'desktop_locked':
			return 'Desktop locked';
		case 'locked':
			return 'Locked';
		case 'unpaired':
			return 'Not paired';
		case 'connecting':
			return 'Connecting';
		case 'error':
			return 'Error';
	}
});

// --- State transition helpers ---

export function setConnected(profileName: string, lastSync: string): void {
	connection.set({ status: 'connected', profileName, lastSync });
}

export function setOffline(profileName: string, lastSync: string, cachedAt: string): void {
	connection.set({ status: 'offline', profileName, lastSync, cachedAt });
}

export function setConnecting(): void {
	connection.set({ status: 'connecting' });
}

export function setLocked(): void {
	connection.set({ status: 'locked' });
}

export function setDesktopLocked(): void {
	connection.set({ status: 'desktop_locked' });
}

export function setUnpaired(): void {
	connection.set({ status: 'unpaired' });
}

export function setError(message: string): void {
	connection.set({ status: 'error', message });
}
