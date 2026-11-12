<script lang="ts">
  // Settings → Tools: everything lessons can use, in plain words. Built-in tools are listed for information; common
  // tools can be installed and removed at any time; an extra tool shows only while it's installed (a course that
  // needs it offers it again). Changes are locked while a course is being built or Claude is working.
  import { onMount } from "svelte";
  import { api, live, type Tool, type ToolsView } from "#lib/api.ts";
  import { size } from "#lib/format.ts";

  let view = $state<ToolsView | null>(null);
  let error = $state<string | null>(null);
  let confirming = $state<string | null>(null);

  const groups = $derived(
    view
      ? [
          { id: "built-in", title: "Part of the app", note: "Always there, nothing to install.", tools: view.tools.filter((t) => t.category === "built-in") },
          { id: "common", title: "Common tools", note: "For subjects many people study. Install the ones you want.", tools: view.tools.filter((t) => t.category === "common") },
          { id: "extra", title: "Tools for particular courses", note: "Installed when a course asked for them. Once removed they leave this list; a course that needs one again will ask.", tools: view.tools.filter((t) => t.category === "extra") },
        ].filter((g) => g.tools.length)
      : [],
  );

  async function load() {
    try {
      view = await api.tools();
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function install(t: Tool) {
    error = null;
    try {
      await api.installTool(t.id);
    } catch (e) {
      error = `${t.name} couldn't be installed: ${e}`;
    }
    await load();
  }

  async function remove(t: Tool) {
    confirming = null;
    error = null;
    try {
      await api.removeTool(t.id);
    } catch (e) {
      error = `${t.name} couldn't be removed: ${e}`;
    }
    await load();
  }

  onMount(() => live(load));
</script>

{#if error}<p class="note bad">{error}</p>{/if}
{#if view?.locked}<p class="note info">{view.locked}</p>{/if}
{#if !view && !error}<p class="muted">Loading…</p>{/if}

{#each groups as g}
  <h3 class="tool-group">{g.title}</h3>
  <p class="small muted">{g.note}</p>
  <ul class="tools">
    {#each g.tools as t (t.id)}
      {@const busy = view?.installing.includes(t.id)}
      <li class="tool">
        <div class="tool-text">
          <div class="tool-name">
            <strong>{t.name}</strong>
            {#if t.category === "built-in"}<span class="pill">Built in</span>
            {:else if t.installed}<span class="pill on">Installed</span>{/if}
          </div>
          <p>{t.summary}</p>
          <p class="small muted">
            Good for: {t.for}{#if t.size}. {size(t.size)}{/if}{#if t.credit}. {t.credit}{/if}{#if t.licence}, {t.licence}{/if}.
          </p>
        </div>
        {#if t.category !== "built-in"}
          <div class="tool-actions">
            {#if busy}
              <button disabled>Installing…</button>
            {:else if confirming === t.id}
              <span class="small">{t.category === "extra" ? "Remove it? You can't install it again from here." : "Remove it?"}</span>
              <button class="danger" disabled={!!view?.locked} onclick={() => remove(t)}>Remove</button>
              <button class="ghost" onclick={() => (confirming = null)}>Keep</button>
            {:else if t.installed}
              <button class="ghost" disabled={!!view?.locked} onclick={() => (confirming = t.id)}>Remove</button>
            {:else}
              <button disabled={!!view?.locked} onclick={() => install(t)}>Install</button>
            {/if}
          </div>
        {/if}
      </li>
    {/each}
  </ul>
{/each}

<style>
  .tool-group { margin: 1.1rem 0 0.15rem; font-size: 1rem; }
  .tools { list-style: none; padding: 0; margin: 0.4rem 0 0; display: grid; gap: 0.5rem; }
  .tool { display: flex; flex-wrap: wrap; gap: 0.5rem 1rem; align-items: center; justify-content: space-between; padding: 0.6rem 0.8rem; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--raised); }
  .tool-text { flex: 1 1 18rem; min-width: 0; }
  .tool-text p { margin: 0.15rem 0 0; }
  .tool-name { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; }
  .tool-actions { display: flex; gap: 0.4rem; align-items: center; flex-wrap: wrap; }
  .pill { font-size: 0.75rem; padding: 0.05rem 0.5rem; border-radius: 999px; border: 1px solid var(--line); color: var(--muted); }
  .pill.on { border-color: var(--good); color: var(--good); }
</style>
