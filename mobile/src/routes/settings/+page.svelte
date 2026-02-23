<!-- M1-03: Settings screen — connection status, unpair, about -->
<script lang="ts">
	import { connection, isConnected, statusLabel, setUnpaired } from '$lib/stores/connection.js';
	import { lastSyncTimestamp, profile, clearCacheStores } from '$lib/stores/cache.js';
	import { resetSessionState } from '$lib/stores/session.js';
	import { secureRemove, STORAGE_KEYS } from '$lib/utils/secure-storage.js';
	import { freshnessLabel } from '$lib/utils/viewer.js';

	let confirmUnpair = $state(false);

	async function handleUnpair(): Promise<void> {
		if (!confirmUnpair) {
			confirmUnpair = true;
			return;
		}

		// Clear secure credentials
		await secureRemove(STORAGE_KEYS.SESSION_TOKEN);
		await secureRemove(STORAGE_KEYS.CACHE_KEY);
		await secureRemove(STORAGE_KEYS.DESKTOP_URL);
		await secureRemove(STORAGE_KEYS.DEVICE_ID);
		await secureRemove(STORAGE_KEYS.LAST_PROFILE);
		await secureRemove(STORAGE_KEYS.PAIRING_CERT);

		// Clear app state
		clearCacheStores();
		resetSessionState();
		setUnpaired();
		confirmUnpair = false;
	}

	function cancelUnpair(): void {
		confirmUnpair = false;
	}
</script>

<div class="settings-screen">
	<h1>Settings</h1>

	<section class="settings-section">
		<h2>Connection</h2>

		<div class="setting-row">
			<span class="setting-label">Status</span>
			<span class="setting-value">{$statusLabel}</span>
		</div>

		{#if $profile}
			<div class="setting-row">
				<span class="setting-label">Profile</span>
				<span class="setting-value">{$profile.name}</span>
			</div>
		{/if}

		<div class="setting-row">
			<span class="setting-label">Last sync</span>
			<span class="setting-value">{freshnessLabel($lastSyncTimestamp)}</span>
		</div>
	</section>

	{#if $connection.status !== 'unpaired'}
		<section class="settings-section">
			<h2>Pairing</h2>

			{#if confirmUnpair}
				<div class="confirm-box">
					<p>This will disconnect from your desktop and erase all cached data on this phone.</p>
					<div class="confirm-actions">
						<button class="btn-cancel" onclick={cancelUnpair}>Cancel</button>
						<button class="btn-danger" onclick={handleUnpair}>Unpair</button>
					</div>
				</div>
			{:else}
				<button class="unpair-btn" onclick={handleUnpair}>
					Disconnect from Desktop
				</button>
			{/if}
		</section>
	{/if}

	<section class="settings-section">
		<h2>About</h2>

		<div class="setting-row">
			<span class="setting-label">App</span>
			<span class="setting-value">Coheara Companion</span>
		</div>

		<div class="setting-row">
			<span class="setting-label">Version</span>
			<span class="setting-value">0.2.0-beta</span>
		</div>

		<p class="about-note">
			Your medical data stays on your desktop. This companion app caches a reduced subset for convenient viewing.
		</p>
	</section>
</div>

<style>
	.settings-screen {
		padding: 16px;
	}

	h1 {
		font-size: 20px;
		font-weight: 700;
		margin: 0 0 20px;
	}

	.settings-section {
		margin-bottom: 24px;
	}

	h2 {
		font-size: 13px;
		font-weight: 600;
		color: var(--color-text-muted);
		text-transform: uppercase;
		letter-spacing: 0.5px;
		margin: 0 0 8px;
	}

	.setting-row {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 14px 16px;
		background: white;
		border: 1px solid #E7E5E4;
		margin-bottom: -1px;
	}

	.setting-row:first-of-type {
		border-radius: 12px 12px 0 0;
	}

	.setting-row:last-of-type {
		border-radius: 0 0 12px 12px;
		margin-bottom: 0;
	}

	.setting-row:only-of-type {
		border-radius: 12px;
	}

	.setting-label {
		font-size: 16px;
		color: var(--color-text);
	}

	.setting-value {
		font-size: 16px;
		color: var(--color-text-muted);
	}

	.unpair-btn {
		width: 100%;
		padding: 14px;
		background: white;
		color: var(--color-error, #DC2626);
		border: 2px solid var(--color-error, #DC2626);
		border-radius: 12px;
		font-size: 16px;
		font-weight: 600;
		cursor: pointer;
		font-family: inherit;
		min-height: var(--min-touch-target);
	}

	.confirm-box {
		padding: 16px;
		background: #FEF2F2;
		border: 1px solid #FECACA;
		border-radius: 12px;
	}

	.confirm-box p {
		font-size: 15px;
		color: var(--color-text);
		line-height: 1.4;
		margin: 0 0 12px;
	}

	.confirm-actions {
		display: flex;
		gap: 12px;
	}

	.btn-cancel {
		flex: 1;
		padding: 12px;
		background: white;
		color: var(--color-text);
		border: 1px solid #E7E5E4;
		border-radius: 12px;
		font-size: 16px;
		font-weight: 600;
		cursor: pointer;
		font-family: inherit;
		min-height: var(--min-touch-target);
	}

	.btn-danger {
		flex: 1;
		padding: 12px;
		background: var(--color-error, #DC2626);
		color: white;
		border: none;
		border-radius: 12px;
		font-size: 16px;
		font-weight: 600;
		cursor: pointer;
		font-family: inherit;
		min-height: var(--min-touch-target);
	}

	.about-note {
		font-size: 14px;
		color: var(--color-text-muted);
		line-height: 1.5;
		margin: 12px 0 0;
	}
</style>
