<!-- Spec 45 [ON-04]: Recovery phrase with copy, print, optional verification -->
<script lang="ts">
  import { t } from 'svelte-i18n';
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
  let verifyError = $state(false);

  const inputClass = `px-4 py-3 rounded-lg border border-stone-300 dark:border-gray-600 text-lg min-h-[44px]
    bg-white dark:bg-gray-900 text-stone-800 dark:text-gray-100
    placeholder:text-stone-300 dark:placeholder:text-gray-600
    focus:border-[var(--color-primary)] focus:outline-none`;

  const btnInteractive = `inline-flex items-center justify-center gap-2 rounded-lg transition-colors
    px-4 py-2.5 min-h-[44px] text-sm font-medium w-full cursor-pointer
    focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--color-interactive)]`;

  const btnPrimary = `${btnInteractive}
    bg-[var(--color-interactive)] text-white
    hover:bg-[var(--color-interactive-hover)]
    active:bg-[var(--color-interactive-active)]
    disabled:bg-stone-300 disabled:dark:bg-gray-700 disabled:text-stone-500 disabled:dark:text-gray-500 disabled:cursor-not-allowed`;

  const btnGhost = `${btnInteractive}
    bg-transparent text-stone-600 dark:text-gray-300 border border-stone-200 dark:border-gray-700
    hover:bg-stone-50 dark:hover:bg-gray-800
    active:bg-stone-100 dark:active:bg-gray-700
    disabled:bg-stone-100 disabled:dark:bg-gray-800 disabled:text-stone-400 disabled:dark:text-gray-600 disabled:cursor-not-allowed`;

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
    verifyError = false;
    verifyMode = true;
  }

  function checkVerify() {
    const word1Ok = verifyInput1.trim().toLowerCase() === words[verifyPositions[0]].toLowerCase();
    const word2Ok = verifyInput2.trim().toLowerCase() === words[verifyPositions[1]].toLowerCase();
    if (word1Ok && word2Ok) {
      onConfirmed();
    } else {
      verifyError = true;
    }
  }

  function handleContinue() {
    if (copyTimer) clearTimeout(copyTimer);
    onConfirmed();
  }
</script>

<!-- Print-only section (hidden on screen) -->
<div class="hidden print:block print:p-8">
  <h1 class="text-xl font-bold mb-2">{$t('profile.recovery_print_heading')}</h1>
  {#if profileName}
    <p class="mb-1">{$t('profile.recovery_print_profile', { values: { name: profileName } })}</p>
  {/if}
  <p class="mb-4">{$t('profile.recovery_print_created', { values: { date: new Date().toLocaleDateString() } })}</p>
  <div class="grid grid-cols-3 gap-2">
    {#each words as word, i}
      <span>{i + 1}. {word}</span>
    {/each}
  </div>
  <p class="mt-4 text-sm">{$t('profile.recovery_print_warning')}</p>
</div>

<!-- Screen content (hidden when printing) -->
<div class="flex flex-col items-center px-8 gap-6 max-w-lg mx-auto w-full print:hidden">
  <h2 class="text-2xl font-bold text-stone-800 dark:text-gray-100">{$t('profile.recovery_heading')}</h2>

  <p class="text-stone-600 dark:text-gray-300 text-center leading-relaxed">
    {$t('profile.recovery_explanation')}
  </p>

  <!-- 12-word grid -->
  <div class="grid grid-cols-3 gap-3 w-full p-6 bg-white dark:bg-gray-900 rounded-xl border border-stone-200 dark:border-gray-700 shadow-sm">
    {#each words as word, i}
      <div class="flex items-center gap-1.5 px-2 py-2 bg-stone-50 dark:bg-gray-950 rounded-lg overflow-hidden">
        <span class="text-stone-400 dark:text-gray-500 text-xs w-5 text-right flex-shrink-0">{i + 1}.</span>
        <span class="text-stone-800 dark:text-gray-100 font-mono text-sm truncate">{word}</span>
      </div>
    {/each}
  </div>

  <!-- Copy & Print actions -->
  <div class="flex gap-3 w-full">
    <button class={btnGhost} onclick={copyToClipboard}>
      {copied ? $t('profile.recovery_copied') : $t('profile.recovery_copy')}
    </button>
    <button class={btnGhost} onclick={handlePrint}>
      {$t('profile.recovery_print')}
    </button>
  </div>

  <!-- Clipboard warning — always holds space -->
  <p class="text-xs text-stone-400 dark:text-gray-500 text-center {copied ? '' : 'invisible'}">
    {$t('profile.recovery_clipboard_warning')}
  </p>

  {#if !verifyMode}
    <div class="flex flex-col gap-3 w-full">
      <p class="text-stone-500 dark:text-gray-400 text-sm text-center">
        {$t('profile.recovery_warning')}
      </p>

      <label class="flex items-center gap-3 justify-center cursor-pointer">
        <input
          type="checkbox"
          bind:checked={confirmed}
          class="w-[44px] h-[44px] rounded-none border-stone-300 dark:border-gray-600
                 text-[var(--color-interactive)] focus:ring-[var(--color-interactive)]
                 bg-white dark:bg-gray-900 flex-shrink-0"
        />
        <span class="text-stone-700 dark:text-gray-200 text-sm">{$t('profile.recovery_confirm_checkbox')}</span>
      </label>

      <div class="flex gap-3 w-full">
        <button class={btnGhost} disabled={!confirmed} onclick={startVerify}>
          {$t('profile.recovery_verify_btn')}
        </button>
        <button class={btnPrimary} disabled={!confirmed} onclick={handleContinue}>
          {$t('common.continue')}
        </button>
      </div>
    </div>
  {:else}
    <!-- Verification mode -->
    <div class="flex flex-col gap-4 w-full p-5 bg-white dark:bg-gray-900 rounded-xl border border-stone-200 dark:border-gray-700">
      <p class="text-stone-600 dark:text-gray-300 text-sm text-center">{$t('profile.recovery_verify_heading')}</p>

      <label class="flex flex-col gap-1">
        <span class="text-stone-600 dark:text-gray-400 text-sm font-medium">
          {$t('profile.recovery_verify_word', { values: { num: verifyPositions[0] + 1 } })}
        </span>
        <input
          type="text"
          bind:value={verifyInput1}
          class={inputClass}
          autocomplete="off"
          autocapitalize="off"
        />
      </label>

      <label class="flex flex-col gap-1">
        <span class="text-stone-600 dark:text-gray-400 text-sm font-medium">
          {$t('profile.recovery_verify_word', { values: { num: verifyPositions[1] + 1 } })}
        </span>
        <input
          type="text"
          bind:value={verifyInput2}
          class={inputClass}
          autocomplete="off"
          autocapitalize="off"
        />
      </label>

      <!-- Verify error — always holds space -->
      <p class="text-[var(--color-danger)] text-sm text-center {verifyError ? '' : 'invisible'}">
        {$t('profile.recovery_verify_error')}
      </p>

      <div class="flex gap-3">
        <button class={btnGhost} onclick={handleContinue}>
          {$t('tour.skip')}
        </button>
        <button class={btnPrimary} onclick={checkVerify}
                disabled={!verifyInput1.trim() || !verifyInput2.trim()}>
          {$t('profile.recovery_verify_submit')}
        </button>
      </div>
    </div>
  {/if}
</div>
