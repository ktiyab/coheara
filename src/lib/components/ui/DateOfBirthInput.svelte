<!-- Year/Month/Day dropdown DOB input — year-first, descending -->
<script lang="ts">
  import { t } from 'svelte-i18n';

  interface Props {
    value: string;
    onchange: (value: string) => void;
  }
  let { value, onchange }: Props = $props();

  const currentYear = new Date().getFullYear();
  const years = Array.from({ length: currentYear - 1899 }, (_, i) => currentYear - i);
  const months = Array.from({ length: 12 }, (_, i) => i + 1);

  // Parse initial value
  let selectedYear = $state<number | null>(null);
  let selectedMonth = $state<number | null>(null);
  let selectedDay = $state<number | null>(null);

  // Parse value prop on mount
  if (value) {
    const parts = value.split('-');
    if (parts.length === 3) {
      selectedYear = parseInt(parts[0], 10);
      selectedMonth = parseInt(parts[1], 10);
      selectedDay = parseInt(parts[2], 10);
    }
  }

  function daysInMonth(year: number | null, month: number | null): number {
    if (!year || !month) return 31;
    return new Date(year, month, 0).getDate();
  }

  let availableDays = $derived(
    Array.from(
      { length: daysInMonth(selectedYear, selectedMonth) },
      (_, i) => i + 1
    )
  );

  function emitValue() {
    if (selectedYear && selectedMonth && selectedDay) {
      const y = String(selectedYear);
      const m = String(selectedMonth).padStart(2, '0');
      const d = String(selectedDay).padStart(2, '0');
      onchange(`${y}-${m}-${d}`);
    } else {
      onchange('');
    }
  }

  function handleYear(e: Event) {
    const val = (e.target as HTMLSelectElement).value;
    selectedYear = val ? parseInt(val, 10) : null;
    // Clamp day if needed
    if (selectedDay && selectedDay > daysInMonth(selectedYear, selectedMonth)) {
      selectedDay = daysInMonth(selectedYear, selectedMonth);
    }
    emitValue();
  }

  function handleMonth(e: Event) {
    const val = (e.target as HTMLSelectElement).value;
    selectedMonth = val ? parseInt(val, 10) : null;
    if (selectedDay && selectedDay > daysInMonth(selectedYear, selectedMonth)) {
      selectedDay = daysInMonth(selectedYear, selectedMonth);
    }
    emitValue();
  }

  function handleDay(e: Event) {
    const val = (e.target as HTMLSelectElement).value;
    selectedDay = val ? parseInt(val, 10) : null;
    emitValue();
  }

  const selectClass = `px-3 py-2.5 rounded-lg border border-stone-300 dark:border-gray-600
    bg-white dark:bg-gray-900 text-stone-800 dark:text-gray-100
    focus:border-[var(--color-primary)] focus:outline-none
    min-h-[44px] text-base`;
</script>

<div class="flex gap-3">
  <label class="flex flex-col gap-1 flex-1">
    <span class="text-stone-500 dark:text-gray-400 text-xs">{$t('profile.dob_year')}</span>
    <select
      class={selectClass}
      value={selectedYear ?? ''}
      onchange={handleYear}
      aria-label={$t('profile.dob_year')}
    >
      <option value="">—</option>
      {#each years as year}
        <option value={year}>{year}</option>
      {/each}
    </select>
  </label>

  <label class="flex flex-col gap-1 flex-1">
    <span class="text-stone-500 dark:text-gray-400 text-xs">{$t('profile.dob_month')}</span>
    <select
      class={selectClass}
      value={selectedMonth ?? ''}
      onchange={handleMonth}
      aria-label={$t('profile.dob_month')}
    >
      <option value="">—</option>
      {#each months as month}
        <option value={month}>{String(month).padStart(2, '0')}</option>
      {/each}
    </select>
  </label>

  <label class="flex flex-col gap-1 flex-[0.8]">
    <span class="text-stone-500 dark:text-gray-400 text-xs">{$t('profile.dob_day')}</span>
    <select
      class={selectClass}
      value={selectedDay ?? ''}
      onchange={handleDay}
      aria-label={$t('profile.dob_day')}
    >
      <option value="">—</option>
      {#each availableDays as day}
        <option value={day}>{String(day).padStart(2, '0')}</option>
      {/each}
    </select>
  </label>
</div>
