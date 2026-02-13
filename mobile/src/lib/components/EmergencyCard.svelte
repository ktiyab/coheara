<!-- M1-01: Emergency card configuration — opt-in lock screen health widget -->
<script lang="ts">
	import type { EmergencyCardConfig } from '$lib/types/index.js';

	const {
		config,
		medications,
		onUpdate
	}: {
		config: EmergencyCardConfig;
		medications: Array<{ id: string; name: string }>;
		onUpdate: (config: EmergencyCardConfig) => void;
	} = $props();

	function toggle(field: keyof EmergencyCardConfig): void {
		if (field === 'selectedMedicationIds') return;
		const current = config[field];
		if (typeof current === 'boolean') {
			onUpdate({ ...config, [field]: !current });
		}
	}

	function toggleMedication(id: string): void {
		const ids = config.selectedMedicationIds.includes(id)
			? config.selectedMedicationIds.filter((m) => m !== id)
			: [...config.selectedMedicationIds, id];
		onUpdate({ ...config, selectedMedicationIds: ids });
	}
</script>

<div class="emergency-card-config">
	<h2>Emergency Health Card</h2>
	<p class="description">
		This information will be visible on your lock screen without unlocking your phone.
		Only enable what you're comfortable sharing publicly.
	</p>

	<label class="toggle-row">
		<input type="checkbox" checked={config.enabled} onchange={() => toggle('enabled')} />
		<span>Enable emergency card</span>
	</label>

	{#if config.enabled}
		<div class="field-options">
			<label class="toggle-row">
				<input type="checkbox" checked={config.showName} onchange={() => toggle('showName')} />
				<span>Show name</span>
			</label>

			<label class="toggle-row">
				<input type="checkbox" checked={config.showBloodType} onchange={() => toggle('showBloodType')} />
				<span>Show blood type</span>
			</label>

			<label class="toggle-row">
				<input type="checkbox" checked={config.showAllergies} onchange={() => toggle('showAllergies')} />
				<span>Show allergies</span>
			</label>

			<label class="toggle-row">
				<input type="checkbox" checked={config.showEmergencyMeds} onchange={() => toggle('showEmergencyMeds')} />
				<span>Show emergency medications</span>
			</label>

			{#if config.showEmergencyMeds && medications.length > 0}
				<div class="med-list">
					<p class="med-label">Select medications to display:</p>
					{#each medications as med (med.id)}
						<label class="toggle-row med-item">
							<input
								type="checkbox"
								checked={config.selectedMedicationIds.includes(med.id)}
								onchange={() => toggleMedication(med.id)}
							/>
							<span>{med.name}</span>
						</label>
					{/each}
				</div>
			{/if}
		</div>

		<div class="privacy-warning" role="alert">
			<strong>Privacy notice:</strong> Emergency card data is stored in plain text
			and accessible from the lock screen. Only include information you're comfortable
			being seen by anyone who handles your phone.
		</div>
	{/if}
</div>

<style>
	.emergency-card-config {
		padding: 16px;
	}

	h2 {
		font-size: var(--font-header);
		font-weight: 700;
		margin: 0 0 8px;
	}

	.description {
		font-size: 15px;
		color: var(--color-text-muted);
		line-height: 1.5;
		margin: 0 0 20px;
	}

	.toggle-row {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 12px 0;
		min-height: var(--min-touch-target);
		cursor: pointer;
		font-size: 16px;
	}

	.toggle-row input[type="checkbox"] {
		width: 22px;
		height: 22px;
		accent-color: var(--color-primary);
	}

	.field-options {
		margin-top: 12px;
		padding-left: 8px;
		border-left: 2px solid #E7E5E4;
	}

	.med-list {
		margin-top: 8px;
		padding-left: 16px;
	}

	.med-label {
		font-size: 14px;
		color: var(--color-text-muted);
		margin: 8px 0;
	}

	.med-item {
		padding: 8px 0;
	}

	.privacy-warning {
		margin-top: 20px;
		padding: 12px 16px;
		background: #FEF3C7;
		border-radius: 8px;
		font-size: 14px;
		line-height: 1.5;
		color: #92400E;
	}
</style>
