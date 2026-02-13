<!-- M1-01: Home screen — Mamadou optimized: meds due now first -->
<script lang="ts">
	import { connection, hasData } from '$lib/stores/connection.js';

	function getGreeting(): string {
		const hour = new Date().getHours();
		if (hour < 12) return 'Good morning';
		if (hour < 18) return 'Good afternoon';
		return 'Good evening';
	}

	function getProfileName(): string {
		const $conn = $connection;
		if ($conn.status === 'connected') return $conn.profileName;
		if ($conn.status === 'offline') return $conn.profileName;
		return '';
	}
</script>

<div class="home-screen">
	{#if $hasData}
		<section class="greeting">
			<h1>{getGreeting()}, {getProfileName()}</h1>
		</section>

		<!-- PRIORITY 1: Medications due now (Dr. Diallo) -->
		<section class="meds-due" aria-label="Medications due now">
			<h2>Morning Medications</h2>
			<p class="placeholder">Medication list loads from M1-03 Viewer Screens</p>
		</section>

		<!-- PRIORITY 2: Alerts -->
		<section class="alerts" aria-label="Health alerts">
			<p class="placeholder">Alert display loads from M1-03 Viewer Screens</p>
		</section>

		<!-- PRIORITY 3: Next appointment -->
		<section class="appointment" aria-label="Next appointment">
			<p class="placeholder">Appointment display loads from M1-03 Viewer Screens</p>
		</section>

		<!-- Recent journal entries -->
		<section class="journal-recent" aria-label="Recent journal entries">
			<p class="placeholder">Journal entries load from M1-04 Journal</p>
		</section>
	{:else}
		<div class="no-data">
			<p>No data available yet. Connect to your desktop to sync your health information.</p>
		</div>
	{/if}
</div>

<style>
	.home-screen {
		padding: 16px;
	}

	.greeting h1 {
		font-size: var(--font-header);
		font-weight: 700;
		margin: 0 0 20px;
	}

	section {
		margin-bottom: 20px;
	}

	h2 {
		font-size: 20px;
		font-weight: 600;
		margin: 0 0 12px;
	}

	.placeholder {
		padding: 24px 16px;
		background: #F5F5F4;
		border-radius: 12px;
		color: var(--color-text-muted);
		font-size: 15px;
		text-align: center;
	}

	.no-data {
		display: flex;
		align-items: center;
		justify-content: center;
		min-height: 200px;
		text-align: center;
		color: var(--color-text-muted);
		padding: 24px;
	}
</style>
