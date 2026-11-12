<script lang="ts">
  import type { MarkPoint } from "#lib/insights.ts";

  let { points, height = 180 }: { points: MarkPoint[]; height?: number } = $props();

  let measured = $state(600);
  const W = $derived(Math.max(measured, 120));
  const L = 34;
  const R = 8;
  const T = 10;
  const B = 22;
  const span = $derived.by(() => {
    if (!points.length) return { from: 0, to: 1 };
    const from = points[0].at.getTime();
    const to = points[points.length - 1].at.getTime();
    return { from, to: to > from ? to : from + 864e5 };
  });
  const x = (t: Date) => L + ((W - L - R) * (t.getTime() - span.from)) / (span.to - span.from);
  const y = (p: number) => T + ((height - T - B) * (100 - Math.max(0, Math.min(100, p)))) / 100;
  const line = $derived(points.map((p, i) => `${i ? "L" : "M"}${x(p.at).toFixed(1)},${y(p.average).toFixed(1)}`).join(" "));
  const fmt = (d: Date) => d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
</script>

<div class="fit" bind:clientWidth={measured}>
<svg viewBox="0 0 {W} {height}" role="img" aria-label="Marks over time, with the average of the last five" class="trend">
  {#each [0, 50, 70, 100] as g}
    <line x1={L} x2={W - R} y1={y(g)} y2={y(g)} class="grid" />
    <text x={L - 6} y={y(g) + 4} class="label end">{g}%</text>
  {/each}
  {#if points.length > 1}
    <path d={line} class="avg" />
  {/if}
  {#each points as p}
    <circle cx={x(p.at)} cy={y(p.percent)} r="4.5" style="--dot: {p.color}"><title>{fmt(p.at)}: {p.percent.toFixed(0)}% · {p.label}</title></circle>
  {/each}
  {#if points.length}
    <text x={L} y={height - 4} class="label">{fmt(points[0].at)}</text>
    <text x={W - R} y={height - 4} class="label end">{fmt(points[points.length - 1].at)}</text>
  {/if}
</svg>
</div>

<style>
  .fit {
    width: 100%;
  }
  .trend {
    width: 100%;
    height: auto;
    display: block;
  }
  .grid {
    stroke: var(--line);
    stroke-dasharray: 2 4;
  }
  .avg {
    fill: none;
    stroke: var(--accent);
    stroke-width: 2.5;
    stroke-linejoin: round;
    stroke-linecap: round;
  }
  circle {
    fill: color-mix(in oklch, var(--dot) var(--course-lift), white);
    stroke: var(--panel);
    stroke-width: 1.5;
  }
  .label {
    font-size: 11px;
    fill: var(--muted);
  }
  .end {
    text-anchor: end;
  }
</style>
