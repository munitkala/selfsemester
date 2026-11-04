<script lang="ts">
  import { onMount } from "svelte";
  import { api, live, type CourseCard } from "$lib/api";
  import { pct, statusLabel, when } from "$lib/format";

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
</script>

<div class="spread">
  <h1>Courses</h1>
  <a class="button" href="/new">New course</a>
</div>
<p class="lead">Every course you've started, in progress first.</p>

{#if error}<p class="note bad">{error}</p>{/if}
{#if courses && courses.length === 0}
  <p class="card muted">No courses yet.</p>
{/if}

{#if courses}
  <div class="grid">
    {#each courses as c (c.id)}
      <a class="card course" href="/course/{encodeURIComponent(c.id)}">
        <div class="spread">
          <h2>{c.title}</h2>
          {#if c.mode === "online"}<span class="pill">Needs internet</span>{/if}
        </div>
        <p class="small {c.error ? 'bad' : 'muted'}">{c.error ? "A step failed. Open it to retry." : statusLabel[c.status] ?? c.status}</p>
        {#if c.items_total}
          <div class="bar" role="progressbar" aria-valuemin="0" aria-valuemax={c.items_total} aria-valuenow={c.items_settled} aria-label="Graded work done">
            <span style="width: {(100 * c.items_settled) / c.items_total}%"></span>
          </div>
          <p class="small muted">
            {c.items_settled} of {c.items_total} graded items done
            · mark {pct(c.status === "completed" ? c.final_percent : c.grade)}
            {#if c.next_due}· next due {when(c.next_due)}{/if}
          </p>
        {/if}
      </a>
    {/each}
  </div>
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(16rem, 1fr));
    gap: 1rem;
  }
  .course {
    color: var(--text);
    margin: 0;
  }
  .course:hover {
    text-decoration: none;
    border-color: var(--accent);
  }
  .course h2 {
    margin: 0;
  }
  .course p {
    margin: 0.3rem 0 0;
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
</style>
