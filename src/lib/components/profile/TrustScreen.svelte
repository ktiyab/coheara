<script lang="ts">
  interface Props {
    onContinue: () => void;
  }
  let { onContinue }: Props = $props();
</script>

<div class="flex flex-col items-center justify-center min-h-screen px-8 gap-8 max-w-lg mx-auto">
  <h1 class="text-3xl font-bold text-stone-800">Welcome to Coheara</h1>
  <p class="text-lg text-stone-600 text-center leading-relaxed">
    Your personal medical document assistant.
  </p>

  <div class="flex flex-col gap-4 text-stone-600 text-base">
    <div class="flex items-start gap-3">
      <span class="text-green-600 mt-1">&#x2713;</span>
      <p>Coheara runs entirely on this computer. Your medical documents are <strong>never sent anywhere</strong>.</p>
    </div>
    <div class="flex items-start gap-3">
      <span class="text-green-600 mt-1">&#x2713;</span>
      <p>No internet connection is needed after installation. <strong>No account required.</strong></p>
    </div>
    <div class="flex items-start gap-3">
      <span class="text-green-600 mt-1">&#x2713;</span>
      <p>Your data is <strong>encrypted</strong> and only you can access it with your password.</p>
    </div>
    <div class="flex items-start gap-3">
      <span class="text-green-600 mt-1">&#x2713;</span>
      <p>Coheara helps you <strong>understand</strong> your health documents. It does <strong>not</strong> give medical advice.</p>
    </div>
  </div>

  <button
    class="mt-4 px-8 py-4 bg-[var(--color-primary)] text-white rounded-xl text-lg font-medium
           hover:brightness-110 focus-visible:outline focus-visible:outline-2
           focus-visible:outline-offset-2 focus-visible:outline-[var(--color-primary)]
           min-h-[44px] min-w-[44px]"
    onclick={onContinue}
  >
    I understand, let's begin
  </button>
</div>
