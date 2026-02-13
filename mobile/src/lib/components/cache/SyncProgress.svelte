<!-- M1-06: First sync progress — "Setting up your phone..." (Lena: patient language) -->
<script lang="ts">
	import type { FirstSyncProgress } from '$lib/types/cache-manager.js';

	const { progress }: {
		progress: FirstSyncProgress;
	} = $props();

	const stageLabel = $derived(() => {
		switch (progress.stage) {
			case 'connecting': return 'Connecting to your desktop...';
			case 'downloading': return 'Downloading your health summary for offline use.';
			case 'storing': return 'Setting up your phone...';
			case 'complete': return 'All set!';
		}
	});
</script>

<div class="sync-progress" role="status" aria-live="polite">
	<div class="progress-card">
		{#if progress.stage === 'complete'}
			<p class="complete-check" aria-hidden="true">&#10003;</p>
			<p class="progress-title">All set!</p>
			<p class="progress-note">Your health data is ready for offline use.</p>
		{:else}
			<p class="progress-title">Setting up your phone...</p>
			<div class="progress-bar-container">
				<div
					class="progress-bar"
					style="width: {progress.percent}%"
					role="progressbar"
					aria-valuenow={progress.percent}
					aria-valuemin={0}
					aria-valuemax={100}
				></div>
			</div>
			<p class="progress-detail">{stageLabel()}</p>
			<p class="progress-note">This happens once. Future updates are instant.</p>
		{/if}
	</div>
</div>

<style>
	.sync-progress {
		display: flex;
		align-items: center;
		justify-content: center;
		min-height: 300px;
		padding: 24px;
	}

	.progress-card {
		width: 100%;
		max-width: 320px;
		padding: 32px 24px;
		background: white;
		border-radius: 16px;
		text-align: center;
		border: 1px solid #E7E5E4;
	}

	.progress-title {
		font-size: 18px;
		font-weight: 600;
		margin: 0 0 16px;
	}

	.progress-bar-container {
		height: 8px;
		background: #E7E5E4;
		border-radius: 4px;
		overflow: hidden;
		margin-bottom: 12px;
	}

	.progress-bar {
		height: 100%;
		background: var(--color-primary);
		border-radius: 4px;
		transition: width 0.3s ease;
	}

	.progress-detail {
		font-size: 15px;
		color: var(--color-text);
		margin: 0 0 8px;
		line-height: 1.4;
	}

	.progress-note {
		font-size: 13px;
		color: var(--color-text-muted);
		margin: 0;
		line-height: 1.4;
	}

	.complete-check {
		font-size: 48px;
		color: var(--color-success);
		margin: 0 0 12px;
	}
</style>
