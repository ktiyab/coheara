<!--
  SvelteKit error page — shown for routing/load failures.
  Uses inline styles only (Tailwind may not be available if CSS failed to load).
-->
<script lang="ts">
  import { page } from '$app/stores';
</script>

<div style="min-height:100vh; display:flex; align-items:center; justify-content:center;
  font-family:system-ui,sans-serif; padding:2rem; background:#fafaf9;">
  <div style="max-width:32rem; width:100%;">
    <h1 style="color:#dc2626; font-size:1.5rem; font-weight:700; margin:0 0 0.5rem;">
      {$page.status} — {$page.error?.message ?? 'Something went wrong'}
    </h1>
    <pre style="background:#fef2f2; border:1px solid #fecaca; padding:1rem; border-radius:0.5rem;
      white-space:pre-wrap; word-break:break-word; font-size:0.875rem; margin:1rem 0;">Status: {$page.status}
Message: {$page.error?.message ?? 'Unknown error'}</pre>
    <a href="/" style="display:inline-block; padding:0.5rem 1rem; background:#4A6FA5;
      color:#fff; text-decoration:none; border-radius:0.375rem; font-size:0.875rem;">
      Go home
    </a>
  </div>
</div>
