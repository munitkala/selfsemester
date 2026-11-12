<script lang="ts">
  import { onMount } from "svelte";
  import { api, live, type CourseCard } from "#lib/api.ts";
  import { pct, statusLabel, when } from "#lib/format.ts";
  import CourseBanner from "#lib/CourseBanner.svelte";

  let courses = $state<CourseCard[] | null>(null);
  let error = $state<string | null>(null);

  onMount(() =>
    live(async () => {
      try {
        courses = await api.courses();
      } catch (e) {
        error = String(e);
      }
    }),
  );

  const href = (c: CourseCard) => (c.status === "draft" ? `/build/${encodeURIComponent(c.id)}` : `/course/${encodeURIComponent(c.id)}`);
  const current = $derived(courses?.filter((c) => c.status !== "completed" && c.status !== "withdrawn") ?? []);
  const past = $derived(courses?.filter((c) => c.status === "completed" || c.status === "withdrawn") ?? []);
</script>

<div class="spread">
  <h1>Courses</h1>
  <div class="row">
    <a class="button ghost" href="/build">Write a course</a>
    <a class="button" href="/new">New course</a>
  </div>
</div>
<p class="lead">Everything you're enrolled in, and what you've finished.</p>

{#if error}<p class="note bad">{error}</p>{/if}
{#if courses && courses.length === 0}
  <p class="card muted">No courses yet. Start one with Claude, or write your own in the course builder.</p>
{/if}

{#snippet card(c: CourseCard)}
  <a class="course" href={href(c)}>
    <CourseBanner id={c.id} title={c.title} meta={c.meta} origin={c.origin} compact />
    <div class="body">
      <p class="small {c.error ? 'bad' : 'muted'}">
        {c.error ? "A step failed. Open it to retry." : statusLabel[c.status] ?? c.status}
        {#if c.strictness === "sandbox" && c.status !== "draft"} · Sandbox{/if}
        {#if c.mode === "online"} · needs internet{/if}
      </p>
      {#if c.items_total}
        <div class="bar" role="progressbar" aria-valuemin="0" aria-valuemax={c.items_total} aria-valuenow={c.items_settled} aria-label="Graded work done">
          <span style="width: {(100 * c.items_settled) / c.items_total}%"></span>
        </div>
        <div class="stats small">
          <span>{c.items_settled} of {c.items_total} graded done</span>
          <strong>{pct(c.status === "completed" ? c.final_percent : c.grade, 0)}</strong>
        </div>
        {#if c.next_due}<p class="small muted">Next due {when(c.next_due)}</p>{/if}
      {/if}
    </div>
  </a>
{/snippet}

{#if current.length}
  <div class="grid">
    {#each current as c (c.id)}{@render card(c)}{/each}
  </div>
{/if}

{#if past.length}
  <h2 class="past-head">Finished</h2>
  <div class="grid">
    {#each past as c (c.id)}{@render card(c)}{/each}
  </div>
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(17rem, 1fr));
    gap: 1rem;
  }
  .course {
    display: flex;
    flex-direction: column;
    color: var(--text);
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
    text-decoration: none;
    box-shadow: var(--shadow);
    transition: border-color var(--fast), transform var(--fast) var(--ease), box-shadow var(--fast);
  }
  .course:hover,
  .course:focus-visible {
    border-color: var(--accent);
    box-shadow: var(--shadow-hover);
  }
  :global(:root:not([data-effects="minimal"])) .course:hover {
    transform: translateY(-2px);
  }
  .body {
    padding: 0.7rem 0.9rem 0.85rem;
    flex: 1;
  }
  .body p {
    margin: 0;
  }
  .bar {
    height: 6px;
    background: var(--line);
    border-radius: 3px;
    margin-top: 0.7rem;
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--accent);
  }
  .stats {
    display: flex;
    justify-content: space-between;
    margin-top: 0.35rem;
  }
  .past-head {
    margin-top: 2rem;
  }
  @media (prefers-reduced-motion: reduce) {
    .course {
      transition: none;
    }
  }
</style>
