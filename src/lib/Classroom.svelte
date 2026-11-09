<script lang="ts">
  // A running course, laid out like a school course page: home, modules, syllabus, grades and materials, with the
  // same tabs and the same module layout for every course so it's always clear where things are.
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { api, type CourseView, type ItemRow } from "#lib/api.ts";
  import { render } from "#lib/markdown.ts";
  import { day, kindLabel, letter, pct, when } from "#lib/format.ts";
  import { agenda, colorVars, markedBy, nextUp, settled, statusOf } from "#lib/course.ts";
  import CourseBanner from "#lib/CourseBanner.svelte";
  import KindIcon from "#lib/KindIcon.svelte";
  import Materials from "#lib/Materials.svelte";

  let { c }: { c: CourseView } = $props();

  const TABS = [
    { id: "home", label: "Home" },
    { id: "modules", label: "Modules" },
    { id: "syllabus", label: "Syllabus" },
    { id: "grades", label: "Grades" },
    { id: "materials", label: "Materials" },
  ] as const;
  type Tab = (typeof TABS)[number]["id"];
  const tab = $derived((TABS.find((t) => t.id === page.url.searchParams.get("tab"))?.id ?? "home") as Tab);

  function show(t: Tab) {
    goto(`?tab=${t}`, { replace: true, reset: false });
  }

  const items = $derived(c.units.flatMap((u) => u.items));
  const graded = $derived(items.filter((i) => i.kind !== "lesson"));
  const lessons = $derived(items.filter((i) => i.kind === "lesson"));
  const next = $derived(nextUp(items));
  const upcoming = $derived(agenda(items));
  const recent = $derived(
    graded
      .filter((i) => i.state === "marked" && i.percent != null)
      .sort((a, b) => (b.due_at ?? "").localeCompare(a.due_at ?? ""))
      .slice(0, 4),
  );
  const weeks = $derived(c.plan?.pacing.total_weeks ?? Math.max(1, ...c.units.map((u) => u.week)));
  const doneAll = $derived(items.filter(settled).length);

  /** Each kind's share of the final grade and the average so far, from the items themselves. */
  const categories = $derived.by(() => {
    const out: { kind: string; weight: number; avg: number | null; count: number; markedCount: number }[] = [];
    for (const it of graded) {
      let row = out.find((r) => r.kind === it.kind);
      if (!row) out.push((row = { kind: it.kind, weight: 0, avg: null, count: 0, markedCount: 0 }));
      row.weight += it.weight;
      row.count += 1;
      if (it.percent != null) {
        row.avg = ((row.avg ?? 0) * row.markedCount + it.percent) / (row.markedCount + 1);
        row.markedCount += 1;
      }
    }
    return out.sort((a, b) => b.weight - a.weight);
  });

  function unitDone(us: ItemRow[]) {
    return us.filter(settled).length;
  }

  const lateText = $derived(
    c.late_policy?.kind === "penalty"
      ? `Late work is accepted for up to ${c.late_policy.max_days} days and loses ${c.late_policy.per_day} percentage points for each day it's late.`
      : "Late work isn't accepted: when a deadline passes, the item locks with a mark of 0.",
  );
</script>

