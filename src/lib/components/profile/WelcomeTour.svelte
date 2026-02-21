<!-- Spec 45 [ON-02]: 3-slide welcome tour after recovery phrase -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import { FileCirclePlusOutline, SearchOutline, CalendarEditOutline } from 'flowbite-svelte-icons';
  import type { Component } from 'svelte';

  interface Props {
    onComplete: () => void;
  }
  let { onComplete }: Props = $props();

  let currentSlide = $state(0);

  type Slide = { titleKey: string; descKey: string; Icon: Component<{class?: string}> };
  const slides: Slide[] = [
    { titleKey: 'tour.slide1_title', descKey: 'tour.slide1_desc', Icon: FileCirclePlusOutline },
    { titleKey: 'tour.slide2_title', descKey: 'tour.slide2_desc', Icon: SearchOutline },
    { titleKey: 'tour.slide3_title', descKey: 'tour.slide3_desc', Icon: CalendarEditOutline },
  ];

  const btnPrimary = `inline-flex items-center justify-center gap-2 rounded-lg transition-colors
    px-4 py-2.5 min-h-[44px] text-sm font-medium w-full cursor-pointer
    focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--color-interactive)]
    bg-[var(--color-interactive)] text-white
    hover:bg-[var(--color-interactive-hover)]
    active:bg-[var(--color-interactive-active)]`;

  const btnGhost = `inline-flex items-center justify-center gap-2 rounded-lg transition-colors
    px-4 py-2.5 min-h-[44px] text-sm font-medium cursor-pointer
    focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--color-interactive)]
    bg-transparent text-stone-600 dark:text-gray-300 border border-stone-200 dark:border-gray-700
    hover:bg-stone-50 dark:hover:bg-gray-800
    active:bg-stone-100 dark:active:bg-gray-700`;

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
      class="text-sm text-stone-400 dark:text-gray-500 hover:text-stone-600 dark:hover:text-gray-300 min-h-[44px] min-w-[44px]
             flex items-center justify-center cursor-pointer"
      onclick={onComplete}
    >
      {$t('tour.skip')}
    </button>
  </div>

  <!-- Slide content -->
  {#each [slides[currentSlide]] as slide (currentSlide)}
    <div class="flex flex-col items-center gap-6 text-center">
      <slide.Icon class="w-16 h-16 text-[var(--color-interactive)]" />
      <h2 class="text-2xl font-bold text-stone-800 dark:text-gray-100">
        {$t(slide.titleKey)}
      </h2>
      <p class="text-stone-600 dark:text-gray-400 leading-relaxed max-w-sm">
        {$t(slide.descKey)}
      </p>
    </div>
  {/each}

  <!-- Dot indicators -->
  <div class="flex gap-2" role="tablist" aria-label={$t('tour.progress')}>
    {#each slides as _, i}
      <span
        class="w-2.5 h-2.5 rounded-full transition-colors {i === currentSlide
          ? 'bg-[var(--color-interactive)]'
          : 'bg-stone-300 dark:bg-gray-600'}"
        role="tab"
        aria-selected={i === currentSlide}
        aria-label="{i + 1} / {slides.length}"
      ></span>
    {/each}
  </div>

  <!-- Navigation -->
  <div class="flex gap-3 w-full">
    {#if currentSlide > 0}
      <button class={btnGhost} onclick={prev}>
        {$t('common.back')}
      </button>
    {/if}
    <div class="flex-1">
      <button class={btnPrimary} onclick={next}>
        {currentSlide < slides.length - 1 ? $t('common.continue') : $t('tour.done')}
      </button>
    </div>
  </div>
</div>
