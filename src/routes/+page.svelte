<script lang="ts">
  import { onMount } from "svelte";
  import { api, live, type Today, type CourseCard, type InsightsData } from "#lib/api.ts";
  import ItemList from "#lib/ItemList.svelte";
  import Icon from "#lib/Icon.svelte";
  import Ring from "#lib/charts/Ring.svelte";
  import { statusLabel } from "#lib/format.ts";
  import { byDay, formatMinutes, localDay, streaks } from "#lib/insights.ts";
  import { prefs } from "#lib/prefs.svelte.ts";

  let today = $state<Today | null>(null);
  let courses = $state<CourseCard[]>([]);
  let stats = $state<InsightsData | null>(null);
  let error = $state<string | null>(null);

  onMount(() =>
    live(async () => {
      try {
        [today, courses] = await Promise.all([api.today(), api.courses()]);
        // The numbers in the header are a bonus; the page still works if they can't load.
        stats = await api.insights().catch(() => null);
      } catch (e) {
        error = String(e);
      }
    }),
  );

  const hour = new Date().getHours();
  const greeting = hour < 5 ? "Working late" : hour < 12 ? "Good morning" : hour < 18 ? "Good afternoon" : "Good evening";
  const days = $derived(stats ? byDay(stats) : new Map());
  const streak = $derived(streaks(days));
  const minutesToday = $derived(days.get(localDay(new Date()))?.minutes ?? 0);
  const goalMet = $derived(prefs.dailyGoal > 0 && minutesToday >= prefs.dailyGoal);
  const dueCount = $derived(today ? today.overdue.length + today.due_soon.length : 0);
  const grades = $derived(courses.filter((c) => c.status === "active" && c.grade != null).map((c) => c.grade as number));
  const average = $derived(grades.length ? grades.reduce((a, b) => a + b, 0) / grades.length : null);

  const needsYou = $derived(courses.filter((c) => c.status === "answering" || c.status === "review" || c.error));
  const building = $derived(courses.filter((c) => ["intake", "scoping", "compiling"].includes(c.status) && !c.error));
  const nothing = $derived(
    today && !today.overdue.length && !today.due_soon.length && !today.new_lessons.length && !today.open.length && !today.awaiting_marks.length,
  );
</script>

<header class="hero page-enter" class:celebrate={goalMet && prefs.celebrate}>
  <div class="hello">
    <p class="date small">{new Date().toLocaleDateString(undefined, { weekday: "long", month: "long", day: "numeric" })}</p>
    <h1>{greeting}</h1>
    <p class="muted">
      {#if goalMet}
        Daily goal done. Nice work.
      {:else if dueCount}
        {dueCount} {dueCount === 1 ? "piece of work is" : "pieces of work are"} due this week.
      {:else}
        Nothing urgent. A good day to get ahead.
      {/if}
    </p>
  </div>
  <div class="stats">
    {#if prefs.dailyGoal > 0 && prefs.trackStudy}
      <a class="stat" href="/insights" title="Study time today against your daily goal">
        <Ring value={minutesToday / prefs.dailyGoal} size={54} stroke={6} label="Today's study time against your daily goal">
          <span class="small"><strong>{Math.round(minutesToday)}</strong></span>
        </Ring>
        <span><strong>{formatMinutes(minutesToday)}</strong><span class="small muted">of {prefs.dailyGoal} min today</span></span>
      </a>
    {/if}
    <a class="stat" href="/insights">
      <span class="badge" class:lit={streak.current > 0}><Icon name="flame" /></span>
      <span><strong>{streak.current} {streak.current === 1 ? "day" : "days"}</strong><span class="small muted">streak · best {streak.best}</span></span>
    </a>
    <a class="stat" href="/courses">
      <span class="badge"><Icon name="target" /></span>
      <span><strong>{average == null ? "-" : `${average.toFixed(0)}%`}</strong><span class="small muted">average now</span></span>
    </a>
  </div>
</header>

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
  .hero {
    position: relative;
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 1.25rem;
    flex-wrap: wrap;
    padding: 1.4rem 1.5rem;
    margin-bottom: calc(var(--space) * 1.25);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
    box-shadow: var(--shadow);
    overflow: hidden;
  }
  /* A soft wash of the accent colour. "full" effects let it drift slowly; "minimal" leaves the card flat. */
  :global([data-effects="balanced"]) .hero,
  :global([data-effects="full"]) .hero {
    background:
      radial-gradient(120% 140% at 0% 0%, color-mix(in oklch, var(--accent) 16%, transparent), transparent 60%),
      radial-gradient(90% 120% at 100% 100%, color-mix(in oklch, var(--chart-2) 12%, transparent), transparent 60%),
      var(--panel);
    background-size: 160% 160%, 160% 160%, auto;
  }
  :global([data-effects="full"][data-motion="full"]) .hero {
    animation: drift 18s ease-in-out infinite alternate;
  }
  @keyframes drift {
    from {
      background-position: 0% 0%, 100% 100%, 0 0;
    }
    to {
      background-position: 40% 30%, 60% 70%, 0 0;
    }
  }
  .hello h1 {
    font-size: 2rem;
  }
  .hello p {
    margin: 0;
  }
  .date {
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--accent);
    font-weight: 600;
  }
  .stats {
    display: flex;
    gap: 0.6rem;
    flex-wrap: wrap;
  }
  .stat {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.5rem 0.8rem;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: color-mix(in oklch, var(--panel) 85%, transparent);
    color: var(--text);
    transition: border-color var(--fast), transform var(--fast);
  }
  .stat:hover {
    text-decoration: none;
    border-color: var(--accent-line);
  }
  :global([data-effects="full"]) .stat:hover,
  :global([data-effects="balanced"]) .stat:hover {
    transform: translateY(-1px);
  }
  .stat strong {
    display: block;
  }
  .badge {
    display: grid;
    place-items: center;
    width: 2.3rem;
    height: 2.3rem;
    border-radius: 50%;
    background: var(--accent-soft);
    color: var(--accent);
  }
  .badge.lit {
    color: var(--warn);
    background: color-mix(in oklch, var(--warn) 14%, var(--panel));
  }
  .celebrate {
    border-color: color-mix(in oklch, var(--good) 45%, var(--line));
  }
  :global([data-motion="full"]) .celebrate .badge.lit {
    animation: pop 600ms var(--ease) 1;
  }
  @keyframes pop {
    40% {
      transform: scale(1.15);
    }
  }
  .plain {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .plain li {
    padding: 0.3rem 0;
  }
</style>
