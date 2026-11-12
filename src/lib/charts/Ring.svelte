<script lang="ts">
  import type { Snippet } from "svelte";

  // A progress ring. `value` is 0..1; it fills on first show unless motion is reduced.
  let {
    value,
    size = 72,
    stroke = 8,
    label,
    tone = "accent",
    children,
  }: { value: number; size?: number; stroke?: number; label: string; tone?: "accent" | "good" | "warn" | "bad"; children?: Snippet } = $props();

  const r = $derived((size - stroke) / 2);
  const c = $derived(2 * Math.PI * r);
  const shown = $derived(Math.max(0, Math.min(1, value)));
</script>

<div class="ring" style="width: {size}px; height: {size}px" role="img" aria-label={label}>
  <svg width={size} height={size} viewBox="0 0 {size} {size}" aria-hidden="true">
    <circle cx={size / 2} cy={size / 2} {r} class="track" stroke-width={stroke} />
    <circle
      cx={size / 2}
      cy={size / 2}
      {r}
      class="fill {tone}"
      class:done={tone === "accent" && value >= 1}
      stroke-width={stroke}
      stroke-dasharray={c}
      stroke-dashoffset={c * (1 - shown)}
      style="--c: {c}"
    />
  </svg>
  {#if children}<div class="inside">{@render children()}</div>{/if}
</div>

<style>
  .ring {
    position: relative;
    flex: none;
  }
  svg {
    transform: rotate(-90deg);
    display: block;
  }
  circle {
    fill: none;
  }
  .track {
    stroke: var(--line);
  }
  .fill {
    stroke: var(--accent);
    stroke-linecap: round;
    transition: stroke-dashoffset var(--slow) var(--ease);
  }
  :global([data-motion="full"]) .fill {
    animation: fill-in 900ms var(--ease) both;
  }
  .fill.done,
  .fill.good {
    stroke: var(--good);
  }
  .fill.warn {
    stroke: var(--warn);
  }
  .fill.bad {
    stroke: var(--bad);
  }
  @keyframes fill-in {
    from {
      stroke-dashoffset: var(--c);
    }
  }
  .inside {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    text-align: center;
    line-height: 1.1;
  }
</style>
