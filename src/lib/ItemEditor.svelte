<script lang="ts">
  // Edit one piece of graded work: what kind it is, its deadline, instructions, documents, questions with their
  // answers, and an optional rubric. Used by the course builder and when adding work to a running course.
  import type { GenItem, Question } from "#lib/api.ts";
  import { kindLabel } from "#lib/format.ts";
  import AttachmentEditor from "#lib/AttachmentEditor.svelte";

  let { it = $bindable(), prefix, courseId }: { it: GenItem; prefix: string; courseId: string } = $props();

  const KINDS = ["homework", "assignment", "quiz", "test", "exam", "project"] as const;
  const QTYPES: { v: Question["type"]; label: string }[] = [
    { v: "mcq", label: "Multiple choice" },
    { v: "numeric", label: "Number" },
    { v: "short", label: "Short answer" },
    { v: "long", label: "Long answer" },
  ];

  function move<T>(list: T[], i: number, by: number) {
    const j = i + by;
    if (j < 0 || j >= list.length) return;
    [list[i], list[j]] = [list[j], list[i]];
  }

  function nextQid() {
    let n = it.questions.length + 1;
    while (it.questions.some((q) => q.id === `q${n}`)) n++;
    return `q${n}`;
  }

  function addQuestion(type: Question["type"]) {
    const qid = nextQid();
    it.questions.push({ id: qid, prompt: "", type, options: type === "mcq" ? ["", ""] : [], points: type === "long" ? 5 : 1 });
    it.answer_key.push({ question_id: qid, answer: "", explanation: "", tolerance: 0 });
  }

  function removeQuestion(i: number) {
    const [q] = it.questions.splice(i, 1);
    it.answer_key = it.answer_key.filter((k) => k.question_id !== q.id);
  }

  /** The answer-key entry for a question, made on first use. */
  function keyOf(q: Question) {
    let k = it.answer_key.find((k) => k.question_id === q.id);
    if (!k) {
      it.answer_key.push({ question_id: q.id, answer: "", explanation: "", tolerance: 0 });
      k = it.answer_key[it.answer_key.length - 1];
    }
    return k;
  }

  function setType(q: Question, type: Question["type"]) {
    q.type = type;
    if (type === "mcq" && q.options.length < 2) q.options = ["", ""];
    keyOf(q).answer = "";
  }
</script>

