<script lang="ts">
  // Pause, continue or end a course that Claude is building. Courses are built one week at a time, so ending early
  // can keep the weeks that are ready. Continuing shows what the rest will use of the learner's Claude plan first.
  import { api, type CourseView, type Remaining } from "#lib/api.ts";
  import { stageLabel, whenAtEnd } from "#lib/format.ts";

  let { c, busy, act }: { c: CourseView; busy: boolean; act: (f: () => Promise<unknown>) => Promise<void> } = $props();

  let panel = $state<"pause" | "continue" | "end" | null>(null);
  let left = $state<Remaining | null>(null);
  let leftError = $state<string | null>(null);

  const building = $derived(["intake", "scoping", "compiling"].includes(c.status));
  const working = $derived(c.jobs.running > 0 || c.jobs.queued > 0);
  const ready = $derived(c.readiness?.ready_units ?? 0);
  const readyWeeks = $derived(c.readiness?.ready_weeks ?? 0);
  const totalWeeks = $derived(c.readiness?.total_weeks ?? 0);
  const weeksText = (n: number) => (n === 1 ? "week 1" : `weeks 1–${n}`);

  function words(min: number) {
    if (min < 1) return "less than a minute";
    const m = Math.round(min);
    if (m < 60) return `${m} min`;
    const h = Math.floor(m / 60);
    return m % 60 ? `${h} h ${m % 60} min` : `${h} h`;
  }

  async function openContinue() {
    panel = "continue";
    left = null;
    leftError = null;
    try {
      left = await api.buildRemaining(c.id);
    } catch (e) {
      leftError = String(e);
    }
  }

  async function run(f: () => Promise<unknown>) {
    await act(f);
    panel = null;
  }
</script>

