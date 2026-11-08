<script lang="ts">
  import { api, type ReviewThread } from "$lib/api";
  import { render } from "$lib/markdown";
  import { when } from "$lib/format";

  let { itemId, thread, keyHidden, courseId }: { itemId: string; thread: ReviewThread; keyHidden: boolean; courseId: string } = $props();

  let text = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);

  const waiting = $derived(thread.pending != null);

  async function send(e: Event) {
    e.preventDefault();
    busy = true;
    error = null;
    try {
      await api.sendReview(itemId, text);
      text = "";
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }
</script>

<section class="card">
  <h2>Review with Claude</h2>
  <p class="small muted">
    Ask about the questions, the feedback or why marks were given or lost. Claude can explain, but it can't change the mark.
    {#if keyHidden}The answer key is still hidden, so Claude guides you rather than giving the answers.{/if}
    Each message uses a little of your Claude plan.
  </p>

  {#if thread.messages.length}
    <ol class="thread">
      {#each thread.messages as m (m.id)}
        <li class={m.role}>
          <span class="who small muted">{m.role === "learner" ? "You" : m.role === "claude" ? "Claude" : "Note"} · {when(m.created_at)}</span>
          {#if m.role === "claude"}
            <div class="prose">{@html render(m.text)}</div>
          {:else}
            <p>{m.text}</p>
          {/if}
        </li>
      {/each}
    </ol>
  {/if}

  {#if thread.pending === "waiting_usage"}
    <div class="row">
      <span class="warn small">Paused until your Claude usage limit resets (next try {when(thread.next_attempt_at)}).</span>
      <button class="ghost small" onclick={() => api.resume(courseId)}>Try now</button>
    </div>
  {:else if waiting}
    <p class="muted small"><span class="spinner" aria-hidden="true"></span> Claude is writing a reply…</p>
  {/if}

  {#if error}<p class="note bad">{error}</p>{/if}
  <form onsubmit={send}>
    <label for="review-text" class="sr-only">Your question</label>
    <textarea id="review-text" rows="3" bind:value={text} maxlength="4000" placeholder="For example: why did I lose marks on question 2?" disabled={waiting}></textarea>
    <div class="row">
      <button type="submit" disabled={busy || waiting || !text.trim()}>{busy ? "Sending…" : "Ask"}</button>
    </div>
  </form>
</section>

<style>
  .thread {
    list-style: none;
    padding: 0;
    margin: 0.75rem 0;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .thread li {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 0.5rem 0.8rem;
  }
  .thread li.learner {
    background: var(--accent-soft);
    margin-left: 2rem;
  }
  .thread li.note {
    color: var(--warn);
  }
  .thread p {
    margin: 0.2rem 0 0;
    white-space: pre-wrap;
  }
  .who {
    display: block;
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }
  form {
    margin-top: 0.5rem;
  }
</style>
