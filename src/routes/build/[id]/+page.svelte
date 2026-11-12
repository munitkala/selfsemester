<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { api, type Draft, type DraftUnit, type LatePolicy, type Strictness } from "#lib/api.ts";
  import { render } from "#lib/markdown.ts";
  import { kindLabel, todayIso } from "#lib/format.ts";
  import { PALETTE, codeOf } from "#lib/course.ts";
  import StrictnessChoice from "#lib/StrictnessChoice.svelte";
  import CourseBanner from "#lib/CourseBanner.svelte";
  import KindIcon from "#lib/KindIcon.svelte";
  import Icon from "#lib/Icon.svelte";
  import ListEditor from "#lib/ListEditor.svelte";
  import ItemEditor from "#lib/ItemEditor.svelte";
  import AttachmentEditor from "#lib/AttachmentEditor.svelte";

  const id = $derived(page.params.id ?? "");
  let draft = $state<Draft | null>(null);
  let problems = $state<string[]>([]);
  let error = $state<string | null>(null);
  let saved = $state<string>(""); // JSON of the last saved draft
  let saving = $state(false);
  let busy = $state(false);
  let exported = $state<string | null>(null);
  let preview = $state<Record<string, boolean>>({});
  let confirmDelete = $state(false);
  let confirmRemove = $state<string | null>(null);
  let openUnit = $state(0);

  // publish choices
  let startDate = $state(todayIso(1));
  let lateKind = $state<"lock_zero" | "penalty">("lock_zero");
  let perDay = $state(10);
  let maxDays = $state(3);
  let strictness = $state<Strictness>("structured");
  let onTranscript = $state(true);

  const KINDS = ["homework", "assignment", "quiz", "test", "exam", "project"] as const;
  // The first three are all a course needs; the last two can wait, or be skipped.
  const STEPS = [
    { id: "start", label: "Name it" },
    { id: "units", label: "Lessons and work" },
    { id: "publish", label: "Publish" },
    { id: "details", label: "Course details (optional)" },
    { id: "grading", label: "Grading (optional)" },
  ];

  $effect(() => {
    const courseId = id;
    api
      .builderGet(courseId)
      .then((r) => {
        // drafts from before course details existed
        r.draft.meta ??= { code: "", term: "", instructor: "", color: "", welcome: "", outcomes: [], objectives: {} };
        r.draft.units.forEach((u) => {
          u.objectives ??= [];
          u.lesson_files ??= [];
          u.items.forEach((it) => (it.files ??= []));
        });
        draft = r.draft;
        problems = r.problems;
        saved = JSON.stringify(r.draft);
      })
      .catch((e) => (error = String(e)));
  });

  // Save a moment after the last change.
  $effect(() => {
    if (!draft) return;
    const now = JSON.stringify(draft);
    if (now === saved) return;
    const t = setTimeout(save, 800);
    return () => clearTimeout(t);
  });

  async function save() {
    if (!draft) return;
    const snap = $state.snapshot(draft) as Draft;
    saving = true;
    try {
      problems = await api.builderSave(id, snap);
      saved = JSON.stringify(snap);
      error = null;
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }

  const dirty = $derived(!!draft && JSON.stringify(draft) !== saved);

  // ---- step checks for the outline ----
  const stepDone = $derived.by(() => {
    if (!draft) return {} as Record<string, boolean>;
    const unitsOk = draft.units.length > 0 && !problems.some((p) => p.startsWith("Unit") || p.startsWith("Add at least"));
    return {
      start: !!draft.title.trim(),
      details: !!draft.summary.trim() && draft.meta.outcomes.some((o) => o.trim()),
      grading: weightTotal > 0,
      units: unitsOk,
      publish: problems.length === 0,
    } as Record<string, boolean>;
  });

  // ---- editing helpers ----
  function move<T>(list: T[], i: number, by: number) {
    const j = i + by;
    if (j < 0 || j >= list.length) return;
    [list[i], list[j]] = [list[j], list[i]];
  }

  function addUnit() {
    if (!draft) return;
    const week = Math.max(0, ...draft.units.map((u) => u.week)) + 1;
    draft.units.push({ title: `Week ${week}`, week, topics: [], objectives: [], lesson_title: "", lesson: "", lesson_files: [], est_minutes: 30, items: [] });
    openUnit = draft.units.length - 1;
  }

  function addItem(u: DraftUnit, kind: (typeof KINDS)[number]) {
    u.items.push({ key: "", kind, title: kindLabel[kind], module: "", instructions: "", questions: [], rubric: [], answer_key: [], due_days: kind === "exam" ? 2 : 7, files: [] });
  }

  /** A text or Markdown document becomes the lesson's text (added after anything already written). */
  function useAsLesson(u: DraftUnit, text: string, name: string) {
    u.lesson = u.lesson.trim() ? `${u.lesson.trim()}\n\n${text.trim()}` : text.trim();
    if (!u.lesson_title.trim()) u.lesson_title = name.replace(/\.(md|markdown|txt)$/i, "");
  }

  const weightTotal = $derived(draft ? Object.values(draft.weights).reduce((a, b) => a + (Number(b) || 0), 0) : 0);
  // kinds the course actually uses, so the weights shown add up the way the schedule will scale them
  const usedKinds = $derived(new Set(draft?.units.flatMap((u) => u.items.map((i) => i.kind)) ?? []));
  const usedTotal = $derived(draft ? [...usedKinds].reduce((a, k) => a + (Number(draft!.weights[k as (typeof KINDS)[number]]) || 0), 0) : 0);

  // ---- actions ----
  async function exportFolder() {
    await save();
    busy = true;
    try {
      exported = await api.builderExport(id);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function publish(e: Event) {
    e.preventDefault();
    await save();
    busy = true;
    error = null;
    try {
      const policy: LatePolicy = lateKind === "lock_zero" ? { kind: "lock_zero" } : { kind: "penalty", per_day: perDay, max_days: maxDays };
      await api.builderPublish(id, startDate, policy, strictness, onTranscript);
      goto(`/course/${encodeURIComponent(id)}`);
    } catch (err) {
      error = String(err);
      busy = false;
    }
  }

  async function remove() {
    busy = true;
    try {
      await api.remove(id);
      goto("/build");
    } catch (e) {
      error = String(e);
      busy = false;
    }
  }
</script>

{#if error}<p class="note bad">{error}</p>{/if}

{#if draft}
  <div class="top">
    <p class="crumbs small"><a href="/build">Course builder</a> · draft</p>
    <span class="save-state small" class:dirty>{saving ? "Saving…" : dirty ? "Unsaved changes" : "All changes saved"}</span>
  </div>

  <div class="layout">
    <nav class="outline" aria-label="Builder steps">
      <ol>
        {#each STEPS as s, i}
          <li class:done={stepDone[s.id]}>
            <a href="#{s.id}"><span class="dot" aria-hidden="true">{#if stepDone[s.id]}<Icon name="check" size={13} />{:else}{i + 1}{/if}</span>{s.label}</a>
          </li>
        {/each}
      </ol>
      <div class="mini-preview">
        <CourseBanner id={id} title={draft.title} meta={draft.meta} origin="manual" compact />
        <div class="mini-body small">
          <span>{draft.units.length} {draft.units.length === 1 ? "week" : "weeks"} · {draft.units.reduce((a, u) => a + u.items.length, 0)} graded</span>
        </div>
      </div>
    </nav>

    <div class="content">
      <!-- 1. The only thing needed up front: a name -->
      <section class="card step" id="start">
        <h2><span class="num">1</span> Name it</h2>
        <p class="small muted">
          A course only needs a name and a first lesson to start. Everything else (more weeks, homework, quizzes, exams,
          course details and grading) is optional, and can be added now or after the course is published.
        </p>
        <div class="field">
          <label for="c-title">Course title</label>
          <input id="c-title" bind:value={draft.title} />
        </div>
      </section>

      <!-- Optional: course details -->
      <details class="card step optional" id="details">
        <summary><h2>Course details <span class="opt">optional</span></h2><span class="small muted">Code, term, instructor, colour, description, welcome message and outcomes</span></summary>
        <div class="cols">
          <div class="field">
            <label for="c-code">Course code <span class="hint">shown on the card</span></label>
            <input id="c-code" bind:value={draft.meta.code} placeholder={codeOf(null, draft.title)} maxlength="16" />
          </div>
          <div class="field">
            <label for="c-term">Term <span class="hint">optional</span></label>
            <input id="c-term" bind:value={draft.meta.term} placeholder="e.g. Winter 2027" maxlength="30" />
          </div>
          <div class="field">
            <label for="c-instr">Instructor</label>
            <input id="c-instr" bind:value={draft.meta.instructor} placeholder="Your name or a nickname" maxlength="40" />
          </div>
        </div>
        <fieldset class="field">
          <legend>Banner colour</legend>
          <div class="swatches">
            {#each PALETTE as p}
              <label class="swatch" style="--sw: {p.deep}" title={p.label}>
                <input type="radio" bind:group={draft.meta.color} value={p.name} aria-label={p.label} />
                <span></span>
              </label>
            {/each}
          </div>
        </fieldset>
        <div class="field">
          <label for="c-subject">Subject <span class="hint">a few words, e.g. "Chess openings"</span></label>
          <input id="c-subject" bind:value={draft.subject} />
        </div>
        <div class="field">
          <label for="c-summary">Course description <span class="hint">what the course covers and who it's for</span></label>
          <textarea id="c-summary" rows="3" bind:value={draft.summary}></textarea>
        </div>
        <div class="field">
          <div class="spread">
            <label for="c-welcome">Welcome message <span class="hint">first thing on the course home page; Markdown</span></label>
            <button class="ghost small" type="button" onclick={() => (preview.welcome = !preview.welcome)}>{preview.welcome ? "Edit" : "Preview"}</button>
          </div>
          {#if preview.welcome}
            <div class="prose preview">{@html render(draft.meta.welcome || "_Nothing yet._")}</div>
          {:else}
            <textarea id="c-welcome" rows="4" bind:value={draft.meta.welcome} placeholder="Welcome! Here's how to get started…"></textarea>
          {/if}
        </div>
        <div class="field">
          <span class="label">Learning outcomes <span class="hint">by the end of the course, learners will be able to…</span></span>
          <ListEditor bind:items={draft.meta.outcomes} label="Learning outcome" placeholder="e.g. Play the first ten moves of the Italian with a plan" add="Add an outcome" />
        </div>
      </details>

      <!-- Optional: grading -->
      <details class="card step optional" id="grading">
        <summary><h2>Grading <span class="opt">optional</span></h2><span class="small muted">How much each kind of work counts. The defaults work; only kinds the course uses count, scaled to 100%.</span></summary>
        <div class="weights">
          {#each KINDS as k}
            {@const share = usedKinds.has(k) && usedTotal > 0 ? (Number(draft.weights[k]) || 0) / usedTotal : 0}
            <div class="weight-row" class:unused={!usedKinds.has(k)}>
              <label for="w-{k}"><KindIcon kind={k} size={16} /> {kindLabel[k]}</label>
              <input id="w-{k}" type="number" min="0" step="5" value={Math.round((Number(draft.weights[k]) || 0) * 100)} oninput={(e) => (draft!.weights[k] = (Number((e.currentTarget as HTMLInputElement).value) || 0) / 100)} />
              <span class="bar" aria-hidden="true"><span style="width: {share * 100}%"></span></span>
              <span class="small nowrap">{usedKinds.has(k) ? `${Math.round(share * 100)}% of the grade` : "not used yet"}</span>
            </div>
          {/each}
        </div>
      </details>

      <!-- 2. Weeks and work -->
      <section class="step" id="units">
        <div class="spread step-head">
          <h2><span class="num">2</span> Lessons and work</h2>
          <button class="ghost" onclick={addUnit}>Add a week</button>
        </div>
        <p class="small muted">
          Start with one week and its lesson: write it here, attach a document you already have (a PDF, Word file or
          slides), or both. Graded work and more weeks are optional; you can add them now, or from the course's Modules
          tab after it's published.
        </p>

        {#each draft.units as u, ui (ui)}
          <section class="card unit" class:open={openUnit === ui}>
            <button class="unit-head" type="button" onclick={() => (openUnit = openUnit === ui ? -1 : ui)} aria-expanded={openUnit === ui}>
              <span class="wk-badge">Week {u.week}</span>
              <strong>{u.title || "Untitled week"}</strong>
              <span class="small muted">{u.items.length} graded · {u.lesson.trim() ? "lesson written" : u.lesson_files?.length ? "lesson attached" : "no lesson yet"}</span>
              <span class="caret" aria-hidden="true">{openUnit === ui ? "▾" : "▸"}</span>
            </button>

            {#if openUnit === ui}
              <div class="unit-body">
                <div class="cols">
                  <div class="field">
                    <label for="u-title-{ui}">Week title</label>
                    <input id="u-title-{ui}" bind:value={u.title} />
                  </div>
                  <div class="field narrow">
                    <label for="u-week-{ui}">Week number</label>
                    <input id="u-week-{ui}" type="number" min="1" max="104" bind:value={u.week} />
                  </div>
                </div>
                <details class="more">
                  <summary class="small">More about this week <span class="hint">objectives, topics, lesson title and reading time</span></summary>
                <div class="field">
                  <span class="label">Objectives <span class="hint">by the end of this week, learners can…</span></span>
                  <ListEditor bind:items={u.objectives} label="Objective" placeholder="e.g. Name the main line of the Italian" add="Add an objective" />
                </div>
                <div class="field">
                  <label for="u-topics-{ui}">Topics <span class="hint">separated by commas; shown in the syllabus</span></label>
                  <input id="u-topics-{ui}" value={u.topics.join(", ")} oninput={(e) => (u.topics = (e.currentTarget as HTMLInputElement).value.split(",").map((t) => t.trim()).filter(Boolean))} />
                </div>

                <div class="cols">
                  <div class="field">
                    <label for="u-lt-{ui}">Lesson title <span class="hint">optional</span></label>
                    <input id="u-lt-{ui}" bind:value={u.lesson_title} placeholder={u.title} />
                  </div>
                  <div class="field narrow">
                    <label for="u-min-{ui}">Reading time <span class="hint">min</span></label>
                    <input id="u-min-{ui}" type="number" min="1" bind:value={u.est_minutes} />
                  </div>
                </div>
                </details>

                <h3 class="sub"><KindIcon kind="lesson" /> Lesson</h3>
                <div class="field">
                  <span class="label">Documents <span class="hint">already have the lesson as a PDF, Word file or slides? Attach it instead of typing</span></span>
                  <AttachmentEditor courseId={id} bind:files={u.lesson_files} useAsText={(t, n) => useAsLesson(u, t, n)} hint="Text and Markdown files can become the lesson text." />
                </div>
                <div class="field">
                  <div class="spread">
                    <label for="u-lesson-{ui}">Lesson text <span class="hint">optional if a document covers it; Markdown, $x^2$ for maths</span></label>
                    <button class="ghost small" type="button" onclick={() => (preview[`u${ui}`] = !preview[`u${ui}`])}>{preview[`u${ui}`] ? "Edit" : "Preview"}</button>
                  </div>
                  {#if preview[`u${ui}`]}
                    <div class="prose preview">{@html render(u.lesson || "_Nothing yet._")}</div>
                  {:else}
                    <textarea id="u-lesson-{ui}" rows="10" bind:value={u.lesson}></textarea>
                  {/if}
                </div>

                <h3 class="sub">Graded work <span class="opt">optional</span></h3>
                {#each u.items as it, ii (ii)}
                  <div class="item">
                    <div class="item-head">
                      <span class="kind-chip"><KindIcon kind={it.kind} size={16} /> {kindLabel[it.kind]}</span>
                      <strong>{it.title || "Untitled"}</strong>
                      <span class="small muted">{it.questions.length} {it.questions.length === 1 ? "question" : "questions"} · due {it.due_days} {it.due_days === 1 ? "day" : "days"} after it opens</span>
                    </div>
                    <ItemEditor bind:it={u.items[ii]} prefix="{ui}-{ii}" courseId={id} />

                    <div class="row tools">
                      <button class="ghost small" onclick={() => move(u.items, ii, -1)} disabled={ii === 0}>Move up</button>
                      <button class="ghost small" onclick={() => move(u.items, ii, 1)} disabled={ii === u.items.length - 1}>Move down</button>
                      {#if confirmRemove === `i${ui}-${ii}`}
                        <button class="danger small" onclick={() => { u.items.splice(ii, 1); confirmRemove = null; }}>Remove this work</button>
                        <button class="ghost small" onclick={() => (confirmRemove = null)}>Keep</button>
                      {:else}
                        <button class="ghost small" onclick={() => (confirmRemove = `i${ui}-${ii}`)}>Remove…</button>
                      {/if}
                    </div>
                  </div>
                {/each}
                <div class="row add-work">
                  <span class="small muted">Add:</span>
                  {#each KINDS as k}<button class="ghost small" onclick={() => addItem(u, k)}><KindIcon kind={k} size={14} /> {kindLabel[k]}</button>{/each}
                </div>

                <div class="row tools unit-tools">
                  <button class="ghost small" onclick={() => { move(draft!.units, ui, -1); openUnit = ui - 1; }} disabled={ui === 0}>Move week up</button>
                  <button class="ghost small" onclick={() => { move(draft!.units, ui, 1); openUnit = ui + 1; }} disabled={ui === draft.units.length - 1}>Move week down</button>
                  {#if confirmRemove === `u${ui}`}
                    <button class="danger small" onclick={() => { draft!.units.splice(ui, 1); confirmRemove = null; openUnit = -1; }}>Remove this week</button>
                    <button class="ghost small" onclick={() => (confirmRemove = null)}>Keep</button>
                  {:else}
                    <button class="ghost small" onclick={() => (confirmRemove = `u${ui}`)}>Remove week…</button>
                  {/if}
                </div>
              </div>
            {/if}
          </section>
        {/each}
        <button class="ghost" onclick={addUnit}>Add a week</button>
      </section>

      <!-- 4. Publish -->
      <form class="card step" id="publish" onsubmit={publish}>
        <h2><span class="num">3</span> Publish</h2>
        {#if problems.length}
          <p class="note">A few things to fix first:</p>
          <ul class="small problems">{#each problems as p}<li>{p}</li>{/each}</ul>
        {:else}
          <p class="note good-note">Ready to publish.</p>
        {/if}
        <div class="field">
          <label for="start">Start date</label>
          <input id="start" type="date" bind:value={startDate} min={todayIso(0)} required style="max-width: 12rem" />
        </div>
        <details class="more">
          <summary class="small">Publishing options <span class="hint">late work, attempts and the transcript</span></summary>
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
        </details>
        <p class="small muted">
          Publishing schedules the course from the start date like any other course. Afterwards you can keep adding
          weeks, homework, quizzes and exams from the course's Modules tab; what's already there can't be changed, so
          export a copy first if you might want to rewrite it.
        </p>
        <div class="row">
          <button type="submit" disabled={busy || problems.length > 0 || dirty}>Publish the course</button>
          <button type="button" class="ghost" onclick={exportFolder} disabled={busy}>Export to a folder</button>
        </div>
        {#if exported}
          <p class="small">Saved to {exported} <button type="button" class="ghost small" onclick={() => api.openFolder(exported!)}>Open folder</button></p>
        {/if}
        <p class="small muted">An exported folder can be edited in any text editor and imported back from the course builder page as a new draft.</p>
      </form>

      <div class="danger-zone">
        {#if confirmDelete}
          <span>Delete this draft?</span>
          <button class="danger" onclick={remove} disabled={busy}>Delete</button>
          <button class="ghost" onclick={() => (confirmDelete = false)}>Cancel</button>
        {:else}
          <button class="ghost small" onclick={() => (confirmDelete = true)}>Delete draft…</button>
        {/if}
      </div>
    </div>
  </div>
{:else if !error}
  <p class="muted">Loading…</p>
{/if}

<style>
  .top {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 1rem;
  }
  .crumbs {
    margin: 0 0 0.6rem;
  }
  .save-state {
    color: var(--good);
  }
  .save-state.dirty {
    color: var(--muted);
  }
  .layout {
    display: grid;
    grid-template-columns: 13.5rem minmax(0, 1fr);
    gap: 1.25rem;
    align-items: start;
  }
  @container page (max-width: 54rem) {
    .layout {
      grid-template-columns: minmax(0, 1fr);
    }
    .outline {
      position: static !important;
    }
  }
  .outline {
    position: sticky;
    top: 1rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  .outline ol {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }
  .outline a {
    display: flex;
    gap: 0.6rem;
    align-items: center;
    padding: 0.45rem 0.5rem;
    border-radius: var(--radius-sm);
    text-decoration: none;
    color: var(--text);
  }
  .outline a:hover,
  .outline a:focus-visible {
    background: var(--accent-soft);
  }
  .dot {
    display: inline-flex;
    width: 1.5rem;
    height: 1.5rem;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    border: 1.5px solid var(--line);
    font-size: 0.8rem;
    color: var(--muted);
    flex: none;
  }
  .done .dot {
    background: var(--good);
    border-color: var(--good);
    color: var(--panel);
  }
  .mini-preview {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
    overflow: hidden;
  }
  .mini-body {
    padding: 0.55rem 0.8rem;
    color: var(--muted);
  }
  .step {
    scroll-margin-top: 1rem;
  }
  .step h2 {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }
  .num {
    display: inline-flex;
    width: 1.8rem;
    height: 1.8rem;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    background: var(--accent);
    color: var(--on-accent);
    font-size: 0.95rem;
    flex: none;
  }
  .optional summary,
  .more summary {
    cursor: pointer;
    list-style: none;
  }
  .optional summary::-webkit-details-marker,
  .more summary::-webkit-details-marker {
    display: none;
  }
  .optional summary h2 {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    margin: 0;
  }
  .optional summary h2::before {
    content: "+";
    display: grid;
    place-items: center;
    width: 1.8rem;
    height: 1.8rem;
    border-radius: 50%;
    border: 1.5px dashed var(--accent-line);
    color: var(--accent);
    font-size: 1.1rem;
  }
  .optional[open] summary h2::before {
    content: "−";
  }
  .optional summary .small {
    display: block;
    margin: 0.25rem 0 0 2.3rem;
  }
  .optional[open] summary {
    margin-bottom: 1rem;
  }
  .opt {
    font-size: 0.72rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
    border: 1px solid var(--line);
    border-radius: 999px;
    padding: 0.05rem 0.5rem;
  }
  .more {
    margin: 0.4rem 0 0.8rem;
    padding: 0.5rem 0.75rem;
    border: 1px dashed var(--line);
    border-radius: var(--radius-sm);
  }
  .more summary {
    font-weight: 600;
  }
  .more summary .hint {
    font-weight: 400;
    color: var(--muted);
    margin-left: 0.4rem;
  }
  .more[open] summary {
    margin-bottom: 0.6rem;
  }
  .step-head {
    margin-top: 1.5rem;
  }
  .step-head h2 {
    margin: 0;
  }
  .label {
    display: block;
    font-weight: 600;
    margin-bottom: 0.35rem;
  }
  .cols {
    display: flex;
    gap: 0.75rem;
    flex-wrap: wrap;
    align-items: flex-end;
  }
  .cols > .field {
    flex: 1 1 12rem;
  }
  .cols > .field.narrow {
    flex: 0 1 8rem;
  }
  .swatches {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }
  .swatch {
    position: relative;
    cursor: pointer;
  }
  .swatch input {
    position: absolute;
    opacity: 0;
    inset: 0;
    margin: 0;
    cursor: pointer;
  }
  .swatch span {
    display: block;
    width: 2rem;
    height: 2rem;
    border-radius: 50%;
    background: var(--sw);
    border: 3px solid var(--panel);
    box-shadow: 0 0 0 1px var(--line);
  }
  .swatch input:checked + span {
    box-shadow: 0 0 0 2px var(--text);
  }
  .swatch input:focus-visible + span {
    outline: 2px solid var(--accent);
    outline-offset: 3px;
  }
  .weights {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
  }
  .weight-row {
    display: grid;
    grid-template-columns: 9rem 5.5rem minmax(4rem, 1fr) 9rem;
    gap: 0.75rem;
    align-items: center;
  }
  .weight-row.unused {
    opacity: 0.6;
  }
  .weight-row label {
    font-weight: 400;
    margin: 0;
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
    background: var(--accent);
  }
  .unit {
    padding: 0;
    overflow: hidden;
  }
  .unit-head {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    width: 100%;
    padding: 0.9rem 1.1rem;
    background: transparent;
    border: 0;
    border-radius: 0;
    color: var(--text);
    text-align: left;
    flex-wrap: wrap;
  }
  .unit-head strong {
    flex: 1;
    min-width: 10rem;
  }
  .unit.open .unit-head {
    border-bottom: 1px solid var(--line);
  }
  .caret {
    color: var(--muted);
  }
  .unit-body {
    padding: 0.9rem 1.1rem 1.1rem;
  }
  .wk-badge {
    background: var(--accent);
    color: var(--on-accent);
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.03em;
    padding: 0.2rem 0.55rem;
    border-radius: calc(var(--radius-sm) - 2px);
    text-transform: uppercase;
  }
  .sub {
    display: flex;
    gap: 0.4rem;
    align-items: center;
    margin: 1.4rem 0 0.6rem;
    padding-top: 0.8rem;
    border-top: 1px solid var(--line);
  }
  .item {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 0.8rem 0.9rem;
    margin: 0.75rem 0;
    background: var(--bg);
  }
  .item-head {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
    margin-bottom: 0.6rem;
  }
  .kind-chip {
    display: inline-flex;
    gap: 0.3rem;
    align-items: center;
    font-size: 0.78rem;
    padding: 0.15rem 0.55rem;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }
  .tools {
    margin-top: 0.5rem;
    gap: 0.4rem;
  }
  .unit-tools {
    margin-top: 1rem;
    padding-top: 0.8rem;
    border-top: 1px solid var(--line);
  }
  .add-work button {
    display: inline-flex;
    gap: 0.3rem;
    align-items: center;
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
    width: 5rem;
  }
  .indent {
    margin-left: 1.6rem;
  }
  .preview {
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    padding: 0.6rem 0.8rem;
    min-height: 6rem;
  }
  .problems {
    margin-top: 0;
  }
  .good-note {
    border-left-color: var(--good);
    color: var(--good);
  }
  .nowrap {
    white-space: nowrap;
  }
  .danger-zone {
    margin-top: 2rem;
    display: flex;
    gap: 0.75rem;
    align-items: center;
    flex-wrap: wrap;
  }
</style>
