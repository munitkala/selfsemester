<script lang="ts">
  import "katex/dist/katex.min.css";
  import "../app.css";
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import { api, live, type JobSummary } from "#lib/api.ts";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { stageLabel, when } from "#lib/format.ts";
  import { goto } from "$app/navigation";
  import { apply, prefs, save, watchSystem } from "#lib/prefs.svelte.ts";
  import Icon from "#lib/Icon.svelte";
  import CommandPalette from "#lib/CommandPalette.svelte";
  import { widgets } from "#lib/widgets.ts";
  import ToolSetup from "#lib/ToolSetup.svelte";
  import type { ToolsView } from "#lib/api.ts";

  let { children } = $props();
  let jobs = $state<JobSummary | null>(null);
  let claudeProblem = $state<string | null>(null);
  let restored = $state<string | null>(null);
  // First-launch terms notice: null while checking, false until accepted.
  let noticeOk = $state<boolean | null>(null);
  let agreed = $state(false);
  let palette = $state(false);
  // First-run tool picker, shown once after the terms are accepted.
  let toolSetup = $state<ToolsView | null>(null);

  async function acceptNotice() {
    await api.acceptNotice();
    noticeOk = true;
    api.tools().then((v) => (toolSetup = v)).catch(() => {});
  }

  const nav = [
    { href: "/", label: "Today", icon: "today" },
    { href: "/courses", label: "Courses", icon: "courses" },
    { href: "/insights", label: "Insights", icon: "insights" },
    { href: "/new", label: "New course", icon: "new" },
    { href: "/build", label: "Course builder", icon: "builder" },
    { href: "/transcript", label: "Transcript", icon: "transcript" },
    { href: "/settings", label: "Settings", icon: "settings" },
  ];
  // In a small window the sidebar shrinks to icons by itself, so the page keeps its room.
  let windowWidth = $state(1200);
  const autoCompact = $derived(windowWidth < 980);
  const compact = $derived(prefs.sidebar === "compact" || autoCompact);
  // Lessons, work and forms read best at a fixed width; the rest of the app fills the window.
  const reading = $derived(/^\/(item\/|new$|build$)/.test(page.url.pathname));

  function toggleSidebar() {
    prefs.sidebar = prefs.sidebar === "compact" ? "full" : "compact";
    save();
  }

  function shortcuts(e: KeyboardEvent) {
    if (prefs.shortcuts && (e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === "k") {
      e.preventDefault();
      palette = !palette;
    }
  }

  function active(href: string) {
    const p = page.url.pathname;
    return href === "/" ? p === "/" : p.startsWith(href) || (href === "/courses" && (p.startsWith("/course/") || p.startsWith("/item/")));
  }

  /** Web links (lesson sources, reading list) open in the default browser, not inside the app window. */
  function externalLinks(e: MouseEvent) {
    const a = (e.target as HTMLElement | null)?.closest?.("a");
    const href = a?.getAttribute("href") ?? "";
    if (/^https?:\/\//i.test(href)) {
      e.preventDefault();
      openUrl(href).catch(() => {});
    }
  }

  onMount(() => {
    apply();
    const unwatch = watchSystem();
    // The start page applies once per launch, so going back to Today later still works.
    try {
      if (page.url.pathname === "/" && prefs.startPage !== "/" && !sessionStorage.getItem("started")) goto(prefs.startPage);
      sessionStorage.setItem("started", "1");
    } catch {
      /* storage blocked: stay on Today */
    }
    document.addEventListener("click", externalLinks);
    document.addEventListener("keydown", shortcuts);
    api.noticeAccepted().then((ok) => {
      noticeOk = ok;
      if (ok) api.tools().then((v) => (toolSetup = v)).catch(() => {});
    }).catch(() => (noticeOk = false));
    api
      .status()
      .then((s) => {
        claudeProblem = s.claude.installed && s.claude.logged_in ? null : s.claude.problem;
        if (s.startup.result?.kind === "restored") restored = s.startup.result.created_at;
      })
      .catch(() => {});
    const stop = live(async () => {
      try {
        jobs = await api.jobs();
      } catch {
        /* status is best-effort */
      }
    });
    return () => {
      stop();
      unwatch();
      document.removeEventListener("click", externalLinks);
      document.removeEventListener("keydown", shortcuts);
    };
  });
</script>

{#if noticeOk && toolSetup && !toolSetup.setup_done}
  <ToolSetup tools={toolSetup.tools} ondone={() => (toolSetup = null)} />
{/if}

{#if noticeOk === false}
  <div class="notice-backdrop">
    <div class="notice" role="dialog" aria-modal="true" aria-labelledby="notice-title">
      <h1 id="notice-title">Before you start</h1>
      <p class="muted">SelfSemester is free software, provided as is. Please read and accept these points.</p>
      <ul>
        <li><strong>No warranty or liability.</strong> You use it at your own risk. The authors aren't responsible for any loss, including wrong marks, lost data, or usage and charges on your Claude account.</li>
        <li><strong>Not professional advice.</strong> Courses Claude writes are AI-generated self-study material and can contain mistakes; courses you write yourself are your own content. For health, law, money, safety or any subject where an error could cause harm, check everything with a qualified professional.</li>
        <li><strong>Not a qualification.</strong> Marks and transcripts aren't accredited and give no degree, credit or certification.</li>
        <li><strong>Personal study only.</strong> Don't publish or sell generated courses or downloaded material. Follow each source's licence, and don't use the app to get around paywalls, logins or site terms. Lessons draw their own figures and credit what they adapt, but generated content can still resemble existing material.</li>
        <li><strong>Your data goes to Anthropic.</strong> Course requests and the work you submit, including photos, are sent to Anthropic through your own Claude Code, under your own agreement with Anthropic. Don't submit other people's personal information.</li>
        <li><strong>Academic integrity.</strong> If you're at a school or university, follow its rules on AI tools.</li>
        <li><strong>Back up your data.</strong> Everything is stored only on this computer. SelfSemester makes a copy in Documents before each update, but keep your own copy somewhere else too.</li>
      </ul>
      <p class="small muted">Full terms: DISCLAIMER.md at <a href="https://github.com/munitkala/selfsemester/blob/main/DISCLAIMER.md">github.com/munitkala/selfsemester</a>. SelfSemester isn't affiliated with or endorsed by Anthropic.</p>
      <label class="agree"><input type="checkbox" bind:checked={agreed} /> I've read and accept these terms</label>
      <button onclick={acceptNotice} disabled={!agreed}>Continue</button>
    </div>
  </div>
{/if}

<svelte:window bind:innerWidth={windowWidth} />

<div class="shell" class:compact inert={noticeOk === false || !!(noticeOk && toolSetup && !toolSetup.setup_done)}>
  <aside>
    <div class="brand">
      <span class="mark" aria-hidden="true">S</span>
      {#if !compact}<span>SelfSemester</span>{/if}
    </div>
    {#if prefs.shortcuts}
      <button class="ghost find" onclick={() => (palette = true)} title="Go to (Ctrl+K)" aria-label="Go to (Ctrl+K)">
        <Icon name="search" size={16} />
        {#if !compact}<span>Go to…</span><kbd>Ctrl K</kbd>{/if}
      </button>
    {/if}
    <nav aria-label="Main">
      {#each nav as n}
        <a href={n.href} class:active={active(n.href)} aria-current={active(n.href) ? "page" : undefined} title={compact ? n.label : undefined}>
          <Icon name={n.icon} />
          <span class:sr={compact}>{n.label}</span>
        </a>
      {/each}
    </nav>
    <div class="activity" aria-live="polite">
      {#if jobs && (jobs.running || jobs.queued)}
        {@const busy = jobs.running ? `Claude is ${stageLabel[jobs.running_kind ?? ""] ?? "working"}` : "Steps waiting to start"}
        <span class="chip busy" title={compact ? busy : undefined}>
          <span class="spinner" aria-hidden="true"></span>
          <span class:sr={compact}>{busy}{jobs.queued ? ` · ${jobs.queued} queued` : ""}</span>
        </span>
      {:else if jobs?.waiting_usage}
        <div>
          <span class="warn">Paused: usage limit reached.<span class:sr={compact}> Next try {when(jobs.next_attempt_at)}.</span></span>
          <button class="ghost resume" onclick={() => api.resume()}>Resume now</button>
        </div>
      {:else}
        <span class="chip" title={compact ? "Claude is idle" : undefined}>
          <span class="dot" aria-hidden="true"></span>
          <span class="muted" class:sr={compact}>Claude is idle</span>
        </span>
      {/if}
      {#if !autoCompact}
      <button
        class="ghost collapse"
        onclick={toggleSidebar}
        aria-label={compact ? "Expand sidebar" : "Collapse sidebar"}
        title={compact ? "Expand sidebar" : "Collapse sidebar"}
      >
        <Icon name={compact ? "expand" : "collapse"} size={16} />
      </button>
      {/if}
    </div>
  </aside>
  <main class:reading use:widgets>
    {#if restored}
      <p class="note">Your data was missing when SelfSemester started, so it restored your newest backup (from {when(restored)}). <a href="/settings">Settings</a> <button class="ghost small" onclick={() => (restored = null)}>OK</button></p>
    {/if}
    {#if claudeProblem}
      <p class="note bad">{claudeProblem} <a href="/settings">Settings</a></p>
    {/if}
    {@render children()}
  </main>
</div>

<CommandPalette bind:open={palette} />

<style>
  :global(.notice-backdrop) {
    position: fixed;
    inset: 0;
    z-index: 50;
    background: color-mix(in srgb, var(--bg) 82%, transparent);
    display: grid;
    place-items: center;
    padding: 1rem;
    overflow-y: auto;
  }
  :global(.notice) {
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 1.5rem 1.75rem;
    max-width: 40rem;
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.18);
  }
  :global([data-effects="full"] .notice-backdrop) {
    background: color-mix(in srgb, var(--bg) 70%, transparent);
    backdrop-filter: blur(4px);
  }
  :global(.notice ul) {
    padding-left: 1.2rem;
  }
  :global(.notice li) {
    margin: 0.45rem 0;
  }
  :global(.notice .agree) {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    font-weight: 600;
    margin: 1rem 0;
  }
  :global(.notice .agree input) {
    width: auto;
  }
  .shell {
    display: grid;
    grid-template-columns: 14rem minmax(0, 1fr);
    min-height: 100vh;
    transition: grid-template-columns var(--slow) var(--ease);
  }
  .shell.compact {
    grid-template-columns: 4.25rem minmax(0, 1fr);
  }
  aside {
    background: var(--sidebar);
    border-right: 1px solid var(--line);
    padding: 1.1rem 0.7rem;
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    position: sticky;
    top: 0;
    height: 100vh;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    font-weight: 700;
    font-size: 1.08rem;
    padding: 0 0.35rem;
    letter-spacing: -0.01em;
  }
  .mark {
    display: grid;
    place-items: center;
    width: 1.9rem;
    height: 1.9rem;
    flex: none;
    border-radius: var(--radius-sm);
    background: var(--accent);
    color: var(--on-accent);
    font-size: 1rem;
  }
  :global([data-effects="full"]) .mark,
  :global([data-effects="balanced"]) .mark {
    background: linear-gradient(135deg, var(--accent), color-mix(in oklch, var(--accent) 55%, var(--chart-2)));
  }
  .find {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    border-color: var(--line);
    background: var(--panel);
    color: var(--muted);
    padding: 0.4rem 0.6rem;
    font-size: 0.88rem;
    text-align: left;
  }
  .find span {
    flex: 1;
  }
  .find kbd {
    font: inherit;
    font-size: 0.72rem;
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 0 0.3rem;
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }
  nav a {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    color: var(--text);
    padding: 0.5rem 0.6rem;
    border-radius: var(--radius-sm);
    transition: background-color var(--fast), color var(--fast);
  }
  nav a :global(.icon) {
    color: var(--muted);
    transition: color var(--fast);
  }
  nav a:hover {
    background: var(--panel);
    text-decoration: none;
  }
  nav a.active {
    background: var(--accent-soft);
    color: var(--text);
    font-weight: 600;
  }
  nav a.active :global(.icon),
  nav a:hover :global(.icon) {
    color: var(--accent);
  }
  .compact nav a,
  .compact .find {
    justify-content: center;
  }
  .compact .brand {
    justify-content: center;
    padding: 0;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
  .resume {
    margin-top: 0.4rem;
    padding: 0.2rem 0.7rem;
    font-size: 0.82rem;
  }
  .activity {
    margin-top: auto;
    font-size: 0.82rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .chip {
    display: flex;
    gap: 0.45rem;
    align-items: center;
    padding: 0.35rem 0.55rem;
    border-radius: 999px;
    border: 1px solid var(--line);
    background: var(--panel);
  }
  .chip.busy {
    border-color: var(--accent-line);
  }
  .compact .chip {
    justify-content: center;
  }
  .dot {
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 50%;
    background: var(--good);
    flex: none;
  }
  .collapse {
    align-self: flex-start;
    border-color: transparent;
    color: var(--muted);
    padding: 0.3rem 0.45rem;
  }
  .compact .collapse {
    align-self: center;
  }
  /* The page is a size container, so panels inside rearrange to the space they actually get (with or without the
     sidebar) rather than to the whole window. */
  main {
    container: page / inline-size;
    padding: 2rem clamp(1rem, 3vw, 2.5rem) 4rem;
    max-width: 96rem;
    width: 100%;
    min-width: 0;
  }
  :global([data-width="normal"]) main {
    max-width: 60rem;
  }
  :global([data-width="wide"]) main {
    max-width: 78rem;
  }
  :global([data-width="full"]) main {
    max-width: none;
  }
  /* Reading pages sit in the middle of the window, so a wide window doesn't leave all the space on one side. */
  main.reading {
    max-width: 64rem;
    margin-inline: auto;
  }
  @media (max-width: 640px) {
    .shell,
    .shell.compact {
      grid-template-columns: minmax(0, 1fr);
    }
    aside {
      position: static;
      height: auto;
      flex-direction: row;
      flex-wrap: wrap;
      align-items: center;
    }
    nav {
      flex-direction: row;
      flex-wrap: wrap;
    }
    .activity {
      margin-top: 0;
      flex-direction: row;
    }
    main {
      padding: 1.25rem 1rem 3rem;
    }
  }
</style>
