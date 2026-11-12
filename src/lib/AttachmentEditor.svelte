<script lang="ts">
  // Attach documents to a lesson or a piece of work while writing a course. Text and Markdown files can also be
  // turned into the lesson's text, so a lesson written elsewhere doesn't have to be typed again.
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type Attachment } from "#lib/api.ts";
  import Icon from "#lib/Icon.svelte";
  import { fileKind, fileSize } from "#lib/files.ts";

  let {
    courseId,
    files = $bindable([]),
    useAsText,
    hint = "",
  }: { courseId: string; files?: Attachment[]; useAsText?: (text: string, name: string) => void; hint?: string } = $props();

  let busy = $state(false);
  let error = $state<string | null>(null);

  async function add() {
    error = null;
    const picked = await open({
      multiple: true,
      title: "Choose documents to attach",
      filters: [
        { name: "Documents, slides, text and images", extensions: ["pdf", "doc", "docx", "odt", "rtf", "txt", "md", "ppt", "pptx", "odp", "key", "xls", "xlsx", "ods", "csv", "png", "jpg", "jpeg", "webp", "gif", "svg", "epub"] },
        { name: "Audio and video", extensions: ["mp3", "m4a", "wav", "mp4", "webm"] },
      ],
    });
    if (!picked) return;
    busy = true;
    try {
      const added = await api.builderAttach(courseId, Array.isArray(picked) ? picked : [picked]);
      files = [...files, ...added];
      // A single text file attached to an empty lesson is most likely the lesson itself.
      if (useAsText && added.length === 1 && /\.(md|markdown|txt)$/i.test(added[0].file)) await asText(added[0]);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function remove(f: Attachment) {
    files = files.filter((x) => x.file !== f.file);
    await api.builderRemoveFile(courseId, f.file).catch(() => {});
  }

  async function asText(f: Attachment) {
    try {
      useAsText?.(await api.builderFileText(courseId, f.file), f.name);
    } catch (e) {
      error = String(e);
    }
  }
</script>

<div class="attachments">
  {#if files.length}
    <ul>
      {#each files as f (f.file)}
        <li>
          <span class="type">{fileKind(f.name)}</span>
          <button class="ghost linkish" type="button" onclick={() => api.openCourseFile(courseId, f.file)} title="Open">{f.name}</button>
          <span class="small muted">{fileSize(f.bytes)}</span>
          {#if useAsText && /\.(md|markdown|txt)$/i.test(f.name)}
            <button class="ghost small" type="button" onclick={() => asText(f)}>Use as lesson text</button>
          {/if}
          <button class="ghost small" type="button" onclick={() => remove(f)} aria-label="Remove {f.name}">Remove</button>
        </li>
      {/each}
    </ul>
  {/if}
  <div class="row">
    <button class="ghost small" type="button" onclick={add} disabled={busy}><Icon name="clip" size={14} /> {busy ? "Attaching…" : "Attach documents"}</button>
    {#if hint}<span class="small muted">{hint}</span>{/if}
  </div>
  {#if error}<p class="small bad">{error}</p>{/if}
</div>

<style>
  .attachments {
    margin: 0.4rem 0 0.9rem;
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0 0 0.5rem;
    display: grid;
    gap: 0.3rem;
  }
  li {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
    padding: 0.35rem 0.6rem;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    background: var(--raised);
  }
  .type {
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--accent);
    min-width: 2.8rem;
  }
  .linkish {
    border: 0;
    padding: 0;
    text-align: left;
    flex: 1;
    min-width: 8rem;
    color: var(--text);
  }
  .linkish:hover {
    text-decoration: underline;
    background: transparent;
  }
  .row button {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
  }
</style>
