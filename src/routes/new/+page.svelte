<script lang="ts">
  import { goto } from "$app/navigation";
  import { api } from "#lib/api.ts";
  import Icon from "#lib/Icon.svelte";

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

<h1 class="page-enter">New course</h1>
<p class="lead">Name a subject. Claude asks a few questions, then plans the course and shows how well it can teach and mark it before anything is built.</p>

<ol class="steps small" aria-label="How it works">
  <li><span>1</span> Name a subject</li>
  <li><span>2</span> Answer a few questions</li>
  <li><span>3</span> Review the plan</li>
  <li><span>4</span> Start learning</li>
</ol>

<form class="card" onsubmit={start}>
  <div class="field">
    <label for="subject">Subject</label>
    <input id="subject" bind:value={subject} placeholder="e.g. Quantum mechanics 1" required autocomplete="off" />
    <div class="row examples">
      {#each examples as ex}
        <button type="button" class="ghost small" aria-pressed={subject === ex} onclick={() => (subject = ex)}><Icon name="sparkle" size={13} /> {ex}</button>
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

<p class="small muted">Rather write the course yourself, without Claude? Use the <a href="/build">course builder</a>.</p>

<style>
  .examples {
    margin-top: 0.5rem;
    gap: 0.4rem;
  }
  .examples button {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0.2rem 0.7rem;
    font-size: 0.85rem;
    border-radius: 999px;
    border-color: var(--line);
    color: var(--text);
  }
  .examples button[aria-pressed="true"] {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .steps {
    list-style: none;
    padding: 0;
    margin: 0 0 var(--space);
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem 1.2rem;
    color: var(--muted);
  }
  .steps li {
    display: flex;
    align-items: center;
    gap: 0.45rem;
  }
  .steps span {
    display: grid;
    place-items: center;
    width: 1.5rem;
    height: 1.5rem;
    border-radius: 50%;
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 700;
    font-size: 0.8rem;
  }
  #subject {
    font-size: 1.1rem;
    padding: 0.6rem 0.8rem;
  }
</style>
