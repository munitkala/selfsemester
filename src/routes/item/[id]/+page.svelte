<script lang="ts">
  import { page } from "$app/state";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, live, type ItemView } from "$lib/api";
  import { render } from "$lib/markdown";
  import { kindLabel, stateLabel, when, relative, pct } from "$lib/format";

  const id = $derived(page.params.id ?? "");
  let v = $state<ItemView | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let focus = $state(false); // distraction-free reading

  let text = $state("");
  let answers = $state<Record<string, string>>({});
  let files = $state<string[]>([]);
  let preview = $state(false);
  let confirmSubmit = $state(false);
  let result = $state<string | null>(null);

  $effect(() => {
    const itemId = id;
    return live(async () => {
      try {
        v = await api.item(itemId);
        error = null;
      } catch (e) {
        error = String(e);
      }
    });
  });

  const content = $derived(v?.content ?? null);
  const late = $derived(!!v?.item.due_at && new Date(v.item.due_at).getTime() < Date.now());
  const isQuiz = $derived(content?.module === "quiz");
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
      result = out.status === "marked" ? "Marked straight away. Your result is below." : "Submitted. Claude will mark it shortly.";
      confirmSubmit = false;
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
</script>

{#if error}<p class="note bad">{error}</p>{/if}

{#if v}
  {#if !focus}
    <p class="crumbs small"><a href="/course/{encodeURIComponent(v.item.course_id)}">{v.item.course_title}</a>{#if v.item.unit_title} · {v.item.unit_title}{/if}</p>
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
              {#if v.item.state === "open" && (q.type === "mcq" || q.type === "numeric")}
                {#if q.type === "mcq"}
                  <div class="options">
                    {#each q.options as o}
                      <label class="radio"><input type="radio" name={q.id} value={o} bind:group={answers[q.id]} /> <span>{@html render(o)}</span></label>
                    {/each}
                  </div>
                {:else}
                  <input type="text" inputmode="decimal" placeholder="Your answer" bind:value={answers[q.id]} aria-label="Answer to {q.id}" class="num" />
                {/if}
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

    {#if v.item.state === "open"}
      <section class="card">
        <h2>Your answer</h2>
        {#if late}<p class="note">The deadline has passed. {policyText(v.late_policy)}</p>{/if}
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
            <span>Submit now? You get one submission.</span>
            <button onclick={submit} disabled={busy}>{busy ? "Submitting…" : "Yes, submit"}</button>
            <button class="ghost" onclick={() => (confirmSubmit = false)}>Not yet</button>
          </div>
        {:else}
          <button onclick={() => (confirmSubmit = true)}>Submit</button>
        {/if}
      </section>
    {/if}

    {#if result}<p class="note info">{result}</p>{/if}

    {#if v.submission && v.item.state === "submitted"}
      <section class="card">
        <h2>Submitted {when(v.submission.submitted_at)}</h2>
        {#if markingState[0] === "waiting_usage"}
          <p class="warn">Waiting for your Claude usage limit to reset (around {when(markingState[1])}). It will be marked then; your submission time is already recorded.</p>
        {:else if markingState[0] === "failed"}
          <p class="bad">Marking failed: {markingState.slice(1).join(" ")}. Open the course page to retry.</p>
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
          <p class="small muted">Marked {v.mark.marker === "auto" ? "automatically" : `by Claude (${v.mark.marker})`} · {when(v.mark.marked_at)}</p>
        {/if}
        {#if v.mark.breakdown.lines?.length}
          <div class="table-wrap">
            <table>
              <thead><tr><th>Criterion</th><th>Score</th><th>Comment</th></tr></thead>
              <tbody>
                {#each v.mark.breakdown.lines as l}
                  <tr><td>{l.criterion}</td><td class="nowrap">{l.awarded} / {l.possible}</td><td class="prose small">{@html render(l.comment)}</td></tr>
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
        {#if v.mark.marker !== "auto" && v.mark.marker !== "missed"}
          <p class="small muted">AI marking can be wrong. If something looks off, check it against the answer key below.</p>
        {/if}
      </section>
    {/if}

    {#if v.answer_key?.length}
      <details class="card">
        <summary>Answer key</summary>
        <ol class="small">
          {#each v.answer_key as k}
            <li><strong>{k.question_id}:</strong> <span class="prose">{@html render(k.answer)}</span>{#if k.explanation}<div class="prose muted">{@html render(k.explanation)}</div>{/if}</li>
          {/each}
        </ol>
      </details>
    {/if}
  {/if}
{:else if !error}
  <p class="muted">Loading…</p>
{/if}

<style>
  .crumbs {
    margin: 0 0 0.3rem;
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
