<!-- M1-03: Appointment prep screen — two-view display -->
<script lang="ts">
	import { isConnected } from '$lib/stores/connection.js';
	import { nextAppointment, lastSyncTimestamp } from '$lib/stores/cache.js';
	import { shareAppointmentPrep, emptyStateMessage } from '$lib/utils/viewer.js';
	import type { AppointmentPrepData, SharePayload } from '$lib/types/viewer.js';
	import FreshnessIndicator from '$lib/components/viewer/FreshnessIndicator.svelte';
	import AppointmentPrepView from '$lib/components/viewer/AppointmentPrepView.svelte';
	import ShareSheet from '$lib/components/viewer/ShareSheet.svelte';

	import { fetchAppointmentPrep } from '$lib/api/viewer.js';

	let prepData = $state<AppointmentPrepData | null>(null);
	let sharePayload = $state<SharePayload | null>(null);
	let loadingPrep = $state(false);

	// Fetch prep data when connected and appointment is available
	$effect(() => {
		const appointment = $nextAppointment;
		const connected = $isConnected;
		if (connected && appointment?.hasPrepData && !prepData && !loadingPrep) {
			loadingPrep = true;
			fetchAppointmentPrep(appointment.id)
				.then((data) => { prepData = data; })
				.finally(() => { loadingPrep = false; });
		}
	});

	function handleShare(view: 'patient' | 'doctor'): void {
		if (!prepData) return;
		sharePayload = shareAppointmentPrep(prepData, view, $lastSyncTimestamp);
	}
</script>

<div class="appointments-screen">
	<div class="appointments-header">
		<FreshnessIndicator syncTimestamp={$lastSyncTimestamp} />
	</div>

	{#if !$nextAppointment}
		<div class="empty-state">
			<p>{emptyStateMessage('appointments')}</p>
		</div>
	{:else if loadingPrep}
		<div class="loading-state">
			<h2>Appointment with {$nextAppointment.doctorName}</h2>
			<p class="appointment-date">{$nextAppointment.date}</p>
			<p class="loading-note">Loading appointment preparation...</p>
		</div>
	{:else if !$isConnected && !prepData}
		<div class="offline-state">
			<h2>Appointment with {$nextAppointment.doctorName}</h2>
			<p class="appointment-date">{$nextAppointment.date}</p>
			<p class="offline-note">Connect to your desktop to generate appointment preparation.</p>
		</div>
	{:else}
		<AppointmentPrepView prep={prepData} onShare={handleShare} />
	{/if}
</div>

{#if sharePayload}
	<ShareSheet payload={sharePayload} onClose={() => sharePayload = null} />
{/if}

<style>
	.appointments-screen {
		padding: 16px;
	}

	.appointments-header {
		margin-bottom: 8px;
	}

	.empty-state, .offline-state, .loading-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		min-height: 200px;
		text-align: center;
		padding: 24px;
	}

	.empty-state p, .offline-note, .loading-note {
		color: var(--color-text-muted);
		font-size: 16px;
		line-height: 1.5;
	}

	.offline-state h2 {
		font-size: 20px;
		font-weight: 700;
		margin: 0 0 4px;
	}

	.appointment-date {
		font-size: 15px;
		color: var(--color-text-muted);
		margin: 0 0 16px;
	}
</style>
