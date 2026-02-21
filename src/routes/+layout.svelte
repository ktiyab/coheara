<script lang="ts">
  import '../app.css';
  import '$lib/i18n'; // Side-effect: initializes svelte-i18n at module load
  import { onMount } from 'svelte';
  import { theme } from '$lib/stores/theme.svelte';
  import ProfileGuard from '$lib/components/profile/ProfileGuard.svelte';
  let { children } = $props();

  let renderError: unknown = $state(null);
  let resetFn: (() => void) | null = $state(null);

  onMount(() => {
    theme.init();
  });
</script>

<svelte:boundary onerror={(error, reset) => { renderError = error; resetFn = reset; }}>
  <ProfileGuard>
    {@render children()}
  </ProfileGuard>

  {#snippet failed()}
    <div style="position:fixed; inset:0; z-index:99999; background:#fff; padding:2rem; font-family:system-ui,sans-serif; overflow:auto;">
      <h1 style="color:#dc2626; margin:0 0 0.5rem;">Render Error</h1>
      <p style="color:#44403c; margin:0 0 1rem;">A component crashed during rendering.</p>
      <pre style="background:#fef2f2; border:1px solid #fecaca; padding:1rem; border-radius:0.5rem;
        white-space:pre-wrap; word-break:break-word; font-size:0.875rem; max-height:50vh; overflow:auto;">{renderError instanceof Error ? (renderError.stack ?? renderError.message) : String(renderError)}</pre>
      <button onclick={() => resetFn?.()} style="margin-top:1rem; padding:0.5rem 1rem;
        background:#4A6FA5; color:#fff; border:none; border-radius:0.375rem; cursor:pointer; font-size:0.875rem;">
        Retry
      </button>
    </div>
  {/snippet}
</svelte:boundary>
