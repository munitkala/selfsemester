<script lang="ts">
  // Shown once, right after installing (first launch, after the terms): pick the common tools for the subjects you
  // plan to study. Everything here can be changed later in Settings → Tools.
  import { api, type Tool } from "#lib/api.ts";
  import { size } from "#lib/format.ts";

  let { tools, ondone }: { tools: Tool[]; ondone: () => void } = $props();

  const common = $derived(tools.filter((t) => t.category === "common" && !t.installed));
  let picked = $state<Record<string, boolean>>({});
  let working = $state<string | null>(null);
  let failed = $state<string[]>([]);
  const total = $derived(common.filter((t) => picked[t.id]).reduce((n, t) => n + t.size, 0));

  async function finish(installSelected: boolean) {
    failed = [];
    if (installSelected) {
      for (const t of common.filter((t) => picked[t.id])) {
        working = t.name;
        try {
          await api.installTool(t.id);
        } catch (e) {
          failed.push(`${t.name}: ${e}`);
        }
      }
      working = null;
      if (failed.length) return;
    }
    await api.toolsSetupDone().catch(() => {});
    ondone();
  }
</script>

<div class="notice-backdrop">
  <div class="notice" role="dialog" aria-modal="true" aria-labelledby="tools-title">
    <h1 id="tools-title">What will you study?</h1>
    <p class="muted">
      Some subjects work better with an extra tool in their lessons. Pick the ones for what you plan to study and
      SelfSemester downloads them now. You can add or remove them later in Settings, Tools, and a course that needs
      something else will ask first.
    </p>
    <ul class="pick">
      {#each common as t (t.id)}
        <li>
          <label class="agree">
            <input type="checkbox" bind:checked={picked[t.id]} disabled={!!working} />
            <span><strong>{t.name}</strong> <span class="small muted">({t.for}, {size(t.size)})</span><br />{t.summary}</span>
          </label>
        </li>
      {/each}
    </ul>
    {#if working}<p class="small" aria-live="polite">Installing {working}…</p>{/if}
    {#if failed.length}
      <p class="note bad">Some tools couldn't be installed. Check your internet connection and try again, or skip for now and install them later in Settings.<br />{failed.join("; ")}</p>
    {/if}
    <div class="row">
      <button onclick={() => finish(true)} disabled={!!working || total === 0}>{total ? `Install selected (${size(total)})` : "Install selected"}</button>
      <button class="ghost" onclick={() => finish(false)} disabled={!!working}>{failed.length ? "Skip for now" : "Not now"}</button>
    </div>
  </div>
</div>

<style>
  .pick { list-style: none; padding: 0; display: grid; gap: 0.6rem; }
  .pick label { align-items: flex-start; font-weight: 400; margin: 0; }
  .pick input { margin-top: 0.3rem; }
  .row { display: flex; gap: 0.5rem; flex-wrap: wrap; }
</style>
