<!-- LP-01: Single pending extraction item card for morning review. -->
<!-- Supports view mode (confirm/dismiss) and inline edit mode (correct before confirming). -->
<script lang="ts">
	import { t } from 'svelte-i18n';
	import type { PendingReviewItem, ExtractionDomain } from '$lib/types/extraction';
	import { DOMAIN_LABELS } from '$lib/types/extraction';
	import {
		ClipboardCheckOutline,
		HeartOutline,
		CalendarMonthOutline,
		EditOutline,
		CloseOutline,
	} from 'flowbite-svelte-icons';

	interface Props {
		item: PendingReviewItem;
		onConfirm: (id: string) => void;
		onConfirmWithEdits: (id: string, edits: Record<string, unknown>) => void;
		onDismiss: (id: string) => void;
	}

	let { item, onConfirm, onConfirmWithEdits, onDismiss }: Props = $props();

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

	// ── Edit mode state ──
	let editing = $state(false);
	let edits = $state<Record<string, unknown>>({});
	let confirming = $state(false);
	let dismissing = $state(false);

	function startEdit() {
		// Shallow-clone extracted_data as starting point for edits
		edits = { ...data };
		editing = true;
	}

	function cancelEdit() {
		editing = false;
		edits = {};
	}

	function handleConfirm() {
		confirming = true;
		if (editing) {
			// Only send fields that actually changed
			const changed: Record<string, unknown> = {};
			for (const key of Object.keys(edits)) {
				if (edits[key] !== data[key]) {
					changed[key] = edits[key];
				}
			}
			if (Object.keys(changed).length > 0) {
				onConfirmWithEdits(item.id, changed);
			} else {
				onConfirm(item.id);
			}
		} else {
			onConfirm(item.id);
		}
	}

	function handleDismiss() {
		dismissing = true;
		onDismiss(item.id);
	}

	function updateField(key: string, value: unknown) {
		edits = { ...edits, [key]: value };
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
			{#if !editing}
				<p class="text-sm font-medium text-[var(--color-text-primary)] mt-1 truncate">
					{title}
				</p>
				{#if subtitle}
					<p class="text-xs text-[var(--color-text-muted)] mt-0.5 truncate">
						{subtitle}
					</p>
				{/if}
			{/if}
		</div>
		<!-- Edit toggle -->
		{#if !editing}
			<button
				class="shrink-0 p-1.5 rounded-lg text-[var(--color-text-muted)] hover:bg-[var(--color-surface-hover)] transition-colors"
				onclick={startEdit}
				aria-label={$t('extraction.edit_btn')}
				title={$t('extraction.edit_btn')}
			>
				<EditOutline class="w-4 h-4" />
			</button>
		{:else}
			<button
				class="shrink-0 p-1.5 rounded-lg text-[var(--color-text-muted)] hover:bg-[var(--color-surface-hover)] transition-colors"
				onclick={cancelEdit}
				aria-label={$t('extraction.cancel_edit')}
			>
				<CloseOutline class="w-4 h-4" />
			</button>
		{/if}
	</div>

	<!-- Edit fields (domain-specific) -->
	{#if editing}
		<div class="mt-3 space-y-2">
			{#if item.domain === 'symptom'}
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_specific')}</span>
					<input
						type="text"
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-primary)] focus:border-[var(--color-primary)]"
						value={edits.specific as string ?? ''}
						oninput={(e) => updateField('specific', e.currentTarget.value)}
					/>
				</label>
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_category')}</span>
					<input
						type="text"
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-primary)] focus:border-[var(--color-primary)]"
						value={edits.category as string ?? ''}
						oninput={(e) => updateField('category', e.currentTarget.value)}
					/>
				</label>
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_severity')}</span>
					<div class="flex items-center gap-2 mt-0.5">
						<input
							type="range"
							min="1"
							max="5"
							step="1"
							class="flex-1 accent-[var(--color-primary)]"
							value={edits.severity_hint as number ?? 3}
							oninput={(e) => updateField('severity_hint', parseInt(e.currentTarget.value))}
						/>
						<span class="text-sm font-medium text-[var(--color-text-primary)] w-6 text-center">
							{edits.severity_hint ?? 3}
						</span>
					</div>
				</label>
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_body_region')}</span>
					<input
						type="text"
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-primary)] focus:border-[var(--color-primary)]"
						value={edits.body_region as string ?? ''}
						oninput={(e) => updateField('body_region', e.currentTarget.value)}
					/>
				</label>
			{:else if item.domain === 'medication'}
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_specific')}</span>
					<input
						type="text"
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-primary)] focus:border-[var(--color-primary)]"
						value={edits.name as string ?? ''}
						oninput={(e) => updateField('name', e.currentTarget.value)}
					/>
				</label>
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_dose')}</span>
					<input
						type="text"
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-primary)] focus:border-[var(--color-primary)]"
						value={edits.dose as string ?? ''}
						oninput={(e) => updateField('dose', e.currentTarget.value)}
					/>
				</label>
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_frequency')}</span>
					<input
						type="text"
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-primary)] focus:border-[var(--color-primary)]"
						value={edits.frequency as string ?? ''}
						oninput={(e) => updateField('frequency', e.currentTarget.value)}
					/>
				</label>
			{:else if item.domain === 'appointment'}
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_professional')}</span>
					<input
						type="text"
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-primary)] focus:border-[var(--color-primary)]"
						value={edits.professional_name as string ?? ''}
						oninput={(e) => updateField('professional_name', e.currentTarget.value)}
					/>
				</label>
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_specialty')}</span>
					<input
						type="text"
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-primary)] focus:border-[var(--color-primary)]"
						value={edits.specialty as string ?? ''}
						oninput={(e) => updateField('specialty', e.currentTarget.value)}
					/>
				</label>
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_date')}</span>
					<input
						type="date"
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-primary)] focus:border-[var(--color-primary)]"
						value={edits.date_hint as string ?? ''}
						oninput={(e) => updateField('date_hint', e.currentTarget.value)}
					/>
				</label>
			{/if}
		</div>
	{/if}

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
			{confirming ? '...' : editing ? $t('extraction.save_btn') : $t('extraction.confirm_btn')}
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
