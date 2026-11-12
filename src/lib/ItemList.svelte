<script lang="ts">
  import type { ItemRow } from "#lib/api.ts";
  import { band, kindLabel, stateLabel, when, relative, pct } from "#lib/format.ts";
  import KindIcon from "#lib/KindIcon.svelte";

  let { items, showCourse = false, empty = "" }: { items: ItemRow[]; showCourse?: boolean; empty?: string } = $props();

  function dueText(i: ItemRow) {
    if (i.state === "locked") return `Opens ${when(i.release_at)}`;
    if (!i.due_at) return i.state === "done" ? "Read" : `Released ${when(i.release_at)}`;
    if (i.state === "open") return `Due ${when(i.due_at)} (${relative(i.due_at)})`;
    const past = new Date(i.due_at).getTime() < Date.now();
    return `${past ? "Was due" : "Due"} ${when(i.due_at)}`;
  }
</script>

{#if items.length === 0}
  {#if empty}<p class="muted small">{empty}</p>{/if}
{:else}
  <ul>
    {#each items as i (i.id)}
      <li>
        <a href={i.state === "locked" ? undefined : `/item/${encodeURIComponent(i.id)}`} class:locked={i.state === "locked"}>
          <span class="kind"><KindIcon kind={i.kind} size={15} /> {kindLabel[i.kind] ?? i.kind}</span>
          <span class="title">{i.title}</span>
        </a>
        <span class="meta">
          {#if showCourse}<span class="muted">{i.course_title} ·</span>{/if}
          <span class="muted">{dueText(i)}</span>
          {#if i.percent != null}<strong class={band(i.percent)}>{pct(i.percent, 0)}</strong>{/if}
          <span class="pill {i.state}">{stateLabel[i.state] ?? i.state}</span>
        </span>
      </li>
    {/each}
  </ul>
{/if}

<style>
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.55rem 0.5rem;
    margin: 0 -0.5rem;
    border-bottom: 1px solid var(--line);
    border-radius: var(--radius-sm);
    flex-wrap: wrap;
    transition: background-color var(--fast);
  }
  li:hover {
    background: color-mix(in oklch, var(--accent) 6%, transparent);
  }
  li:last-child {
    border-bottom: 0;
  }
  a {
    display: flex;
    gap: 0.6rem;
    align-items: baseline;
    color: var(--text);
    min-width: 0;
  }
  a.locked {
    color: var(--muted);
    cursor: default;
  }
  a.locked:hover {
    text-decoration: none;
  }
  .kind {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.78rem;
    color: var(--muted);
    width: 6.5rem;
    flex: none;
  }
  .title {
    font-weight: 500;
  }
  .meta {
    display: flex;
    gap: 0.6rem;
    align-items: baseline;
    font-size: 0.86rem;
    flex-wrap: wrap;
  }
</style>
