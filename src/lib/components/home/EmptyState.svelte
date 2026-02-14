<script lang="ts">
  import { navigation } from '$lib/stores/navigation.svelte';
</script>

<div class="flex flex-col items-center justify-center px-8 py-12 text-center">
  <div class="w-24 h-24 bg-stone-100 rounded-2xl flex items-center justify-center mb-6">
    <span class="text-4xl text-stone-300">&#128196;</span>
  </div>

  <h2 class="text-lg font-medium text-stone-700 mb-2">
    No documents yet
  </h2>
  <p class="text-sm text-stone-500 mb-6 max-w-[280px]">
    Load your first medical document to get started.
    Coheara will help you understand your health records.
  </p>

  <button
    class="px-8 py-4 bg-teal-600 text-white rounded-xl text-base font-medium
           hover:bg-teal-700 focus-visible:outline focus-visible:outline-2
           focus-visible:outline-offset-2 focus-visible:outline-teal-600
           min-h-[44px]"
    onclick={() => navigation.navigate('import')}
  >
    Load your first document
  </button>
</div>
