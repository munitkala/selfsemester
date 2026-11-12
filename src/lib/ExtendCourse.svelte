<script lang="ts">
  // Grow a published course you wrote: add a week, or add homework, a quiz, a test, an exam or a project to a week.
  // Nothing already released or marked changes; new work opens with its week, or straight away if the week has begun.
  import { api, type CourseView, type DraftUnit, type GenItem } from "#lib/api.ts";
  import { kindLabel } from "#lib/format.ts";
  import AttachmentEditor from "#lib/AttachmentEditor.svelte";
  import ItemEditor from "#lib/ItemEditor.svelte";
  import KindIcon from "#lib/KindIcon.svelte";
  import Icon from "#lib/Icon.svelte";

  let { c }: { c: CourseView } = $props();

  type Mode = null | "week" | "work";
  let mode = $state<Mode>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let done = $state<string | null>(null);

  const KINDS = ["homework", "quiz", "assignment", "test", "exam", "project"] as const;
  const nextWeek = $derived(Math.max(0, ...c.units.map((u) => u.week)) + 1);

  let week = $state<DraftUnit>(blankWeek(1));
  let item = $state<GenItem>(blankItem("homework"));
  let unitKey = $state("");

  function blankWeek(n: number): DraftUnit {
    return { title: `Week ${n}`, week: n, topics: [], objectives: [], lesson_title: "", lesson: "", lesson_files: [], est_minutes: 30, items: [] };
  }
  function blankItem(kind: string): GenItem {
    return { key: "", kind, title: kindLabel[kind], module: "", instructions: "", questions: [], rubric: [], answer_key: [], due_days: kind === "exam" ? 2 : 7, files: [] };
  }

  function start(m: Mode, kind = "homework") {
    error = done = null;
    if (m === "week") week = blankWeek(nextWeek);
    if (m === "work") {
      item = blankItem(kind);
      unitKey = c.units[c.units.length - 1]?.key ?? "";
    }
    mode = m;
  }

  /** Files attached to a form that's then cancelled are removed again. */
  async function cancel() {
    const files = mode === "week" ? (week.lesson_files ?? []) : (item.files ?? []);
    for (const f of files) await api.builderRemoveFile(c.id, f.file).catch(() => {});
    mode = null;
  }

  async function save() {
    busy = true;
    error = null;
    try {
      if (mode === "week") {
        await api.addWeek(c.id, $state.snapshot(week) as DraftUnit);
        done = `${week.title} was added.`;
      } else if (mode === "work") {
        await api.addWork(c.id, unitKey, $state.snapshot(item) as GenItem);
        done = `${item.title} was added to ${c.units.find((u) => u.key === unitKey)?.title ?? "the week"}.`;
      }
      mode = null;
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<section class="card grow">
  <h2><Icon name="new" size={18} /> Add to this course</h2>
  <p class="small muted">
    You wrote this course, so you can keep building it while it runs. New work opens with its week, or straight away
    if that week has already started. Nothing that's already been released or marked changes.
  </p>

  {#if done}<p class="note info small">{done}</p>{/if}

  {#if mode === null}
    <div class="choices">
      <button class="choice" onclick={() => start("week")}>
        <KindIcon kind="lesson" size={20} />
        <span><strong>A new week</strong><span class="small muted">with its lesson, typed or as a document</span></span>
      </button>
      {#each KINDS as k}
        <button class="choice" onclick={() => start("work", k)} disabled={!c.units.length}>
          <KindIcon kind={k} size={20} />
          <span><strong>{kindLabel[k]}</strong><span class="small muted">in a week you choose</span></span>
        </button>
      {/each}
    </div>
  {:else if mode === "week"}
    <div class="form">
      <div class="cols">
        <div class="field">
          <label for="w-title">Week title</label>
          <input id="w-title" bind:value={week.title} />
        </div>
        <div class="field narrow">
          <label for="w-num">Week number</label>
          <input id="w-num" type="number" min="1" max="104" bind:value={week.week} />
        </div>
      </div>
      <div class="field">
        <span class="label">Lesson documents <span class="hint">a PDF, Word file or slides with the lesson</span></span>
        <AttachmentEditor
          courseId={c.id}
          bind:files={week.lesson_files}
          useAsText={(t) => (week.lesson = week.lesson.trim() ? `${week.lesson.trim()}\n\n${t.trim()}` : t.trim())}
        />
      </div>
      <div class="field">
        <label for="w-lesson">Lesson text <span class="hint">optional if a document covers it; Markdown, $x^2$ for maths</span></label>
        <textarea id="w-lesson" rows="6" bind:value={week.lesson}></textarea>
      </div>
      {#if error}<p class="note bad small">{error}</p>{/if}
      <div class="row">
        <button onclick={save} disabled={busy}>{busy ? "Adding…" : "Add this week"}</button>
        <button class="ghost" onclick={cancel} disabled={busy}>Cancel</button>
      </div>
    </div>
  {:else}
    <div class="form">
      <div class="field">
        <label for="w-unit">Add it to</label>
        <select id="w-unit" bind:value={unitKey}>
          {#each c.units as u (u.key)}<option value={u.key}>Week {u.week}: {u.title}</option>{/each}
        </select>
      </div>
      <ItemEditor bind:it={item} prefix="add" courseId={c.id} />
      {#if error}<p class="note bad small">{error}</p>{/if}
      <div class="row">
        <button onclick={save} disabled={busy || !unitKey}>{busy ? "Adding…" : `Add this ${kindLabel[item.kind]?.toLowerCase() ?? "work"}`}</button>
        <button class="ghost" onclick={cancel} disabled={busy}>Cancel</button>
      </div>
    </div>
  {/if}
</section>

<style>
  .grow h2 {
    display: flex;
    align-items: center;
    gap: 0.45rem;
  }
  .grow h2 :global(.icon) {
    color: var(--accent);
  }
  .choices {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 14rem), 1fr));
    gap: 0.5rem;
  }
  .choice {
    display: flex;
    align-items: center;
    gap: 0.7rem;
    text-align: left;
    background: var(--panel);
    color: var(--text);
    border: 1px dashed var(--accent-line);
    border-radius: var(--radius-sm);
    padding: 0.65rem 0.8rem;
  }
  .choice:hover:not(:disabled) {
    border-style: solid;
    border-color: var(--accent);
    background: var(--accent-soft);
    filter: none;
  }
  .choice :global(svg) {
    color: var(--accent);
    flex: none;
  }
  .choice span {
    display: flex;
    flex-direction: column;
  }
  .form {
    border-top: 1px solid var(--line);
    padding-top: 0.75rem;
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
</style>
