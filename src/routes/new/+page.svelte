<script lang="ts">
  import { goto } from "$app/navigation";
  import { api } from "$lib/api";

  let subject = $state("");
  let notes = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);

  const examples = ["Quantum mechanics 1", "Godot game engine for beginners", "Beginner piano", "Git and GitHub", "Linear algebra"];

  async function start(e: Event) {
    e.preventDefault();
    busy = true;
    error = null;
    try {
      const id = await api.create(subject, notes);
      goto(`/course/${encodeURIComponent(id)}`);
    } catch (err) {
      error = String(err);
      busy = false;
    }
  }
</script>

<h1>New course</h1>
<p class="lead">Name a subject. Claude asks a few questions, then plans the course and shows how well it can teach and mark it before anything is built.</p>

<form class="card" onsubmit={start}>
  <div class="field">
    <label for="subject">Subject</label>
    <input id="subject" bind:value={subject} placeholder="e.g. Quantum mechanics 1" required autocomplete="off" />
    <div class="row examples">
      {#each examples as ex}
        <button type="button" class="ghost small" onclick={() => (subject = ex)}>{ex}</button>
      {/each}
    </div>
  </div>
  <div class="field">
    <label for="notes">Anything else? <span class="hint">optional: why you're learning it, what you already know, a textbook you own</span></label>
    <textarea id="notes" bind:value={notes} rows="3"></textarea>
  </div>
  {#if error}<p class="note bad">{error}</p>{/if}
  <button type="submit" disabled={busy || !subject.trim()}>{busy ? "Starting…" : "Continue"}</button>
  <p class="small muted">This uses your Claude plan's usage. Nothing is built until you approve the plan.</p>
</form>

<style>
  .examples {
    margin-top: 0.5rem;
    gap: 0.4rem;
  }
  .examples button {
    padding: 0.15rem 0.6rem;
    font-size: 0.85rem;
  }
</style>
