<script lang="ts">
  import { api, type BuildEstimate, type BuildLevel, type BuildSettings, type Detail, type Effort, type Model } from "#lib/api.ts";

  let {
    courseId,
    weights,
    value = $bindable(null),
  }: { courseId: string; weights: Record<string, number>; value: BuildSettings | null } = $props();

  const workNames: Record<string, string> = {
    homework: "Homework",
    assignment: "Assignments",
    quiz: "Quizzes",
    test: "Tests",
    exam: "Exams",
    project: "Projects",
  };
  // The kinds of graded work this plan has, in a fixed order.
  const planned = $derived(Object.keys(workNames).filter((k) => (weights[k] ?? 0) > 0));
  const lessonsOnly = $derived(!!value && planned.every((k) => value!.content.skip.includes(k)));
  const details: { id: Detail; name: string; text: string }[] = [
    { id: "brief", name: "Brief", text: "the key ideas and one worked example each" },
    { id: "standard", name: "Standard", text: "full lessons with worked examples" },
    { id: "in_depth", name: "In depth", text: "like a demanding university course, with derivations, harder examples and extension notes" },
  ];

  function include(kind: string, on: boolean) {
    if (!value) return;
    const skip = value.content.skip.filter((k) => k !== kind);
    value.content = { ...value.content, skip: on ? skip : [...skip, kind] };
  }

  let presets = $state<BuildSettings[]>([]);
  let estimate = $state<BuildEstimate | null>(null);

  const about: Record<Exclude<BuildLevel, "custom">, { name: string; text: string }> = {
    quick: { name: "Quick", text: "Sonnet writes everything at low effort and Haiku checks it. No rewrites and a short reading list. The fastest build and the lightest on your plan." },
    standard: { name: "Standard", text: "Sonnet writes the lessons, Opus writes units with a test or exam, and Haiku checks every unit. A unit is rewritten once if its check finds a real mistake." },
    thorough: { name: "Thorough", text: "Opus writes everything at high effort and Sonnet checks it. Up to two rewrites and a longer reading list." },
    max: { name: "Maximum", text: "Fable writes everything at the highest effort and Opus checks it. Up to three rewrites and the longest reading list. The slowest build and the heaviest on your plan; it can take several usage windows." },
  };
  const models: { id: Model; name: string }[] = [
    { id: "haiku", name: "Haiku (fastest, lightest)" },
    { id: "sonnet", name: "Sonnet" },
    { id: "opus", name: "Opus" },
    { id: "fable", name: "Fable (most capable)" },
  ];
  const efforts: { id: Effort | null; name: string }[] = [
    { id: null, name: "Claude Code's default" },
    { id: "low", name: "Low" },
    { id: "medium", name: "Medium" },
    { id: "high", name: "High" },
    { id: "xhigh", name: "Extra high" },
    { id: "max", name: "Max" },
  ];

  $effect(() => {
    api.buildLevels().then((l) => {
      presets = l.presets;
      if (!value) value = l.last;
    }).catch(() => {});
  });

  function pick(level: BuildLevel) {
    if (!value) return;
    if (level === "custom") value = { ...value, level: "custom" };
    else value = { ...(presets.find((p) => p.level === level) ?? value), content: value.content };
  }

  // Refresh the estimate whenever the choice changes.
  $effect(() => {
    const b = value ? $state.snapshot(value) : null;
    if (!b) return;
    let stale = false;
    api.buildEstimate(courseId, b).then((e) => { if (!stale) estimate = e; }).catch(() => (estimate = null));
    return () => { stale = true; };
  });

  function usage(x: number): string {
    if (x < 0.1) return "under a tenth of the usage of a Standard build";
    if (x >= 0.95 && x < 1.05) return "about the same usage as a Standard build";
    return `about ${x < 10 ? x.toFixed(1) : Math.round(x)}× the usage of a Standard build`;
  }
  function duration(min: number): string {
    if (min < 60) return `${Math.max(1, Math.round(min))} min`;
    const h = min / 60;
    return `${h < 10 ? h.toFixed(1) : Math.round(h)} hours`;
  }
  const calls = $derived(estimate ? (estimate.calls_min === estimate.calls_max ? `${estimate.calls_min}` : `${estimate.calls_min} to ${estimate.calls_max}`) : "");
