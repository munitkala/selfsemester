<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { api, live, type CourseView, type LatePolicy, type Strictness } from "#lib/api.ts";
  import StrictnessChoice from "#lib/StrictnessChoice.svelte";
  import Materials from "#lib/Materials.svelte";
  import Classroom from "#lib/Classroom.svelte";
  import ProgressBar from "#lib/Progress.svelte";
  import { day, pct, letter, statusLabel, stageLabel, when, todayIso, kindLabel } from "#lib/format.ts";

  const id = $derived(page.params.id ?? "");
  let c = $state<CourseView | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);

  // intake answers
  let answers = $state<Record<string, string>>({});
  // review choices
  let startDate = $state(todayIso(1));
  let lateKind = $state<"lock_zero" | "penalty">("lock_zero");
  let perDay = $state(10);
  let maxDays = $state(3);
  let onTranscript = $state(true);
  let strictness = $state<Strictness>("structured");
  let confirmDelete = $state(false);

  $effect(() => {
    const courseId = id;
    return live(async () => {
      try {
        c = await api.course(courseId);
        error = null;
      } catch (e) {
        error = String(e);
      }
    });
  });

  async function act(f: () => Promise<unknown>) {
    busy = true;
    error = null;
    try {
      await f();
      c = await api.course(id);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  function submitAnswers(e: Event) {
    e.preventDefault();
    act(() => api.answerIntake(id, $state.snapshot(answers)));
  }

  function approve(e: Event) {
    e.preventDefault();
    const policy: LatePolicy = lateKind === "lock_zero" ? { kind: "lock_zero" } : { kind: "penalty", per_day: perDay, max_days: maxDays };
    act(() => api.approve(id, startDate, policy, strictness, onTranscript));
  }

  async function remove() {
    await act(() => api.remove(id));
    goto("/courses");
  }

  const working = $derived(c && (c.jobs.running > 0 || c.jobs.queued > 0));
  const plan = $derived(c?.plan ?? null);
  const unitsDone = $derived(c ? c.units.filter((u) => u.state === "validated").length : 0);
  const warnings = $derived(c ? c.units.flatMap((u) => (u.warnings ?? []).map((w) => `${u.title}: ${w}`)) : []);
  const bandClass = (b: string) => (b.startsWith("High") ? "good" : b.startsWith("Medium") ? "" : "warn");
</script>

{#if error}<p class="note bad">{error}</p>{/if}

{#if c}
  {@const running = c.status === "active" || c.status === "completed"}
  {#if running}
    <Classroom {c} />
  {:else}
  <div class="spread">
    <div>
      <h1>{c.title}</h1>
      <p class="lead">
        {statusLabel[c.status] ?? c.status}
        {#if c.status === "active" || c.status === "completed"}
          · {day(c.starts_at)} – {day(c.ends_at)}
        {/if}
        {#if c.mode === "online"} · <span class="warn">needs internet</span>{/if}
        {#if c.strictness === "sandbox"} · <span class="pill">Sandbox</span>{/if}
        {#if c.origin === "manual"} · <span class="pill">Written by you</span>{/if}
      </p>
    </div>
    {#if c.status === "active" || c.status === "completed"}
      <div class="grade" aria-label="Current mark">
        {#if c.status === "completed" && c.final_percent != null}
          <strong>{pct(c.final_percent)}</strong><span>{letter(c.final_percent)} · final</span>
        {:else if c.grades.current != null}
          <strong>{pct(c.grades.current)}</strong><span>{letter(c.grades.current)} · {Math.round(c.grades.decided * 100)}% of grade decided</span>
        {:else}
          <strong>-</strong><span>no marks yet</span>
        {/if}
      </div>
    {/if}
  </div>
  {/if}

  {#if c.error}
    <div class="note bad">
      <p>{c.error}</p>
      <button onclick={() => act(() => api.retry(id))} disabled={busy}>Retry</button>
    </div>
  {/if}

  {#if c.jobs.waiting_usage}
    <div class="note row">
      <span>Paused because your Claude plan's usage limit was reached. It tries again automatically at {when(c.jobs.next_attempt_at)}. If your limit has already reset, resume now.</span>
      <button onclick={() => act(() => api.resume(id))} disabled={busy}>Resume now</button>
    </div>
  {:else if working}
    <p class="note info"><span class="spinner" aria-hidden="true"></span> {c.jobs.running_kind ? `Claude is ${stageLabel[c.jobs.running_kind] ?? "working"}…` : "Waiting for the next step to start…"} You can leave this page; it carries on in the background.</p>
  {/if}
  {#if c.progress}
    <ProgressBar p={c.progress} />
  {/if}

  <!-- 0. A course still being written in the course builder -->
  {#if c.status === "draft"}
    <section class="card">
      <p>This course is a draft in the course builder.</p>
      <a class="button" href="/build/{encodeURIComponent(id)}">Open it in the course builder</a>
    </section>

  <!-- 1. Intake -->
  {:else if c.status === "intake"}
    <section class="card"><p>Claude is reading your request and writing a few questions about your goals and level.</p></section>
  {:else if c.status === "answering" && c.questions}
    <form class="card" onsubmit={submitAnswers}>
      <h2>A few questions first</h2>
      {#each c.questions.questions as q (q.id)}
        <div class="field">
          <label for={q.id}>{q.text}</label>
          {#if q.kind === "choice" && q.options.length}
            <select id={q.id} bind:value={answers[q.id]}>
              <option value="" disabled selected>Choose…</option>
              {#each q.options as o}<option value={o}>{o}</option>{/each}
            </select>
          {:else if q.kind === "number"}
            <input id={q.id} type="number" min="0" step="any" bind:value={answers[q.id]} />
          {:else if q.kind === "date"}
            <input id={q.id} type="date" bind:value={answers[q.id]} />
          {:else}
            <textarea id={q.id} rows="2" bind:value={answers[q.id]}></textarea>
          {/if}
        </div>
      {/each}
      <button type="submit" disabled={busy}>Plan my course</button>
    </form>

  <!-- 2. Scope -->
  {:else if c.status === "scoping"}
    <section class="card"><p>Claude is planning the syllabus, pacing and assessments, and estimating how well it can teach and mark this course.</p></section>

  <!-- 3. Review: readiness report -->
  {:else if c.status === "review" && plan}
    {#if plan.bundle.detected && !c.bundle_choice}
      <section class="card">
        <h2>This looks like more than one course</h2>
        <p>{plan.bundle.parts.join(" · ")}</p>
        <p class="muted">{plan.bundle.recommendation}</p>
        <div class="row">
          <button onclick={() => act(() => api.chooseBundle(id, "first"))} disabled={busy}>Start with the first one</button>
          <button class="ghost" onclick={() => act(() => api.chooseBundle(id, "combined"))} disabled={busy}>One combined course</button>
          <button class="ghost" onclick={() => act(() => api.chooseBundle(id, "sequence"))} disabled={busy}>Separate courses, one after another</button>
        </div>
      </section>
    {/if}

    <section class="card">
      <h2>{plan.title}</h2>
      <p>{plan.summary}</p>
      <p class="small muted">
        {plan.pacing.total_weeks} {plan.pacing.total_weeks === 1 ? "week" : "weeks"} · about {plan.pacing.hours_per_week} hours a week · {plan.units.length} units
      </p>
      {#if !plan.pacing.fits_user_hours}<p class="note">{plan.pacing.note || "Your hours per week may not fit this syllabus."}</p>{:else if plan.pacing.note}<p class="small muted">{plan.pacing.note}</p>{/if}
      {#if plan.dependencies.length}
        <h3>You'll need</h3>
        <ul>{#each plan.dependencies as d}<li>{d}</li>{/each}</ul>
      {/if}
      {#if plan.unsupported_inputs.length}
        <p class="note">This version leaves out work the app can't capture yet: {plan.unsupported_inputs.map((s) => s.replace(/[.\s]+$/, "")).join("; ")}.</p>
      {/if}
    </section>

    <section class="card">
      <h2>How well Claude can run this course</h2>
      <p class="small muted">{plan.readiness.basis} These are starting estimates, never 100%.</p>
      <div class="bands">
        <div><span class="muted small">Delivery</span><strong class={bandClass(plan.readiness.overall_delivery)}>{plan.readiness.overall_delivery}</strong></div>
        <div><span class="muted small">Marking</span><strong class={bandClass(plan.readiness.overall_marking)}>{plan.readiness.overall_marking}</strong></div>
      </div>
      <div class="table-wrap">
        <table>
          <thead><tr><th>Step</th><th>Estimate</th><th>Why</th></tr></thead>
          <tbody>
            {#each [...plan.readiness.delivery.map((s) => ({ ...s, group: "Teach" })), ...plan.readiness.marking.map((s) => ({ ...s, group: "Mark" }))] as s}
              <tr>
                <td><span class="muted small">{s.group}</span><br />{s.step}</td>
                <td class="nowrap"><span class={bandClass(s.band)}>{s.band}</span> <span class="muted">~{Math.min(99, Math.round(s.percent))}%</span></td>
                <td class="small">{s.note}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
      {#if plan.readiness.weak_spots.length}
        <h3>Weak spots</h3>
        <ul>{#each plan.readiness.weak_spots as w}<li>{w}</li>{/each}</ul>
      {/if}
    </section>

    <section class="card">
      <h2>Syllabus</h2>
      <div class="table-wrap">
        <table>
          <thead><tr><th>Week</th><th>Unit</th><th>Topics</th></tr></thead>
          <tbody>
            {#each plan.units as u}
              <tr>
                <td>{u.week}</td>
                <td>{u.title}{#if u.exam !== "none"} <span class="pill">{u.exam}</span>{/if}</td>
                <td class="small">{u.topics.join(", ")}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
      <p class="small muted">
        Grade weights:
        {Object.entries(plan.weights).filter(([, w]) => w > 0).map(([k, w]) => `${kindLabel[k] ?? k} ${Math.round(w * 100)}%`).join(" · ")}
      </p>
    </section>

    <form class="card" onsubmit={approve}>
      <h2>Enrol</h2>
      <div class="field">
        <label for="start">Start date</label>
        <input id="start" type="date" bind:value={startDate} min={todayIso(0)} required style="max-width: 12rem" />
      </div>
      <fieldset class="field">
        <legend>When a deadline passes</legend>
        <label class="radio"><input type="radio" bind:group={lateKind} value="lock_zero" /> Lock the item and record zero</label>
        <label class="radio"><input type="radio" bind:group={lateKind} value="penalty" /> Accept late work with a penalty</label>
        {#if lateKind === "penalty"}
          <div class="row indent">
            <label class="inline">−<input type="number" min="1" max="100" bind:value={perDay} /> points per day,</label>
            <label class="inline">for up to <input type="number" min="1" max="14" bind:value={maxDays} /> days</label>
          </div>
        {/if}
      </fieldset>
      <StrictnessChoice bind:value={strictness} />
      <label class="radio"><input type="checkbox" bind:checked={onTranscript} /> Put the final mark on my transcript</label>
      <p class="small muted">Approving builds the whole course now: Claude finds free sources, then writes and checks every unit. This uses a fair amount of your plan's usage; if a limit is hit, it pauses and continues later.</p>
      <button type="submit" disabled={busy}>Approve and build the course</button>
    </form>

  <!-- 4. Building -->
  {:else if c.status === "compiling"}
    <section class="card">
      <h2>Building your course</h2>
      <p class="muted">{c.sources.length ? `${c.sources.length} sources found. ` : "Finding free sources first. "}{unitsDone} of {c.units.length} units written and checked.</p>
      <ul class="units">
        {#each c.units as u}
          <li>
            <span>Week {u.week}: {u.title}</span>
            <span class="pill {u.state === 'validated' ? 'done' : u.state === 'generated' ? 'submitted' : ''}">
              {u.state === "validated" ? "Ready" : u.state === "generated" ? "Checking" : "Waiting"}
            </span>
          </li>
        {/each}
      </ul>
    </section>

  <!-- 5. Studying: the classroom above has it all -->
  {:else if running}
    {#if warnings.length}
      <details class="card">
        <summary>Checker notes ({warnings.length})</summary>
        <p class="small muted">Minor issues a second model flagged when checking the course. Keep them in mind; dispute a mark if one affected you.</p>
        <ul class="small">{#each warnings as w}<li>{w}</li>{/each}</ul>
      </details>
    {/if}
  {/if}

  {#if !running}
    <Materials courseId={id} status={c.status} />
  {/if}

  {#if c.sources.length && c.status !== "compiling" && !running}
    <details class="card">
      <summary>Reading list ({c.sources.length})</summary>
      <ul>
        {#each c.sources as s}
          <li>
            <a href={s.url} target="_blank" rel="noreferrer">{s.title || s.url}</a>
            <span class="small muted">· {s.licence}{s.free ? "" : " · get this yourself"}</span>
          </li>
        {/each}
      </ul>
    </details>
  {/if}

  <div class="danger-zone">
    {#if confirmDelete}
      <span>Delete this course and its files? Your archive in Documents is kept.</span>
      <button class="danger" onclick={remove} disabled={busy}>Delete</button>
      <button class="ghost" onclick={() => (confirmDelete = false)}>Cancel</button>
    {:else}
      <button class="ghost small" onclick={() => (confirmDelete = true)}>Delete course…</button>
    {/if}
  </div>
{:else if !error}
  <p class="muted">Loading…</p>
{/if}

<style>
  .grade {
    text-align: right;
    display: flex;
    flex-direction: column;
  }
  .grade strong {
    font-size: 1.8rem;
    line-height: 1.1;
  }
  .grade span {
    font-size: 0.82rem;
    color: var(--muted);
  }
  .bands {
    display: flex;
    gap: 2.5rem;
    margin: 0.5rem 0 1rem;
  }
  .bands div {
    display: flex;
    flex-direction: column;
  }
  .bands strong {
    font-size: 1.3rem;
  }
  .nowrap {
    white-space: nowrap;
  }
  fieldset {
    border: 0;
    padding: 0;
  }
  legend {
    font-weight: 600;
    margin-bottom: 0.3rem;
  }
  .radio {
    font-weight: 400;
    display: flex;
    gap: 0.5rem;
    align-items: center;
    margin: 0.25rem 0;
  }
  .radio input {
    width: auto;
  }
  .inline {
    font-weight: 400;
    display: inline-flex;
    gap: 0.35rem;
    align-items: center;
  }
  .inline input {
    width: 4.5rem;
  }
  .indent {
    margin-left: 1.6rem;
  }
  .units {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .units li {
    display: flex;
    justify-content: space-between;
    padding: 0.4rem 0;
    border-bottom: 1px solid var(--line);
  }
  details summary {
    cursor: pointer;
    font-weight: 600;
  }
  .danger-zone {
    margin-top: 2rem;
    display: flex;
    gap: 0.75rem;
    align-items: center;
    flex-wrap: wrap;
  }
  .note p {
    margin: 0 0 0.5rem;
  }
</style>
