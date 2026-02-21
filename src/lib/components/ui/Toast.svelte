<!--
  C14: Toast — Shared UI Primitive
  Spec: 24-UX-COMPONENTS C14

  Transient notification for non-critical feedback.
  Auto-dismisses after duration. NOT for safety alerts (use CriticalAlertBanner).
-->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { CheckOutline, InfoCircleOutline, ExclamationCircleOutline, CloseOutline } from 'flowbite-svelte-icons';

  interface Props {
    message: string;
    severity?: 'info' | 'success' | 'warning' | 'error';
    duration?: number;
    ondismiss: () => void;
  }

  let {
    message,
    severity = 'info',
    duration,
    ondismiss,
  }: Props = $props();

  const defaultDurations: Record<string, number> = { info: 5000, success: 5000, warning: 8000, error: 0 };
  let effectiveDuration = $derived(duration ?? defaultDurations[severity] ?? 5000);

  $effect(() => {
    if (effectiveDuration > 0) {
      const timer = setTimeout(ondismiss, effectiveDuration);
      return () => clearTimeout(timer);
    }
  });

  const variantClasses: Record<string, { bg: string; border: string; text: string; icon: string }> = {
    info: { bg: 'bg-blue-50 dark:bg-blue-900/30', border: 'border-blue-200 dark:border-blue-800', text: 'text-blue-800 dark:text-blue-200', icon: 'text-blue-500 dark:text-blue-400' },
    success: { bg: 'bg-green-50 dark:bg-green-900/30', border: 'border-green-200 dark:border-green-800', text: 'text-green-800 dark:text-green-200', icon: 'text-green-500 dark:text-green-400' },
    warning: { bg: 'bg-amber-50 dark:bg-amber-900/30', border: 'border-amber-200 dark:border-amber-800', text: 'text-amber-800 dark:text-amber-200', icon: 'text-amber-500 dark:text-amber-400' },
    error: { bg: 'bg-red-50 dark:bg-red-900/30', border: 'border-red-200 dark:border-red-800', text: 'text-red-800 dark:text-red-200', icon: 'text-red-500 dark:text-red-400' },
  };

  let s = $derived(variantClasses[severity]);

  const iconMap: Record<string, typeof CheckOutline> = { info: InfoCircleOutline, success: CheckOutline, warning: ExclamationCircleOutline, error: ExclamationCircleOutline };
  let Icon = $derived(iconMap[severity]);
</script>

<div
  class="fixed top-[calc(var(--header-height)+0.5rem)] left-1/2 -translate-x-1/2 z-40
         {s.bg} border {s.border} rounded-xl px-5 py-3 shadow-lg
         max-w-sm w-full mx-4 flex items-center gap-3 animate-slide-down"
  role="status"
  aria-live="polite"
>
  <Icon class="w-4 h-4 flex-shrink-0 {s.icon}" />
  <p class="text-sm {s.text} flex-1">{message}</p>
  <button
    class="{s.icon} hover:opacity-80 min-h-[44px] min-w-[44px] flex items-center justify-center flex-shrink-0"
    onclick={ondismiss}
    aria-label={$t('common.dismiss')}
  >
    <CloseOutline class="w-4 h-4" />
  </button>
</div>
