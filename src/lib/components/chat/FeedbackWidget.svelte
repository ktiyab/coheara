<script lang="ts">
  import { t } from 'svelte-i18n';
  import { setMessageFeedback } from '$lib/api/chat';

  interface Props {
    messageId: string;
    currentFeedback: 'Helpful' | 'NotHelpful' | null;
  }
  let { messageId, currentFeedback }: Props = $props();

  let feedback: 'Helpful' | 'NotHelpful' | null = $state(currentFeedback);
  let showThankYou = $state(false);
  let saving = $state(false);

  async function handleFeedback(value: 'Helpful' | 'NotHelpful') {
    if (saving) return;
    saving = true;

    try {
      if (feedback === value) {
        feedback = null;
        await setMessageFeedback(messageId, null);
      } else {
        feedback = value;
        await setMessageFeedback(messageId, value);
        showThankYou = true;
        setTimeout(() => { showThankYou = false; }, 2000);
      }
    } catch (e) {
      console.error('Failed to save feedback:', e);
    } finally {
      saving = false;
    }
  }
</script>

<div class="flex items-center gap-2">
  <span class="text-xs text-stone-500">{$t('chat.feedback_question')}</span>

  <button
    class="min-h-[32px] min-w-[32px] flex items-center justify-center rounded-full
           transition-all
           {feedback === 'Helpful'
             ? 'bg-[var(--color-success-50)] text-[var(--color-success)]'
             : feedback === 'NotHelpful'
               ? 'opacity-30 text-stone-500'
               : 'text-stone-500 hover:bg-stone-100'}"
    onclick={() => handleFeedback('Helpful')}
    aria-label={$t('chat.feedback_helpful')}
    aria-pressed={feedback === 'Helpful'}
    disabled={saving}
  >
    <span class="text-sm" aria-hidden="true">&#128077;</span>
  </button>

  <button
    class="min-h-[32px] min-w-[32px] flex items-center justify-center rounded-full
           transition-all
           {feedback === 'NotHelpful'
             ? 'bg-stone-100 text-stone-600'
             : feedback === 'Helpful'
               ? 'opacity-30 text-stone-500'
               : 'text-stone-500 hover:bg-stone-100'}"
    onclick={() => handleFeedback('NotHelpful')}
    aria-label={$t('chat.feedback_not_helpful')}
    aria-pressed={feedback === 'NotHelpful'}
    disabled={saving}
  >
    <span class="text-sm" aria-hidden="true">&#128078;</span>
  </button>

  {#if showThankYou}
    <span class="text-xs text-stone-500 animate-fade-out">
      {feedback === 'Helpful' ? $t('chat.feedback_thanks_positive') : $t('chat.feedback_thanks_negative')}
    </span>
  {/if}
</div>

<style>
  @keyframes fade-out {
    0% { opacity: 1; }
    70% { opacity: 1; }
    100% { opacity: 0; }
  }
  .animate-fade-out {
    animation: fade-out 2s ease-out forwards;
  }
</style>
