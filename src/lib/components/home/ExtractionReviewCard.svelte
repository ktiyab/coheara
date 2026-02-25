<!-- LP-01: Single pending extraction item card for morning review. -->
<!-- Supports view mode (confirm/dismiss) and inline edit mode (correct before confirming). -->
<!-- LP-02/03/04: Enhanced with severity buttons, category dropdown, additional fields, duplicate warnings. -->
<script lang="ts">
	import { t } from 'svelte-i18n';
	import type { PendingReviewItem, ExtractionDomain } from '$lib/types/extraction';
	import { DOMAIN_LABELS, SYMPTOM_CATEGORIES, MEDICATION_ROUTES, SEVERITY_COLORS } from '$lib/types/extraction';
	import type { Component } from 'svelte';
	import {
		ClipboardIcon,
		HeartIcon,
		CalendarIcon,
		EditIcon,
		CloseIcon,
		WarningIcon,
	} from '$lib/components/icons/md';

	interface Props {
		item: PendingReviewItem;
		onConfirm: (id: string) => void;
		onConfirmWithEdits: (id: string, edits: Record<string, unknown>) => void;
		onDismiss: (id: string) => void;
	}

	let { item, onConfirm, onConfirmWithEdits, onDismiss }: Props = $props();

	const domainIcon: Record<ExtractionDomain, Component<{ class?: string }>> = {
		symptom: HeartIcon,
		medication: ClipboardIcon,
		appointment: CalendarIcon,
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
		if (item.grounding === 'grounded') return 'text-[var(--color-success)]';
		if (item.grounding === 'partial') return 'text-amber-600 dark:text-amber-400';
		return 'text-red-600 dark:text-red-400';
	});

	// Duplicate warning from item data
	let duplicateWarning = $derived(item.duplicate_of ? $t('extraction.duplicate_warning_text') : null);

	// ── Edit mode state ──
	let editing = $state(false);
	let edits = $state<Record<string, unknown>>({});
	let confirming = $state(false);
	let dismissing = $state(false);

	// Severity required: block confirm when severity is null and user hasn't picked one
	let severityRequired = $derived(
		item.domain === 'symptom' && editing && (edits.severity_hint == null || edits.severity_hint === undefined)
	);

	let canConfirm = $derived(!confirming && !severityRequired);

	function startEdit() {
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

	function selectSeverity(level: number) {
		updateField('severity_hint', level);
	}
</script>

<div
	class="rounded-xl border border-[var(--color-border)] bg-white dark:bg-gray-900 p-4"
	role="article"
	aria-label={title}
>
	<!-- Duplicate warning banner -->
	{#if duplicateWarning}
		<div class="flex items-center gap-2 mb-3 px-3 py-2 rounded-lg bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800">
			<WarningIcon class="w-4 h-4 text-amber-600 dark:text-amber-400 shrink-0" />
			<span class="text-xs text-amber-700 dark:text-amber-300">{duplicateWarning}</span>
		</div>
	{/if}

	<!-- Header row -->
	<div class="flex items-start gap-3">
		<div
			class="shrink-0 w-9 h-9 rounded-lg bg-[var(--color-success)] flex items-center justify-center"
		>
			<Icon class="w-4 h-4 text-white" />
		</div>
		<div class="flex-1 min-w-0">
			<div class="flex items-center gap-2">
				<span
					class="text-xs font-medium px-2 py-0.5 rounded-full bg-[var(--color-success-50)] dark:bg-gray-800 text-[var(--color-success)]"
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
				<!-- Source quote (REV-12) -->
				{#if item.source_quote}
					<p class="text-xs italic text-[var(--color-text-muted)] mt-1.5 line-clamp-2">
						"{item.source_quote}"
					</p>
				{/if}
				<!-- Severity badge in view mode -->
				{#if item.domain === 'symptom' && data.severity_hint != null}
					{@const sev = data.severity_hint as number}
					{@const colors = SEVERITY_COLORS[sev] || SEVERITY_COLORS[3]}
					<span class="inline-block mt-1 text-xs px-2 py-0.5 rounded-full {colors.bg} {colors.text}">
						{$t('extraction.field_severity')}: {sev}/5
					</span>
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
				<EditIcon class="w-4 h-4" />
			</button>
		{:else}
			<button
				class="shrink-0 p-1.5 rounded-lg text-[var(--color-text-muted)] hover:bg-[var(--color-surface-hover)] transition-colors"
				onclick={cancelEdit}
				aria-label={$t('extraction.cancel_edit')}
			>
				<CloseIcon class="w-4 h-4" />
			</button>
		{/if}
	</div>

	<!-- Edit fields (domain-specific) -->
	{#if editing}
		<div class="mt-3 space-y-2">
			{#if item.domain === 'symptom'}
				<!-- Specific name -->
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_specific')}</span>
					<input
						type="text"
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-success)] focus:border-[var(--color-success)]"
						value={edits.specific as string ?? ''}
						oninput={(e) => updateField('specific', e.currentTarget.value)}
					/>
				</label>
				<!-- Category dropdown -->
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_category')}</span>
					<select
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-success)] focus:border-[var(--color-success)]"
						value={edits.category as string ?? 'Other'}
						onchange={(e) => updateField('category', e.currentTarget.value)}
					>
						{#each SYMPTOM_CATEGORIES as cat}
							<option value={cat}>{cat}</option>
						{/each}
					</select>
				</label>
				<!-- Severity buttons (1-5) -->
				<div>
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_severity')}</span>
					{#if severityRequired}
						<span class="text-xs text-amber-600 dark:text-amber-400 ml-1">{$t('extraction.severity_required_hint')}</span>
					{/if}
					<div class="flex gap-1.5 mt-1" role="radiogroup" aria-label={$t('extraction.field_severity')}>
						{#each [1, 2, 3, 4, 5] as level}
							{@const colors = SEVERITY_COLORS[level]}
							{@const selected = (edits.severity_hint as number) === level}
							<button
								type="button"
								class="flex-1 py-1.5 rounded-lg text-xs font-medium border transition-all min-h-[32px]
									   {selected ? `${colors.bg} ${colors.text} ${colors.border} ring-1 ring-current` : `border-[var(--color-border)] text-[var(--color-text-muted)] hover:${colors.bg}`}"
								onclick={() => selectSeverity(level)}
								role="radio"
								aria-checked={selected}
								aria-label="{level}/5"
							>
								{level}
							</button>
						{/each}
					</div>
				</div>
				<!-- Body region -->
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_body_region')}</span>
					<input
						type="text"
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-success)] focus:border-[var(--color-success)]"
						value={edits.body_region as string ?? ''}
						oninput={(e) => updateField('body_region', e.currentTarget.value)}
					/>
				</label>
				<!-- Onset date -->
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_onset_date')}</span>
					<input
						type="date"
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-success)] focus:border-[var(--color-success)]"
						value={edits.onset_hint as string ?? ''}
						oninput={(e) => updateField('onset_hint', e.currentTarget.value)}
					/>
				</label>
			{:else if item.domain === 'medication'}
				<!-- Name -->
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_specific')}</span>
					<input
						type="text"
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-success)] focus:border-[var(--color-success)]"
						value={edits.name as string ?? ''}
						oninput={(e) => updateField('name', e.currentTarget.value)}
					/>
				</label>
				<!-- Dose -->
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_dose')}</span>
					<input
						type="text"
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-success)] focus:border-[var(--color-success)]"
						value={edits.dose as string ?? ''}
						oninput={(e) => updateField('dose', e.currentTarget.value)}
					/>
				</label>
				<!-- Frequency -->
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_frequency')}</span>
					<input
						type="text"
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-success)] focus:border-[var(--color-success)]"
						value={edits.frequency as string ?? ''}
						oninput={(e) => updateField('frequency', e.currentTarget.value)}
					/>
				</label>
				<!-- Route -->
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_route')}</span>
					<select
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-success)] focus:border-[var(--color-success)]"
						value={edits.route as string ?? 'oral'}
						onchange={(e) => updateField('route', e.currentTarget.value)}
					>
						{#each MEDICATION_ROUTES as route}
							<option value={route}>{route}</option>
						{/each}
					</select>
				</label>
				<!-- Reason -->
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_reason')}</span>
					<input
						type="text"
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-success)] focus:border-[var(--color-success)]"
						value={edits.reason as string ?? ''}
						oninput={(e) => updateField('reason', e.currentTarget.value)}
					/>
				</label>
				<!-- Start date -->
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_start_date')}</span>
					<input
						type="date"
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-success)] focus:border-[var(--color-success)]"
						value={edits.start_date_hint as string ?? ''}
						oninput={(e) => updateField('start_date_hint', e.currentTarget.value)}
					/>
				</label>
			{:else if item.domain === 'appointment'}
				<!-- Professional -->
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_professional')}</span>
					<input
						type="text"
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-success)] focus:border-[var(--color-success)]"
						value={edits.professional_name as string ?? ''}
						oninput={(e) => updateField('professional_name', e.currentTarget.value)}
					/>
				</label>
				<!-- Specialty -->
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_specialty')}</span>
					<input
						type="text"
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-success)] focus:border-[var(--color-success)]"
						value={edits.specialty as string ?? ''}
						oninput={(e) => updateField('specialty', e.currentTarget.value)}
					/>
				</label>
				<!-- Date -->
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_date')}</span>
					<input
						type="date"
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-success)] focus:border-[var(--color-success)]"
						value={edits.date_hint as string ?? ''}
						oninput={(e) => updateField('date_hint', e.currentTarget.value)}
					/>
				</label>
				<!-- Time -->
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_time')}</span>
					<input
						type="time"
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-success)] focus:border-[var(--color-success)]"
						value={edits.time_hint as string ?? ''}
						oninput={(e) => updateField('time_hint', e.currentTarget.value)}
					/>
				</label>
				<!-- Reason -->
				<label class="block">
					<span class="text-xs font-medium text-[var(--color-text-secondary)]">{$t('extraction.field_reason')}</span>
					<input
						type="text"
						class="mt-0.5 w-full text-sm rounded-lg border border-[var(--color-border)] bg-white dark:bg-gray-800
							   text-[var(--color-text-primary)] px-2.5 py-1.5 focus:ring-1 focus:ring-[var(--color-success)] focus:border-[var(--color-success)]"
						value={edits.reason as string ?? ''}
						oninput={(e) => updateField('reason', e.currentTarget.value)}
					/>
				</label>
			{/if}
		</div>
	{/if}

	<!-- Action buttons -->
	<div class="flex items-center gap-2 mt-3 pt-3 border-t border-[var(--color-border)]">
		<button
			class="flex-1 text-xs font-medium py-2 rounded-lg
				   bg-[var(--color-success)] text-white
				   hover:opacity-90 transition-opacity
				   disabled:opacity-50"
			onclick={handleConfirm}
			disabled={!canConfirm}
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
