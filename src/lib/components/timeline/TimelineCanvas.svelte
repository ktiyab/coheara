<!-- L4-04: Timeline SVG canvas — renders events, correlation lines, date axis. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type {
    TimelineEvent, TimelineCorrelation, DateRange, ZoomLevel,
  } from '$lib/types/timeline';
  import {
    calculateCanvasWidth, dateToX, eventToY,
    generateTicks, correlationPath, CANVAS_HEIGHT,
    MARKER_RADIUS, TOUCH_TARGET_RADIUS, HEADER_HEIGHT, PADDING_X,
    EVENT_COLORS, eventColorGroup, LANE_LABELS, LANE_HEIGHT, LANE_GAP, PADDING_Y,
  } from '$lib/utils/timeline';

  interface Props {
    events: TimelineEvent[];
    correlations: TimelineCorrelation[];
    dateRange: DateRange;
    zoom: ZoomLevel;
    sinceDate: string | null;
    onEventTap: (event: TimelineEvent, anchor: { x: number; y: number }) => void;
    selectedEventId: string | null;
  }
  let {
    events, correlations, dateRange, zoom, sinceDate,
    onEventTap, selectedEventId,
  }: Props = $props();

  let scrollContainer: HTMLDivElement | undefined = $state(undefined);

  let earliest = $derived(dateRange.earliest ? new Date(dateRange.earliest) : new Date());
  let latest = $derived(dateRange.latest ? new Date(dateRange.latest) : new Date());
  let canvasWidth = $derived(calculateCanvasWidth(earliest, latest, zoom));
  let ticks = $derived(generateTicks(earliest, latest, zoom));

  let sinceDateX = $derived(
    sinceDate ? dateToX(new Date(sinceDate), earliest, zoom) : null
  );

  /** Map event ID → {x, y} for correlation line endpoints */
  let eventPositions = $derived<Map<string, { x: number; y: number }>>(
    new Map(events.map(e => [
      e.id,
      {
        x: dateToX(new Date(e.date), earliest, zoom),
        y: eventToY(e.event_type),
      },
    ]))
  );

  function handleMarkerClick(event: TimelineEvent, svgEvent: MouseEvent) {
    const rect = scrollContainer?.getBoundingClientRect();
    if (!rect) return;
    onEventTap(event, {
      x: svgEvent.clientX - rect.left,
      y: svgEvent.clientY - rect.top,
    });
  }

  function handleMarkerKeydown(event: TimelineEvent, keyEvent: KeyboardEvent) {
    if (keyEvent.key === 'Enter' || keyEvent.key === ' ') {
      keyEvent.preventDefault();
      const target = keyEvent.target as SVGElement;
      const rect = target.getBoundingClientRect();
      const containerRect = scrollContainer?.getBoundingClientRect();
      if (!containerRect) return;
      onEventTap(event, {
        x: rect.left + rect.width / 2 - containerRect.left,
        y: rect.top + rect.height / 2 - containerRect.top,
      });
    }
  }

  function markerSymbol(eventType: string): string {
    switch (eventType) {
      case 'MedicationStart': return '+';
      case 'MedicationStop': return '\u00d7';
      case 'MedicationDoseChange': return '\u0394';
      default: return '';
    }
  }

  let showLabels = $derived(zoom === 'Day' || zoom === 'Week');
</script>

