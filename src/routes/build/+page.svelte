<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, live, type CourseCard } from "#lib/api.ts";
  import CourseBanner from "#lib/CourseBanner.svelte";
  import Icon from "#lib/Icon.svelte";

  let title = $state("");
  let drafts = $state<CourseCard[]>([]);
  let busy = $state(false);
  let error = $state<string | null>(null);

  onMount(() =>
    live(async () => {
      try {
        drafts = (await api.courses()).filter((c) => c.status === "draft");
      } catch (e) {
        error = String(e);
      }
    }),
  );

  async function start(e: Event) {
    e.preventDefault();
    busy = true;
    error = null;
    try {
      const id = await api.builderCreate(title);
      goto(`/build/${encodeURIComponent(id)}`);
    } catch (err) {
      error = String(err);
      busy = false;
    }
  }

  async function importFolder() {
    const folder = await open({ directory: true, title: "Choose a course folder exported from the course builder" });
    if (!folder || Array.isArray(folder)) return;
    busy = true;
    error = null;
    try {
      const id = await api.builderImport(folder);
      goto(`/build/${encodeURIComponent(id)}`);
    } catch (err) {
      error = String(err);
      busy = false;
    }
  }
</script>

<h1 class="page-enter">Course builder</h1>
<p class="lead">Write a course yourself, with no Claude needed. Start with just a title and a first lesson, typed or as a document you already have (a PDF, Word file or slides), and add weeks, homework, quizzes and exams whenever you like, even after it's published. Multiple-choice and numeric questions are marked by the app, and you mark written answers yourself against your own answer key.</p>

{#if error}<p class="note bad">{error}</p>{/if}

<form class="card" onsubmit={start}>
  <h2><Icon name="builder" size={18} /> Start a course</h2>
  <div class="field">
    <label for="title">Title</label>
    <input id="title" bind:value={title} placeholder="e.g. Chess openings for club players" required autocomplete="off" />
  </div>
  <div class="row">
    <button type="submit" disabled={busy || !title.trim()}>Start writing</button>
    <button type="button" class="ghost" onclick={importFolder} disabled={busy}>Import a folder…</button>
  </div>
  <p class="small muted">A course can be exported to a plain folder (course.json, plus a lesson.md and work.json for each unit) and imported back, so you can also edit it in any text editor.</p>
</form>

{#if drafts.length}
  <section class="card">
    <h2>Drafts</h2>
    <ul class="drafts">
      {#each drafts as d (d.id)}
        <li>
          <a href="/build/{encodeURIComponent(d.id)}">
            <CourseBanner id={d.id} title={d.title} meta={d.meta} origin={d.origin} compact />
            <span class="small muted">Keep writing <span aria-hidden="true">→</span></span>
          </a>
        </li>
      {/each}
    </ul>
    <p class="small muted">Published courses appear under Courses and run like any other course.</p>
  </section>
{/if}

<style>
  h2 {
    display: flex;
    align-items: center;
    gap: 0.45rem;
  }
  h2 :global(.icon) {
    color: var(--accent);
  }
  .drafts {
    list-style: none;
    padding: 0;
    margin: 0 0 0.75rem;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(15rem, 1fr));
    gap: var(--space);
  }
  .drafts a {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
    color: var(--text);
    background: var(--panel);
    box-shadow: var(--shadow);
    transition: border-color var(--fast), box-shadow var(--fast), transform var(--fast) var(--ease);
  }
  .drafts a:hover {
    text-decoration: none;
    border-color: var(--accent);
    box-shadow: var(--shadow-hover);
  }
  :global(:root:not([data-effects="minimal"])) .drafts a:hover {
    transform: translateY(-2px);
  }
  .drafts a > .small {
    padding: 0.55rem 0.9rem;
  }
</style>
