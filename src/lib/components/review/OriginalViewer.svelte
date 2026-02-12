<!-- L3-04: Original document viewer with zoom, pan, rotate (image/PDF). -->
<script lang="ts">
  interface Props {
    fileBase64: string | null;
    fileType: 'Image' | 'Pdf';
  }
  let { fileBase64, fileType }: Props = $props();

  let zoom = $state(1.0);
  let panX = $state(0);
  let panY = $state(0);
  let isDragging = $state(false);
  let dragStartX = $state(0);
  let dragStartY = $state(0);
  let rotation = $state(0);
  let currentPage = $state(1);
  let totalPages = $state(1);

  function zoomIn() {
    zoom = Math.min(zoom + 0.25, 5.0);
  }

  function zoomOut() {
    zoom = Math.max(zoom - 0.25, 0.25);
  }

  function fitToWidth() {
    zoom = 1.0;
    panX = 0;
    panY = 0;
  }

  function rotate90() {
    rotation = (rotation + 90) % 360;
  }

  function handleWheel(e: WheelEvent) {
    e.preventDefault();
    if (e.deltaY < 0) zoomIn();
    else zoomOut();
  }

  function handleMouseDown(e: MouseEvent) {
    if (zoom > 1.0) {
      isDragging = true;
      dragStartX = e.clientX - panX;
      dragStartY = e.clientY - panY;
    }
  }

  function handleMouseMove(e: MouseEvent) {
    if (isDragging) {
      panX = e.clientX - dragStartX;
      panY = e.clientY - dragStartY;
    }
  }

  function handleMouseUp() {
    isDragging = false;
  }

  let mimePrefix = $derived(
    fileType === 'Pdf' ? 'data:application/pdf;base64,' : 'data:image/jpeg;base64,'
  );

  let dataUrl = $derived(fileBase64 ? `${mimePrefix}${fileBase64}` : null);
</script>

<div class="flex flex-col h-full">
  <!-- Toolbar -->
  <div class="flex items-center gap-2 px-3 py-2 bg-stone-100 border-b border-stone-200 shrink-0">
    <button
      class="min-h-[44px] min-w-[44px] flex items-center justify-center
             rounded-lg hover:bg-stone-200 text-stone-600"
      onclick={zoomOut}
      aria-label="Zoom out"
    >
      &minus;
    </button>
    <span class="text-sm text-stone-500 w-12 text-center">{Math.round(zoom * 100)}%</span>
    <button
      class="min-h-[44px] min-w-[44px] flex items-center justify-center
             rounded-lg hover:bg-stone-200 text-stone-600"
      onclick={zoomIn}
      aria-label="Zoom in"
    >
      +
    </button>
    <button
      class="min-h-[44px] min-w-[44px] flex items-center justify-center
             rounded-lg hover:bg-stone-200 text-stone-600 text-xs"
      onclick={fitToWidth}
      aria-label="Fit to width"
    >
      Fit
    </button>
    <button
      class="min-h-[44px] min-w-[44px] flex items-center justify-center
             rounded-lg hover:bg-stone-200 text-stone-600 text-xs"
      onclick={rotate90}
      aria-label="Rotate 90 degrees"
    >
      &#8635;
    </button>

    {#if fileType === 'Pdf'}
      <div class="ml-auto flex items-center gap-2">
        <button
          class="min-h-[44px] min-w-[44px] flex items-center justify-center
                 rounded-lg hover:bg-stone-200 text-stone-600 text-xs"
          onclick={() => currentPage = Math.max(1, currentPage - 1)}
          disabled={currentPage <= 1}
          aria-label="Previous page"
        >
          &lt;
        </button>
        <span class="text-sm text-stone-500">{currentPage} / {totalPages}</span>
        <button
          class="min-h-[44px] min-w-[44px] flex items-center justify-center
                 rounded-lg hover:bg-stone-200 text-stone-600 text-xs"
          onclick={() => currentPage = Math.min(totalPages, currentPage + 1)}
          disabled={currentPage >= totalPages}
          aria-label="Next page"
        >
          &gt;
        </button>
      </div>
    {/if}
  </div>

  <!-- Viewer area -->
  <div
    class="flex-1 overflow-hidden bg-stone-200 flex items-center justify-center
           {isDragging ? 'cursor-grabbing' : zoom > 1.0 ? 'cursor-grab' : 'cursor-default'}"
    onwheel={handleWheel}
    onmousedown={handleMouseDown}
    onmousemove={handleMouseMove}
    onmouseup={handleMouseUp}
    onmouseleave={handleMouseUp}
    role="img"
    aria-label="Original document viewer"
  >
    {#if !dataUrl}
      <p class="text-stone-400">Loading document...</p>
    {:else if fileType === 'Image'}
      <img
        src={dataUrl}
        alt="Original document"
        class="max-w-full max-h-full object-contain select-none"
        style="transform: scale({zoom}) rotate({rotation}deg) translate({panX / zoom}px, {panY / zoom}px);
               transform-origin: center center;
               transition: {isDragging ? 'none' : 'transform 0.15s ease'};"
        draggable="false"
      />
    {:else}
      <iframe
        src={dataUrl}
        title="Original PDF document"
        class="w-full h-full border-none"
        style="transform: scale({zoom}); transform-origin: top left;"
      ></iframe>
    {/if}
  </div>
</div>
