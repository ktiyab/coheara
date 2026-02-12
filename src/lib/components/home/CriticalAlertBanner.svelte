<script lang="ts">
  interface AlertInfo {
    id: string;
    summary: string;
  }

  interface Props {
    alerts: AlertInfo[];
    onNavigate: (screen: string, params?: Record<string, string>) => void;
  }
  let { alerts, onNavigate }: Props = $props();
</script>

<div class="px-6 py-2">
  {#each alerts as alert}
    <div class="bg-amber-50 border border-amber-200 rounded-xl p-4 mb-2"
         role="alert">
      <div class="flex items-start gap-3">
        <span class="text-amber-600 mt-0.5 flex-shrink-0" aria-hidden="true">&#9888;</span>
        <div class="flex-1">
          <p class="text-sm text-amber-800">{alert.summary}</p>
          <button
            class="text-sm text-amber-700 font-medium mt-2 underline
                   min-h-[44px] min-w-[44px] -ml-1 px-1"
            onclick={() => onNavigate('alert-detail', { alertId: alert.id })}
          >
            View details
          </button>
        </div>
      </div>
    </div>
  {/each}
</div>
