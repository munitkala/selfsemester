<script lang="ts">
  import "katex/dist/katex.min.css";
  import "../app.css";
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import { api, live, type JobSummary } from "$lib/api";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { stageLabel, when } from "$lib/format";

  let { children } = $props();
  let jobs = $state<JobSummary | null>(null);
  let claudeProblem = $state<string | null>(null);

  const nav = [
    { href: "/", label: "Today" },
    { href: "/courses", label: "Courses" },
    { href: "/new", label: "New course" },
    { href: "/transcript", label: "Transcript" },
    { href: "/settings", label: "Settings" },
  ];

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
    document.addEventListener("click", externalLinks);
    api.status().then((s) => (claudeProblem = s.claude.installed && s.claude.logged_in ? null : s.claude.problem)).catch(() => {});
    const stop = live(async () => {
      try {
        jobs = await api.jobs();
      } catch {
        /* status is best-effort */
      }
    });
    return () => {
      stop();
      document.removeEventListener("click", externalLinks);
    };
  });
</script>

<div class="shell">
  <aside>
    <div class="brand">SelfSemester</div>
    <nav aria-label="Main">
      {#each nav as n}
        <a href={n.href} class:active={active(n.href)} aria-current={active(n.href) ? "page" : undefined}>{n.label}</a>
      {/each}
    </nav>
    <div class="activity" aria-live="polite">
      {#if jobs && (jobs.running || jobs.queued)}
        <span class="spinner" aria-hidden="true"></span>
        <span>Claude is {stageLabel[jobs.running_kind ?? ""] ?? "working"}{jobs.queued ? ` · ${jobs.queued} queued` : ""}</span>
      {:else if jobs?.waiting_usage}
        <span class="warn">Usage limit reached. Resumes {when(jobs.next_attempt_at)}.</span>
      {:else}
        <span class="muted">Claude is idle</span>
      {/if}
    </div>
  </aside>
  <main>
    {#if claudeProblem}
      <p class="note bad">{claudeProblem} <a href="/settings">Settings</a></p>
    {/if}
    {@render children()}
  </main>
</div>

<style>
  .shell {
    display: grid;
    grid-template-columns: 13rem 1fr;
    min-height: 100vh;
  }
  aside {
    background: var(--sidebar);
    border-right: 1px solid var(--line);
    padding: 1.25rem 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
    position: sticky;
    top: 0;
    height: 100vh;
  }
  .brand {
    font-weight: 700;
    font-size: 1.1rem;
    padding: 0 0.6rem;
    letter-spacing: -0.01em;
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }
  nav a {
    color: var(--text);
    padding: 0.45rem 0.6rem;
    border-radius: 7px;
  }
  nav a:hover {
    background: var(--panel);
    text-decoration: none;
  }
  nav a.active {
    background: var(--panel);
    font-weight: 600;
    box-shadow: inset 3px 0 0 var(--accent);
  }
  .activity {
    margin-top: auto;
    font-size: 0.82rem;
    padding: 0 0.6rem;
    display: flex;
    gap: 0.5rem;
    align-items: flex-start;
  }
  main {
    padding: 2rem 2rem 4rem;
    max-width: 58rem;
    width: 100%;
  }
  @media (max-width: 760px) {
    .shell {
      grid-template-columns: 1fr;
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
    }
    main {
      padding: 1.25rem 1rem 3rem;
    }
  }
</style>
