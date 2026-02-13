<!-- M1-01: Bottom tab navigation — 5 tabs with 48dp touch targets -->
<script lang="ts">
	import { activeTab, navigateToTab, TAB_CONFIGS } from '$lib/stores/navigation.js';
	import { hasData } from '$lib/stores/connection.js';
	import type { TabId } from '$lib/types/index.js';

	function handleTabClick(tabId: TabId): void {
		navigateToTab(tabId);
	}
</script>

<div
	class="bottom-tabs"
	role="tablist"
	aria-label="Main navigation"
>
	{#each TAB_CONFIGS as tab (tab.id)}
		{@const isActive = $activeTab === tab.id}
		{@const isDisabled = !tab.offlineAvailable && !$hasData}
		<button
			class="tab-button"
			class:active={isActive}
			class:disabled={isDisabled}
			role="tab"
			aria-selected={isActive}
			aria-label={tab.ariaLabel}
			aria-disabled={isDisabled}
			tabindex={isActive ? 0 : -1}
			onclick={() => !isDisabled && handleTabClick(tab.id)}
		>
			<span class="tab-icon" aria-hidden="true">{tab.icon}</span>
			<span class="tab-label">{tab.label}</span>
			{#if isActive}
				<span class="active-indicator" aria-hidden="true"></span>
			{/if}
		</button>
	{/each}
</div>

<style>
	.bottom-tabs {
		display: flex;
		justify-content: space-around;
		align-items: stretch;
		background: var(--color-surface);
		border-top: 1px solid #E7E5E4;
		padding-bottom: max(4px, env(safe-area-inset-bottom));
	}

	.tab-button {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		flex: 1;
		padding: 8px 4px 4px;
		min-height: var(--min-touch-target);
		background: none;
		border: none;
		cursor: pointer;
		position: relative;
		color: var(--color-text-muted);
		transition: color 0.15s;
	}

	.tab-button.active {
		color: var(--color-primary);
	}

	.tab-button.disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.tab-icon {
		font-size: 22px;
		line-height: 1;
	}

	.tab-label {
		font-size: 11px;
		font-weight: 500;
		margin-top: 2px;
	}

	.active-indicator {
		position: absolute;
		top: 0;
		left: 50%;
		transform: translateX(-50%);
		width: 24px;
		height: 3px;
		background: var(--color-primary);
		border-radius: 0 0 3px 3px;
	}
</style>
