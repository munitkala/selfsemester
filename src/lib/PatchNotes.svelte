<script lang="ts">
  import { patchNotes } from "#lib/patchNotes.ts";
  import Icon from "#lib/Icon.svelte";

  const date = (d: string) => (d ? new Date(d + "T12:00:00").toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" }) : "");
</script>

<section class="card" id="whats-new">
  <div class="spread">
    <h2><Icon name="sparkle" size={18} /> What's new</h2>
    <a class="small" href="https://github.com/munitkala/selfsemester/blob/main/CHANGELOG.md">Full changelog</a>
  </div>
  {#each patchNotes as n, i (n.version)}
    <details open={i === 0}>
      <summary>
        <span class="version" class:next={n.version === "Next"}>{n.version === "Next" ? "This update" : n.version}</span>
        <strong>{n.title}</strong>
        {#if n.date}<span class="small muted">{date(n.date)}</span>{/if}
      </summary>
      <ul>
        {#each n.notes as line}<li>{line}</li>{/each}
      </ul>
    </details>
  {/each}
</section>

<style>
  section {
    scroll-margin-top: 3.5rem;
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 0.45rem;
  }
  h2 :global(.icon) {
    color: var(--accent);
  }
  details {
    border-top: 1px solid var(--line);
    padding: 0.5rem 0;
  }
  summary {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
    cursor: pointer;
    list-style: none;
  }
  summary::-webkit-details-marker {
    display: none;
  }
  summary::before {
    content: "";
    width: 0.45rem;
    height: 0.45rem;
    border-right: 2px solid var(--muted);
    border-bottom: 2px solid var(--muted);
    transform: rotate(-45deg);
    transition: transform var(--fast);
    flex: none;
  }
  details[open] summary::before {
    transform: rotate(45deg);
  }
  .version {
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 0.8rem;
    padding: 0.05rem 0.5rem;
    border-radius: 999px;
    border: 1px solid var(--line);
    color: var(--muted);
  }
  .version.next {
    border-color: var(--accent);
    color: var(--accent);
    background: var(--accent-soft);
  }
  ul {
    margin: 0.4rem 0 0.2rem;
    padding-left: 1.6rem;
  }
  li {
    margin: 0.15rem 0;
  }
</style>
