<script lang="ts">
  interface Props {
    words: string[];
    onConfirmed: () => void;
  }
  let { words, onConfirmed }: Props = $props();

  let confirmed = $state(false);
</script>

<div class="flex flex-col items-center justify-center min-h-screen px-8 gap-6 max-w-lg mx-auto">
  <h2 class="text-2xl font-bold text-stone-800">Your recovery phrase</h2>

  <p class="text-stone-600 text-center leading-relaxed">
    Write these 12 words down on paper and keep them safe.
    If you forget your password, these words are the <strong>only way</strong>
    to recover your data.
  </p>

  <div class="grid grid-cols-3 gap-3 w-full p-6 bg-white rounded-xl border border-stone-200 shadow-sm">
    {#each words as word, i}
      <div class="flex items-center gap-2 p-2 bg-stone-50 rounded-lg">
        <span class="text-stone-400 text-sm w-5 text-right">{i + 1}.</span>
        <span class="text-stone-800 font-mono text-lg">{word}</span>
      </div>
    {/each}
  </div>

  <div class="flex flex-col gap-3 w-full mt-4">
    <p class="text-stone-500 text-sm text-center">
      This phrase will NOT be shown again. Please write it down now.
    </p>

    <label class="flex items-center gap-3 justify-center">
      <input type="checkbox" bind:checked={confirmed}
             class="min-h-[44px] min-w-[44px]" />
      <span class="text-stone-700">I have written down my recovery phrase</span>
    </label>

    <button
      class="px-8 py-4 bg-[var(--color-primary)] text-white rounded-xl text-lg
             font-medium hover:brightness-110 disabled:opacity-50 min-h-[44px]"
      onclick={onConfirmed}
      disabled={!confirmed}
    >
      Continue
    </button>
  </div>
</div>
