<script lang="ts">
  import { onMount } from "svelte";
  import { api, live, type Transcript } from "#lib/api.ts";
  import { band, day, pct } from "#lib/format.ts";
  import Icon from "#lib/Icon.svelte";

  let t = $state<Transcript | null>(null);
  let error = $state<string | null>(null);

  onMount(() =>
    live(async () => {
      try {
        t = await api.transcript();
      } catch (e) {
        error = String(e);
      }
    }),
  );
</script>

<div class="spread page-enter">
  <h1>Transcript</h1>
  {#if t?.entries.length}<button class="ghost" onclick={() => window.print()}>Print or save as PDF</button>{/if}
</div>
<p class="lead">Final marks for completed courses you chose to put on your record.</p>

{#if error}<p class="note bad">{error}</p>{/if}

{#if t}
  {#if t.entries.length === 0}
    <p class="card muted">No completed courses yet. A course is added here when its last graded item is marked.</p>
  {:else}
    <section class="tiles">
      <div class="tile"><span class="icon"><Icon name="transcript" /></span><div><strong>{t.entries.length}</strong><span class="small muted">{t.entries.length === 1 ? "course" : "courses"} completed</span></div></div>
      <div class="tile"><span class="icon"><Icon name="target" /></span><div><strong>{pct(t.average)}</strong><span class="small muted">cumulative average</span></div></div>
      <div class="tile"><span class="icon"><Icon name="sparkle" /></span><div><strong>{t.gpa?.toFixed(2) ?? "-"}</strong><span class="small muted">GPA on a 4.0 scale</span></div></div>
    </section>
    <section class="card table-wrap">
      <table>
        <thead>
          <tr><th>Course</th><th>Started</th><th>Finished</th><th>Mark</th><th>Grade</th><th>GPA</th></tr>
        </thead>
        <tbody>
          {#each t.entries as e}
            <tr>
              <td><a href="/course/{encodeURIComponent(e.course_id)}">{e.title}</a>{#if e.sandbox} <span class="pill" title="Unlimited attempts, no late penalties">Sandbox</span>{/if}</td>
              <td>{day(e.started_on)}</td>
              <td>{day(e.finished_on)}</td>
              <td>{pct(e.final_percent)}</td>
              <td><span class="grade {band(e.final_percent)}">{e.letter}</span></td>
              <td>{e.gpa.toFixed(1)}</td>
            </tr>
          {/each}
        </tbody>
        <tfoot>
          <tr><th colspan="3">Cumulative</th><th>{pct(t.average)}</th><th></th><th>{t.gpa?.toFixed(2)}</th></tr>
        </tfoot>
      </table>
    </section>
    <p class="small muted">Marks are given by Claude against each course's rubrics; they are not an accredited qualification.</p>
  {/if}
{/if}

<style>
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(11rem, 1fr));
    gap: var(--space);
    margin-bottom: var(--space);
  }
  .tile {
    display: flex;
    gap: 0.8rem;
    align-items: center;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 0.9rem 1rem;
    box-shadow: var(--shadow);
  }
  .tile strong {
    display: block;
    font-size: 1.35rem;
    line-height: 1.2;
  }
  .icon {
    display: grid;
    place-items: center;
    width: 2.4rem;
    height: 2.4rem;
    border-radius: 50%;
    background: var(--accent-soft);
    color: var(--accent);
    flex: none;
  }
  .grade {
    display: inline-block;
    min-width: 2.4rem;
    text-align: center;
    font-weight: 700;
    padding: 0.05rem 0.5rem;
    border-radius: 999px;
    border: 1px solid currentColor;
  }
  .grade.good {
    color: var(--good);
  }
  .grade.warn {
    color: var(--warn);
  }
  .grade.bad {
    color: var(--bad);
  }
  @media print {
    .tiles {
      display: none;
    }
  }
</style>