{#if c.paused}
  <div class="note row">
    <span>
      {#if c.jobs.running > 0}
        Pausing: the step in progress finishes and is kept, then nothing more is sent to Claude for this course until you carry on.
      {:else}
        Building is paused. Nothing is sent to Claude for this course until you carry on.
      {/if}
      {#if c.status === "compiling" && ready > 0}{readyWeeks === 1 ? "Week 1" : `Weeks 1–${readyWeeks}`} of {totalWeeks} {readyWeeks === 1 ? "is" : "are"} ready.{/if}
    </span>
    <button onclick={openContinue} disabled={busy}>Continue building…</button>
    {#if c.status === "compiling"}<button class="ghost" onclick={() => (panel = "end")} disabled={busy}>End building…</button>{/if}
  </div>
{:else if c.jobs.waiting_usage}
  <div class="note row">
    <span>Paused because your Claude plan's usage limit was reached. It tries again automatically at {whenAtEnd(c.jobs.next_attempt_at)}. If your limit has already reset, resume now.</span>
    <button onclick={() => act(() => api.resume(c.id))} disabled={busy}>Resume now</button>
    {#if building}<button class="ghost" onclick={() => act(() => api.pauseCourse(c.id, true))} disabled={busy}>Pause building</button>{/if}
    {#if c.status === "compiling"}<button class="ghost" onclick={() => (panel = "end")} disabled={busy}>End building…</button>{/if}
  </div>
{:else if working}
  <div class="note info row">
    <span><span class="spinner" aria-hidden="true"></span> {c.jobs.running_kind ? `Claude is ${stageLabel[c.jobs.running_kind] ?? "working"}…` : "Waiting for the next step to start…"} You can leave this page; it carries on in the background.</span>
    {#if building}<button class="ghost" onclick={() => (panel = "pause")} disabled={busy}>Pause building…</button>{/if}
    {#if c.status === "compiling"}<button class="ghost" onclick={() => (panel = "end")} disabled={busy}>End building…</button>{/if}
  </div>
{/if}

{#if panel === "pause"}
  <section class="card panel" aria-label="Pause building">
    <h3>Pause building</h3>
    <p>You can carry on at any time; nothing is lost while it's paused.</p>
    <div class="choices">
      <button onclick={() => run(() => api.pauseCourse(c.id, false))} disabled={busy}>Pause after this step</button>
      <span class="small muted">Recommended. The step Claude is on finishes and is kept, so none of your plan's usage is wasted.</span>
      <button class="ghost" onclick={() => run(() => api.pauseCourse(c.id, true))} disabled={busy}>Pause now</button>
      <span class="small muted">Stops the step in progress. What it has used of your plan so far is lost, and it starts over when you continue.</span>
    </div>
    <button class="ghost" onclick={() => (panel = null)}>Cancel</button>
  </section>
{:else if panel === "continue"}
  <section class="card panel" aria-label="Continue building">
    <h3>Continue building?</h3>
    {#if leftError}
      <p class="note bad">{leftError}</p>
    {:else if !left}
      <p class="muted">Working out what's left…</p>
    {:else}
      <p>
        {#if left.units_left}{left.units_left} {left.units_left === 1 ? "week" : "weeks"} left to write and check:{:else}Almost done:{/if}
        about {left.calls_min === left.calls_max ? left.calls_min : `${left.calls_min}–${left.calls_max}`} Claude calls and
        {words(left.minutes)} of Claude work{#if left.minutes_per_week_so_far}, going by about {words(left.minutes_per_week_so_far)} for each week so far{/if}.
      </p>
      <p class="small muted">
        That's roughly {left.usage_vs_standard < 0.95 || left.usage_vs_standard > 1.05 ? `${left.usage_vs_standard.toFixed(1)}×` : "the same as"} the usage of building this whole course at the Standard level.
      </p>
      <p class="note {left.heavy ? 'bad' : ''}">
        Building uses your Claude plan's usage limits, not extra charges.
        {#if left.heavy}
          This course is built at a heavy level (high effort, research or the largest models): one week can use most of a
          5-hour usage window, and the whole course may reach your weekly limit. Other Claude work you do meanwhile shares the same limits.
        {:else}
          Long builds can still reach your 5-hour or weekly limit, and other Claude work you do meanwhile shares it.
        {/if}
        When a limit is reached, building waits and carries on by itself once it resets. You can pause again at any time, or end
        building and keep the weeks that are ready.
      </p>
    {/if}
    <div class="row">
      <button onclick={() => run(() => api.continueCourse(c.id))} disabled={busy || (!left && !leftError)}>Continue building</button>
      <button class="ghost" onclick={() => (panel = null)}>Keep it paused</button>
    </div>
  </section>
{:else if panel === "end"}
  <section class="card panel" aria-label="End building">
    <h3>End building</h3>
    {#if ready > 0}
      <p>
        Start the course now with the {ready === 1 ? "week" : "weeks"} that {ready === 1 ? "is" : "are"} ready ({weeksText(readyWeeks)} of {totalWeeks}).
        Everything after {readyWeeks === 1 ? "it" : "them"} is dropped and nothing more is sent to Claude for this course.
        {#if working && !c.paused}The step in progress is stopped, and what it used is lost.{/if}
      </p>
      <p class="small muted">
        The syllabus still lists the dropped weeks as not built, and grades are shared across the work that's left.
      </p>
      <div class="row">
        <button onclick={() => run(() => api.finishEarly(c.id))} disabled={busy}>Start with {weeksText(readyWeeks)}</button>
        <button class="ghost" onclick={() => (panel = null)}>Keep building</button>
      </div>
    {:else}
      <p>No week is ready yet: the first week still has to be written and checked. Pause building to stop for now, or delete the course below.</p>
      <div class="row">
        {#if !c.paused}<button onclick={() => run(() => api.pauseCourse(c.id, false))} disabled={busy}>Pause after this step</button>{/if}
        <button class="ghost" onclick={() => (panel = null)}>Back</button>
      </div>
    {/if}
  </section>
{/if}

<style>
  .panel h3 {
    margin-top: 0;
  }
  .choices {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 0.5rem 1rem;
    align-items: center;
    margin-bottom: 1rem;
  }
  @media (max-width: 40rem) {
    .choices {
      grid-template-columns: 1fr;
    }
  }
</style>
