<script lang="ts">
  // Settings → AI services. Claude Code does every job unless the learner says otherwise. They can save any number
  // of other services (any API, or a program on this computer) and pick, job by job, who does what.
  import { onMount } from "svelte";
  import { api, type AiView, type ApiKind, type Roles, type Service, type ServiceView } from "#lib/api.ts";
  import { isClaude, type Job } from "#lib/ai.ts";
  import { when } from "#lib/format.ts";
  import FitReport from "#lib/FitReport.svelte";

  let { onchange }: { onchange?: (v: AiView) => void } = $props();

  type Kind = { value: ApiKind; label: string; hint: string; placeholder: string; header: string };
  const KINDS: Kind[] = [
    { value: "openai_chat", label: "OpenAI-style chat completions", hint: "Most services and local servers: OpenAI, OpenRouter, Groq, Mistral, DeepSeek, LM Studio, llama.cpp, vLLM and more.", placeholder: "https://api.example.com/v1", header: "Authorization" },
    { value: "openai_responses", label: "OpenAI Responses API", hint: "OpenAI's newer API. It can search the web itself.", placeholder: "https://api.openai.com/v1", header: "Authorization" },
    { value: "anthropic", label: "Anthropic Messages API", hint: "Claude models billed per use with an API key. It can search the web and read PDFs.", placeholder: "https://api.anthropic.com/v1", header: "x-api-key" },
    { value: "gemini", label: "Google Gemini API", hint: "Google's own API. It can search the web and read PDFs.", placeholder: "https://generativelanguage.googleapis.com/v1beta", header: "x-goog-api-key" },
    { value: "ollama", label: "Ollama's own API", hint: "Lets you set the context size. The address has no /v1.", placeholder: "http://localhost:11434", header: "Authorization" },
    { value: "custom", label: "Any other HTTP API", hint: "Write the request body yourself and say where the answer is.", placeholder: "https://example.com/api", header: "Authorization" },
    { value: "command", label: "A program on this computer", hint: "Any command-line tool that reads a prompt and prints an answer.", placeholder: "", header: "" },
  ];
  const kindOf = (k: ApiKind) => KINDS.find((x) => x.value === k) ?? KINDS[0];

  type Preset = { id: string; name: string; api: ApiKind; url: string; images: boolean; web: boolean; pdfs: boolean; hint: string; header?: string };
  // Model names change too often to list; "Load the list" asks the service.
  const PRESETS: Preset[] = [
    { id: "openai", name: "OpenAI", api: "openai_responses", url: "https://api.openai.com/v1", images: true, web: true, pdfs: false, hint: "Make a key at platform.openai.com, under API keys." },
    { id: "openrouter", name: "OpenRouter", api: "openai_chat", url: "https://openrouter.ai/api/v1", images: false, web: false, pdfs: false, hint: "One key for models from many companies. Make one at openrouter.ai/keys." },
    { id: "anthropic", name: "Anthropic API", api: "anthropic", url: "https://api.anthropic.com/v1", images: true, web: true, pdfs: true, hint: "Billed per use, separately from a Claude subscription. To use your subscription, keep Claude Code for the jobs above." },
    { id: "gemini", name: "Google Gemini", api: "gemini", url: "https://generativelanguage.googleapis.com/v1beta", images: true, web: true, pdfs: true, hint: "Make a key in Google AI Studio." },
    { id: "azure", name: "Azure OpenAI", api: "openai_chat", url: "", images: false, web: false, pdfs: false, header: "api-key", hint: "Use your resource's address ending in /openai/v1. The key goes in the api-key header. Older deployments also need api-version under More settings." },
    { id: "mistral", name: "Mistral", api: "openai_chat", url: "https://api.mistral.ai/v1", images: false, web: false, pdfs: false, hint: "Make a key at console.mistral.ai." },
    { id: "deepseek", name: "DeepSeek", api: "openai_chat", url: "https://api.deepseek.com/v1", images: false, web: false, pdfs: false, hint: "Make a key at platform.deepseek.com." },
    { id: "groq", name: "Groq", api: "openai_chat", url: "https://api.groq.com/openai/v1", images: false, web: false, pdfs: false, hint: "Fast, with a free tier that has tight rate limits." },
    { id: "xai", name: "xAI", api: "openai_chat", url: "https://api.x.ai/v1", images: false, web: false, pdfs: false, hint: "Make a key at console.x.ai." },
    { id: "together", name: "Together AI", api: "openai_chat", url: "https://api.together.xyz/v1", images: false, web: false, pdfs: false, hint: "Open models in the cloud." },
    { id: "ollama", name: "Ollama (on this computer)", api: "ollama", url: "http://localhost:11434", images: false, web: false, pdfs: false, hint: "Free and private. Start Ollama and pull a model first. Bigger models make better courses." },
    { id: "lmstudio", name: "LM Studio (on this computer)", api: "openai_chat", url: "http://localhost:1234/v1", images: false, web: false, pdfs: false, hint: "Free and private. In LM Studio, open Developer and start the server." },
    { id: "custom_http", name: "Any other HTTP API", api: "custom", url: "", images: false, web: false, pdfs: false, hint: "Write the request yourself." },
    { id: "command", name: "A program on this computer", api: "command", url: "", images: false, web: false, pdfs: false, hint: "For example: ollama run {{model}}, or llm -m {{model}}" },
  ];

  const JOBS: { id: Job; label: string; hint?: string }[] = [
    { id: "plan", label: "Planning and exams" },
    { id: "write", label: "Lessons, assignments and marking" },
    { id: "check", label: "Quick checks" },
    { id: "research", label: "Web research", hint: "The reading list, and research while writing on Thorough and Max builds" },
  ];

  // Text with {{...}} in it, kept out of the markup where braces mean code.
  const BODY_EXAMPLE = '{"model": {{model}}, "messages": {{messages}}}';
  const PLACEHOLDERS = "{{model}}, {{system}}, {{prompt}}, {{full_prompt}} (system and prompt together), {{messages}} (an OpenAI-style list), {{schema}}, {{max_output}}";
  const COMMAND_EXAMPLE = "ollama run {{model}}";

  const blank = (): Service => ({
    id: "", name: "", api: "openai_chat", preset: "", base_url: "", api_key: "", model: "", strong_model: "", quick_model: "",
    sees_images: false, web_search: false, reads_pdfs: false, auth_header: "", auth_prefix: "", auth_query: "", headers: "",
    query: "", extra_body: "", max_output: null, context: null, timeout_minutes: null, custom_path: "", custom_body: "",
    custom_text_path: "", command: "", check: null,
  });

  let view = $state<AiView | null>(null);
  let roles = $state<Roles | null>(null);
  let rolesSaved = $state(false);
  /** The service being added or edited; null when the form is closed. */
  let draft = $state<Service | null>(null);
  let keyInput = $state("");
  /** Loaded model names, by service id ("" for a service not saved yet). */
  let models = $state<Record<string, string[]>>({});
  let modelsMsg = $state<string | null>(null);
  let busy = $state<"" | "save" | "check" | "models" | "roles">("");
  let error = $state<string | null>(null);
  let saved = $state(false);
  let removing = $state<string | null>(null);

  const saving = $derived(view && draft ? view.services.find((s) => s.id === draft!.id) ?? null : null);
  const kind = $derived(draft ? kindOf(draft.api) : KINDS[0]);
  const preset = $derived(draft ? PRESETS.find((p) => p.id === draft!.preset) : undefined);
  const insecure = $derived(
    !!draft && draft.base_url.startsWith("http://") && !/^http:\/\/(localhost|127\.|\[::1\]|10\.|192\.168\.|100\.(6[4-9]|[7-9]\d|1[01]\d|12[0-7])\.)/.test(draft.base_url),
  );
  const rolesDirty = $derived(!!view && !!roles && JSON.stringify(roles) !== JSON.stringify(view.roles));
  const dirty = $derived.by(() => {
    if (!draft) return false;
    if (!saving || keyInput !== "") return true;
    const { check: _a, key_set: _b, fit: _c, ...was } = saving;
    const { check: _d, ...now } = draft;
    return JSON.stringify({ ...was, api_key: "" }) !== JSON.stringify({ ...now, api_key: "" });
  });

  function take(v: AiView) {
    view = v;
    roles = structuredClone($state.snapshot(v.roles));
    onchange?.(v);
  }

  function nameOf(id: string) {
    return view?.services.find((s) => s.id === id)?.name || "your service";
  }

  // ---------- jobs ----------

  function pickService(job: Job, service: string) {
    if (!roles) return;
    roles[job] = { service, model: "" };
    rolesSaved = false;
  }

  function allClaude() {
    if (!roles) return;
    for (const j of JOBS) roles[j.id] = { service: "claude_code", model: "" };
    rolesSaved = false;
  }

  async function saveRoles() {
    if (!roles) return;
    busy = "roles";
    error = null;
    try {
      take(await api.setRoles($state.snapshot(roles)));
      rolesSaved = true;
    } catch (e) {
      error = String(e);
    } finally {
      busy = "";
    }
  }

  // ---------- services ----------

  function add() {
    draft = blank();
    pickPreset("openai");
    keyInput = "";
    saved = false;
    modelsMsg = null;
  }

  function edit(s: ServiceView) {
    const { key_set: _k, fit: _f, ...service } = $state.snapshot(s);
    draft = service;
    keyInput = "";
    saved = false;
    modelsMsg = null;
  }

  function close() {
    draft = null;
    keyInput = "";
  }

  function pickPreset(id: string) {
    const p = PRESETS.find((x) => x.id === id);
    if (!p || !draft) return;
    const wasPresetUrl = !draft.base_url || PRESETS.some((x) => x.url && x.url === draft!.base_url);
    const wasPresetName = !draft.name || PRESETS.some((x) => x.name === draft!.name);
    draft.preset = id;
    draft.api = p.api;
    if (wasPresetUrl) draft.base_url = p.url;
    if (wasPresetName) draft.name = p.name;
    draft.sees_images = p.images;
    draft.web_search = p.web;
    draft.reads_pdfs = p.pdfs;
    draft.auth_header = p.header ?? "";
    draft.auth_prefix = "";
  }

  /** Save the form. Returns the saved service's id, or null if it didn't save. */
  async function save(key?: string): Promise<string | null> {
    if (!draft) return null;
    busy = "save";
    error = null;
    saved = false;
    const before = new Set(view?.services.map((s) => s.id) ?? []);
    try {
      const v = await api.saveService($state.snapshot(draft), key ?? (keyInput || undefined));
      // A new service gets its id from the app: it's the one that wasn't there before.
      const id = draft.id || v.services.find((s) => !before.has(s.id))?.id || "";
      take(v);
      const now = v.services.find((s) => s.id === id);
      if (now) edit(now);
      saved = true;
      return id;
    } catch (e) {
      error = String(e);
      return null;
    } finally {
      busy = "";
    }
  }

  async function check() {
    const id = dirty || !draft?.id ? await save() : draft.id;
    if (!id) return;
    busy = "check";
    error = null;
    try {
      const [, v] = await api.checkService(id);
      take(v);
      const now = v.services.find((s) => s.id === id);
      if (now) edit(now);
    } catch (e) {
      error = String(e);
    } finally {
      busy = "";
    }
  }

  async function loadModels() {
    if (!draft) return;
    busy = "models";
    modelsMsg = null;
    try {
      const list = await api.serviceModels($state.snapshot(draft), keyInput || undefined);
      models[draft.id] = list;
      modelsMsg = list.length ? `${list.length} models found. Start typing in a model box to pick one.` : "The service didn't list any models. Type the name instead.";
    } catch (e) {
      modelsMsg = `Couldn't load the list: ${e}. You can still type a model name.`;
    } finally {
      busy = "";
    }
  }

  async function remove(id: string) {
    error = null;
    try {
      take(await api.deleteService(id));
      if (draft?.id === id) close();
    } catch (e) {
      error = String(e);
    } finally {
      removing = null;
    }
  }

  const num = (v: string) => (v.trim() === "" ? null : Math.max(0, Math.round(Number(v))) || null);

  onMount(() => {
    api.ai().then(take).catch((e) => (error = String(e)));
  });
