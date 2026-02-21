<!-- V8-B7: Smartphone companion surface card — points to existing pairing feature -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { navigation } from '$lib/stores/navigation.svelte';
  import { MobilePhoneOutline, ArrowRightOutline } from 'flowbite-svelte-icons';

  interface Props {
    /** Hide this card when the user already has paired devices */
    hasPairedDevices?: boolean;
  }
  let { hasPairedDevices = false }: Props = $props();
</script>

{#if !hasPairedDevices}
  <div class="px-6 mt-4 mb-6">
    <button
      class="w-full bg-stone-50 dark:bg-gray-900 border border-stone-200 dark:border-gray-700
             rounded-xl p-5 flex items-center gap-4 text-left
             hover:bg-stone-100 dark:hover:bg-gray-800 transition-colors cursor-pointer"
      onclick={() => navigation.navigate('pairing')}
    >
      <!-- Icon -->
      <div class="w-10 h-10 rounded-lg bg-stone-100 dark:bg-gray-800 flex items-center justify-center flex-shrink-0">
        <MobilePhoneOutline class="w-5 h-5 text-stone-500 dark:text-gray-400" />
      </div>

      <!-- Text -->
      <div class="flex-1 min-w-0">
        <h3 class="text-sm font-semibold text-stone-800 dark:text-gray-100">
          {$t('home.companion_title')}
        </h3>
        <p class="text-xs text-stone-500 dark:text-gray-400 mt-0.5 leading-relaxed">
          {$t('home.companion_body')}
        </p>
      </div>

      <!-- Arrow -->
      <ArrowRightOutline class="w-4 h-4 text-stone-400 dark:text-gray-500 flex-shrink-0" />
    </button>
  </div>
{/if}
