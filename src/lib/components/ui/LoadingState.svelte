<!--
  C3: LoadingState — Shared UI Primitive
  Spec: 24-UX-COMPONENTS C3 + D4 Tier 2
  Replaces: 14 identical loading indicators

  3 variants: fullscreen, inline, skeleton
  Uses Flowbite Spinner for smooth SVG animation.
-->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { Spinner } from 'flowbite-svelte';

  interface Props {
    message?: string;
    variant?: 'fullscreen' | 'inline' | 'skeleton';
    lines?: number;
  }

  let {
    message,
    variant = 'fullscreen',
    lines = 3,
  }: Props = $props();
</script>

{#if variant === 'skeleton'}
  <div
    class="space-y-3 p-4"
    role="status"
    aria-live="polite"
    aria-label={message ?? $t('common.loading')}
  >
    {#each Array(lines) as _, i}
      <div
        class="h-3 bg-stone-200 dark:bg-gray-700 rounded animate-pulse"
        style="width: {i === lines - 1 ? '60%' : '100%'}"
      ></div>
    {/each}
    <span class="sr-only">{message ?? $t('common.loading')}</span>
  </div>
{:else if variant === 'inline'}
  <div
    class="flex items-center gap-3 py-4"
    role="status"
    aria-live="polite"
  >
    <Spinner size="5" color="gray" />
    {#if message}
      <span class="text-sm text-stone-500 dark:text-gray-400">{message}</span>
    {/if}
  </div>
{:else}
  <!-- fullscreen (default) -->
  <div
    class="flex flex-col items-center justify-center py-16 px-6"
    role="status"
    aria-live="polite"
  >
    <div class="mb-4">
      <Spinner size="8" color="gray" />
    </div>
    {#if message}
      <p class="text-sm text-stone-500 dark:text-gray-400">{message}</p>
    {/if}
  </div>
{/if}
