<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, live, type CourseCard } from "#lib/api.ts";

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

<h1>Course builder</h1>
<p class="lead">Write a course yourself: syllabus, lessons, questions, answer keys and rubrics. No Claude needed. Multiple-choice and numeric questions are marked by the app, and you mark written answers yourself against your own answer key.</p>

{#if error}<p class="note bad">{error}</p>{/if}

<form class="card" onsubmit={start}>
  <h2>Start a course</h2>
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
        <li><a href="/build/{encodeURIComponent(d.id)}">{d.title}</a></li>
      {/each}
    </ul>
    <p class="small muted">Published courses appear under Courses and run like any other course.</p>
  </section>
{/if}

<style>
  .drafts {
    padding-left: 1.2rem;
  }
  .drafts li {
    margin: 0.3rem 0;
  }
</style>
