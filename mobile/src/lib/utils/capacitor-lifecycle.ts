// IMP-010: CapacitorLifecycleListener — wraps @capacitor/app + @capacitor/network
import { App } from '@capacitor/app';
import { Network } from '@capacitor/network';
import type { LifecycleListener } from './lifecycle.js';

/** Capacitor implementation using @capacitor/app and @capacitor/network */
export class CapacitorLifecycleListener implements LifecycleListener {
	onForeground(callback: () => void): void {
		App.addListener('appStateChange', (state) => {
			if (state.isActive) {
				callback();
			}
		});
	}

	onBackground(callback: () => void): void {
		App.addListener('appStateChange', (state) => {
			if (!state.isActive) {
				callback();
			}
		});
	}

	onNetworkChange(callback: (connected: boolean) => void): void {
		Network.addListener('networkStatusChange', (status) => {
			callback(status.connected);
		});
	}
}
