<script lang="ts">
  import { goto } from "$app/navigation";
  import { tick } from "svelte";
  import { api, type CourseCard } from "#lib/api.ts";
  import { isDark, prefs, save } from "#lib/prefs.svelte.ts";
  import Icon from "#lib/Icon.svelte";

  let { open = $bindable(false) }: { open?: boolean } = $props();

  type Command = { id: string; label: string; hint: string; icon: string; run: () => void };

  let query = $state("");
  let selected = $state(0);
  let courses = $state<CourseCard[]>([]);
  let input = $state<HTMLInputElement | null>(null);

  const go = (href: string) => () => goto(href);

  const fixed: Command[] = [
    { id: "today", label: "Today", hint: "Page", icon: "today", run: go("/") },
    { id: "courses", label: "Courses", hint: "Page", icon: "courses", run: go("/courses") },
    { id: "insights", label: "Insights", hint: "Page", icon: "insights", run: go("/insights") },
    { id: "new", label: "New course", hint: "Page", icon: "new", run: go("/new") },
    { id: "build", label: "Course builder", hint: "Page", icon: "builder", run: go("/build") },
    { id: "transcript", label: "Transcript", hint: "Page", icon: "transcript", run: go("/transcript") },
    { id: "settings", label: "Settings", hint: "Page", icon: "settings", run: go("/settings") },
    {
      id: "scheme",
      label: "Switch light / dark",
      hint: "Appearance",
      icon: "moon",
      run: () => {
        prefs.mode = isDark() ? "light" : "dark";
        save();
      },
    },
    {
      id: "effects",
      label: "Cycle visual effects (full, balanced, minimal)",
      hint: "Performance",
      icon: "sparkle",
      run: () => {
        prefs.effects = prefs.effects === "full" ? "balanced" : prefs.effects === "balanced" ? "minimal" : "full";
        save();
      },
    },
    {
      id: "sidebar",
      label: "Toggle compact sidebar",
      hint: "Layout",
      icon: "collapse",
      run: () => {
        prefs.sidebar = prefs.sidebar === "full" ? "compact" : "full";
        save();
      },
    },
  ];

  const all = $derived([
    ...fixed,
    ...courses
      .filter((c) => c.status !== "withdrawn")
      .map((c) => ({
        id: `course-${c.id}`,
        label: c.title,
        hint: "Course",
        icon: "courses",
        run: go(c.status === "draft" ? `/build/${encodeURIComponent(c.id)}` : `/course/${encodeURIComponent(c.id)}`),
      })),
  ]);

  const matches = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return all.slice(0, 12);
    const words = q.split(/\s+/);
    return all.filter((c) => words.every((w) => `${c.label} ${c.hint}`.toLowerCase().includes(w))).slice(0, 12);
  });

  $effect(() => {
    if (open) {
      query = "";
      selected = 0;
      api.courses().then((c) => (courses = c)).catch(() => {});
      tick().then(() => input?.focus());
    }
  });

  function run(c: Command | undefined) {
    if (!c) return;
    open = false;
    c.run();
  }

  function keys(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      open = false;
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      selected = (selected + 1) % Math.max(1, matches.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      selected = (selected - 1 + matches.length) % Math.max(1, matches.length);
    } else if (e.key === "Enter") {
      e.preventDefault();
      run(matches[selected]);
    }
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="backdrop" onclick={() => (open = false)}>
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="palette" role="dialog" aria-modal="true" aria-label="Go to" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <div class="search">
        <Icon name="search" />
        <input
          bind:this={input}
          bind:value={query}
          oninput={() => (selected = 0)}
          onkeydown={keys}
          placeholder="Go to a page or course, or change a setting"
          aria-label="Search"
          role="combobox"
          aria-expanded="true"
          aria-controls="palette-list"
          aria-activedescendant={matches[selected] ? `cmd-${matches[selected].id}` : undefined}
        />
        <kbd>Esc</kbd>
      </div>
      <ul id="palette-list" role="listbox">
        {#each matches as c, i (c.id)}
          <li
            id="cmd-{c.id}"
            role="option"
            aria-selected={i === selected}
            class:sel={i === selected}
            onpointermove={() => (selected = i)}
            onclick={() => run(c)}
          >
            <Icon name={c.icon} size={16} />
            <span class="label">{c.label}</span>
            <span class="hint small muted">{c.hint}</span>
          </li>
        {:else}
          <li class="none muted small">Nothing matches.</li>
        {/each}
      </ul>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    background: color-mix(in srgb, var(--bg) 60%, transparent);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 12vh;
  }
  :global([data-effects="full"]) .backdrop {
    backdrop-filter: blur(6px);
  }
  .palette {
    width: min(36rem, calc(100vw - 2rem));
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: 0 18px 50px rgb(0 0 0 / 0.25);
    overflow: hidden;
  }
  :global([data-motion="full"]) .palette {
    animation: rise var(--fast) var(--ease) both;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.6rem 0.9rem;
    border-bottom: 1px solid var(--line);
    color: var(--muted);
  }
  .search input {
    border: 0;
    background: transparent;
    padding: 0.35rem 0;
    font-size: 1.02rem;
  }
  .search input:focus-visible {
    outline: none;
  }
  kbd {
    font: inherit;
    font-size: 0.75rem;
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 0 0.35rem;
    color: var(--muted);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0.35rem;
    max-height: 22rem;
    overflow-y: auto;
  }
  li {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.5rem 0.6rem;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  li.sel {
    background: var(--accent-soft);
    color: var(--text);
  }
  .label {
    flex: 1;
  }
  .none {
    cursor: default;
  }
</style>
