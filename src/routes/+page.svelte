<script lang="ts">
  import { onMount } from "svelte";
  import { api, live, type Today, type CourseCard } from "$lib/api";
  import ItemList from "$lib/ItemList.svelte";
  import { statusLabel } from "$lib/format";

  let today = $state<Today | null>(null);
  let courses = $state<CourseCard[]>([]);
  let error = $state<string | null>(null);

  onMount(() =>
    live(async () => {
      try {
        [today, courses] = await Promise.all([api.today(), api.courses()]);
      } catch (e) {
        error = String(e);
      }
    }),
  );

  const needsYou = $derived(courses.filter((c) => c.status === "answering" || c.status === "review" || c.error));
  const building = $derived(courses.filter((c) => ["intake", "scoping", "compiling"].includes(c.status) && !c.error));
  const nothing = $derived(
    today && !today.overdue.length && !today.due_soon.length && !today.new_lessons.length && !today.open.length && !today.awaiting_marks.length,
  );
</script>

<h1>Today</h1>
<p class="lead">{new Date().toLocaleDateString(undefined, { weekday: "long", month: "long", day: "numeric" })}</p>

{#if error}<p class="note bad">{error}</p>{/if}

{#if needsYou.length}
  <section class="card">
    <h2>Needs you</h2>
    <ul class="plain">
      {#each needsYou as c}
        <li>
          <a href="/course/{encodeURIComponent(c.id)}">{c.title}</a>
          <span class="muted small">({c.error ? "a step failed; open to retry" : statusLabel[c.status]})</span>
        </li>
      {/each}
    </ul>
  </section>
{/if}

{#if building.length}
  <section class="card">
    <h2>Being built</h2>
    <ul class="plain">
      {#each building as c}
        <li><a href="/course/{encodeURIComponent(c.id)}">{c.title}</a> <span class="muted small">({statusLabel[c.status]})</span></li>
      {/each}
    </ul>
  </section>
{/if}

{#if today}
  {#if today.overdue.length}
    <section class="card">
      <h2 class="bad">Late</h2>
      <p class="small muted">Past the deadline but still accepted under the course's late policy.</p>
      <ItemList items={today.overdue} showCourse />
    </section>
  {/if}
  {#if today.due_soon.length}
    <section class="card">
      <h2>Due this week</h2>
      <ItemList items={today.due_soon} showCourse />
    </section>
  {/if}
  {#if today.new_lessons.length}
    <section class="card">
      <h2>Lessons to read</h2>
      <ItemList items={today.new_lessons} showCourse />
    </section>
  {/if}
  {#if today.open.length}
    <section class="card">
      <h2>Open, due later</h2>
      <ItemList items={today.open} showCourse />
    </section>
  {/if}
  {#if today.awaiting_marks.length}
    <section class="card">
      <h2>Waiting for marks</h2>
      <ItemList items={today.awaiting_marks} showCourse />
    </section>
  {/if}
  {#if today.recently_marked.length}
    <section class="card">
      <h2>Marked this week</h2>
      <ItemList items={today.recently_marked} showCourse />
    </section>
  {/if}
  {#if today.upcoming.length}
    <section class="card">
      <h2>Coming up</h2>
      <ItemList items={today.upcoming} showCourse />
    </section>
  {/if}
  {#if nothing && !needsYou.length && !building.length}
    <section class="card">
      {#if courses.length === 0}
        <h2>Start your first course</h2>
        <p>Name any subject. Claude asks a few questions, plans the course with you, then builds every lesson, assignment and exam up front.</p>
        <a class="button" href="/new">New course</a>
      {:else}
        <p class="muted">Nothing due right now.</p>
      {/if}
    </section>
  {/if}
{/if}

<style>
  .plain {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .plain li {
    padding: 0.3rem 0;
  }
</style>
