<script lang="ts">
  // The standards a course was built to meet (read from the learner's reference, such as a curriculum framework),
  // with the units that teach each one. Shown on the review step and in the syllabus.
  import type { Plan } from "#lib/api.ts";

  let { plan, heading = "h2" }: { plan: Plan; heading?: "h2" | "h3" } = $props();

  const standards = $derived(plan.standards ?? []);
  const built = (key: string) => plan.units.some((x) => x.key === key);
  const unitTitle = (key: string) => {
    const u = plan.units.find((x) => x.key === key);
    if (u) return `Week ${u.week}: ${u.title}`;
    // A week left out when building was ended early.
    const n = (plan.unbuilt ?? []).find((x) => x.key === key);
    return n ? `Week ${n.week}: ${n.title} (not built)` : null;
  };
  const uncovered = $derived(standards.filter((s) => !s.units.some((k) => unitTitle(k))));
  // Taught only in weeks that weren't built.
  const notBuilt = $derived(standards.filter((s) => s.units.some((k) => unitTitle(k)) && !s.units.some(built)));
  let filter = $state("");
  const shown = $derived(
    filter.trim()
      ? standards.filter((s) => `${s.id} ${s.text}`.toLowerCase().includes(filter.trim().toLowerCase()))
      : standards,
  );
</script>

{#if standards.length}
  <svelte:element this={heading}>Standards covered</svelte:element>
  <p class="small muted">
    {standards.length} {standards.length === 1 ? "standard" : "standards"} from your reference, each with the units that teach it.
    {#if !uncovered.length && !notBuilt.length}Every one is taught by at least one unit.{/if}
  </p>
  {#if uncovered.length}
    <p class="note">Not taught by any unit yet: {uncovered.map((s) => s.id).join(", ")}.</p>
  {/if}
  {#if notBuilt.length}
    <p class="note">
      {notBuilt.length} of {standards.length} {standards.length === 1 ? "standard is" : "standards are"} taught only in weeks that
      weren't built, because building was ended early. They're listed so you can see the whole plan.
    </p>
  {/if}
  {#if standards.length > 8}
    <input class="filter" type="search" placeholder="Find a standard" bind:value={filter} aria-label="Find a standard" />
  {/if}
  <div class="table-wrap">
    <table>
      <thead><tr><th>Standard</th><th>What it asks for</th><th>Taught in</th></tr></thead>
      <tbody>
        {#each shown as s (s.id)}
          <tr>
            <td class="id"><strong>{s.id}</strong></td>
            <td class="small">{s.text}</td>
            <td class="small">
              {#each s.units as k}
                {@const t = unitTitle(k)}
                {#if t}<div>{t}</div>{/if}
              {/each}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{/if}

<style>
  .id {
    white-space: nowrap;
  }
  .filter {
    max-width: 18rem;
    margin-bottom: 0.5rem;
  }
</style>
