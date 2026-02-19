<!-- Spec 45 [ON-02]: 3-slide welcome tour after recovery phrase -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import Button from '$lib/components/ui/Button.svelte';

  interface Props {
    onComplete: () => void;
  }
  let { onComplete }: Props = $props();

  let currentSlide = $state(0);

  const slides = [
    { titleKey: 'tour.slide1_title', descKey: 'tour.slide1_desc', icon: '&#x1F4C4;' },
    { titleKey: 'tour.slide2_title', descKey: 'tour.slide2_desc', icon: '&#x1F4AC;' },
    { titleKey: 'tour.slide3_title', descKey: 'tour.slide3_desc', icon: '&#x1F4CB;' },
  ] as const;

  function next() {
    if (currentSlide < slides.length - 1) {
      currentSlide++;
    } else {
      onComplete();
    }
  }

  function prev() {
    if (currentSlide > 0) {
      currentSlide--;
    }
  }
</script>

<div class="flex flex-col items-center justify-center min-h-screen px-8 gap-8 max-w-md mx-auto">
  <!-- Skip button -->
  <div class="absolute top-6 right-6">
    <button
      class="text-sm text-stone-400 hover:text-stone-600 min-h-[44px] min-w-[44px]
             flex items-center justify-center"
      onclick={onComplete}
    >
      {$t('tour.skip')}
    </button>
  </div>

  <!-- Slide content -->
  <div class="flex flex-col items-center gap-6 text-center">
    <span class="text-6xl" aria-hidden="true">{@html slides[currentSlide].icon}</span>
    <h2 class="text-2xl font-bold text-stone-800">
      {$t(slides[currentSlide].titleKey)}
    </h2>
    <p class="text-stone-600 leading-relaxed max-w-sm">
      {$t(slides[currentSlide].descKey)}
    </p>
  </div>

  <!-- Dot indicators -->
  <div class="flex gap-2" role="tablist" aria-label={$t('tour.progress')}>
    {#each slides as _, i}
      <span
        class="w-2.5 h-2.5 rounded-full transition-colors {i === currentSlide
          ? 'bg-[var(--color-primary)]'
          : 'bg-stone-300'}"
        role="tab"
        aria-selected={i === currentSlide}
        aria-label="{i + 1} / {slides.length}"
      ></span>
    {/each}
  </div>

  <!-- Navigation -->
  <div class="flex gap-3 w-full">
    {#if currentSlide > 0}
      <Button variant="ghost" onclick={prev}>
        {$t('common.back')}
      </Button>
    {/if}
    <div class="flex-1">
      <Button variant="primary" fullWidth onclick={next}>
        {currentSlide < slides.length - 1 ? $t('common.continue') : $t('tour.done')}
      </Button>
    </div>
  </div>
</div>
