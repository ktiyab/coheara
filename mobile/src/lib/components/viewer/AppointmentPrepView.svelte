<!-- M1-03: Appointment prep — two-view display (patient + doctor) -->
<script lang="ts">
	import type { AppointmentPrepData } from '$lib/types/viewer.js';

	const { prep, onShare }: {
		prep: AppointmentPrepData | null;
		onShare: (view: 'patient' | 'doctor') => void;
	} = $props();

	let activeView = $state<'patient' | 'doctor'>('patient');
</script>

{#if prep}
	<div class="prep-view">
		<div class="prep-header">
			<h2 class="prep-title">Appointment Prep</h2>
			<p class="prep-subtitle">{prep.doctorName} &middot; {prep.appointmentDate}</p>
		</div>

		<!-- Tab toggle -->
		<div class="view-toggle" role="tablist">
			<button
				class="toggle-btn"
				class:active={activeView === 'patient'}
				role="tab"
				aria-selected={activeView === 'patient'}
				onclick={() => activeView = 'patient'}
			>
				For me
			</button>
			<button
				class="toggle-btn"
				class:active={activeView === 'doctor'}
				role="tab"
				aria-selected={activeView === 'doctor'}
				onclick={() => activeView = 'doctor'}
			>
				For my doctor
			</button>
		</div>

		<!-- Patient view -->
		{#if activeView === 'patient'}
			<div class="prep-content" role="tabpanel" aria-label="Patient view">
				{#if prep.forPatient.thingsToMention.length > 0}
					<h3>Things to mention:</h3>
					<ul>
						{#each prep.forPatient.thingsToMention as item}
							<li>{item}</li>
						{/each}
					</ul>
				{/if}

				{#if prep.forPatient.questionsToConsider.length > 0}
					<h3>Questions to consider:</h3>
					<ul>
						{#each prep.forPatient.questionsToConsider as question}
							<li>{question}</li>
						{/each}
					</ul>
				{/if}
			</div>
		{/if}

		<!-- Doctor view -->
		{#if activeView === 'doctor'}
			<div class="prep-content doctor" role="tabpanel" aria-label="Doctor view">
				{#if prep.forDoctor.lastVisitDate}
					<p class="last-visit">Changes since last visit ({prep.forDoctor.lastVisitDate})</p>
				{/if}

				{#if prep.forDoctor.medicationChanges.length > 0}
					<h3>Medications:</h3>
					<ul>
						{#each prep.forDoctor.medicationChanges as change}
							<li>{change}</li>
						{/each}
					</ul>
				{/if}

				{#if prep.forDoctor.labResults.length > 0}
					<h3>Lab Results:</h3>
					<ul>
						{#each prep.forDoctor.labResults as result}
							<li>{result}</li>
						{/each}
					</ul>
				{/if}

				{#if prep.forDoctor.patientReportedSymptoms.length > 0}
					<h3>Patient-Reported Symptoms:</h3>
					<ul>
						{#each prep.forDoctor.patientReportedSymptoms as symptom}
							<li>{symptom}</li>
						{/each}
					</ul>
				{/if}

				{#if prep.forDoctor.activeAlerts.length > 0}
					<h3>Active Alerts:</h3>
					<ul>
						{#each prep.forDoctor.activeAlerts as alert}
							<li>{alert}</li>
						{/each}
					</ul>
				{/if}
			</div>
		{/if}

		<button class="share-btn" onclick={() => onShare(activeView)}>
			Share {activeView === 'patient' ? 'For Me' : 'For Doctor'} View
		</button>
	</div>
{:else}
	<div class="no-prep">
		<p>Connect to your desktop to generate appointment preparation.</p>
	</div>
{/if}

<style>
	.prep-view {
		padding: 16px;
	}

	.prep-header {
		margin-bottom: 16px;
	}

	.prep-title {
		font-size: 20px;
		font-weight: 700;
		margin: 0;
	}

	.prep-subtitle {
		font-size: 14px;
		color: var(--color-text-muted);
		margin: 4px 0 0;
	}

	.view-toggle {
		display: flex;
		gap: 0;
		border: 1px solid #D6D3D1;
		border-radius: 12px;
		overflow: hidden;
		margin-bottom: 16px;
	}

	.toggle-btn {
		flex: 1;
		padding: 10px;
		border: none;
		background: white;
		font-size: 15px;
		font-weight: 500;
		cursor: pointer;
		font-family: inherit;
		min-height: var(--min-touch-target);
	}

	.toggle-btn.active {
		background: var(--color-primary);
		color: white;
	}

	.prep-content {
		background: #F5F5F4;
		border-radius: 12px;
		padding: 16px;
		margin-bottom: 16px;
	}

	.prep-content h3 {
		font-size: 15px;
		font-weight: 600;
		margin: 0 0 8px;
	}

	.prep-content h3:not(:first-child) {
		margin-top: 16px;
	}

	.prep-content ul {
		margin: 0;
		padding-left: 20px;
	}

	.prep-content li {
		font-size: 15px;
		line-height: 1.5;
		margin-bottom: 6px;
	}

	.prep-content.doctor {
		background: #EFF6FF;
	}

	.last-visit {
		font-size: 14px;
		font-weight: 600;
		color: var(--color-text-muted);
		text-transform: uppercase;
		letter-spacing: 0.3px;
		margin: 0 0 12px;
	}

	.share-btn {
		width: 100%;
		padding: 14px;
		background: var(--color-primary);
		color: white;
		border: none;
		border-radius: 12px;
		font-size: 16px;
		font-weight: 600;
		cursor: pointer;
		min-height: var(--min-touch-target);
	}

	.no-prep {
		text-align: center;
		padding: 40px 20px;
		color: var(--color-text-muted);
		font-size: 16px;
	}
</style>
