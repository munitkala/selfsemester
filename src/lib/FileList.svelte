<script lang="ts">
  // Documents that go with a lesson or a piece of work, for the learner to open in their usual app.
  import { api, type Attachment } from "#lib/api.ts";
  import { fileKind, fileSize } from "#lib/files.ts";

  let { courseId, files, title = "Files" }: { courseId: string; files: Attachment[]; title?: string } = $props();
  let error = $state<string | null>(null);

  async function openFile(f: Attachment) {
    error = null;
    try {
      await api.openCourseFile(courseId, f.file);
    } catch (e) {
      error = String(e);
    }
  }
</script>

{#if files.length}
  <section class="files" aria-label={title}>
    <h3>{title}</h3>
    <ul>
      {#each files as f (f.file)}
        <li>
          <button type="button" onclick={() => openFile(f)}>
            <span class="type">{fileKind(f.name)}</span>
            <span class="name">{f.name}</span>
            <span class="small muted">{fileSize(f.bytes)}</span>
            <span class="open small">Open</span>
          </button>
        </li>
      {/each}
    </ul>
    {#if error}<p class="small bad">{error}</p>{/if}
  </section>
{/if}

<style>
  .files {
    margin: 0 0 1.2rem;
  }
  h3 {
    font-size: 0.85rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
    margin-bottom: 0.5rem;
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 21rem), 1fr));
    gap: 0.5rem;
  }
  button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 0.7rem;
    text-align: left;
    background: var(--panel);
    color: var(--text);
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    padding: 0.6rem 0.8rem;
    box-shadow: var(--shadow);
  }
  button:hover {
    border-color: var(--accent);
    filter: none;
  }
  .type {
    display: grid;
    place-items: center;
    min-width: 3.2rem;
    padding: 0.3rem 0.4rem;
    border-radius: var(--radius-sm);
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .name {
    flex: 1;
    min-width: 0;
    font-weight: 600;
    overflow-wrap: anywhere;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .open {
    color: var(--accent);
  }
</style>
