<!-- REVIEW-01: Pure SVG sparkline chart (Apple Health pattern). No external dependencies. -->
<script lang="ts">
  let {
    points = [],
    width = 80,
    height = 24,
    color = 'var(--color-interactive)',
  }: {
    points?: number[];
    width?: number;
    height?: number;
    color?: string;
  } = $props();

  let pathD = $derived.by(() => {
    if (points.length < 2) return '';
    const min = Math.min(...points);
    const max = Math.max(...points);
    const range = max - min || 1;
    const pad = 2;
    const w = width - pad * 2;
    const h = height - pad * 2;
    const step = w / (points.length - 1);

    return points
      .map((v, i) => {
        const x = pad + i * step;
        const y = pad + h - ((v - min) / range) * h;
        return `${i === 0 ? 'M' : 'L'}${x.toFixed(1)},${y.toFixed(1)}`;
      })
      .join(' ');
  });

  let lastDot = $derived.by(() => {
    if (points.length < 2) return null;
    const min = Math.min(...points);
    const max = Math.max(...points);
    const range = max - min || 1;
    const pad = 2;
    const w = width - pad * 2;
    const h = height - pad * 2;
    const step = w / (points.length - 1);
    const last = points[points.length - 1];
    return {
      x: pad + (points.length - 1) * step,
      y: pad + h - ((last - min) / range) * h,
    };
  });
</script>

{#if points.length >= 2}
  <svg {width} {height} class="flex-shrink-0" aria-hidden="true">
    <path d={pathD} fill="none" stroke={color} stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
    {#if lastDot}
      <circle cx={lastDot.x} cy={lastDot.y} r="2" fill={color} />
    {/if}
  </svg>
{:else}
  <svg {width} {height} class="flex-shrink-0 opacity-30" aria-hidden="true">
    <line x1="2" y1={height / 2} x2={width - 2} y2={height / 2}
      stroke={color} stroke-width="1" stroke-dasharray="3,3" />
  </svg>
{/if}
