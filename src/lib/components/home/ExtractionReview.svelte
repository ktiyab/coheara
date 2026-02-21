<!-- LP-01: Morning review section for pending extraction items on Home screen. -->
<script lang="ts">
	import { t } from 'svelte-i18n';
	import { extraction } from '$lib/stores/extraction.svelte';
	import ExtractionReviewCard from './ExtractionReviewCard.svelte';
	import { ClipboardCheckOutline } from 'flowbite-svelte-icons';

	let successMsg = $state<string | null>(null);
	let errorMsg = $state<string | null>(null);

	async function handleConfirm(itemId: string) {
		try {
			errorMsg = null;
			await extraction.confirm(itemId);
			successMsg = $t('extraction.confirmed_msg');
			setTimeout(() => (successMsg = null), 2000);
		} catch (e) {
			errorMsg = e instanceof Error ? e.message : String(e);
		}
	}

	async function handleConfirmWithEdits(itemId: string, edits: Record<string, unknown>) {
		try {
			errorMsg = null;
			await extraction.confirmWithEdits(itemId, edits);
			successMsg = $t('extraction.confirmed_msg');
			setTimeout(() => (successMsg = null), 2000);
		} catch (e) {
			errorMsg = e instanceof Error ? e.message : String(e);
		}
	}

	async function handleDismiss(itemId: string) {
		try {
			errorMsg = null;
			await extraction.dismiss(itemId);
		} catch (e) {
			errorMsg = e instanceof Error ? e.message : String(e);
		}
	}

	async function handleDismissAll() {
		try {
			errorMsg = null;
			await extraction.dismissAll();
		} catch (e) {
			errorMsg = e instanceof Error ? e.message : String(e);
		}
	}
</script>

{#if extraction.items.length > 0}
	<section class="px-6 py-3" aria-label={$t('extraction.review_heading')}>
		<!-- Section header -->
		<div class="flex items-center justify-between mb-3">
			<div class="flex items-center gap-2">
				<ClipboardCheckOutline class="w-4 h-4 text-[var(--color-primary)]" />
				<h2 class="text-sm font-semibold text-[var(--color-text-primary)]">
					{$t('extraction.review_heading')}
				</h2>
				<span
					class="text-xs font-medium px-1.5 py-0.5 rounded-full bg-[var(--color-primary)] text-white"
				>
					{extraction.items.length}
				</span>
			</div>
			{#if extraction.items.length > 1}
				<button
					class="text-xs text-[var(--color-text-muted)] hover:text-[var(--color-text-secondary)] transition-colors"
					onclick={handleDismissAll}
				>
					{$t('extraction.dismiss_all_btn')}
				</button>
			{/if}
		</div>

		<!-- Subtitle -->
		<p class="text-xs text-[var(--color-text-muted)] mb-3">
			{$t('extraction.review_subtitle')}
		</p>

		<!-- Success feedback -->
		{#if successMsg}
			<div
				class="mb-2 px-3 py-2 text-xs font-medium rounded-lg bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-300"
				role="status"
			>
				{successMsg}
			</div>
		{/if}

		<!-- Error feedback -->
		{#if errorMsg}
			<div
				class="mb-2 px-3 py-2 text-xs font-medium rounded-lg bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300"
				role="alert"
			>
				{errorMsg}
			</div>
		{/if}

		<!-- Item cards -->
		<div class="flex flex-col gap-3">
			{#each extraction.items as item (item.id)}
				<ExtractionReviewCard
					{item}
					onConfirm={handleConfirm}
					onConfirmWithEdits={handleConfirmWithEdits}
					onDismiss={handleDismiss}
				/>
			{/each}
		</div>
	</section>
{/if}
