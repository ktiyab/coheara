<script lang="ts">
  import '../app.css';
  import ProfileGuard from '$lib/components/profile/ProfileGuard.svelte';
  let { children } = $props();
</script>

<main class="min-h-screen bg-stone-50 text-stone-900 font-sans">
  <ProfileGuard>
    {@render children()}
  </ProfileGuard>
</main>
