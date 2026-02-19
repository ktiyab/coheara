<!-- L3-05: Empty state when no medications are recorded. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import Button from '$lib/components/ui/Button.svelte';

  interface Props {
    onAddOtc: () => void;
  }
  let { onAddOtc }: Props = $props();
</script>

<div class="flex flex-col items-center justify-center px-8 py-12 text-center">
  <div class="w-24 h-24 bg-stone-100 rounded-2xl flex items-center justify-center mb-6">
    <span class="text-4xl text-stone-300" aria-hidden="true">&#x1F48A;</span>
  </div>

  <h2 class="text-lg font-medium text-stone-700 mb-2">
    {$t('medications.empty_title')}
  </h2>
  <p class="text-sm text-stone-500 mb-6 max-w-[300px]">
    {$t('medications.empty_description')}
  </p>

  <Button variant="primary" size="lg" onclick={onAddOtc}>
    {$t('medications.empty_add_otc')}
  </Button>
</div>