</script>

{#if value}
  <fieldset class="field">
    <legend>What to build</legend>
    {#if planned.length}
      <div class="row work">
        {#each planned as k (k)}
          <label class="radio"><input type="checkbox" checked={!value.content.skip.includes(k)} onchange={(e) => include(k, e.currentTarget.checked)} /> {workNames[k]}</label>
        {/each}
      </div>
      {#if lessonsOnly}
        <p class="small note">Lessons only: nothing is graded or marked. Practice questions with worked answers go in the lessons instead.</p>
      {:else}
        <p class="small muted">Untick any kind of work to leave it out. Its share of the grade moves to the work you keep.</p>
      {/if}
    {/if}
    <div class="detail">
      <span>Lessons:</span>
      {#each details as d (d.id)}
        <label class="radio"><input type="radio" name="build-detail" checked={value.content.detail === d.id} onchange={() => (value!.content = { ...value!.content, detail: d.id })} /> <span><strong>{d.name}</strong>: {d.text}</span></label>
      {/each}
    </div>
    <label class="radio"><input type="checkbox" checked={value.content.media} onchange={(e) => (value!.content = { ...value!.content, media: e.currentTarget.checked })} /> <span>Figures, interactive figures and sound in lessons (untick for text and formulas only)</span></label>
  </fieldset>

  <fieldset class="field">
    <legend>Build level</legend>
    {#each presets as p (p.level)}
      {@const a = about[p.level as Exclude<BuildLevel, "custom">]}
      {#if a}
        <label class="radio">
          <input type="radio" name="build-level" checked={value.level === p.level} onchange={() => pick(p.level)} />
          <span><strong>{a.name}</strong>: {a.text}</span>
        </label>
      {/if}
    {/each}
    <label class="radio">
      <input type="radio" name="build-level" checked={value.level === "custom"} onchange={() => pick("custom")} />
      <span><strong>Custom</strong>: choose the models, effort, rewrites and reading list yourself.</span>
    </label>

    {#if value.level === "custom"}
      <div class="custom indent">
        <label>Lessons and homework
          <select bind:value={value.writer}>{#each models as m}<option value={m.id}>{m.name}</option>{/each}</select>
        </label>
        <label>Units with a test or exam
          <select bind:value={value.exam_writer}>{#each models as m}<option value={m.id}>{m.name}</option>{/each}</select>
        </label>
        <label>Checking answer keys
          <select bind:value={value.checker}>{#each models as m}<option value={m.id}>{m.name}</option>{/each}</select>
        </label>
        <label>Effort
          <select bind:value={value.effort}>{#each efforts as e}<option value={e.id}>{e.name}</option>{/each}</select>
        </label>
        <label>Rewrites after a failed check
          <input type="number" min="0" max="5" bind:value={value.rewrites} />
        </label>
        <label>Most sources to find
          <input type="number" min="2" max="30" bind:value={value.sources} />
        </label>
      </div>
    {/if}

    {#if estimate}
      <p class="small" aria-live="polite">
        {calls} Claude calls, {usage(estimate.usage_vs_standard)}, roughly {duration(estimate.minutes)} if your plan doesn't hit a limit.
      </p>
    {/if}
    <p class="small muted">Usage figures are rough and come from each model's published price; your plan's real limits aren't published. If Fable isn't available on your plan, Claude Code falls back to Opus for those steps.</p>
  </fieldset>
{/if}

<style>
  fieldset {
    border: 0;
    padding: 0;
    margin-top: 0.9rem;
  }
  legend {
    font-weight: 600;
    margin-bottom: 0.3rem;
  }
  .radio {
    font-weight: 400;
    display: flex;
    gap: 0.5rem;
    align-items: baseline;
    margin: 0.25rem 0;
  }
  .radio input {
    width: auto;
  }
  .custom {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr));
    gap: 0.6rem 1rem;
    margin: 0.4rem 0 0.6rem 1.6rem;
  }
  .custom label {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    font-weight: 400;
  }
  .custom input {
    max-width: 6rem;
  }
  .work {
    gap: 0.4rem 1.2rem;
    flex-wrap: wrap;
  }
  .detail {
    margin: 0.4rem 0;
  }
</style>
