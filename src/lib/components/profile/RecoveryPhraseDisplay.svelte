<!-- Spec 45 [ON-04]: Recovery phrase with copy, print, optional verification -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import Button from '$lib/components/ui/Button.svelte';

  interface Props {
    words: string[];
    profileName?: string;
    onConfirmed: () => void;
  }
  let { words, profileName, onConfirmed }: Props = $props();

  let confirmed = $state(false);
  let copied = $state(false);
  let copyTimer: ReturnType<typeof setTimeout> | null = null;

  // Verification state
  let verifyMode = $state(false);
  let verifyPositions = $state<[number, number]>([0, 0]);
  let verifyInput1 = $state('');
  let verifyInput2 = $state('');
  let verifyError = $state('');

  function generateVerifyPositions(): [number, number] {
    const pos1 = Math.floor(Math.random() * 12);
    let pos2 = Math.floor(Math.random() * 11);
    if (pos2 >= pos1) pos2++;
    return [pos1, pos2].sort((a, b) => a - b) as [number, number];
  }

  async function copyToClipboard() {
    try {
      await navigator.clipboard.writeText(words.join(' '));
      copied = true;
      // Auto-clear clipboard after 60 seconds (security)
      copyTimer = setTimeout(async () => {
        try { await navigator.clipboard.writeText(''); } catch { /* best effort */ }
        copied = false;
      }, 60_000);
    } catch {
      // Clipboard API not available
    }
  }

  function handlePrint() {
    window.print();
  }

  function startVerify() {
    verifyPositions = generateVerifyPositions();
    verifyInput1 = '';
    verifyInput2 = '';
    verifyError = '';
    verifyMode = true;
  }

  function checkVerify() {
    const word1Ok = verifyInput1.trim().toLowerCase() === words[verifyPositions[0]].toLowerCase();
    const word2Ok = verifyInput2.trim().toLowerCase() === words[verifyPositions[1]].toLowerCase();
    if (word1Ok && word2Ok) {
      onConfirmed();
    } else {
      verifyError = $t('profile.recovery_verify_error');
    }
  }

  function handleContinue() {
    if (copyTimer) clearTimeout(copyTimer);
    onConfirmed();
  }
</script>

<!-- Print-only section (hidden on screen) -->
<div class="hidden print:block print:p-8">
  <h1 class="text-xl font-bold mb-2">COHEARA RECOVERY PHRASE</h1>
  {#if profileName}
    <p class="mb-1">Profile: {profileName}</p>
  {/if}
  <p class="mb-4">Created: {new Date().toLocaleDateString()}</p>
  <div class="grid grid-cols-3 gap-2">
    {#each words as word, i}
      <span>{i + 1}. {word}</span>
    {/each}
  </div>
  <p class="mt-4 text-sm">Keep this page in a safe place. Anyone with these words can recover this profile.</p>
</div>

<!-- Screen content (hidden when printing) -->
<div class="flex flex-col items-center justify-center min-h-screen px-8 gap-6 max-w-lg mx-auto print:hidden">
  <h2 class="text-2xl font-bold text-stone-800">{$t('profile.recovery_heading')}</h2>

  <p class="text-stone-600 text-center leading-relaxed">
    {$t('profile.recovery_explanation')}
  </p>

  <div class="grid grid-cols-3 gap-3 w-full p-6 bg-white rounded-xl border border-stone-200 shadow-sm">
    {#each words as word, i}
      <div class="flex items-center gap-2 p-2 bg-stone-50 rounded-lg">
        <span class="text-stone-500 text-sm w-5 text-right">{i + 1}.</span>
        <span class="text-stone-800 font-mono text-lg">{word}</span>
      </div>
    {/each}
  </div>

  <!-- Copy & Print actions -->
  <div class="flex gap-3 w-full">
    <Button variant="ghost" fullWidth onclick={copyToClipboard}>
      {copied ? $t('profile.recovery_copied') : $t('profile.recovery_copy')}
    </Button>
    <Button variant="ghost" fullWidth onclick={handlePrint}>
      {$t('profile.recovery_print')}
    </Button>
  </div>

  {#if copied}
    <p class="text-xs text-stone-400 text-center">
      {$t('profile.recovery_clipboard_warning')}
    </p>
  {/if}

  {#if !verifyMode}
    <div class="flex flex-col gap-3 w-full mt-2">
      <p class="text-stone-500 text-sm text-center">
        {$t('profile.recovery_warning')}
      </p>

      <label class="flex items-center gap-3 justify-center">
        <input type="checkbox" bind:checked={confirmed}
               class="min-h-[44px] min-w-[44px]" />
        <span class="text-stone-700">{$t('profile.recovery_confirm_checkbox')}</span>
      </label>

      <div class="flex gap-3 w-full">
        <Button variant="ghost" fullWidth disabled={!confirmed} onclick={startVerify}>
          {$t('profile.recovery_verify_btn')}
        </Button>
        <Button variant="primary" fullWidth disabled={!confirmed} onclick={handleContinue}>
          {$t('common.continue')}
        </Button>
      </div>
    </div>
  {:else}
    <!-- Verification mode -->
    <div class="flex flex-col gap-4 w-full mt-2 p-5 bg-white rounded-xl border border-stone-200">
      <p class="text-stone-600 text-sm text-center">{$t('profile.recovery_verify_heading')}</p>

      <label class="flex flex-col gap-1">
        <span class="text-stone-600 text-sm font-medium">
          {$t('profile.recovery_verify_word', { values: { num: verifyPositions[0] + 1 } })}
        </span>
        <input
          type="text"
          bind:value={verifyInput1}
          class="px-4 py-3 rounded-lg border border-stone-300 min-h-[44px]
                 focus:border-[var(--color-primary)] focus:outline-none"
          autocomplete="off"
          autocapitalize="off"
        />
      </label>

      <label class="flex flex-col gap-1">
        <span class="text-stone-600 text-sm font-medium">
          {$t('profile.recovery_verify_word', { values: { num: verifyPositions[1] + 1 } })}
        </span>
        <input
          type="text"
          bind:value={verifyInput2}
          class="px-4 py-3 rounded-lg border border-stone-300 min-h-[44px]
                 focus:border-[var(--color-primary)] focus:outline-none"
          autocomplete="off"
          autocapitalize="off"
        />
      </label>

      {#if verifyError}
        <p class="text-[var(--color-danger)] text-sm text-center">{verifyError}</p>
      {/if}

      <div class="flex gap-3">
        <Button variant="ghost" fullWidth onclick={handleContinue}>
          {$t('tour.skip')}
        </Button>
        <Button variant="primary" fullWidth onclick={checkVerify}
                disabled={!verifyInput1.trim() || !verifyInput2.trim()}>
          {$t('profile.recovery_verify_submit')}
        </Button>
      </div>
    </div>
  {/if}
</div>
