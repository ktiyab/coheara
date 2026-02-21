<!-- LP-01: Single pending extraction item card for morning review. -->
<script lang="ts">
	import { t } from 'svelte-i18n';
	import type { PendingReviewItem, ExtractionDomain } from '$lib/types/extraction';
	import { DOMAIN_LABELS, GROUNDING_LABELS } from '$lib/types/extraction';
	import {
		ClipboardCheckOutline,
		HeartOutline,
		CalendarMonthOutline,
	} from 'flowbite-svelte-icons';

	interface Props {
		item: PendingReviewItem;
		onConfirm: (id: string) => void;
		onDismiss: (id: string) => void;
	}

	let { item, onConfirm, onDismiss }: Props = $props();

	const domainIcon: Record<ExtractionDomain, typeof HeartOutline> = {
		symptom: HeartOutline,
		medication: ClipboardCheckOutline,
		appointment: CalendarMonthOutline,
	};

	let Icon = $derived(domainIcon[item.domain]);
	let data = $derived(item.extracted_data as Record<string, unknown>);

	let title = $derived.by(() => {
		if (item.domain === 'symptom') {
			return (data.specific as string) || (data.category as string) || 'Symptom';
		}
		if (item.domain === 'medication') {
			return (data.name as string) || 'Medication';
		}
		if (item.domain === 'appointment') {
			return (data.professional_name as string) || 'Appointment';
		}
		return '';
	});

	let subtitle = $derived.by(() => {
		if (item.domain === 'symptom') {
			const parts: string[] = [];
			if (data.category) parts.push(data.category as string);
			if (data.body_region) parts.push(data.body_region as string);
			return parts.join(' \u00b7 ');
		}
		if (item.domain === 'medication') {
			const parts: string[] = [];
			if (data.dose) parts.push(data.dose as string);
			if (data.frequency) parts.push(data.frequency as string);
			return parts.join(' \u00b7 ');
		}
		if (item.domain === 'appointment') {
			const parts: string[] = [];
			if (data.specialty) parts.push(data.specialty as string);
			if (data.date_hint) parts.push(data.date_hint as string);
			return parts.join(' \u00b7 ');
		}
		return '';
	});

	let confidencePct = $derived(Math.round(item.confidence * 100));

	let confidenceColor = $derived.by(() => {
		if (item.grounding === 'grounded') return 'text-green-600 dark:text-green-400';
		if (item.grounding === 'partial') return 'text-amber-600 dark:text-amber-400';
		return 'text-red-600 dark:text-red-400';
	});

	let confirming = $state(false);
	let dismissing = $state(false);

	async function handleConfirm() {
		confirming = true;
		onConfirm(item.id);
	}

	async function handleDismiss() {
		dismissing = true;
		onDismiss(item.id);
	}
</script>

<div
	class="rounded-xl border border-[var(--color-border)] bg-white dark:bg-gray-900 p-4"
	role="article"
	aria-label={title}
>
	<!-- Header row -->
	<div class="flex items-start gap-3">
		<div
			class="shrink-0 w-9 h-9 rounded-lg bg-[var(--color-primary-50)] dark:bg-gray-800 flex items-center justify-center"
		>
			<Icon class="w-4 h-4 text-[var(--color-primary)]" />
		</div>
		<div class="flex-1 min-w-0">
			<div class="flex items-center gap-2">
				<span
					class="text-xs font-medium px-2 py-0.5 rounded-full bg-[var(--color-primary-50)] dark:bg-gray-800 text-[var(--color-primary)]"
				>
					{$t(DOMAIN_LABELS[item.domain])}
				</span>
				<span class="text-xs {confidenceColor}">
					{confidencePct}%
				</span>
			</div>
			<p class="text-sm font-medium text-[var(--color-text-primary)] mt-1 truncate">
				{title}
			</p>
			{#if subtitle}
				<p class="text-xs text-[var(--color-text-muted)] mt-0.5 truncate">
					{subtitle}
				</p>
			{/if}
		</div>
	</div>

	<!-- Action buttons -->
	<div class="flex items-center gap-2 mt-3 pt-3 border-t border-[var(--color-border)]">
		<button
			class="flex-1 text-xs font-medium py-2 rounded-lg
				   bg-[var(--color-primary)] text-white
				   hover:opacity-90 transition-opacity
				   disabled:opacity-50"
			onclick={handleConfirm}
			disabled={confirming}
		>
			{confirming ? '...' : $t('extraction.confirm_btn')}
		</button>
		<button
			class="flex-1 text-xs font-medium py-2 rounded-lg
				   border border-[var(--color-border)]
				   text-[var(--color-text-secondary)]
				   hover:bg-[var(--color-surface-hover)] transition-colors
				   disabled:opacity-50"
			onclick={handleDismiss}
			disabled={dismissing}
		>
			{dismissing ? '...' : $t('extraction.dismiss_btn')}
		</button>
	</div>
</div>
