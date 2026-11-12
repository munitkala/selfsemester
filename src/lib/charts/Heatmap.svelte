<script lang="ts">
  import type { HeatCell } from "#lib/insights.ts";
  import { formatMinutes } from "#lib/insights.ts";

  let { grid, weekStart = 1 }: { grid: HeatCell[][]; weekStart?: 0 | 1 } = $props();

  const CELL = 12;
  const GAP = 3;
  const LEFT = 26;
  const TOP = 16;
  const names = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
  const rowLabel = (r: number) => names[(r + weekStart) % 7];

  const months = $derived.by(() => {
    const out: { x: number; label: string }[] = [];
    let last = -1;
    grid.forEach((col, w) => {
      const m = Number(col[0].day.slice(5, 7));
      if (m !== last) {
        last = m;
        out.push({ x: LEFT + w * (CELL + GAP), label: new Date(2000, m - 1, 1).toLocaleDateString(undefined, { month: "short" }) });
      }
    });
    // Drop a label squeezed against the start.
    return out.length > 1 && out[1].x - out[0].x < 28 ? out.slice(1) : out;
  });

  const width = $derived(LEFT + grid.length * (CELL + GAP));
  const height = TOP + 7 * (CELL + GAP);

  function title(c: HeatCell) {
    const date = new Date(c.day + "T12:00:00").toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric" });
    const parts = [c.minutes >= 1 ? formatMinutes(c.minutes) : "No study time"];
    if (c.events) parts.push(`${c.events} handed in or reviewed`);
    return `${date}: ${parts.join(", ")}`;
  }
</script>

<div class="wrap">
  <svg viewBox="0 0 {width} {height}" width={width} {height} role="img" aria-label="Study activity by day">
    {#each months as m}
      <text x={m.x} y="10" class="label">{m.label}</text>
    {/each}
    {#each [1, 3, 5] as r}
      <text x="0" y={TOP + r * (CELL + GAP) + CELL - 2} class="label">{rowLabel(r)}</text>
    {/each}
    {#each grid as col, w}
      {#each col as c, d}
        {#if !c.future}
          <rect
            x={LEFT + w * (CELL + GAP)}
            y={TOP + d * (CELL + GAP)}
            width={CELL}
            height={CELL}
            rx="2.5"
            class="l{c.level}"
          ><title>{title(c)}</title></rect>
        {/if}
      {/each}
    {/each}
  </svg>
  <div class="legend small muted" aria-hidden="true">
    Less
    {#each [0, 1, 2, 3, 4] as l}<span class="key l{l}"></span>{/each}
    More
  </div>
</div>

<style>
  .wrap {
    overflow-x: auto;
  }
  svg {
    display: block;
    max-width: none;
  }
  .label {
    font-size: 9px;
    fill: var(--muted);
  }
  rect,
  .key {
    fill: var(--line);
    background: var(--line);
  }
  .l1 { fill: color-mix(in oklch, var(--accent) 30%, var(--line)); background: color-mix(in oklch, var(--accent) 30%, var(--line)); }
  .l2 { fill: color-mix(in oklch, var(--accent) 55%, var(--line)); background: color-mix(in oklch, var(--accent) 55%, var(--line)); }
  .l3 { fill: color-mix(in oklch, var(--accent) 78%, var(--line)); background: color-mix(in oklch, var(--accent) 78%, var(--line)); }
  .l4 { fill: var(--accent); background: var(--accent); }
  rect:hover {
    stroke: var(--text);
    stroke-width: 1;
  }
  .legend {
    display: flex;
    align-items: center;
    gap: 3px;
    justify-content: flex-end;
    margin-top: 0.35rem;
  }
  .key {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 2px;
  }
</style>
