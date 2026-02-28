<!--
  BTL-10 C6: DeleteConfirmModal — Confirmation dialog for document deletion.
  Uses existing Modal primitive (C11). Destructive action = requires explicit confirmation (CP10).
-->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import Modal from '$lib/components/ui/Modal.svelte';
  import Button from '$lib/components/ui/Button.svelte';

  interface Props {
    open: boolean;
    filename: string;
    loading?: boolean;
    onconfirm: () => void;
    onclose: () => void;
  }

  let { open, filename, loading = false, onconfirm, onclose }: Props = $props();
</script>

<Modal {open} title={$t('documents.delete_confirm_title')} {onclose}>
  <p class="text-sm text-stone-600 dark:text-gray-300">
    {$t('documents.delete_confirm_message', { values: { filename } })}
  </p>
  <p class="text-xs text-stone-500 dark:text-gray-400 mt-2">
    {$t('documents.delete_confirm_warning')}
  </p>

  {#snippet actions()}
    <Button variant="ghost" size="sm" onclick={onclose} disabled={loading}>
      {$t('common.cancel')}
    </Button>
    <Button variant="danger" size="sm" onclick={onconfirm} {loading}>
      {$t('documents.delete_confirm_action')}
    </Button>
  {/snippet}
</Modal>
