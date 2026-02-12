<!-- L3-05: Empty state when no medications are recorded. -->
<script lang="ts">
  interface Props {
    onAddOtc: () => void;
  }
  let { onAddOtc }: Props = $props();
</script>

<div class="flex flex-col items-center justify-center px-8 py-12 text-center">
  <div class="w-24 h-24 bg-stone-100 rounded-2xl flex items-center justify-center mb-6">
    <span class="text-4xl text-stone-300" aria-hidden="true">&#x1F48A;</span>
  </div>

  <h2 class="text-lg font-medium text-stone-700 mb-2">
    No medications recorded yet
  </h2>
  <p class="text-sm text-stone-500 mb-6 max-w-[300px]">
    Medications will appear here when you load a prescription or add an over-the-counter medication.
  </p>

  <button
    class="px-8 py-4 bg-[var(--color-primary)] text-white rounded-xl text-base font-medium
           hover:brightness-110 min-h-[44px]"
    onclick={onAddOtc}
  >
    + Add an over-the-counter medication
  </button>
</div>