<div class="flex h-full">
  <!-- Fixed lane labels -->
  <div class="flex-shrink-0 w-24 bg-white border-r border-stone-200 z-10">
    <div style="height: {HEADER_HEIGHT}px" class="border-b border-stone-100"></div>
    {#each LANE_LABELS as label, i}
      <div
        class="flex items-center px-2 text-xs text-stone-500 font-medium"
        style="height: {LANE_HEIGHT + LANE_GAP}px; padding-top: {i === 0 ? PADDING_Y : 0}px"
      >
        {label}
      </div>
    {/each}
  </div>

  <!-- Scrollable SVG -->
  <div
    bind:this={scrollContainer}
    class="flex-1 overflow-x-auto overflow-y-hidden"
    role="application"
    aria-label="Medical timeline"
    aria-roledescription="Scrollable timeline of medical events"
  >
    <svg
      width={canvasWidth}
      height={CANVAS_HEIGHT}
      viewBox="0 0 {canvasWidth} {CANVAS_HEIGHT}"
      class="select-none"
    >
      <!-- "Since last visit" dimming overlay -->
      {#if sinceDateX !== null}
        <rect
          x="0" y={HEADER_HEIGHT}
          width={sinceDateX} height={CANVAS_HEIGHT - HEADER_HEIGHT}
          fill="#78716C" opacity="0.08"
        />
        <line
          x1={sinceDateX} y1={HEADER_HEIGHT}
          x2={sinceDateX} y2={CANVAS_HEIGHT}
          stroke="var(--color-interactive)" stroke-width="2" stroke-dasharray="6 4"
        />
        <text
          x={sinceDateX + 6} y={HEADER_HEIGHT + 14}
          class="text-xs font-medium"
          fill="var(--color-interactive)"
        >
          {$t('timeline.canvas_last_visit')}
        </text>
      {/if}

      <!-- Date axis ticks -->
      {#each ticks as tick}
        <g>
          <line
            x1={tick.x} y1={HEADER_HEIGHT - 4}
            x2={tick.x} y2={HEADER_HEIGHT}
            stroke="#D6D3D1" stroke-width="1"
          />
          <line
            x1={tick.x} y1={HEADER_HEIGHT}
            x2={tick.x} y2={CANVAS_HEIGHT}
            stroke="#F5F5F4" stroke-width="1"
          />
          <text
            x={tick.x} y={HEADER_HEIGHT - 8}
            text-anchor="middle"
            class="text-xs fill-stone-400"
          >
            {tick.label}
          </text>
        </g>
      {/each}

      <!-- Horizontal lane separator lines -->
      {#each Array(7) as _, laneIdx}
        <line
          x1={0}
          y1={HEADER_HEIGHT + PADDING_Y + laneIdx * (LANE_HEIGHT + LANE_GAP)}
          x2={canvasWidth}
          y2={HEADER_HEIGHT + PADDING_Y + laneIdx * (LANE_HEIGHT + LANE_GAP)}
          stroke="#F5F5F4" stroke-width="1"
        />
      {/each}

      <!-- Correlation lines (behind event markers) -->
      {#each correlations as corr}
        {@const source = eventPositions.get(corr.source_id)}
        {@const target = eventPositions.get(corr.target_id)}
        {#if source && target && (zoom === 'Day' || zoom === 'Week')}
          <path
            d={correlationPath(source.x, source.y, target.x, target.y)}
            class="fill-none stroke-stone-300 opacity-60"
            class:!stroke-orange-500={selectedEventId === corr.source_id || selectedEventId === corr.target_id}
            class:!opacity-100={selectedEventId === corr.source_id || selectedEventId === corr.target_id}
            stroke-width={selectedEventId === corr.source_id || selectedEventId === corr.target_id ? 2 : 1.5}
            stroke-dasharray={selectedEventId === corr.source_id || selectedEventId === corr.target_id ? 'none' : '4 3'}
            pointer-events="none"
          />
        {/if}
      {/each}

      <!-- Event markers -->
      {#each events as event}
        {@const pos = eventPositions.get(event.id)}
        {@const colorGroup = eventColorGroup(event.event_type)}
        {@const colors = EVENT_COLORS[colorGroup]}
        {#if pos && colors}
          <g
            role="button"
            tabindex="0"
            aria-label="{event.title} on {new Date(event.date).toLocaleDateString()}"
            onclick={(e) => handleMarkerClick(event, e)}
            onkeydown={(e) => handleMarkerKeydown(event, e)}
            class="cursor-pointer focus:outline-none"
            style="opacity: {sinceDate && pos.x < (sinceDateX ?? 0) ? 0.3 : 1}"
          >
            <!-- Invisible touch target (44px diameter) -->
            <circle
              cx={pos.x} cy={pos.y}
              r={TOUCH_TARGET_RADIUS}
              fill="transparent"
            />

            <!-- Selection ring -->
            {#if selectedEventId === event.id}
              <circle
                cx={pos.x} cy={pos.y}
                r={MARKER_RADIUS + 4}
                fill="none" stroke={colors.stroke} stroke-width="2" opacity="0.4"
              />
            {/if}

            <!-- Visible marker -->
            <circle
              cx={pos.x} cy={pos.y}
              r={MARKER_RADIUS}
              fill={colors.fill} stroke={colors.stroke} stroke-width="2"
            />

            <!-- Marker symbol for medication events -->
            {#if markerSymbol(event.event_type)}
              <text
                x={pos.x} y={pos.y + 4}
                text-anchor="middle"
                class="text-xs font-bold"
                fill={colors.stroke}
                pointer-events="none"
              >
                {markerSymbol(event.event_type)}
              </text>
            {/if}

            <!-- Label (only at Day/Week zoom) -->
            {#if showLabels}
              <text
                x={pos.x} y={pos.y + MARKER_RADIUS + 14}
                text-anchor="middle"
                class="text-xs fill-stone-600"
                pointer-events="none"
              >
                {event.title.length > 20 ? event.title.slice(0, 18) + '...' : event.title}
              </text>
            {/if}
          </g>
        {/if}
      {/each}
    </svg>
  </div>
</div>
