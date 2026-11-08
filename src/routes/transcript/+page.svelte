<script lang="ts">
  import { onMount } from "svelte";
  import { api, live, type Transcript } from "$lib/api";
  import { day, pct } from "$lib/format";

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

<div class="spread">
  <h1>Transcript</h1>
  {#if t?.entries.length}<button class="ghost" onclick={() => window.print()}>Print or save as PDF</button>{/if}
</div>
<p class="lead">Final marks for completed courses you chose to put on your record.</p>

{#if error}<p class="note bad">{error}</p>{/if}

{#if t}
  {#if t.entries.length === 0}
    <p class="card muted">No completed courses yet. A course is added here when its last graded item is marked.</p>
  {:else}
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
              <td><strong>{e.letter}</strong></td>
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
