<!-- L4-01: Date + optional time selector for symptom onset. -->
<script lang="ts">
  interface Props {
    date: string;
    time: string | null;
    onDateChange: (date: string) => void;
    onTimeChange: (time: string | null) => void;
  }
  let { date, time, onDateChange, onTimeChange }: Props = $props();

  const today = new Date().toISOString().split('T')[0];
  const yesterday = new Date(Date.now() - 86400000).toISOString().split('T')[0];

  const timeOptions = [
    { label: 'Morning', value: '09:00' },
    { label: 'Afternoon', value: '14:00' },
    { label: 'Evening', value: '20:00' },
    { label: 'Not sure', value: null as string | null },
  ];
</script>

<div class="flex flex-col gap-4">
  <!-- Quick date buttons -->
  <div class="flex gap-3">
    <button
      class="flex-1 px-4 py-3 rounded-xl border text-sm font-medium min-h-[44px]
             transition-colors
             {date === today
               ? 'border-[var(--color-primary)] bg-blue-50 text-[var(--color-primary)]'
               : 'border-stone-200 bg-white text-stone-600 hover:bg-stone-50'}"
      onclick={() => onDateChange(today)}
    >
      Today
    </button>
    <button
      class="flex-1 px-4 py-3 rounded-xl border text-sm font-medium min-h-[44px]
             transition-colors
             {date === yesterday
               ? 'border-[var(--color-primary)] bg-blue-50 text-[var(--color-primary)]'
               : 'border-stone-200 bg-white text-stone-600 hover:bg-stone-50'}"
      onclick={() => onDateChange(yesterday)}
    >
      Yesterday
    </button>
  </div>

  <!-- Calendar picker for older dates -->
  <label class="flex flex-col gap-1">
    <span class="text-sm text-stone-500">Or pick a date:</span>
    <input
      type="date"
      value={date}
      max={today}
      oninput={(e) => onDateChange(e.currentTarget.value)}
      class="px-4 py-3 rounded-lg border border-stone-300 text-base min-h-[44px]
             focus:border-[var(--color-primary)] focus:outline-none"
    />
  </label>

  <!-- Optional time of day -->
  <div>
    <span class="text-sm text-stone-500 mb-2 block">What time? (optional)</span>
    <div class="grid grid-cols-4 gap-2">
      {#each timeOptions as option}
        <button
          class="px-3 py-2 rounded-lg border text-xs font-medium min-h-[44px]
                 transition-colors
                 {time === option.value
                   ? 'border-[var(--color-primary)] bg-blue-50 text-[var(--color-primary)]'
                   : 'border-stone-200 bg-white text-stone-600 hover:bg-stone-50'}"
          onclick={() => onTimeChange(option.value)}
        >
          {option.label}
        </button>
      {/each}
    </div>
  </div>
</div>