<div class="classroom" style={colorVars(c.meta, c.id)}>
  <CourseBanner id={c.id} title={c.title} meta={c.meta} origin={c.origin} subtitle={`${day(c.starts_at)} – ${day(c.ends_at)}`}>
    {#snippet aside()}
      <div class="grade-badge" aria-label="Current mark">
        {#if c.status === "completed" && c.final_percent != null}
          <strong>{pct(c.final_percent, 0)}</strong><span>{letter(c.final_percent)} · final</span>
        {:else if c.grades.current != null}
          <strong>{pct(c.grades.current, 0)}</strong><span>{letter(c.grades.current)} so far</span>
        {:else}
          <strong>–</strong><span>no marks yet</span>
        {/if}
      </div>
    {/snippet}
  </CourseBanner>

  <nav class="tabs" aria-label="Course sections">
    {#each TABS as t}
      <button class:active={tab === t.id} aria-current={tab === t.id ? "page" : undefined} onclick={() => show(t.id)}>{t.label}</button>
    {/each}
    <span class="chips">
      {#if c.strictness === "sandbox"}<span class="chip">Sandbox</span>{:else}<span class="chip">Structured</span>{/if}
      {#if c.origin === "manual"}<span class="chip">Written by you</span>{/if}
      {#if c.mode === "online"}<span class="chip warn">Needs internet</span>{/if}
    </span>
  </nav>

  {#if c.status === "completed" && c.archive_dir}
    <section class="card done-card">
      <div>
        <h2>Course complete</h2>
        <p class="muted">Your summary, lesson notes, work, feedback and reference list are saved in Documents.</p>
      </div>
      <button onclick={() => api.openFolder(c.archive_dir!)}>Open the archive</button>
    </section>
  {/if}

  <!-- ===== Home ===== -->
  {#if tab === "home"}
    <div class="home">
      <div class="main-col">
        <section class="card welcome">
          <h2>Welcome</h2>
          {#if c.meta.welcome.trim()}
            <div class="prose">{@html render(c.meta.welcome)}</div>
          {:else}
            <p>Welcome to <strong>{c.title}</strong>. {c.plan?.summary ?? ""}</p>
          {/if}
          <h3>How this course works</h3>
          <ul class="how">
            <li>It runs for {weeks} {weeks === 1 ? "week" : "weeks"}. Each week opens at 8:00 with its lesson; read it first, then do that week's work.</li>
            <li>Deadlines are at 11:59 p.m. on the day shown. {lateText}</li>
            <li>{c.strictness === "sandbox" ? "Sandbox: you can resubmit as often as you like, and your latest attempt counts. The answer key stays hidden until you choose to see it." : "Structured: you get one attempt at each piece of work, so check it before you submit."}</li>
            <li>{markedBy[c.origin]}</li>
          </ul>
          <button class="ghost small" onclick={() => show("syllabus")}>Read the full syllabus</button>
        </section>

        <section class="card">
          <h2>Up next</h2>
          {#if next}
            {@const s = statusOf(next)}
            <a class="next" href="/item/{encodeURIComponent(next.id)}">
              <span class="icon-box"><KindIcon kind={next.kind} size={22} /></span>
              <span class="next-text">
                <span class="small muted">{kindLabel[next.kind]}{next.unit_title ? ` · ${next.unit_title}` : ""}</span>
                <strong>{next.title}</strong>
                <span class="status {s.tone}">{s.label}</span>
              </span>
              <span class="go" aria-hidden="true">→</span>
            </a>
          {:else if doneAll === items.length}
            <p class="muted">Everything's done. Well done.</p>
          {:else}
            <p class="muted">Nothing open right now. The next week opens {when(items.find((i) => i.state === "locked")?.release_at)}.</p>
          {/if}
        </section>

        <section class="card">
          <div class="spread"><h2>Progress</h2><span class="small muted">{doneAll} of {items.length} done</span></div>
          <div class="bar big" role="progressbar" aria-valuemin="0" aria-valuemax={items.length} aria-valuenow={doneAll} aria-label="Course progress">
            <span style="width: {items.length ? (100 * doneAll) / items.length : 0}%"></span>
          </div>
          <ul class="unit-progress">
            {#each c.units as u (u.key)}
              <li>
                <span class="wk">Week {u.week}</span>
                <span class="ut">{u.title}</span>
                <span class="bar" aria-hidden="true"><span style="width: {u.items.length ? (100 * unitDone(u.items)) / u.items.length : 0}%"></span></span>
                <span class="small muted nowrap">{unitDone(u.items)}/{u.items.length}</span>
              </li>
            {/each}
          </ul>
        </section>
      </div>

      <aside class="side-col">
        <section class="card">
          <h2>Coming up</h2>
          {#if upcoming.length}
            <ol class="agenda">
              {#each upcoming as g}
                <li>
                  <span class="agenda-day">{g.day}</span>
                  <ul>
                    {#each g.items as it}
                      <li><KindIcon kind={it.kind} size={15} /> <a href="/item/{encodeURIComponent(it.id)}">{it.title}</a>{#if it.state === "locked"}{" "}<span class="small muted">(opens {day(it.release_at)})</span>{/if}</li>
                    {/each}
                  </ul>
                </li>
              {/each}
            </ol>
          {:else}
            <p class="muted small">No deadlines in the next three weeks.</p>
          {/if}
        </section>

        <section class="card">
          <div class="spread"><h2>Recent marks</h2><button class="ghost small" onclick={() => show("grades")}>All grades</button></div>
          {#if recent.length}
            <ul class="recent">
              {#each recent as it}
                <li><a href="/item/{encodeURIComponent(it.id)}">{it.title}</a><strong>{pct(it.percent, 0)}</strong></li>
              {/each}
            </ul>
          {:else}
            <p class="muted small">Marks appear here as your work is marked.</p>
          {/if}
        </section>
      </aside>
    </div>

  <!-- ===== Modules ===== -->
  {:else if tab === "modules"}
    {#each c.units as u (u.key)}
      {@const objectives = c.meta.objectives?.[u.key] ?? []}
      {@const topics = c.plan?.units.find((p) => p.key === u.key)?.topics ?? []}
      <section class="card module">
        <header class="module-head">
          <span class="wk-badge">Week {u.week}</span>
          <h2>{u.title}</h2>
          <span class="small muted nowrap">{unitDone(u.items)} of {u.items.length} done</span>
        </header>
        {#if objectives.length}
          <div class="objectives">
            <span class="small muted">By the end of this week you can:</span>
            <ul>{#each objectives as o}<li>{o}</li>{/each}</ul>
          </div>
        {:else if topics.length}
          <p class="small muted">Topics: {topics.join(" · ")}</p>
        {/if}
        <ul class="module-items">
          {#each u.items as it (it.id)}
            {@const s = statusOf(it)}
            <li class:locked={it.state === "locked"}>
              <span class="icon-box small-box"><KindIcon kind={it.kind} /></span>
              <span class="mi-text">
                {#if it.state === "locked"}
                  <span class="mi-title">{it.title}</span>
                {:else}
                  <a class="mi-title" href="/item/{encodeURIComponent(it.id)}">{it.title}</a>
                {/if}
                <span class="small muted">{kindLabel[it.kind]}{it.kind !== "lesson" ? ` · ${Math.round(it.weight * 1000) / 10}% of grade` : ""}</span>
              </span>
              <span class="status {s.tone}">{s.label}</span>
            </li>
          {/each}
        </ul>
      </section>
    {/each}

  <!-- ===== Syllabus ===== -->
  {:else if tab === "syllabus"}
    <section class="card syllabus">
      <h2>About this course</h2>
      <p>{c.plan?.summary ?? c.subject}</p>
      <dl class="facts">
        <div><dt>Instructor</dt><dd>{c.origin === "manual" ? (c.meta.instructor || "You") : "Claude (AI), through your own Claude Code"}</dd></div>
        {#if c.meta.term}<div><dt>Term</dt><dd>{c.meta.term}</dd></div>{/if}
        <div><dt>Dates</dt><dd>{day(c.starts_at)} – {day(c.ends_at)} ({weeks} {weeks === 1 ? "week" : "weeks"})</dd></div>
        <div><dt>Mode</dt><dd>{c.strictness === "sandbox" ? "Sandbox (practice)" : "Structured"}{c.mode === "online" ? " · needs internet" : ""}</dd></div>
      </dl>

      {#if c.meta.outcomes.length}
        <h3>Learning outcomes</h3>
        <p class="small muted">By the end of the course you will be able to:</p>
        <ol>{#each c.meta.outcomes as o}<li>{o}</li>{/each}</ol>
      {/if}

      <h3>Schedule</h3>
      <div class="table-wrap">
        <table>
          <thead><tr><th>Week</th><th>Unit</th><th>Graded work</th></tr></thead>
          <tbody>
            {#each c.units as u (u.key)}
              {@const topics = c.plan?.units.find((p) => p.key === u.key)?.topics ?? []}
              <tr>
                <td class="nowrap">{u.week}</td>
                <td><strong>{u.title}</strong>{#if topics.length}<br /><span class="small muted">{topics.join(", ")}</span>{/if}</td>
                <td class="small">
                  {#each u.items.filter((i) => i.kind !== "lesson") as it}
                    <div>{kindLabel[it.kind]}: {it.title}{#if it.due_at}{" "}<span class="muted">· due {day(it.due_at)}</span>{/if}</div>
                  {:else}
                    <span class="muted">Reading only</span>
                  {/each}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>

      <h3>How you're graded</h3>
      <div class="weights">
        {#each categories as cat}
          <div class="weight-row">
            <span>{kindLabel[cat.kind]} <span class="small muted">({cat.count})</span></span>
            <span class="bar" aria-hidden="true"><span style="width: {cat.weight * 100}%"></span></span>
            <strong>{Math.round(cat.weight * 100)}%</strong>
          </div>
        {/each}
      </div>
      <p class="small muted">Letter grades: A+ from 90%, A 85, A− 80, B+ 77, B 73, B− 70, C+ 67, C 63, C− 60, D 50–59, F below 50.</p>

      <h3>Course policies</h3>
      <ul>
        <li><strong>Deadlines.</strong> {lateText}</li>
        <li><strong>Attempts.</strong> {c.strictness === "sandbox" ? "Unlimited, with no late penalties; your latest attempt counts. Revealing an item's answer key closes it. Late work is taken until a week after the last deadline." : "One attempt per piece of work. Check it before you submit."}</li>
        <li><strong>Marking.</strong> {markedBy[c.origin]}{c.origin === "claude" ? " AI marking can be wrong: every mark shows its reasoning, and you can talk it through in Review with Claude." : ""}</li>
        <li><strong>Honesty.</strong> This course is for your own learning. Do the work yourself before looking anything up; the marks only mean something if they're yours.</li>
        <li><strong>Accessibility.</strong> Lessons have a distraction-free reading mode, everything works from the keyboard, and the app follows your system's light or dark setting and text size.</li>
      </ul>
    </section>

  <!-- ===== Grades ===== -->
  {:else if tab === "grades"}
    <section class="card">
      <div class="grade-summary">
        <div>
          <span class="small muted">{c.status === "completed" ? "Final mark" : "Current mark"}</span>
          <strong class="big-grade">{pct(c.status === "completed" ? c.final_percent : c.grades.current)}</strong>
          {#if (c.status === "completed" ? c.final_percent : c.grades.current) != null}
            <span class="letter">{letter((c.status === "completed" ? c.final_percent : c.grades.current)!)}</span>
          {/if}
        </div>
        <div class="decided">
          <span class="small muted">{Math.round(c.grades.decided * 100)}% of the final grade is decided</span>
          <div class="bar" aria-hidden="true"><span style="width: {c.grades.decided * 100}%"></span></div>
          <span class="small muted">The current mark averages only the work marked so far, by weight.</span>
        </div>
      </div>
    </section>

    <section class="card">
      <h2>By category</h2>
      <div class="table-wrap">
        <table>
          <thead><tr><th>Category</th><th>Weight</th><th>Marked</th><th>Average</th></tr></thead>
          <tbody>
            {#each categories as cat}
              <tr><td>{kindLabel[cat.kind]}</td><td>{Math.round(cat.weight * 100)}%</td><td>{cat.markedCount} of {cat.count}</td><td>{pct(cat.avg)}</td></tr>
            {/each}
          </tbody>
        </table>
      </div>
    </section>

    <section class="card">
      <h2>All graded work</h2>
      <div class="table-wrap">
        <table>
          <thead><tr><th>Work</th><th>Due</th><th>Weight</th><th>Status</th><th class="num">Score</th></tr></thead>
          <tbody>
            {#each graded as it (it.id)}
              {@const s = statusOf(it)}
              <tr>
                <td><KindIcon kind={it.kind} size={15} /> {#if it.state === "locked"}{it.title}{:else}<a href="/item/{encodeURIComponent(it.id)}">{it.title}</a>{/if}<br /><span class="small muted">{kindLabel[it.kind]}{it.unit_title ? ` · ${it.unit_title}` : ""}</span></td>
                <td class="small nowrap">{day(it.due_at)}</td>
                <td class="small">{Math.round(it.weight * 1000) / 10}%</td>
                <td><span class="status {s.tone}">{it.state === "marked" ? "Marked" : s.label}</span></td>
                <td class="num"><strong>{it.percent != null ? pct(it.percent, 0) : "–"}</strong></td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </section>

  <!-- ===== Materials ===== -->
  {:else if tab === "materials"}
    <Materials courseId={c.id} status={c.status} />
    {#if c.sources.length}
      <section class="card">
        <h2>Reading list</h2>
        <ul>
          {#each c.sources as s}
            <li>
              <a href={s.url} target="_blank" rel="noreferrer">{s.title || s.url}</a>
              <span class="small muted">· {s.licence}{s.free ? "" : " · get this yourself"}</span>
            </li>
          {/each}
        </ul>
      </section>
    {/if}
    <section class="card">
      <h2>Lessons</h2>
      <ul class="lesson-list">
        {#each lessons as l (l.id)}
          <li><KindIcon kind="lesson" size={15} /> {#if l.state === "locked"}<span class="muted">{l.title} (opens {day(l.release_at)})</span>{:else}<a href="/item/{encodeURIComponent(l.id)}">{l.title}</a>{/if}</li>
        {/each}
      </ul>
    </section>
  {/if}
</div>

<style>
  .classroom {
    --course: var(--accent);
    --course-tint: var(--accent-soft);
  }
  .grade-badge {
    background: rgb(255 255 255 / 0.16);
    border: 1px solid rgb(255 255 255 / 0.35);
    border-radius: var(--radius);
    padding: 0.5rem 0.9rem;
    text-align: right;
    display: flex;
    flex-direction: column;
    min-width: 7rem;
  }
  .grade-badge strong {
    font-size: 1.7rem;
    line-height: 1.1;
  }
  .grade-badge span {
    font-size: 0.8rem;
    opacity: 0.9;
  }
  .tabs {
    display: flex;
    gap: 0.25rem;
    align-items: center;
    flex-wrap: wrap;
    border-bottom: 1px solid var(--line);
    margin-bottom: 1rem;
  }
  .tabs button {
    background: transparent;
    border: 0;
    border-bottom: 3px solid transparent;
    border-radius: 0;
    color: var(--muted);
    padding: 0.55rem 0.8rem;
    font-weight: 600;
  }
  .tabs button:hover {
    color: var(--text);
  }
  .tabs button.active {
    color: var(--text);
    border-bottom-color: var(--course);
  }
  .chips {
    margin-left: auto;
    display: flex;
    gap: 0.35rem;
    flex-wrap: wrap;
  }
  .chip {
    font-size: 0.75rem;
    padding: 0.1rem 0.55rem;
    border-radius: 999px;
    border: 1px solid var(--line);
    color: var(--muted);
  }
  .chip.warn {
    color: var(--warn);
  }
  .done-card {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
    flex-wrap: wrap;
    border-left: 4px solid var(--good);
  }
  .done-card h2 {
    margin: 0;
  }
  .home {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(15rem, 20rem);
    gap: 1rem;
    align-items: start;
  }
  @media (max-width: 860px) {
    .home {
      grid-template-columns: 1fr;
    }
  }
  .main-col > :global(.card),
  .side-col > :global(.card) {
    margin-top: 0;
  }
  .welcome {
    border-top: 4px solid var(--course);
  }
  .how {
    padding-left: 1.2rem;
  }
  .how li {
    margin: 0.3rem 0;
  }
  .next {
    display: flex;
    align-items: center;
    gap: 0.9rem;
    padding: 0.8rem;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    text-decoration: none;
    color: inherit;
  }
  .next:hover,
  .next:focus-visible {
    border-color: var(--course);
  }
  .next-text {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    flex: 1;
    min-width: 0;
  }
  .go {
    font-size: 1.3rem;
    color: var(--course);
  }
  .icon-box {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 2.6rem;
    height: 2.6rem;
    border-radius: 10px;
    background: var(--course-tint);
    color: var(--course);
    flex: none;
  }
  .small-box {
    width: 2.1rem;
    height: 2.1rem;
    border-radius: 8px;
  }
  .status {
    font-size: 0.8rem;
    padding: 0.1rem 0.55rem;
    border-radius: 999px;
    white-space: nowrap;
    align-self: flex-start;
    border: 1px solid var(--line);
    color: var(--muted);
  }
  .status.done {
    color: var(--good);
    border-color: color-mix(in srgb, var(--good) 40%, transparent);
  }
  .status.due {
    color: var(--warn);
    border-color: color-mix(in srgb, var(--warn) 45%, transparent);
    font-weight: 600;
  }
  .status.late,
  .status.bad {
    color: var(--bad);
    border-color: color-mix(in srgb, var(--bad) 45%, transparent);
  }
  .status.open {
    color: var(--text);
  }
  .bar {
    display: block;
    height: 0.45rem;
    background: var(--line);
    border-radius: 999px;
    overflow: hidden;
  }
  .bar > span {
    display: block;
    height: 100%;
    background: var(--course);
    border-radius: inherit;
  }
  .bar.big {
    height: 0.65rem;
    margin: 0.4rem 0 1rem;
  }
  .unit-progress {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .unit-progress li {
    display: grid;
    grid-template-columns: 4.2rem minmax(0, 1fr) 6rem 2.5rem;
    gap: 0.6rem;
    align-items: center;
    padding: 0.3rem 0;
  }
  .wk {
    font-size: 0.8rem;
    color: var(--muted);
  }
  .ut {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .agenda {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .agenda > li {
    padding: 0.4rem 0;
    border-bottom: 1px solid var(--line);
  }
  .agenda > li:last-child {
    border-bottom: 0;
  }
  .agenda-day {
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--course);
  }
  .agenda ul {
    list-style: none;
    padding: 0;
    margin: 0.2rem 0 0;
  }
  .agenda ul li {
    padding: 0.15rem 0;
    font-size: 0.92rem;
  }
  .recent {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .recent li {
    display: flex;
    justify-content: space-between;
    gap: 0.5rem;
    padding: 0.3rem 0;
  }
  .module-head {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex-wrap: wrap;
  }
  .module-head h2 {
    margin: 0;
    flex: 1;
    min-width: 12rem;
  }
  .wk-badge {
    background: var(--course);
    color: #fff;
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.03em;
    padding: 0.2rem 0.6rem;
    border-radius: 6px;
    text-transform: uppercase;
  }
  .objectives {
    margin: 0.6rem 0 0.2rem;
    padding: 0.6rem 0.8rem;
    background: var(--course-tint);
    border-radius: 8px;
    color: var(--text);
  }
  .objectives ul {
    margin: 0.2rem 0 0;
    padding-left: 1.2rem;
  }
  @media (prefers-color-scheme: dark) {
    .objectives {
      background: color-mix(in srgb, var(--course) 18%, transparent);
    }
    .icon-box {
      background: color-mix(in srgb, var(--course) 22%, transparent);
      color: color-mix(in srgb, var(--course) 40%, white);
    }
  }
  .module-items {
    list-style: none;
    padding: 0;
    margin: 0.6rem 0 0;
  }
  .module-items li {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.55rem 0;
    border-top: 1px solid var(--line);
  }
  .module-items li.locked {
    opacity: 0.65;
  }
  .mi-text {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .mi-title {
    font-weight: 600;
  }
  .facts {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(13rem, 1fr));
    gap: 0.6rem 1.2rem;
    margin: 1rem 0;
  }
  .facts div {
    display: flex;
    flex-direction: column;
  }
  .facts dt {
    font-size: 0.78rem;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .facts dd {
    margin: 0.1rem 0 0;
  }
  .syllabus h3 {
    margin-top: 1.6rem;
    padding-top: 0.6rem;
    border-top: 1px solid var(--line);
  }
  .weights {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    max-width: 32rem;
  }
  .weight-row {
    display: grid;
    grid-template-columns: 9rem 1fr 3rem;
    gap: 0.75rem;
    align-items: center;
  }
  .weight-row strong {
    text-align: right;
  }
  .grade-summary {
    display: flex;
    gap: 2rem;
    flex-wrap: wrap;
    align-items: center;
  }
  .grade-summary > div:first-child {
    display: flex;
    flex-direction: column;
  }
  .big-grade {
    font-size: 2.6rem;
    line-height: 1.1;
    color: var(--course);
  }
  .letter {
    font-weight: 700;
  }
  .decided {
    flex: 1;
    min-width: 14rem;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }
  .num {
    text-align: right;
  }
  .nowrap {
    white-space: nowrap;
  }
  .lesson-list {
    list-style: none;
    padding: 0;
  }
  .lesson-list li {
    padding: 0.25rem 0;
  }
</style>
