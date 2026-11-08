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
  // First-launch terms notice: null while checking, false until accepted.
  let noticeOk = $state<boolean | null>(null);
  let agreed = $state(false);

  async function acceptNotice() {
    await api.acceptNotice();
    noticeOk = true;
  }

  const nav = [
    { href: "/", label: "Today" },
    { href: "/courses", label: "Courses" },
    { href: "/new", label: "New course" },
    { href: "/build", label: "Course builder" },
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
    api.noticeAccepted().then((ok) => (noticeOk = ok)).catch(() => (noticeOk = false));
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

{#if noticeOk === false}
  <div class="notice-backdrop">
    <div class="notice" role="dialog" aria-modal="true" aria-labelledby="notice-title">
      <h1 id="notice-title">Before you start</h1>
      <p class="muted">SelfSemester is free software, provided as is. Please read and accept these points.</p>
      <ul>
        <li><strong>No warranty or liability.</strong> You use it at your own risk. The authors aren't responsible for any loss, including wrong marks, lost data, or usage and charges on your Claude account.</li>
        <li><strong>Not professional advice.</strong> Courses are AI-generated self-study material and can contain mistakes. For health, law, money, safety or any subject where an error could cause harm, check everything with a qualified professional.</li>
        <li><strong>Not a qualification.</strong> Marks and transcripts aren't accredited and give no degree, credit or certification.</li>
        <li><strong>Personal study only.</strong> Don't publish or sell generated courses or downloaded material. Follow each source's licence, and don't use the app to get around paywalls, logins or site terms.</li>
        <li><strong>Your data goes to Anthropic.</strong> Course requests and the work you submit, including photos, are sent to Anthropic through your own Claude Code, under your own agreement with Anthropic. Don't submit other people's personal information.</li>
        <li><strong>Academic integrity.</strong> If you're at a school or university, follow its rules on AI tools.</li>
        <li><strong>Back up your data.</strong> Everything is stored only on this computer.</li>
      </ul>
      <p class="small muted">Full terms: DISCLAIMER.md at <a href="https://github.com/munitkala/selfsemester/blob/main/DISCLAIMER.md">github.com/munitkala/selfsemester</a>. SelfSemester isn't affiliated with or endorsed by Anthropic.</p>
      <label class="agree"><input type="checkbox" bind:checked={agreed} /> I've read and accept these terms</label>
      <button onclick={acceptNotice} disabled={!agreed}>Continue</button>
    </div>
  </div>
{/if}

<div class="shell" inert={noticeOk === false}>
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
        <span>{jobs.running ? `Claude is ${stageLabel[jobs.running_kind ?? ""] ?? "working"}` : "Steps waiting to start"}{jobs.queued ? ` · ${jobs.queued} queued` : ""}</span>
      {:else if jobs?.waiting_usage}
        <div>
          <span class="warn">Paused: usage limit reached. Next try {when(jobs.next_attempt_at)}.</span>
          <button class="ghost resume" onclick={() => api.resume()}>Resume now</button>
        </div>
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
  .notice-backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    background: color-mix(in srgb, var(--bg) 70%, transparent);
    backdrop-filter: blur(3px);
    display: grid;
    place-items: center;
    padding: 1rem;
    overflow-y: auto;
  }
  .notice {
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 1.5rem 1.75rem;
    max-width: 40rem;
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.18);
  }
  .notice ul {
    padding-left: 1.2rem;
  }
  .notice li {
    margin: 0.45rem 0;
  }
  .agree {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    font-weight: 600;
    margin: 1rem 0;
  }
  .agree input {
    width: auto;
  }
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
  .resume {
    margin-top: 0.4rem;
    padding: 0.2rem 0.7rem;
    font-size: 0.82rem;
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
