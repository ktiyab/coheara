<!-- L4-04: Empty timeline state — prompts user to load first document. -->
<script lang="ts">
  interface Props {
    onNavigate: (screen: string) => void;
  }
  let { onNavigate }: Props = $props();
</script>

<div class="flex flex-col items-center justify-center flex-1 px-8 py-12 text-center">
  <div class="w-24 h-24 bg-stone-100 rounded-2xl flex items-center justify-center mb-6">
    <span class="text-4xl text-stone-300">&#x1F4C5;</span>
  </div>

  <h2 class="text-lg font-medium text-stone-700 mb-2">
    Your timeline is empty
  </h2>
  <p class="text-sm text-stone-500 mb-6 max-w-[280px]">
    Load your first medical document and your timeline will start building automatically.
  </p>

  <button
    class="px-8 py-4 bg-[var(--color-primary)] text-white rounded-xl text-base font-medium
           hover:brightness-110 focus-visible:outline focus-visible:outline-2
           focus-visible:outline-offset-2 focus-visible:outline-[var(--color-primary)]
           min-h-[44px]"
    onclick={() => onNavigate('import')}
  >
    Load a document
  </button>
</div>
