<script lang="ts">
  import { onMount } from "svelte";
  import { api, type AppStatus, type BackupInfo } from "#lib/api.ts";
  import { when } from "#lib/format.ts";
  import DisplaySettings from "#lib/DisplaySettings.svelte";
  import PatchNotes from "#lib/PatchNotes.svelte";

  const sections = [
    { id: "whats-new", label: "What's new" },
    { id: "appearance", label: "Appearance" },
    { id: "layout", label: "Layout" },
    { id: "performance", label: "Motion and performance" },
    { id: "study", label: "Study and goals" },
    { id: "shortcuts", label: "Shortcuts" },
    { id: "claude", label: "Claude Code" },
    { id: "data", label: "Data and backups" },
  ];

  let status = $state<AppStatus | null>(null);
  let loadError = $state<string | null>(null);
  let testing = $state(false);
  let test = $state<Awaited<ReturnType<typeof api.test>> | null>(null);
  let testError = $state<string | null>(null);
  let backups = $state<BackupInfo[]>([]);
  let backingUp = $state(false);
  let backupMsg = $state<string | null>(null);
  let backupError = $state<string | null>(null);

  async function loadBackups() {
    backups = await api.backups().catch(() => []);
  }

  async function backupNow() {
    backingUp = true;
    backupMsg = backupError = null;
    try {
      await api.backupNow();
      backupMsg = "Backed up.";
      await loadBackups();
    } catch (e) {
      backupError = String(e);
    } finally {
      backingUp = false;
    }
  }

  function size(b: number) {
    return b < 1024 * 1024 ? `${Math.max(1, Math.round(b / 1024))} KB` : `${(b / 1024 / 1024).toFixed(1)} MB`;
  }

  async function refresh() {
    loadError = null;
    try {
      status = await api.status();
    } catch (e) {
      loadError = String(e);
    }
  }

  async function runTest() {
    testing = true;
    test = null;
    testError = null;
    try {
      test = await api.test();
    } catch (e) {
      testError = String(e);
    } finally {
      testing = false;
    }
  }

  const ready = $derived(!!status?.claude.installed && !!status?.claude.logged_in);
  onMount(() => {
    refresh();
    loadBackups();
  });
</script>

<div class="page-enter">
<h1>Settings</h1>
<p class="lead">How SelfSemester looks and runs, how it reaches Claude, and where it keeps your files.</p>

