// M1-01: Mobile-specific type definitions

/** Connection state — 7 variants covering all phone↔desktop states */
export type ConnectionState =
	| { status: 'unpaired' }
	| { status: 'connecting' }
	| { status: 'connected'; profileName: string; lastSync: string }
	| { status: 'offline'; profileName: string; lastSync: string; cachedAt: string }
	| { status: 'locked' }
	| { status: 'desktop_locked' }
	| { status: 'error'; message: string };

/** All possible connection status values */
export type ConnectionStatus = ConnectionState['status'];

/** Authentication state for biometric/session gating */
export type AuthState =
	| { state: 'unauthenticated' }
	| { state: 'authenticating' }
	| { state: 'authenticated'; cacheKey: string }
	| { state: 'locked' }
	| { state: 'locked_out'; attemptsRemaining: 0; cooldownUntil: number };

/** App lifecycle states */
export type AppLifecycleState =
	| 'cold_start'
	| 'foreground'
	| 'background'
	| 'killed';

/** Bottom tab identifiers — mirrors desktop nav (Home, Ask, Documents, Timeline, Settings) */
export type TabId = 'home' | 'ask' | 'documents' | 'timeline' | 'settings';

/** Tab configuration for navigation */
export interface TabConfig {
	id: TabId;
	label: string;
	icon: string;
	ariaLabel: string;
	offlineAvailable: boolean;
}

/** Emergency card configuration */
export interface EmergencyCardConfig {
	enabled: boolean;
	showName: boolean;
	showBloodType: boolean;
	showAllergies: boolean;
	showEmergencyMeds: boolean;
	selectedMedicationIds: string[];
}

/** Emergency card display data */
export interface EmergencyCardData {
	name: string;
	bloodType?: string;
	allergies: string[];
	emergencyMedications: string[];
}

/** Biometric capability info */
export interface BiometricCapability {
	available: boolean;
	type: 'face' | 'fingerprint' | 'iris' | 'none';
}

/** Device integrity check result */
export interface DeviceIntegrityResult {
	compromised: boolean;
	reason?: string;
}

/** Session timeout configuration */
export interface SessionConfig {
	timeoutMs: number;
	biometricEnabled: boolean;
}

/** Sensitive screen identifiers (screenshot prevention) */
export type SensitiveScreen =
	| 'medications'
	| 'labs'
	| 'alerts'
	| 'ask'
	| 'journal'
	| 'appointment_prep';

/** Accessibility configuration derived from system settings */
export interface AccessibilityConfig {
	fontScale: number;
	reduceMotion: boolean;
	highContrast: boolean;
	simplifiedLayout: boolean;
}
