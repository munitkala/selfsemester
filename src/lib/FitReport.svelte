<script lang="ts">
  // How well the AI that's set up can build a course, part by part. Shown in Settings and on the New course page,
  // so nobody finds out halfway through a build that their model can't mark photos or find sources.
  import type { Fit, FitLevel } from "#lib/api.ts";
  import Icon from "#lib/Icon.svelte";

  let { fit, compact = false }: { fit: Fit; compact?: boolean } = $props();

  const overallWord: Record<Fit["overall"], string> = {
    best: "Everything works",
    good: "Good",
    fair: "Fair",
    limited: "Limited",
    unknown: "Not checked yet",
  };
  const levelWord: Record<FitLevel, string> = { full: "Works", reduced: "Reduced", none: "Not available", unknown: "Unknown" };
</script>

<div class="fit {fit.overall}" class:compact>
  <p class="headline">
    <strong>{overallWord[fit.overall]}.</strong>
    {fit.headline}
  </p>
  {#if fit.areas.length}
    <ul>
      {#each fit.areas as a (a.id)}
        <li class={a.level}>
          <span class="mark" aria-hidden="true">
            {#if a.level === "full"}<Icon name="check" size={14} />{:else if a.level === "none"}<Icon name="x" size={14} />{:else}<span class="dash">{a.level === "unknown" ? "?" : "~"}</span>{/if}
          </span>
          <span class="what">
            <span class="label">{a.label}</span>
            <span class="sr">: {levelWord[a.level]}.</span>
            {#if !compact || a.level !== "full"}<span class="note-text small muted">{a.note}</span>{/if}
          </span>
        </li>
      {/each}
    </ul>
  {/if}
  <p class="small muted cost">{fit.cost}</p>
</div>

<style>
  .fit {
    border-left: 3px solid var(--line);
    padding: 0.5rem 0 0.5rem 0.85rem;
    margin: 0.75rem 0;
  }
  .fit.best,
  .fit.good {
    border-color: var(--good);
  }
  .fit.fair,
  .fit.unknown {
    border-color: var(--warn);
  }
  .fit.limited {
    border-color: var(--bad);
  }
  .headline {
    margin: 0 0 0.5rem;
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    gap: 0.4rem;
  }
  li {
    display: grid;
    grid-template-columns: 1.3rem minmax(0, 1fr);
    gap: 0.4rem;
    align-items: start;
  }
  .mark {
    display: inline-flex;
    justify-content: center;
    padding-top: 0.15rem;
  }
  .full .mark {
    color: var(--good);
  }
  .none .mark {
    color: var(--bad);
  }
  .reduced .mark,
  .unknown .mark {
    color: var(--warn);
    font-weight: 700;
  }
  .what {
    display: flex;
    flex-direction: column;
  }
  .label {
    font-weight: 600;
  }
  .compact .label {
    font-weight: 500;
  }
  .cost {
    margin: 0.6rem 0 0;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }
</style>
