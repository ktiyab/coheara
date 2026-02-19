<!--
  C14: Toast — Shared UI Primitive
  Spec: 24-UX-COMPONENTS C14

  Transient notification for non-critical feedback.
  Auto-dismisses after duration. NOT for safety alerts (use CriticalAlertBanner).
-->
<script lang="ts">
  import { t } from 'svelte-i18n';

  interface Props {
    message: string;
    severity?: 'info' | 'success' | 'warning';
    duration?: number;
    ondismiss: () => void;
  }

  let {
    message,
    severity = 'info',
    duration = 5000,
    ondismiss,
  }: Props = $props();

  $effect(() => {
    if (duration > 0) {
      const timer = setTimeout(ondismiss, duration);
      return () => clearTimeout(timer);
    }
  });

  const variantClasses: Record<string, { bg: string; border: string; text: string; icon: string }> = {
    info: { bg: 'bg-blue-50', border: 'border-blue-200', text: 'text-blue-800', icon: 'text-blue-500' },
    success: { bg: 'bg-green-50', border: 'border-green-200', text: 'text-green-800', icon: 'text-green-500' },
    warning: { bg: 'bg-amber-50', border: 'border-amber-200', text: 'text-amber-800', icon: 'text-amber-500' },
  };

  let s = $derived(variantClasses[severity]);

  const icons: Record<string, string> = {
    info: 'i',
    success: '\u2713',
    warning: '!',
  };
</script>

<div
  class="fixed top-4 left-1/2 -translate-x-1/2 z-40
         {s.bg} border {s.border} rounded-xl px-5 py-3 shadow-lg
         max-w-sm w-full mx-4 flex items-center gap-3 animate-slide-down"
  role="status"
  aria-live="polite"
>
  <span class="{s.icon} flex-shrink-0" aria-hidden="true">{icons[severity]}</span>
  <p class="text-sm {s.text} flex-1">{message}</p>
  <button
    class="{s.icon} hover:opacity-80 min-h-[44px] min-w-[44px] flex items-center justify-center flex-shrink-0"
    onclick={ondismiss}
    aria-label={$t('common.dismiss')}
  >
    &times;
  </button>
</div>
