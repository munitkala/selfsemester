<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { api, type Draft, type DraftUnit, type GenItem, type LatePolicy, type Question, type Strictness } from "$lib/api";
  import { render } from "$lib/markdown";
  import { kindLabel, todayIso } from "$lib/format";
  import StrictnessChoice from "$lib/StrictnessChoice.svelte";

  const id = $derived(page.params.id ?? "");
  let draft = $state<Draft | null>(null);
  let problems = $state<string[]>([]);
  let error = $state<string | null>(null);
  let saved = $state<string>(""); // JSON of the last saved draft
  let saving = $state(false);
  let busy = $state(false);
  let exported = $state<string | null>(null);
  let preview = $state<Record<number, boolean>>({});
  let confirmDelete = $state(false);
  let confirmRemove = $state<string | null>(null);

  // publish choices
  let startDate = $state(todayIso(1));
  let lateKind = $state<"lock_zero" | "penalty">("lock_zero");
  let perDay = $state(10);
  let maxDays = $state(3);
  let strictness = $state<Strictness>("structured");
  let onTranscript = $state(true);

  const KINDS = ["homework", "assignment", "quiz", "test", "exam", "project"] as const;
  const QTYPES: { v: Question["type"]; label: string }[] = [
    { v: "mcq", label: "Multiple choice" },
    { v: "numeric", label: "Number" },
    { v: "short", label: "Short answer" },
    { v: "long", label: "Long answer" },
  ];

  $effect(() => {
    const courseId = id;
    api
      .builderGet(courseId)
      .then((r) => {
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

  // ---- editing helpers ----
  function move<T>(list: T[], i: number, by: number) {
    const j = i + by;
    if (j < 0 || j >= list.length) return;
    [list[i], list[j]] = [list[j], list[i]];
  }

  function addUnit() {
    if (!draft) return;
    const week = Math.max(0, ...draft.units.map((u) => u.week)) + 1;
    draft.units.push({ title: `Week ${week}`, week, topics: [], lesson_title: "", lesson: "", est_minutes: 30, items: [] });
  }

  function addItem(u: DraftUnit, kind: (typeof KINDS)[number]) {
    u.items.push({ key: "", kind, title: kindLabel[kind], module: "", instructions: "", questions: [], rubric: [], answer_key: [], due_days: kind === "exam" ? 2 : 7 });
  }

  function nextQid(it: GenItem) {
    let n = it.questions.length + 1;
    while (it.questions.some((q) => q.id === `q${n}`)) n++;
    return `q${n}`;
  }

  function addQuestion(it: GenItem, type: Question["type"]) {
    const qid = nextQid(it);
    it.questions.push({ id: qid, prompt: "", type, options: type === "mcq" ? ["", ""] : [], points: type === "long" ? 5 : 1 });
    it.answer_key.push({ question_id: qid, answer: "", explanation: "", tolerance: 0 });
  }

  function removeQuestion(it: GenItem, i: number) {
    const [q] = it.questions.splice(i, 1);
    it.answer_key = it.answer_key.filter((k) => k.question_id !== q.id);
  }

  /** The answer-key entry for a question, made on first use. */
  function keyOf(it: GenItem, q: Question) {
    let k = it.answer_key.find((k) => k.question_id === q.id);
    if (!k) {
      it.answer_key.push({ question_id: q.id, answer: "", explanation: "", tolerance: 0 });
      k = it.answer_key[it.answer_key.length - 1];
    }
    return k;
  }

  function setType(it: GenItem, q: Question, type: Question["type"]) {
    q.type = type;
    if (type === "mcq" && q.options.length < 2) q.options = ["", ""];
    keyOf(it, q).answer = "";
  }

  const weightTotal = $derived(draft ? Object.values(draft.weights).reduce((a, b) => a + (Number(b) || 0), 0) : 0);

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
  <p class="crumbs small"><a href="/build">Course builder</a></p>
  <div class="spread">
    <h1>{draft.title || "Untitled course"}</h1>
    <span class="small muted">{saving ? "Saving…" : dirty ? "Unsaved changes" : "Saved"}</span>
  </div>

  <section class="card">
    <h2>Course</h2>
    <div class="field">
      <label for="c-title">Title</label>
      <input id="c-title" bind:value={draft.title} />
    </div>
    <div class="field">
      <label for="c-subject">Subject <span class="hint">shown under the title</span></label>
      <input id="c-subject" bind:value={draft.subject} />
    </div>
    <div class="field">
      <label for="c-summary">Summary</label>
      <textarea id="c-summary" rows="2" bind:value={draft.summary}></textarea>
    </div>
    <fieldset class="field">
      <legend>Grade weights <span class="hint">how much each kind of work counts; kinds you don't use drop out</span></legend>
      <div class="weights">
        {#each KINDS as k}
          <label class="inline">{kindLabel[k]} <input type="number" min="0" step="0.05" bind:value={draft.weights[k]} /></label>
        {/each}
      </div>
      {#if Math.abs(weightTotal - 1) > 0.001 && weightTotal > 0}<p class="small muted">They add up to {weightTotal.toFixed(2)}; they're scaled to 100% when the course is built.</p>{/if}
    </fieldset>
  </section>

  {#each draft.units as u, ui (ui)}
    <details class="card unit" open={draft.units.length <= 2}>
      <summary>
        <span>Unit {ui + 1} · Week {u.week}: {u.title || "untitled"}</span>
        <span class="small muted">{u.items.length} {u.items.length === 1 ? "piece" : "pieces"} of work</span>
      </summary>
      <div class="row tools">
        <button class="ghost small" onclick={() => move(draft!.units, ui, -1)} disabled={ui === 0}>Move up</button>
        <button class="ghost small" onclick={() => move(draft!.units, ui, 1)} disabled={ui === draft.units.length - 1}>Move down</button>
        {#if confirmRemove === `u${ui}`}
          <button class="danger small" onclick={() => { draft!.units.splice(ui, 1); confirmRemove = null; }}>Remove this unit</button>
          <button class="ghost small" onclick={() => (confirmRemove = null)}>Keep</button>
        {:else}
          <button class="ghost small" onclick={() => (confirmRemove = `u${ui}`)}>Remove…</button>
        {/if}
      </div>
      <div class="cols">
        <div class="field">
          <label for="u-title-{ui}">Unit title</label>
          <input id="u-title-{ui}" bind:value={u.title} />
        </div>
        <div class="field narrow">
          <label for="u-week-{ui}">Week</label>
          <input id="u-week-{ui}" type="number" min="1" max="104" bind:value={u.week} />
        </div>
      </div>
      <div class="field">
        <label for="u-topics-{ui}">Topics <span class="hint">separated by commas</span></label>
        <input id="u-topics-{ui}" value={u.topics.join(", ")} oninput={(e) => (u.topics = (e.currentTarget as HTMLInputElement).value.split(",").map((t) => t.trim()).filter(Boolean))} />
      </div>
      <div class="cols">
        <div class="field">
          <label for="u-lt-{ui}">Lesson title <span class="hint">optional: the unit title is used otherwise</span></label>
          <input id="u-lt-{ui}" bind:value={u.lesson_title} />
        </div>
        <div class="field narrow">
          <label for="u-min-{ui}">Minutes</label>
          <input id="u-min-{ui}" type="number" min="1" bind:value={u.est_minutes} />
        </div>
      </div>
      <div class="field">
        <div class="spread">
          <label for="u-lesson-{ui}">Lesson <span class="hint">Markdown, $x^2$ for maths</span></label>
          <button class="ghost small" type="button" onclick={() => (preview[ui] = !preview[ui])}>{preview[ui] ? "Edit" : "Preview"}</button>
        </div>
        {#if preview[ui]}
          <div class="prose preview">{@html render(u.lesson || "_Nothing yet._")}</div>
        {:else}
          <textarea id="u-lesson-{ui}" rows="10" bind:value={u.lesson}></textarea>
        {/if}
      </div>

      <h3>Graded work</h3>
      {#each u.items as it, ii (ii)}
        <div class="item">
          <div class="cols">
            <div class="field narrow">
              <label for="i-kind-{ui}-{ii}">Kind</label>
              <select id="i-kind-{ui}-{ii}" bind:value={it.kind}>
                {#each KINDS as k}<option value={k}>{kindLabel[k]}</option>{/each}
              </select>
            </div>
            <div class="field">
              <label for="i-title-{ui}-{ii}">Title</label>
              <input id="i-title-{ui}-{ii}" bind:value={it.title} />
            </div>
            <div class="field narrow">
              <label for="i-due-{ui}-{ii}">Due after <span class="hint">days</span></label>
              <input id="i-due-{ui}-{ii}" type="number" min="1" max="21" bind:value={it.due_days} />
            </div>
          </div>
          <div class="field">
            <label for="i-ins-{ui}-{ii}">Instructions <span class="hint">Markdown</span></label>
            <textarea id="i-ins-{ui}-{ii}" rows="3" bind:value={it.instructions}></textarea>
          </div>

          {#each it.questions as q, qi (q.id)}
            {@const k = keyOf(it, q)}
            <div class="question">
              <div class="cols">
                <div class="field narrow">
                  <label for="q-type-{ui}-{ii}-{qi}">Question {qi + 1}</label>
                  <select id="q-type-{ui}-{ii}-{qi}" value={q.type} onchange={(e) => setType(it, q, (e.currentTarget as HTMLSelectElement).value as Question["type"])}>
                    {#each QTYPES as t}<option value={t.v}>{t.label}</option>{/each}
                  </select>
                </div>
                <div class="field narrow">
                  <label for="q-pts-{ui}-{ii}-{qi}">Points</label>
                  <input id="q-pts-{ui}-{ii}-{qi}" type="number" min="0.5" step="0.5" bind:value={q.points} />
                </div>
                <div class="row end">
                  <button class="ghost small" onclick={() => move(it.questions, qi, -1)} disabled={qi === 0}>Up</button>
                  <button class="ghost small" onclick={() => move(it.questions, qi, 1)} disabled={qi === it.questions.length - 1}>Down</button>
                  <button class="ghost small" onclick={() => removeQuestion(it, qi)}>Remove</button>
                </div>
              </div>
              <div class="field">
                <label for="q-p-{ui}-{ii}-{qi}" class="sr-only">Question</label>
                <textarea id="q-p-{ui}-{ii}-{qi}" rows="2" bind:value={q.prompt} placeholder="The question"></textarea>
              </div>
              {#if q.type === "mcq"}
                <fieldset class="field">
                  <legend class="small">Options <span class="hint">tick the right one</span></legend>
                  {#each q.options as _, oi}
                    <div class="row option">
                      <input type="radio" name="right-{ui}-{ii}-{q.id}" checked={k.answer !== "" && k.answer === q.options[oi]} onchange={() => (k.answer = q.options[oi])} aria-label="Option {oi + 1} is right" />
                      <input
                        value={q.options[oi]}
                        oninput={(e) => {
                          const was = q.options[oi];
                          q.options[oi] = (e.currentTarget as HTMLInputElement).value;
                          if (k.answer === was && was !== "") k.answer = q.options[oi];
                        }}
                        placeholder="Option {oi + 1}"
                        aria-label="Option {oi + 1}"
                      />
                      <button class="ghost small" onclick={() => { if (k.answer === q.options[oi]) k.answer = ""; q.options.splice(oi, 1); }} disabled={q.options.length <= 2}>×</button>
                    </div>
                  {/each}
                  <button class="ghost small" onclick={() => q.options.push("")}>Add option</button>
                </fieldset>
              {:else if q.type === "numeric"}
                <div class="cols">
                  <div class="field">
                    <label for="q-a-{ui}-{ii}-{qi}">Right answer</label>
                    <input id="q-a-{ui}-{ii}-{qi}" inputmode="decimal" bind:value={k.answer} />
                  </div>
                  <div class="field narrow">
                    <label for="q-tol-{ui}-{ii}-{qi}">Allowed error <span class="hint">±</span></label>
                    <input id="q-tol-{ui}-{ii}-{qi}" type="number" min="0" step="any" bind:value={k.tolerance} />
                  </div>
                </div>
              {:else}
                <div class="field">
                  <label for="q-a-{ui}-{ii}-{qi}">Model answer <span class="hint">what you'll mark against</span></label>
                  <textarea id="q-a-{ui}-{ii}-{qi}" rows="2" bind:value={k.answer}></textarea>
                </div>
              {/if}
              <div class="field">
                <label for="q-ex-{ui}-{ii}-{qi}">Explanation <span class="hint">optional, shown with the answer key</span></label>
                <input id="q-ex-{ui}-{ii}-{qi}" bind:value={k.explanation} />
              </div>
            </div>
          {/each}
          <div class="row">
            <span class="small muted">Add a question:</span>
            {#each QTYPES as t}<button class="ghost small" onclick={() => addQuestion(it, t.v)}>{t.label}</button>{/each}
          </div>

          <details class="rubric">
            <summary class="small">Rubric ({it.rubric.length}) <span class="hint">optional: what you mark work without questions on</span></summary>
            {#each it.rubric as r, ri}
              <div class="cols">
                <div class="field"><input bind:value={r.criterion} placeholder="Criterion" aria-label="Criterion" /></div>
                <div class="field narrow"><input type="number" min="0.5" step="0.5" bind:value={r.points} aria-label="Points" /></div>
                <div class="field"><input bind:value={r.description} placeholder="What earns the points" aria-label="Description" /></div>
                <button class="ghost small" onclick={() => it.rubric.splice(ri, 1)}>×</button>
              </div>
            {/each}
            <button class="ghost small" onclick={() => it.rubric.push({ criterion: "", points: 5, description: "" })}>Add a rubric line</button>
          </details>

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
      <div class="row">
        <span class="small muted">Add:</span>
        {#each KINDS as k}<button class="ghost small" onclick={() => addItem(u, k)}>{kindLabel[k]}</button>{/each}
      </div>
    </details>
  {/each}
  <button class="ghost" onclick={addUnit}>Add a unit</button>

  <section class="card">
    <h2>Folder</h2>
    <p class="small muted">Export this draft to a folder in Documents to edit it in any text editor, then import the folder back from the course builder page. Importing makes a new draft.</p>
    <div class="row">
      <button class="ghost" onclick={exportFolder} disabled={busy}>Export to a folder</button>
      {#if exported}
        <span class="small">Saved to {exported}</span>
        <button class="ghost small" onclick={() => api.openFolder(exported!)}>Open folder</button>
      {/if}
    </div>
  </section>

  <form class="card" onsubmit={publish}>
    <h2>Publish</h2>
    {#if problems.length}
      <p class="note">Fix these before publishing:</p>
      <ul class="small problems">{#each problems as p}<li>{p}</li>{/each}</ul>
    {:else}
      <p class="small good">Ready to publish.</p>
    {/if}
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
    <p class="small muted">Publishing schedules the course from the start date like any other course. After that it can't be edited here.</p>
    <button type="submit" disabled={busy || problems.length > 0 || dirty}>Publish the course</button>
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
{:else if !error}
  <p class="muted">Loading…</p>
{/if}

<style>
  .crumbs {
    margin: 0 0 0.3rem;
  }
  .unit summary {
    cursor: pointer;
    font-weight: 600;
    display: flex;
    justify-content: space-between;
    gap: 1rem;
  }
  .tools {
    margin: 0.5rem 0;
    gap: 0.4rem;
  }
  .cols {
    display: flex;
    gap: 0.75rem;
    flex-wrap: wrap;
    align-items: flex-end;
  }
  .cols > .field {
    flex: 1 1 14rem;
  }
  .cols > .field.narrow {
    flex: 0 1 8rem;
  }
  .end {
    margin-bottom: 0.9rem;
    gap: 0.3rem;
  }
  .item {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 0.75rem 0.9rem;
    margin: 0.75rem 0;
  }
  .question {
    border-left: 3px solid var(--accent-soft);
    padding-left: 0.75rem;
    margin: 0.75rem 0;
  }
  .option {
    gap: 0.4rem;
    margin: 0.2rem 0;
    flex-wrap: nowrap;
  }
  .option input[type="radio"] {
    width: auto;
  }
  .rubric {
    margin: 0.5rem 0;
  }
  .rubric summary {
    cursor: pointer;
  }
  .weights {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem 1.25rem;
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
    border-radius: 7px;
    padding: 0.6rem 0.8rem;
    min-height: 7rem;
  }
  .problems {
    margin-top: 0;
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }
  .danger-zone {
    margin-top: 2rem;
    display: flex;
    gap: 0.75rem;
    align-items: center;
    flex-wrap: wrap;
  }
</style>