<div class="settings-grid">
<nav class="jump" aria-label="Settings sections">
  {#each sections as s}<a href="#{s.id}">{s.label}</a>{/each}
</nav>

<div class="settings-body">
<PatchNotes />

<DisplaySettings />

<section class="card" id="claude" aria-live="polite">
  <div class="spread">
    <h2>Claude Code</h2>
    <button class="ghost" onclick={refresh}>Check again</button>
  </div>
  {#if loadError}
    <p class="bad">{loadError}</p>
  {:else if !status}
    <p class="muted">Checking…</p>
  {:else}
    <dl>
      <dt>Installed</dt>
      <dd>{#if status.claude.installed}<span class="good">Yes</span> <span class="muted">{status.claude.version}</span>{:else}<span class="bad">No</span>{/if}</dd>
      <dt>Signed in</dt>
      <dd>{#if status.claude.logged_in}<span class="good">Yes</span>{:else}<span class="bad">No</span>{/if}</dd>
      <dt>Billing</dt>
      <dd>
        {#if status.claude.on_subscription}
          <span class="good">Your Claude subscription</span> <span class="muted">· no extra charges</span>
        {:else if status.claude.logged_in}
          <span class="warn">Per-use ({status.claude.auth_method})</span>
        {:else}<span class="muted">-</span>{/if}
      </dd>
    </dl>
    {#if status.claude.problem}<p class="note">{status.claude.problem}</p>{/if}
    <p class="small muted">
      SelfSemester runs your own Claude Code and never sees your sign-in. Work counts toward your plan's usage limits; if you
      hit one, work pauses and picks up again when the limit resets.
    </p>
  {/if}
</section>

<section class="card">
  <div class="spread">
    <h2>Test call</h2>
    <button onclick={runTest} disabled={!ready || testing}>{testing ? "Running…" : "Run test"}</button>
  </div>
  <p class="muted small">One small Haiku request through the job queue, to check everything works end to end.</p>
  {#if testError}<p class="bad">{testError}</p>{/if}
  {#if test}
    {#if test.job.status === "done" && test.result}
      <p class="good">{test.result.message}</p>
    {:else if test.job.status === "waiting_usage"}
      <p class="warn">Usage limit reached. It will retry after {when(test.job.next_attempt_at)}.</p>
    {:else}
      <p class="bad">{test.job.status}: {test.job.error}</p>
    {/if}
  {/if}
</section>

<section class="card" id="data">
  <div class="spread">
    <h2>Your data and backups</h2>
    <button class="ghost" onclick={backupNow} disabled={backingUp}>{backingUp ? "Backing up…" : "Back up now"}</button>
  </div>
  <p class="small muted">
    Updating SelfSemester never touches your courses: the installer only replaces the program. On top of that, before a new
    version opens your data for the first time, the app copies your database and every course's files to
    <code>Documents\SelfSemester\Backups</code>. If it ever starts and finds your data missing, it puts the newest backup
    back by itself. The last 5 backups are kept.
  </p>
  {#if status?.startup.problem}<p class="note bad">The backup at start-up didn't work: {status.startup.problem}</p>{/if}
  {#if status?.startup.result?.kind === "backed_up"}<p class="small good">Version {status.startup.version} backed up your data before its first start.</p>{/if}
  {#if status?.startup.result?.kind === "restored"}<p class="note">Your data was missing at start-up, so the backup from {when(status.startup.result.created_at)} was restored.</p>{/if}
  {#if backupMsg}<p class="small good">{backupMsg}</p>{/if}
  {#if backupError}<p class="note bad">{backupError}</p>{/if}
  {#if backups.length}
    <div class="table-wrap">
      <table>
        <thead><tr><th>Made</th><th>Why</th><th>Size</th></tr></thead>
        <tbody>
          {#each backups as b (b.folder)}
            <tr>
              <td>{b.manifest ? when(b.manifest.created_at) : b.folder}</td>
              <td class="small">{b.manifest?.reason === "update" ? `Before version ${b.manifest.app_version}${b.manifest.from_version ? ` (from ${b.manifest.from_version})` : ""}` : "By you"}</td>
              <td class="small">{size(b.bytes)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {:else}
    <p class="small muted">No backups yet. One is made before each update, or press Back up now.</p>
  {/if}
  {#if status}<button class="ghost small" onclick={() => api.openFolder(status!.user_dir)}>Open the Documents folder</button>{/if}
</section>

{#if status}
  <section class="card">
    <h2>Folders</h2>
    <dl>
      <dt>Your files</dt>
      <dd><button class="ghost linkish" onclick={() => api.openFolder(status!.user_dir)}><code>{status.user_dir}</code></button></dd>
      <dt>App data</dt>
      <dd><button class="ghost linkish" onclick={() => api.openFolder(status!.data_dir)}><code>{status.data_dir}</code></button></dd>
      <dt>Grading</dt>
      <dd>Percent, letter grade and 4.0 GPA (Ontario-style bands)</dd>
    </dl>
    <p class="small muted">SelfSemester {status.startup.version} · database schema v{status.schema_version}</p>
  </section>
{/if}
</div>
</div>
</div>

<style>
  .settings-grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 0 1.5rem;
    align-items: start;
  }
  /* A wide window gets the section list as a column on the left. */
  @container page (min-width: 60rem) {
    .settings-grid {
      grid-template-columns: 12rem minmax(0, 54rem);
    }
    .jump {
      flex-direction: column;
      top: 1rem;
      border-bottom: 0;
      background: transparent;
    }
    .jump a {
      border-radius: var(--radius-sm);
    }
  }
  .jump {
    position: sticky;
    top: 0;
    z-index: 5;
    display: flex;
    gap: 0.3rem;
    flex-wrap: wrap;
    padding: 0.5rem 0;
    margin-bottom: 0.75rem;
    background: var(--bg);
    border-bottom: 1px solid var(--line);
  }
  .jump a {
    padding: 0.25rem 0.7rem;
    border-radius: 999px;
    color: var(--text);
    font-size: 0.88rem;
  }
  .jump a:hover {
    background: var(--accent-soft);
    text-decoration: none;
  }
  section {
    scroll-margin-top: 3.5rem;
  }
  dl {
    display: grid;
    grid-template-columns: 7rem 1fr;
    gap: 0.4rem 1rem;
    margin: 0;
  }
  dt {
    color: var(--muted);
  }
  dd {
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .linkish {
    border: 0;
    padding: 0;
    text-align: left;
  }
</style>
