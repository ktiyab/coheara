<!--
  C3: LoadingState — Shared UI Primitive
  Spec: 24-UX-COMPONENTS C3
  Replaces: 14 identical loading indicators

  3 variants: fullscreen, inline, skeleton
-->
<script lang="ts">
  import { t } from 'svelte-i18n';

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
    class="space-y-3"
    role="status"
    aria-live="polite"
    aria-label={message ?? $t('common.loading')}
  >
    {#each Array(lines) as _, i}
      <div
        class="h-4 bg-stone-200 rounded animate-pulse"
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
    <div class="flex items-center gap-1" aria-hidden="true">
      <span class="w-2 h-2 rounded-full bg-stone-400 animate-pulse"></span>
      <span class="w-2 h-2 rounded-full bg-stone-400 animate-pulse [animation-delay:150ms]"></span>
      <span class="w-2 h-2 rounded-full bg-stone-400 animate-pulse [animation-delay:300ms]"></span>
    </div>
    {#if message}
      <span class="text-sm text-stone-500">{message}</span>
    {/if}
  </div>
{:else}
  <!-- fullscreen (default) -->
  <div
    class="flex flex-col items-center justify-center py-16 px-6"
    role="status"
    aria-live="polite"
  >
    <div class="flex items-center gap-1.5 mb-4" aria-hidden="true">
      <span class="w-2.5 h-2.5 rounded-full bg-stone-400 animate-pulse"></span>
      <span class="w-2.5 h-2.5 rounded-full bg-stone-400 animate-pulse [animation-delay:150ms]"></span>
      <span class="w-2.5 h-2.5 rounded-full bg-stone-400 animate-pulse [animation-delay:300ms]"></span>
    </div>
    {#if message}
      <p class="text-sm text-stone-500">{message}</p>
    {/if}
  </div>
{/if}
