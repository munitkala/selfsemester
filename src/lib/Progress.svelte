<script lang="ts">
  import { onMount } from "svelte";
  import type { Progress } from "$lib/api";

  let { p }: { p: Progress } = $props();
  let now = $state(Date.now());

  onMount(() => {
    const t = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(t);
  });

  // Count down from the backend's estimate; it refreshes whenever a step finishes.
  const left = $derived(p.paused ? p.remaining_secs : p.remaining_secs - (now - new Date(p.as_of).getTime()) / 1000);
  const fraction = $derived(p.total_secs > 0 ? Math.min(0.99, Math.max(0.02, 1 - Math.max(left, 0) / p.total_secs)) : 0);

  function words(s: number) {
    if (s < 60) return "less than a minute";
    const m = Math.round(s / 60);
    if (m < 60) return `${m} min`;
    const h = Math.floor(m / 60);
    return `${h} h ${m % 60} min`;
  }
</script>

<div class="progress" aria-live="polite">
  <div class="spread small">
    <span>{p.stage}</span>
    <span class="muted">
      {#if p.paused}Paused for your usage limit
      {:else if left > 5}About {words(left)} left
      {:else}Finishing up, taking a little longer than usual{/if}
    </span>
  </div>
  <div class="track" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(fraction * 100)} aria-label={p.stage}>
    <span class:paused={p.paused} style="width: {fraction * 100}%"></span>
  </div>
  <p class="small muted">Estimate based on how long these steps have taken before on this computer.</p>
</div>

<style>
  .progress {
    margin: 0.5rem 0 1rem;
  }
  .track {
    height: 8px;
    background: var(--line);
    border-radius: 4px;
    overflow: hidden;
    margin-top: 0.35rem;
  }
  .track span {
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width 1s linear;
  }
  .track span.paused {
    background: var(--warn);
  }
  p {
    margin: 0.3rem 0 0;
  }
  @media (prefers-reduced-motion: reduce) {
    .track span {
      transition: none;
    }
  }
</style>
