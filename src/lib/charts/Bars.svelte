<script lang="ts">
  // Vertical bars with labels under them and an optional dashed goal line.
  let {
    data,
    goal = 0,
    height = 140,
    format = (v: number) => String(Math.round(v)),
    label = "Bar chart",
    every = 1,
  }: {
    data: { label: string; value: number; title?: string; color?: string; highlight?: boolean }[];
    goal?: number;
    height?: number;
    format?: (v: number) => string;
    label?: string;
    every?: number;
  } = $props();

  let measured = $state(600);
  const W = $derived(Math.max(measured, 120));
  const BOTTOM = 18;
  const TOP = 14;
  const max = $derived(Math.max(goal, ...data.map((d) => d.value), 1));
  const slot = $derived(W / Math.max(1, data.length));
  const barW = $derived(Math.max(2, slot * 0.68));
  const y = (v: number) => TOP + (height - TOP - BOTTOM) * (1 - v / max);
</script>

<div class="fit" bind:clientWidth={measured}>
<svg viewBox="0 0 {W} {height}" role="img" aria-label={label} class="bars">
  <line x1="0" x2={W} y1={height - BOTTOM} y2={height - BOTTOM} class="axis" />
  {#if goal > 0}
    <line x1="0" x2={W} y1={y(goal)} y2={y(goal)} class="goal" />
  {/if}
  {#each data as d, i}
    {@const top = y(d.value)}
    <rect
      x={i * slot + (slot - barW) / 2}
      y={top}
      width={barW}
      height={Math.max(0, height - BOTTOM - top)}
      rx="3"
      class:highlight={d.highlight}
      style={d.color ? `fill: ${d.color}` : ""}
    ><title>{d.title ?? `${d.label}: ${format(d.value)}`}</title></rect>
    {#if i % every === 0}
      <text x={i * slot + slot / 2} y={height - 4} class="label">{d.label}</text>
    {/if}
  {/each}
</svg>
</div>

<style>
  .fit {
    width: 100%;
  }
  .bars {
    width: 100%;
    height: auto;
    display: block;
    overflow: visible;
  }
  rect {
    fill: color-mix(in oklch, var(--accent) 55%, var(--line));
    transition: fill var(--fast);
  }
  rect.highlight,
  rect:hover {
    fill: var(--accent);
  }
  .axis {
    stroke: var(--line);
    vector-effect: non-scaling-stroke;
  }
  .goal {
    stroke: var(--good);
    stroke-dasharray: 5 4;
    vector-effect: non-scaling-stroke;
  }
  .label {
    font-size: 11px;
    fill: var(--muted);
    text-anchor: middle;
  }
</style>
