<!-- Base Material Design icon component. All MD icons delegate to this. -->
<script lang="ts">
  let { d, class: className = 'w-5 h-5', title }: { d: string; class?: string; title?: string } = $props();
</script>
<svg
  class={className}
  fill="currentColor"
  viewBox="0 -960 960 960"
  xmlns="http://www.w3.org/2000/svg"
  aria-hidden={!title}
  role={title ? 'img' : undefined}
>
  {#if title}<title>{title}</title>{/if}
  <path d={d} />
</svg>
