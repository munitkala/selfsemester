<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, live, type Material } from "$lib/api";

  let { courseId, status }: { courseId: string; status: string } = $props();

  let items = $state<Material[]>([]);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let writing = $state(false);
  let noteTitle = $state("");
  let noteText = $state("");
  let confirmRemove = $state<string | null>(null);

  $effect(() => {
    const id = courseId;
    return live(async () => {
      try {
        items = await api.materials(id);
      } catch (e) {
        error = String(e);
      }
    });
  });

  // When the materials still change what Claude writes, and when they only help with marking.
  const hint = $derived(
    ["intake", "answering", "scoping", "review"].includes(status)
      ? "Claude reads these when it plans the course, writes each unit and marks your work. Add them before you approve the course so every unit can use them."
      : status === "compiling"
        ? "Units that haven't been written yet will use anything you add now. Marking uses them too."
        : "The course is already written, so new materials are used when your work is marked.",
  );

  async function act(f: () => Promise<unknown>) {
    busy = true;
    error = null;
    try {
      await f();
      items = await api.materials(courseId);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function pick() {
    const picked = await open({
      multiple: true,
      title: "Add reference files, images or notes to this course",
      filters: [
        { name: "PDFs, images and text", extensions: ["pdf", "png", "jpg", "jpeg", "webp", "gif", "txt", "md", "csv", "json", "tex"] },
        { name: "All supported files", extensions: ["pdf", "png", "jpg", "jpeg", "webp", "gif", "txt", "md", "markdown", "csv", "tsv", "json", "tex", "bib", "html", "xml", "yaml", "yml", "py", "rs", "js", "ts", "c", "h", "cpp", "hpp", "cs", "java", "go", "gd", "lua", "sql", "sh", "ps1"] },
      ],
    });
    if (!picked) return;
    const files = Array.isArray(picked) ? picked : [picked];
    if (files.length) act(() => api.addMaterials(courseId, files));
  }

  function saveNote(e: Event) {
    e.preventDefault();
    act(async () => {
      await api.addNote(courseId, noteTitle, noteText);
      noteTitle = "";
      noteText = "";
      writing = false;
    });
  }

  function size(b: number) {
    if (b < 1024) return `${b} B`;
    if (b < 1024 * 1024) return `${Math.round(b / 1024)} KB`;
    return `${(b / 1024 / 1024).toFixed(1)} MB`;
  }

  const kindName = { file: "File", image: "Image", note: "Note" } as const;
</script>

<details class="card" open={items.length > 0 || ["answering", "review"].includes(status)}>
  <summary>Your materials ({items.length})</summary>
  <p class="small muted">{hint}</p>

  {#if error}<p class="note bad">{error}</p>{/if}

  {#if items.length}
    <ul class="materials">
      {#each items as m (m.id)}
        <li>
          <button class="link" onclick={() => act(() => api.openMaterial(m.id))} title="Open">{m.name}</button>
          <span class="small muted">{kindName[m.kind]} · {size(m.bytes)}</span>
          {#if confirmRemove === m.id}
            <span class="row">
              <button class="danger small" onclick={() => act(async () => { await api.removeMaterial(m.id); confirmRemove = null; })} disabled={busy}>Remove</button>
              <button class="ghost small" onclick={() => (confirmRemove = null)}>Keep</button>
            </span>
          {:else}
            <button class="ghost small" onclick={() => (confirmRemove = m.id)} disabled={busy}>Remove…</button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}

  {#if writing}
    <form onsubmit={saveNote}>
      <div class="field">
        <label for="note-title">Title</label>
        <input id="note-title" bind:value={noteTitle} placeholder="For example: What I already know" maxlength="80" />
      </div>
      <div class="field">
        <label for="note-text">Note</label>
        <textarea id="note-text" rows="5" bind:value={noteText} placeholder="Anything Claude should know: topics to focus on, your exam board, notation your teacher uses…"></textarea>
      </div>
      <div class="row">
        <button type="submit" disabled={busy || !noteText.trim()}>Save note</button>
        <button type="button" class="ghost" onclick={() => (writing = false)}>Cancel</button>
      </div>
    </form>
  {:else}
    <div class="row">
      <button onclick={pick} disabled={busy}>Add files…</button>
      <button class="ghost" onclick={() => (writing = true)} disabled={busy}>Write a note</button>
    </div>
  {/if}
  <p class="small muted">PDFs, images and text files (notes, data, code), up to 30 MB each. They're copied into the course, so you can move or delete the originals.</p>
</details>

<style>
  details summary {
    cursor: pointer;
    font-weight: 600;
  }
  .materials {
    list-style: none;
    padding: 0;
    margin: 0.5rem 0 1rem;
  }
  .materials li {
    display: flex;
    gap: 0.75rem;
    align-items: center;
    flex-wrap: wrap;
    padding: 0.4rem 0;
    border-bottom: 1px solid var(--line);
  }
  .materials li > .small.muted {
    margin-right: auto;
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--accent);
    text-decoration: underline;
    cursor: pointer;
    text-align: left;
    font: inherit;
  }
  form {
    margin-top: 0.5rem;
  }
</style>
