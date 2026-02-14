// E2E-B04: Coherence Engine API

import { invoke } from '@tauri-apps/api/core';
import type {
	CoherenceResult,
	CoherenceAlert,
	AlertType,
	EmergencyAction
} from '$lib/types/coherence';

/** Run full coherence analysis on all patient data. */
export async function runCoherenceScan(): Promise<CoherenceResult> {
	return invoke('run_coherence_scan');
}

/** Run coherence analysis scoped to a specific document. */
export async function runCoherenceScanDocument(documentId: string): Promise<CoherenceResult> {
	return invoke('run_coherence_scan_document', { documentId });
}

/** Get active coherence alerts, optionally filtered by type. */
export async function getCoherenceAlerts(alertType?: AlertType): Promise<CoherenceAlert[]> {
	return invoke('get_coherence_alerts', { alertType: alertType ?? null });
}

/** Dismiss a standard coherence alert. */
export async function dismissCoherenceAlert(alertId: string, reason: string): Promise<void> {
	return invoke('dismiss_coherence_alert', { alertId, reason });
}

/** Dismiss a CRITICAL coherence alert (requires 2-step confirmation). */
export async function dismissCriticalCoherenceAlert(
	alertId: string,
	reason: string,
	twoStepConfirmed: boolean
): Promise<void> {
	return invoke('dismiss_critical_coherence_alert', { alertId, reason, twoStepConfirmed });
}

/** Get emergency actions for currently active critical alerts. */
export async function getCoherenceEmergencyActions(): Promise<EmergencyAction[]> {
	return invoke('get_coherence_emergency_actions');
}