</script>

<section class="card" id="ai-service" aria-live="polite">
  <h2>AI services</h2>
  <p class="small muted">
    Claude Code does every job unless you choose otherwise. Add any other AI service below, from a big cloud API to a
    model or program on this computer, then pick who does each job. Check the report before you build: some things
    only Claude Code can do.
  </p>

  {#if view && roles}
    <h3>Who does what</h3>
    <div class="table-wrap">
      <table class="jobs">
        <thead><tr><th>Job</th><th>Done by</th><th>Model</th></tr></thead>
        <tbody>
          {#each JOBS as j (j.id)}
            {@const t = roles[j.id]}
            <tr>
              <td>
                {j.label}
                {#if j.hint}<span class="hint block">{j.hint}</span>{/if}
              </td>
              <td>
                <select aria-label={`Who does ${j.label}`} value={isClaude(t) ? "claude_code" : t.service} onchange={(e) => pickService(j.id, e.currentTarget.value)}>
                  <option value="claude_code">Claude Code</option>
                  {#each view.services as s (s.id)}<option value={s.id}>{s.name || s.base_url || "Unnamed service"}</option>{/each}
                </select>
              </td>
              <td>
                {#if !isClaude(t)}
                  <input
                    aria-label={`Model for ${j.label}`}
                    bind:value={t.model}
                    oninput={() => (rolesSaved = false)}
                    list={`models-${t.service}`}
                    placeholder={`Usual model: ${view.services.find((s) => s.id === t.service)?.model || "none set"}`}
                    autocomplete="off"
                    spellcheck="false"
                  />
                {:else}
                  <span class="muted small">Picked by the build level</span>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    {#each Object.entries(models) as [id, list] (id)}
      <datalist id={`models-${id}`}>{#each list as m}<option value={m}></option>{/each}</datalist>
    {/each}
    <div class="row actions">
      <button onclick={saveRoles} disabled={!!busy || !rolesDirty}>{busy === "roles" ? "Saving…" : "Save"}</button>
      <button class="ghost" onclick={allClaude} disabled={!!busy}>Use Claude Code for everything</button>
      {#if rolesSaved && !rolesDirty}<span class="small good">Saved.</span>{/if}
    </div>
    {#if error && !draft}<p class="note bad">{error}</p>{/if}

    <h3>How well this can build a course</h3>
    <FitReport fit={view.fit} />

    <h3>Your services</h3>
    {#if !view.services.length}
      <p class="small muted">None yet.</p>
    {/if}
    {#each view.services as s (s.id)}
      <div class="svc">
        <div class="grow">
          <strong>{s.name || "Unnamed service"}</strong>
          <span class="small muted">{kindOf(s.api).label}{s.model ? ` · ${s.model}` : ""}</span>
          {#if s.check?.json_ok}<span class="small good">Checked</span>{:else if s.check}<span class="small bad">Check failed</span>{:else}<span class="small muted">Not checked</span>{/if}
        </div>
        <div class="row">
          <button class="ghost small" onclick={() => edit(s)} disabled={!!busy}>Edit</button>
          {#if removing === s.id}
            <button class="ghost small danger" onclick={() => remove(s.id)}>Remove for good</button>
            <button class="ghost small" onclick={() => (removing = null)}>Keep</button>
          {:else}
            <button class="ghost small" onclick={() => (removing = s.id)} disabled={!!busy}>Remove</button>
          {/if}
        </div>
      </div>
    {/each}
    {#if !draft}
      <div class="row actions"><button class="ghost" onclick={add}>Add a service</button></div>
    {/if}

    {#if draft}
      <div class="editor">
        <h3>{draft.id ? `Edit ${draft.name || "service"}` : "Add a service"}</h3>

        <div class="field">
          <label for="svc-preset">Start from</label>
          <select id="svc-preset" value={draft.preset} onchange={(e) => pickPreset(e.currentTarget.value)}>
            {#if !preset}<option value={draft.preset}>Your own settings</option>{/if}
            {#each PRESETS as p}<option value={p.id}>{p.name}</option>{/each}
          </select>
          {#if preset}<p class="hint">{preset.hint}</p>{/if}
        </div>

        <div class="field">
          <label for="svc-name">Name</label>
          <input id="svc-name" bind:value={draft.name} placeholder="e.g. OpenAI" autocomplete="off" />
        </div>

        <div class="field">
          <label for="svc-api">Kind of API</label>
          <select id="svc-api" bind:value={draft.api}>
            {#each KINDS as k}<option value={k.value}>{k.label}</option>{/each}
          </select>
          <p class="hint">{kind.hint}</p>
        </div>

        {#if draft.api === "command"}
          <div class="field">
            <label for="svc-command">Command</label>
            <input id="svc-command" class="mono" bind:value={draft.command} placeholder={COMMAND_EXAMPLE} autocomplete="off" spellcheck="false" />
            <p class="hint">
              The program and its arguments. {"{{model}}"} is replaced with the model name. The prompt goes in on stdin and
              the answer is read from stdout. The key, if any, is in the SELFSEMESTER_API_KEY variable.
            </p>
          </div>
        {:else}
          <div class="field">
            <label for="svc-url">Address</label>
            <input id="svc-url" bind:value={draft.base_url} placeholder={kind.placeholder} autocomplete="off" spellcheck="false" />
            {#if insecure}<p class="small warn">This address isn't encrypted (http, not https) and isn't on your own computer or network, so your key and course work would travel in the open.</p>{/if}
          </div>
        {/if}

        <div class="field">
          <label for="svc-key">API key</label>
          <div class="row">
            <input id="svc-key" type="password" bind:value={keyInput} autocomplete="off" spellcheck="false" placeholder={saving?.key_set ? "Saved. Type a new key to replace it." : "Paste the key here, if it needs one"} />
            {#if saving?.key_set}<button type="button" class="ghost small" onclick={() => save("")} disabled={!!busy}>Remove saved key</button>{/if}
          </div>
          <p class="hint">Kept only on this computer, in the app's own folder, and left out of backups. It's sent only to this service.</p>
        </div>

        <div class="field">
          <div class="spread">
            <label for="svc-model">{draft.api === "custom" || draft.api === "command" ? "Model (optional)" : "Model"}</label>
            {#if draft.api !== "custom" && draft.api !== "command"}
              <button type="button" class="ghost small" onclick={loadModels} disabled={!!busy || !draft.base_url}>{busy === "models" ? "Loading…" : "Load the list"}</button>
            {/if}
          </div>
          <input id="svc-model" bind:value={draft.model} list={`models-${draft.id}`} autocomplete="off" spellcheck="false" />
          <p class="hint">The usual model. Each job above can use a different one. The bigger the model, the better the course.</p>
          {#if modelsMsg}<p class="small muted">{modelsMsg}</p>{/if}
        </div>

        {#if draft.api !== "custom" && draft.api !== "command"}
          <label class="radio"><input type="checkbox" bind:checked={draft.sees_images} /> This model can see images <span class="hint">(needed to mark photos of handwritten work)</span></label>
        {/if}
        {#if draft.api === "openai_responses" || draft.api === "anthropic" || draft.api === "gemini"}
          <label class="radio"><input type="checkbox" bind:checked={draft.web_search} /> The service searches the web itself <span class="hint">(uses the API's own search tool, so the reading list is found on the web)</span></label>
        {/if}
        {#if draft.api === "anthropic" || draft.api === "gemini"}
          <label class="radio"><input type="checkbox" bind:checked={draft.reads_pdfs} /> Send PDFs along</label>
        {/if}

        {#if draft.api === "custom"}
          <fieldset>
            <legend>The request</legend>
            <div class="field">
              <label for="svc-path">Path after the address <span class="hint">optional</span></label>
              <input id="svc-path" class="mono" bind:value={draft.custom_path} placeholder="chat/completions" autocomplete="off" spellcheck="false" />
            </div>
            <div class="field">
              <label for="svc-body">Request body</label>
              <textarea id="svc-body" class="mono" rows="6" bind:value={draft.custom_body} placeholder={BODY_EXAMPLE} spellcheck="false"></textarea>
              <p class="hint">JSON with placeholders that are replaced by JSON values, so don't put quotes around them: {PLACEHOLDERS}.</p>
              <button type="button" class="ghost small" onclick={() => { draft!.custom_body = BODY_EXAMPLE; draft!.custom_text_path = "choices.0.message.content"; }}>Fill in an example</button>
            </div>
            <div class="field">
              <label for="svc-text">Where the answer is</label>
              <input id="svc-text" class="mono" bind:value={draft.custom_text_path} placeholder="choices.0.message.content" autocomplete="off" spellcheck="false" />
            </div>
          </fieldset>
        {/if}

        <details class="more">
          <summary>More settings</summary>
          <div class="pair">
            <div class="field">
              <label for="svc-header">Key header</label>
              <input id="svc-header" bind:value={draft.auth_header} placeholder={kind.header || "Authorization"} autocomplete="off" spellcheck="false" />
            </div>
            <div class="field">
              <label for="svc-prefix">Key prefix</label>
              <input id="svc-prefix" bind:value={draft.auth_prefix} placeholder={draft.auth_header ? "none" : "Bearer "} autocomplete="off" spellcheck="false" />
            </div>
          </div>
          <p class="hint">Leave both empty for the usual header. For Azure OpenAI, use api-key with no prefix.</p>
          <div class="field">
            <label for="svc-auth-query">Send the key as a query parameter instead</label>
            <input id="svc-auth-query" bind:value={draft.auth_query} placeholder="e.g. key" autocomplete="off" spellcheck="false" />
          </div>
          <div class="field">
            <label for="svc-headers">Extra headers</label>
            <textarea id="svc-headers" class="mono" rows="3" bind:value={draft.headers} placeholder={"One per line, e.g.\nHTTP-Referer: https://example.com"} spellcheck="false"></textarea>
          </div>
          <div class="field">
            <label for="svc-query">Extra query parameters</label>
            <textarea id="svc-query" class="mono" rows="2" bind:value={draft.query} placeholder={"One per line, e.g.\napi-version=2024-10-21"} spellcheck="false"></textarea>
          </div>
          <div class="field">
            <label for="svc-extra">Extra body fields</label>
            <textarea id="svc-extra" class="mono" rows="3" bind:value={draft.extra_body} placeholder={'{"temperature": 0.3}'} spellcheck="false"></textarea>
            <p class="hint">A JSON object added to every request, for anything this page doesn't have a box for.</p>
          </div>
          <div class="pair">
            <div class="field">
              <label for="svc-max">Most tokens per answer</label>
              <input id="svc-max" type="number" min="0" value={draft.max_output ?? ""} oninput={(e) => (draft!.max_output = num(e.currentTarget.value))} placeholder="service's default" />
            </div>
            <div class="field">
              <label for="svc-timeout">Minutes before a step counts as stuck</label>
              <input id="svc-timeout" type="number" min="1" value={draft.timeout_minutes ?? ""} oninput={(e) => (draft!.timeout_minutes = num(e.currentTarget.value))} placeholder="20" />
            </div>
          </div>
          {#if draft.api === "ollama"}
            <div class="field">
              <label for="svc-context">Context size</label>
              <input id="svc-context" type="number" min="0" value={draft.context ?? ""} oninput={(e) => (draft!.context = num(e.currentTarget.value))} placeholder="the model's default" />
              <p class="hint">Ollama starts many models with a small context. 32768 or more is safer.</p>
            </div>
          {/if}
          <div class="pair">
            <div class="field">
              <label for="svc-strong">Planning model</label>
              <input id="svc-strong" bind:value={draft.strong_model} list={`models-${draft.id}`} autocomplete="off" spellcheck="false" />
            </div>
            <div class="field">
              <label for="svc-quick">Quick-check model</label>
              <input id="svc-quick" bind:value={draft.quick_model} list={`models-${draft.id}`} autocomplete="off" spellcheck="false" />
            </div>
          </div>
          <p class="hint">Used when a job above doesn't name a model. Empty uses the usual model.</p>
        </details>

        <div class="row actions">
          <button onclick={check} disabled={!!busy}>{busy === "check" ? "Checking…" : "Check"}</button>
          <button class="ghost" onclick={() => save()} disabled={!!busy || !dirty}>{busy === "save" ? "Saving…" : "Save"}</button>
          <button class="ghost" onclick={close} disabled={!!busy}>{dirty ? "Cancel" : "Close"}</button>
          {#if saved && !dirty}<span class="small good">Saved.</span>{/if}
        </div>
        <p class="small muted">Check saves, then asks the model one small question (and shows it a tiny picture, if it can see images). It uses a few tokens.</p>
        {#if error}<p class="note bad">{error}</p>{/if}
        {#if saving?.check}
          <p class="small {saving.check.json_ok ? 'good' : 'bad'}">{saving.check.message} <span class="muted">Checked {when(saving.check.at)}</span></p>
        {/if}

        <h3>If this service did everything</h3>
        {#if saving}
          <FitReport fit={saving.fit} compact />
        {:else}
          <p class="small muted">Save to see the report.</p>
        {/if}
      </div>
    {/if}

    <p class="small muted">
      Your course requests and work go to the services you choose, under their own terms and privacy policies. Keys are
      kept only on this computer, in the app's own folder, and left out of backups.
    </p>
  {:else if error}
    <p class="bad">{error}</p>
  {:else}
    <p class="muted">Loading…</p>
  {/if}
</section>

<style>
  h3 {
    margin: 1.25rem 0 0.5rem;
    font-size: 1rem;
  }
  .jobs {
    width: 100%;
    border-collapse: collapse;
  }
  .jobs th {
    text-align: left;
    font-weight: 600;
    padding-bottom: 0.25rem;
  }
  .jobs td {
    padding: 0.35rem 0.5rem 0.35rem 0;
    vertical-align: top;
  }
  .jobs td:first-child {
    width: 34%;
    min-width: 10rem;
  }
  .jobs td:first-child .hint {
    font-size: 0.82rem;
  }
  .jobs select,
  .jobs input {
    width: 100%;
  }
  .block {
    display: block;
  }
  .svc {
    display: flex;
    gap: 0.75rem;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid var(--line);
    padding: 0.5rem 0;
  }
  .svc .grow {
    display: flex;
    gap: 0.6rem;
    flex-wrap: wrap;
    align-items: baseline;
  }
  .editor {
    margin-top: 1rem;
    padding: 1rem;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm, 6px);
  }
  .editor h3 {
    margin-top: 0;
  }
  .pair {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(12rem, 1fr));
    gap: 0 1rem;
  }
  fieldset {
    border: 1px solid var(--line);
    border-radius: var(--radius-sm, 6px);
    margin: 0 0 1rem;
    padding: 0.75rem 1rem 0;
  }
  .more {
    margin: 0.75rem 0;
  }
  .more summary {
    cursor: pointer;
    margin-bottom: 0.75rem;
  }
  .mono {
    font-family: var(--mono, ui-monospace, monospace);
    font-size: 0.9rem;
  }
  .actions {
    margin-top: 0.75rem;
  }
  .hint {
    margin: 0.25rem 0 0;
    font-weight: 400;
    color: var(--muted);
    font-size: 0.86rem;
  }
  p.hint {
    margin-bottom: 0.75rem;
  }
</style>
