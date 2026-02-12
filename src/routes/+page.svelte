<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

  let status = $state('checking...');

  onMount(async () => {
    try {
      status = await invoke<string>('health_check');
    } catch (e) {
      status = 'error: ' + String(e);
    }
  });
</script>

<div class="flex flex-col items-center justify-center min-h-screen gap-4">
  <h1 class="text-4xl font-bold text-stone-800">Coheara</h1>
  <p class="text-lg text-stone-500">Your Personal MedAI</p>
  <p class="text-sm text-stone-400">
    Backend status: <span class="font-mono">{status}</span>
  </p>
</div>
