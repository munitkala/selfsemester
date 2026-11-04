<script lang="ts">
  import { onMount } from "svelte";
  import { api, type AppStatus } from "$lib/api";
  import { when } from "$lib/format";

  let status = $state<AppStatus | null>(null);
  let loadError = $state<string | null>(null);
  let testing = $state(false);
  let test = $state<Awaited<ReturnType<typeof api.test>> | null>(null);
  let testError = $state<string | null>(null);

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
  onMount(refresh);
</script>

<h1>Settings</h1>
<p class="lead">How SelfSemester reaches Claude, and where it keeps your files.</p>

<section class="card" aria-live="polite">
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
    <p class="small muted">Database schema v{status.schema_version}</p>
  </section>
{/if}

<style>
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