<div class="cols">
  <div class="field narrow">
    <label for="i-kind-{prefix}">Kind</label>
    <select id="i-kind-{prefix}" bind:value={it.kind}>
      {#each KINDS as k}<option value={k}>{kindLabel[k]}</option>{/each}
    </select>
  </div>
  <div class="field">
    <label for="i-title-{prefix}">Title</label>
    <input id="i-title-{prefix}" bind:value={it.title} />
  </div>
  <div class="field narrow">
    <label for="i-due-{prefix}">Due after <span class="hint">days</span></label>
    <input id="i-due-{prefix}" type="number" min="1" max="21" bind:value={it.due_days} />
  </div>
</div>
<div class="field">
  <label for="i-ins-{prefix}">Instructions <span class="hint">Markdown; optional if the documents explain it</span></label>
  <textarea id="i-ins-{prefix}" rows="3" bind:value={it.instructions}></textarea>
</div>
<div class="field">
  <span class="label">Documents <span class="hint">a worksheet, a past paper, slides: anything learners need for this work</span></span>
  <AttachmentEditor {courseId} bind:files={it.files} />
</div>

{#each it.questions as q, qi (q.id)}
  {@const k = keyOf(q)}
  <div class="question">
    <div class="cols">
      <div class="field narrow">
        <label for="q-type-{prefix}-{qi}">Question {qi + 1}</label>
        <select id="q-type-{prefix}-{qi}" value={q.type} onchange={(e) => setType(q, (e.currentTarget as HTMLSelectElement).value as Question["type"])}>
          {#each QTYPES as t}<option value={t.v}>{t.label}</option>{/each}
        </select>
      </div>
      <div class="field narrow">
        <label for="q-pts-{prefix}-{qi}">Points</label>
        <input id="q-pts-{prefix}-{qi}" type="number" min="0.5" step="0.5" bind:value={q.points} />
      </div>
      <div class="row end">
        <button class="ghost small" onclick={() => move(it.questions, qi, -1)} disabled={qi === 0}>Up</button>
        <button class="ghost small" onclick={() => move(it.questions, qi, 1)} disabled={qi === it.questions.length - 1}>Down</button>
        <button class="ghost small" onclick={() => removeQuestion(qi)}>Remove</button>
      </div>
    </div>
    <div class="field">
      <label for="q-p-{prefix}-{qi}" class="sr-only">Question</label>
      <textarea id="q-p-{prefix}-{qi}" rows="2" bind:value={q.prompt} placeholder="The question"></textarea>
    </div>
    {#if q.type === "mcq"}
      <fieldset class="field">
        <legend class="small">Options <span class="hint">tick the right one</span></legend>
        {#each q.options as _, oi}
          <div class="row option">
            <input type="radio" name="right-{prefix}-{q.id}" checked={k.answer !== "" && k.answer === q.options[oi]} onchange={() => (k.answer = q.options[oi])} aria-label="Option {oi + 1} is right" />
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
            <button class="ghost small" onclick={() => { if (k.answer === q.options[oi]) k.answer = ""; q.options.splice(oi, 1); }} disabled={q.options.length <= 2} aria-label="Remove option {oi + 1}">×</button>
          </div>
        {/each}
        <button class="ghost small" onclick={() => q.options.push("")}>Add option</button>
      </fieldset>
    {:else if q.type === "numeric"}
      <div class="cols">
        <div class="field">
          <label for="q-a-{prefix}-{qi}">Right answer</label>
          <input id="q-a-{prefix}-{qi}" inputmode="decimal" bind:value={k.answer} />
        </div>
        <div class="field narrow">
          <label for="q-tol-{prefix}-{qi}">Allowed error <span class="hint">±</span></label>
          <input id="q-tol-{prefix}-{qi}" type="number" min="0" step="any" bind:value={k.tolerance} />
        </div>
      </div>
    {:else}
      <div class="field">
        <label for="q-a-{prefix}-{qi}">Model answer <span class="hint">what you'll mark against</span></label>
        <textarea id="q-a-{prefix}-{qi}" rows="2" bind:value={k.answer}></textarea>
      </div>
    {/if}
    <div class="field">
      <label for="q-ex-{prefix}-{qi}">Explanation <span class="hint">optional, shown with the answer key</span></label>
      <input id="q-ex-{prefix}-{qi}" bind:value={k.explanation} />
    </div>
  </div>
{/each}
<div class="row">
  <span class="small muted">Add a question:</span>
  {#each QTYPES as t}<button class="ghost small" onclick={() => addQuestion(t.v)}>{t.label}</button>{/each}
</div>

<details class="rubric">
  <summary class="small">Rubric ({it.rubric.length}) <span class="hint">optional: what you mark work without questions on</span></summary>
  {#each it.rubric as r, ri}
    <div class="cols">
      <div class="field"><input bind:value={r.criterion} placeholder="Criterion" aria-label="Criterion" /></div>
      <div class="field narrow"><input type="number" min="0.5" step="0.5" bind:value={r.points} aria-label="Points" /></div>
      <div class="field"><input bind:value={r.description} placeholder="What earns the points" aria-label="Description" /></div>
      <button class="ghost small" onclick={() => it.rubric.splice(ri, 1)} aria-label="Remove rubric line">×</button>
    </div>
  {/each}
  <button class="ghost small" onclick={() => it.rubric.push({ criterion: "", points: 5, description: "" })}>Add a rubric line</button>
</details>

<style>
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
  .end {
    margin-bottom: 0.9rem;
    gap: 0.3rem;
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
  fieldset {
    border: 0;
    padding: 0;
  }
  legend {
    font-weight: 600;
    margin-bottom: 0.3rem;
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }
</style>
