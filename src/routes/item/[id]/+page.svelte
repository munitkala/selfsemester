<script lang="ts">
  import { page } from "$app/state";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, live, type CourseView, type ItemView } from "#lib/api.ts";
  import { codeOf, colorVars } from "#lib/course.ts";
  import KindIcon from "#lib/KindIcon.svelte";
  import { render } from "#lib/markdown.ts";
  import ProgressBar from "#lib/Progress.svelte";
  import Review from "#lib/Review.svelte";
  import { kindLabel, stateLabel, when, relative, pct } from "#lib/format.ts";

  const id = $derived(page.params.id ?? "");
  let v = $state<ItemView | null>(null);
  let course = $state<CourseView | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let focus = $state(false); // distraction-free reading

  let text = $state("");
  let answers = $state<Record<string, string>>({});
  let files = $state<string[]>([]);
  let preview = $state(false);
  let confirmSubmit = $state(false);
  let result = $state<string | null>(null);
  let retrying = $state(false); // sandbox: answering again after a marked attempt
  let confirmReveal = $state(false);
  let selfMarks = $state<Record<number, number>>({});
  let selfComment = $state("");

  $effect(() => {
    const itemId = id;
    return live(async () => {
      try {
        v = await api.item(itemId);
        error = null;
        if (course?.id !== v.item.course_id || course.units.some((u) => u.items.some((i) => i.id === itemId && i.state !== v!.item.state))) {
          course = await api.course(v.item.course_id).catch(() => null);
        }
      } catch (e) {
        error = String(e);
      }
    });
  });

  const content = $derived(v?.content ?? null);
  const late = $derived(!!v?.item.due_at && new Date(v.item.due_at).getTime() < Date.now());
  const isQuiz = $derived(content?.module === "quiz");
  const sandbox = $derived(v?.strictness === "sandbox");
  // the answer form shows for open items, and in sandbox courses when trying again
  const answering = $derived(!!v && (v.item.state === "open" || (v.can_resubmit && retrying)));
  const needsText = $derived(!!content && !isQuiz && content.questions.some((q) => q.type === "short" || q.type === "long"));

  async function pickFiles() {
    const picked = await open({
      multiple: true,
      title: "Choose photos or PDFs of your work",
      filters: [{ name: "Images or PDF", extensions: ["png", "jpg", "jpeg", "webp", "gif", "pdf"] }],
    });
    if (picked) files = [...files, ...(Array.isArray(picked) ? picked : [picked])];
  }

  async function submit() {
    busy = true;
    error = null;
    try {
      const out = await api.submit(id, text, $state.snapshot(answers), $state.snapshot(files));
      result =
        out.status === "marked"
          ? "Marked straight away. Your result is below."
          : out.status === "self"
            ? "Submitted. Mark it yourself below, against the answer key."
            : "Submitted. Any multiple-choice and numeric answers are checked below already; Claude marks the rest shortly.";
      confirmSubmit = false;
      retrying = false;
      files = [];
      v = await api.item(id);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  /** "Question 2" for a question id, the criterion's own name otherwise. */
  function questionLabel(criterion: string) {
    const i = content?.questions.findIndex((q) => q.id === criterion) ?? -1;
    return i >= 0 ? `Question ${i + 1}` : criterion;
  }

  async function selfMark() {
    if (!v?.self_mark) return;
    busy = true;
    error = null;
    try {
      await api.selfMark(id, v.self_mark.map((_, i) => Number(selfMarks[i] ?? 0)), selfComment);
      result = "Marked. Your result is below.";
      v = await api.item(id);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function reveal() {
    busy = true;
    try {
      await api.revealKey(id);
      confirmReveal = false;
      v = await api.item(id);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function markRead() {
    await api.markRead(id);
    focus = false;
    v = await api.item(id);
  }

  function policyText(p: ItemView["late_policy"]) {
    return p.kind === "penalty" ? `Late work loses ${p.per_day} points per day, for up to ${p.max_days} days.` : "Late work isn't accepted: the item locks with a mark of 0.";
  }

  const markingState = $derived(v?.marking_status?.split("|") ?? []);

  // the course's items in order, for Previous / Next
  const order = $derived(course?.units.flatMap((u) => u.items) ?? []);
  const here = $derived(order.findIndex((i) => i.id === id));
  const prev = $derived(here > 0 ? order[here - 1] : null);
  const next = $derived(here >= 0 && here < order.length - 1 ? order[here + 1] : null);
  const courseHref = $derived(v ? `/course/${encodeURIComponent(v.item.course_id)}` : "/courses");
</script>

{#if error}<p class="note bad">{error}</p>{/if}

{#if v}
  {#if !focus}
    <nav class="course-strip small" style={course ? colorVars(course.meta, course.id) : ""} aria-label="Course">
      <a href={courseHref}><span class="code">{codeOf(course?.meta, v.item.course_title)}</span> {v.item.course_title}</a>
      {#if v.item.unit_title}<span class="muted">· {v.item.unit_title}</span>{/if}
      <span class="strip-links"><a href="{courseHref}?tab=modules">Modules</a> <a href="{courseHref}?tab=grades">Grades</a> <a href="{courseHref}?tab=syllabus">Syllabus</a></span>
    </nav>
    <div class="spread">
      <h1>{v.item.title}</h1>
      <span class="pill {v.item.state}">{stateLabel[v.item.state]}</span>
    </div>
    <p class="lead">
      {kindLabel[v.item.kind]}
      {#if v.item.due_at}
        · due {when(v.item.due_at)} <span class={late ? "bad" : ""}>({relative(v.item.due_at)})</span>
        · {v.points} points · {Math.round(v.item.weight * 1000) / 10}% of the course
      {/if}
      {#if sandbox && v.item.kind !== "lesson"} · <span class="pill">Sandbox</span>{#if v.attempts}<span> · attempt {v.attempts}</span>{/if}{/if}
    </p>
  {/if}

  {#if v.item.state === "locked"}
    <p class="card muted">This opens {when(v.item.release_at)}.</p>

  <!-- Lesson reader -->
  {:else if v.lesson != null}
    <div class="reader" class:focus>
      {#if focus}<button class="ghost exit" onclick={() => (focus = false)}>Exit reading mode</button>{/if}
      <article class="prose">{@html render(v.lesson)}</article>
      <div class="row end">
        {#if v.item.state === "open"}
          <button onclick={markRead}>I've finished this lesson</button>
        {:else}
          <span class="good">Read</span>
        {/if}
        {#if !focus}<button class="ghost" onclick={() => (focus = true)}>Reading mode</button>{/if}
      </div>
    </div>

  <!-- Graded work -->
  {:else if content}
    <section class="card prose-card">
      <div class="prose">{@html render(content.instructions)}</div>
      {#if content.questions.length}
        <ol class="questions">
          {#each content.questions as q (q.id)}
            <li>
              <div class="spread"><div class="prose">{@html render(q.prompt)}</div><span class="muted small">{q.points} pts</span></div>
              {#if answering && (q.type === "mcq" || q.type === "numeric")}
                {#if q.type === "mcq"}
                  <div class="options">
                    {#each q.options as o}
                      <label class="radio"><input type="radio" name={q.id} value={o} bind:group={answers[q.id]} /> <span>{@html render(o)}</span></label>
                    {/each}
                  </div>
                {:else}
                  <input type="text" inputmode="decimal" placeholder="Your answer" bind:value={answers[q.id]} aria-label="Answer to {q.id}" class="num" />
                {/if}
              {:else if q.type === "mcq" && q.options.length}
                <ul class="given">
                  {#each q.options as o}
                    <li class:chosen={v.submission?.answers?.[q.id] === o}>{@html render(o)}{#if v.submission?.answers?.[q.id] === o}{" "}<span class="small muted">(your answer)</span>{/if}</li>
                  {/each}
                </ul>
              {:else if q.type === "numeric" && v.submission?.answers?.[q.id]}
                <p class="small">Your answer: <strong>{v.submission.answers[q.id]}</strong></p>
              {/if}
            </li>
          {/each}
        </ol>
      {/if}
      {#if content.rubric.length}
        <details>
          <summary>How this is marked</summary>
          <ul class="small">{#each content.rubric as r}<li><strong>{r.criterion}</strong> ({r.points} pts) {r.description}</li>{/each}</ul>
        </details>
      {/if}
    </section>

    {#if answering}
      <section class="card">
        <h2>{v.attempts ? `Attempt ${v.attempts + 1}` : "Your answer"}</h2>
        {#if late && !sandbox}<p class="note">The deadline has passed. {policyText(v.late_policy)}</p>{/if}
        {#if late && sandbox}<p class="note">The deadline has passed. This is a sandbox course, so there's no penalty.</p>{/if}
        {#if needsText || (!isQuiz && content.questions.length === 0)}
          <div class="field">
            <div class="spread">
              <label for="answer">Typed answer <span class="hint">Markdown and LaTeX ($x^2$) work</span></label>
              <button class="ghost small" type="button" onclick={() => (preview = !preview)}>{preview ? "Edit" : "Preview"}</button>
            </div>
            {#if preview}
              <div class="prose preview">{@html render(text || "_Nothing yet._")}</div>
            {:else}
              <textarea id="answer" rows="12" bind:value={text} placeholder="Label each answer with its question number."></textarea>
            {/if}
          </div>
        {/if}
        {#if !isQuiz}
          <div class="field">
            <div class="row">
              <button class="ghost" type="button" onclick={pickFiles}>Add photos or PDFs</button>
              <span class="small muted">e.g. handwritten working</span>
            </div>
            {#if files.length}
              <ul class="small">
                {#each files as f, i}
                  <li>{f.split(/[\\/]/).pop()} <button class="ghost tiny" type="button" onclick={() => (files = files.filter((_, j) => j !== i))}>remove</button></li>
                {/each}
              </ul>
            {/if}
          </div>
        {/if}
        {#if confirmSubmit}
          <div class="row">
            <span>{sandbox ? "Submit this attempt? You can try again once it's marked." : "Submit now? You get one submission."}</span>
            <button onclick={submit} disabled={busy}>{busy ? "Submitting…" : "Yes, submit"}</button>
            <button class="ghost" onclick={() => (confirmSubmit = false)}>Not yet</button>
          </div>
        {:else}
          <div class="row">
            <button onclick={() => (confirmSubmit = true)}>Submit</button>
            {#if retrying}<button class="ghost" onclick={() => (retrying = false)}>Cancel</button>{/if}
          </div>
        {/if}
      </section>
    {/if}

    {#if result}<p class="note info">{result}</p>{/if}

    {#if v.submission?.instant && v.item.state === "submitted"}
      <section class="card">
        <div class="spread">
          <h2>Instant feedback</h2>
          <strong>{v.submission.instant.awarded} / {v.submission.instant.possible}</strong>
        </div>
        <p class="small muted">Your multiple-choice and numeric answers, checked the moment you submitted. The full mark follows.</p>
        <ul class="instant">
          {#each v.submission.instant.lines as l}
            <li><span class={l.awarded > 0 ? "good" : "bad"}>{l.awarded > 0 ? "✓" : "✗"}</span> {questionLabel(l.criterion)}: {l.comment} <span class="muted small">({l.awarded} / {l.possible})</span></li>
          {/each}
        </ul>
      </section>
    {/if}

    {#if v.self_mark && v.item.state === "submitted"}
      <section class="card">
        <h2>Mark your work</h2>
        <p class="small muted">You wrote this course, so you mark it. Compare your work with the answer key below and give each line its points. Multiple-choice and numeric questions are marked for you.</p>
        {#if v.submission?.text_answer}
          <h3>Your answer</h3>
          <div class="prose preview">{@html render(v.submission.text_answer)}</div>
        {/if}
        <div class="table-wrap">
          <table>
            <thead><tr><th>Line</th><th>Points</th></tr></thead>
            <tbody>
              {#each v.self_mark as line, i}
                <tr>
                  <td>{questionLabel(line.criterion)}</td>
                  <td class="nowrap">
                    {#if line.auto}
                      <span class="muted small">marked by the app</span>
                    {:else}
                      <input type="number" min="0" max={line.points} step="0.5" bind:value={selfMarks[i]} class="pts" aria-label="Points for {line.criterion}" /> / {line.points}
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
        <div class="field">
          <label for="self-comment">Note to yourself <span class="hint">optional</span></label>
          <textarea id="self-comment" rows="2" bind:value={selfComment}></textarea>
        </div>
        <button onclick={selfMark} disabled={busy}>Save my mark</button>
      </section>
    {:else if v.submission && v.item.state === "submitted"}
      <section class="card">
        <h2>Submitted {when(v.submission.submitted_at)}</h2>
        {#if markingState[0] === "waiting_usage"}
          <p class="warn">Marking is paused until your Claude usage limit resets. Next try {when(markingState[1])}; your submission time is already recorded.</p>
          <button onclick={() => api.resume(v!.item.course_id)}>Resume marking now</button>
        {:else if markingState[0] === "failed"}
          <p class="bad">Marking failed: {markingState.slice(1).join(" ")}. Open the course page to retry.</p>
        {:else if v.marking_progress}
          <ProgressBar p={v.marking_progress} />
        {:else}
          <p class="muted"><span class="spinner" aria-hidden="true"></span> Waiting for marking…</p>
        {/if}
      </section>
    {/if}

    {#if v.mark}
      <section class="card">
        <div class="spread">
          <h2>Result</h2>
          <strong class="score">{pct(v.mark.percent)}</strong>
        </div>
        {#if v.mark.late_penalty > 0}<p class="small warn">Raw mark {pct(v.mark.raw_percent)}, minus {v.mark.late_penalty} late-penalty points.</p>{/if}
        {#if v.mark.marker === "missed"}
          <p class="bad">Not submitted before the deadline.</p>
        {:else}
          <p class="small muted">Marked {v.mark.marker === "auto" ? "automatically" : v.mark.marker === "self" ? "by you" : `by Claude (${v.mark.marker})`} · {when(v.mark.marked_at)}</p>
        {/if}
        {#if v.mark.breakdown.lines?.length}
          <div class="table-wrap">
            <table>
              <thead><tr><th>Criterion</th><th>Score</th><th>Comment</th></tr></thead>
              <tbody>
                {#each v.mark.breakdown.lines as l}
                  <tr><td>{questionLabel(l.criterion)}</td><td class="nowrap">{l.awarded} / {l.possible}</td><td class="prose small">{@html render(l.comment)}</td></tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}
        {#if v.mark.breakdown.overall_comment}<div class="prose">{@html render(v.mark.breakdown.overall_comment)}</div>{/if}
        {#if v.mark.breakdown.revise?.length}
          <h3>To revise</h3>
          <ul>{#each v.mark.breakdown.revise as r}<li>{r}</li>{/each}</ul>
        {/if}
        {#if v.mark.marker !== "auto" && v.mark.marker !== "missed" && v.mark.marker !== "self"}
          <p class="small muted">AI marking can be wrong. If something looks off, check it against the answer key{sandbox && !v.key_revealed ? " once you reveal it" : " below"}.</p>
        {/if}
        {#if v.can_resubmit && !retrying}
          <div class="row sandbox">
            <button onclick={() => { retrying = true; result = null; }}>Try again</button>
            {#if confirmReveal}
              <span class="small">Show the answer key? This item closes and this mark stands.</span>
              <button class="danger" onclick={reveal} disabled={busy}>Show it</button>
              <button class="ghost" onclick={() => (confirmReveal = false)}>Not yet</button>
            {:else}
              <button class="ghost" onclick={() => (confirmReveal = true)}>Show the answer key…</button>
            {/if}
          </div>
          <p class="small muted">Sandbox: your latest attempt is the one that counts.</p>
        {/if}
      </section>
    {/if}

    {#if v.answer_key?.length}
      <details class="card" open={v.item.state === "submitted"}>
        <summary>Answer key</summary>
        <ol class="small">
          {#each v.answer_key as k}
            <li><strong>{k.question_id}:</strong> <span class="prose">{@html render(k.answer)}</span>{#if k.explanation}<div class="prose muted">{@html render(k.explanation)}</div>{/if}</li>
          {/each}
        </ol>
      </details>
    {/if}

    {#if v.review}
      <Review itemId={id} thread={v.review} keyHidden={sandbox && v.item.state === "marked" && !v.key_revealed} courseId={v.item.course_id} />
    {/if}
  {/if}

  {#if !focus && (prev || next)}
    <nav class="pager" aria-label="Previous and next in this course">
      {#if prev}
        <a class="pg" href="/item/{encodeURIComponent(prev.id)}"><span class="small muted">← Previous</span><span><KindIcon kind={prev.kind} size={15} /> {prev.title}</span></a>
      {:else}<span></span>{/if}
      {#if next}
        {#if next.state === "locked"}
          <span class="pg right locked"><span class="small muted">Next · opens {when(next.release_at)}</span><span><KindIcon kind={next.kind} size={15} /> {next.title}</span></span>
        {:else}
          <a class="pg right" href="/item/{encodeURIComponent(next.id)}"><span class="small muted">Next →</span><span><KindIcon kind={next.kind} size={15} /> {next.title}</span></a>
        {/if}
      {/if}
    </nav>
  {/if}
{:else if !error}
  <p class="muted">Loading…</p>
{/if}

<style>
  .course-strip {
    --course: var(--accent);
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem 0.6rem;
    align-items: center;
    padding: 0.45rem 0.8rem;
    margin-bottom: 0.8rem;
    border-left: 4px solid var(--course);
    background: var(--panel);
    border-radius: 0 8px 8px 0;
  }
  .course-strip .code {
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 0.78rem;
    color: var(--course);
    font-weight: 700;
    margin-right: 0.2rem;
  }
  .strip-links {
    margin-left: auto;
    display: flex;
    gap: 0.8rem;
  }
  .pager {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
    margin-top: 2rem;
  }
  .pg {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    padding: 0.7rem 0.9rem;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
    color: var(--text);
    text-decoration: none;
  }
  a.pg:hover,
  a.pg:focus-visible {
    border-color: var(--accent);
  }
  .pg.right {
    text-align: right;
    align-items: flex-end;
  }
  .pg.locked {
    opacity: 0.6;
  }
  .reader.focus {
    position: fixed;
    inset: 0;
    background: var(--bg);
    overflow-y: auto;
    padding: 3rem max(1.5rem, calc((100vw - 42rem) / 2)) 4rem;
    z-index: 10;
  }
  .exit {
    position: fixed;
    top: 0.75rem;
    right: 1rem;
    opacity: 0.4;
  }
  .exit:hover,
  .exit:focus-visible {
    opacity: 1;
  }
  .end {
    margin-top: 2rem;
  }
  .questions {
    padding-left: 1.3rem;
  }
  .questions li {
    margin: 1rem 0;
  }
  .options {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }
  .radio {
    font-weight: 400;
    display: flex;
    gap: 0.5rem;
    align-items: baseline;
  }
  .radio input {
    width: auto;
  }
  .num {
    max-width: 14rem;
  }
  .given {
    margin: 0.3rem 0 0;
    padding-left: 1.2rem;
    color: var(--muted);
  }
  .given li.chosen {
    color: var(--text);
    font-weight: 600;
  }
  .preview {
    border: 1px solid var(--line);
    border-radius: 7px;
    padding: 0.6rem 0.8rem;
    min-height: 7rem;
  }
  .tiny {
    padding: 0 0.4rem;
    font-size: 0.8rem;
    border: 0;
  }
  .score {
    font-size: 1.6rem;
  }
  .instant {
    list-style: none;
    padding: 0;
    margin: 0.4rem 0 0;
  }
  .instant li {
    padding: 0.2rem 0;
  }
  .pts {
    width: 5rem;
  }
  .sandbox {
    margin-top: 1rem;
  }
  .nowrap {
    white-space: nowrap;
  }
  details summary {
    cursor: pointer;
    font-weight: 600;
  }
  .prose-card .prose {
    max-width: none;
  }
</style>
