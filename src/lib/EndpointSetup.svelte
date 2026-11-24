<script lang="ts">
  // Settings → Another AI service. For people who'd rather not use Claude Code: any service with an
  // OpenAI-style chat API, including models running on their own computer. Claude Code stays the default; this
  // only takes over once the learner presses "Build with this service".
  import { onMount } from "svelte";
  import { api, type AiView, type Endpoint } from "#lib/api.ts";
  import { when } from "#lib/format.ts";
  import FitReport from "#lib/FitReport.svelte";

  let { onchange }: { onchange?: (v: AiView) => void } = $props();

  type Preset = { id: string; name: string; url: string; key: boolean; example: string; images: boolean; hint: string };
  // Model names change often, so these are only examples. "Load the list" asks the service for its real ones.
  const presets: Preset[] = [
    { id: "openai", name: "OpenAI", url: "https://api.openai.com/v1", key: true, example: "gpt-5", images: true, hint: "Make a key at platform.openai.com, under API keys." },
    { id: "openrouter", name: "OpenRouter", url: "https://openrouter.ai/api/v1", key: true, example: "openai/gpt-5", images: false, hint: "One key for models from many companies. Make one at openrouter.ai/keys." },
    { id: "gemini", name: "Google Gemini", url: "https://generativelanguage.googleapis.com/v1beta/openai", key: true, example: "gemini-2.5-pro", images: true, hint: "Make a key in Google AI Studio." },
    { id: "anthropic", name: "Anthropic API (Claude, billed per use)", url: "https://api.anthropic.com/v1", key: true, example: "claude-sonnet-4-5", images: true, hint: "Billed per use, separately from a Claude subscription. To use your subscription, stay with Claude Code above." },
    { id: "mistral", name: "Mistral", url: "https://api.mistral.ai/v1", key: true, example: "mistral-large-latest", images: false, hint: "Make a key at console.mistral.ai." },
    { id: "deepseek", name: "DeepSeek", url: "https://api.deepseek.com/v1", key: true, example: "deepseek-chat", images: false, hint: "Make a key at platform.deepseek.com." },
    { id: "groq", name: "Groq", url: "https://api.groq.com/openai/v1", key: true, example: "llama-3.3-70b-versatile", images: false, hint: "Fast, with a free tier that has tight rate limits." },
    { id: "ollama", name: "Ollama (on this computer)", url: "http://localhost:11434/v1", key: false, example: "qwen3:32b", images: false, hint: "Free and private. Start Ollama and pull a model first. Bigger models make better courses." },
    { id: "lmstudio", name: "LM Studio (on this computer)", url: "http://localhost:1234/v1", key: false, example: "qwen/qwen3-30b", images: false, hint: "Free and private. In LM Studio, open Developer and start the server." },
    { id: "custom", name: "Something else", url: "", key: true, example: "model-name", images: false, hint: "Anything with an OpenAI-compatible /chat/completions endpoint. Leave the key empty if it doesn't need one." },
  ];

  let view = $state<AiView | null>(null);
  let draft = $state<Endpoint>({ preset: "", base_url: "", api_key: "", model: "", strong_model: "", quick_model: "", sees_images: false, check: null });
  /** what's typed in the key box; empty means "keep the saved key" */
  let keyInput = $state("");
  let models = $state<string[]>([]);
  let modelsMsg = $state<string | null>(null);
  let busy = $state<"" | "save" | "check" | "models" | "switch">("");
  let error = $state<string | null>(null);
  let saved = $state(false);

  const preset = $derived(presets.find((p) => p.id === draft.preset) ?? presets[presets.length - 1]);
  const inUse = $derived(view?.provider === "endpoint");
  const insecure = $derived(draft.base_url.startsWith("http://") && !/^http:\/\/(localhost|127\.|\[::1\]|10\.|192\.168\.|100\.(6[4-9]|[7-9]\d|1[01]\d|12[0-7])\.)/.test(draft.base_url));
  const dirty = $derived(
    !!view &&
      (keyInput !== "" ||
        (["preset", "base_url", "model", "strong_model", "quick_model", "sees_images"] as const).some((k) => draft[k] !== view!.endpoint[k])),
  );

  function take(v: AiView) {
    view = v;
    draft = { ...v.endpoint };
    if (!draft.preset) draft.preset = "openai";
    if (!draft.base_url) draft.base_url = presets.find((p) => p.id === draft.preset)?.url ?? "";
    keyInput = "";
    onchange?.(v);
  }

  function pickPreset(id: string) {
    const p = presets.find((x) => x.id === id);
    if (!p) return;
    const wasPresetUrl = presets.some((x) => x.url && x.url === draft.base_url) || !draft.base_url;
    draft.preset = id;
    if (wasPresetUrl) draft.base_url = p.url;
    draft.sees_images = p.images;
    models = [];
    modelsMsg = null;
  }

  async function save(): Promise<boolean> {
    busy = "save";
    error = null;
    saved = false;
    try {
      take(await api.saveEndpoint($state.snapshot(draft), keyInput || undefined));
      saved = true;
      return true;
    } catch (e) {
      error = String(e);
      return false;
    } finally {
      busy = "";
    }
  }

  async function check() {
    if (dirty && !(await save())) return;
    busy = "check";
    error = null;
    try {
      const [, v] = await api.checkEndpoint();
      take(v);
    } catch (e) {
      error = String(e);
    } finally {
      busy = "";
    }
  }

  async function loadModels() {
    busy = "models";
    modelsMsg = null;
    try {
      models = await api.endpointModels($state.snapshot(draft), keyInput || undefined);
      modelsMsg = models.length ? `${models.length} models found. Start typing in a model box to pick one.` : "The service didn't list any models. Type the name instead.";
    } catch (e) {
      modelsMsg = `Couldn't load the list: ${e}. You can still type a model name.`;
    } finally {
      busy = "";
    }
  }

  async function switchTo(provider: "claude_code" | "endpoint") {
    if (provider === "endpoint" && dirty && !(await save())) return;
    busy = "switch";
    error = null;
    try {
      take(await api.useProvider(provider));
    } catch (e) {
      error = String(e);
    } finally {
      busy = "";
    }
  }

  async function forgetKey() {
    busy = "save";
    try {
      take(await api.saveEndpoint($state.snapshot(draft), ""));
    } catch (e) {
      error = String(e);
    } finally {
      busy = "";
    }
  }

  onMount(() => {
    api.ai().then(take).catch((e) => (error = String(e)));
  });
</script>

<section class="card" id="ai-service" aria-live="polite">
  <div class="spread">
    <h2>Another AI service</h2>
    {#if inUse}<span class="chip good-chip">Building courses with this</span>{/if}
  </div>
  <p class="small muted">
    If you'd rather not use Claude Code, SelfSemester can build and mark courses with another AI service instead:
    anything with an OpenAI-style API, including a model running on your own computer. Some things only Claude Code
    can do, like searching the web for the reading list, so check the report below before you switch.
  </p>

  {#if view}
    <div class="field">
      <label for="ep-preset">Service</label>
      <select id="ep-preset" value={draft.preset} onchange={(e) => pickPreset(e.currentTarget.value)}>
        {#each presets as p}<option value={p.id}>{p.name}</option>{/each}
      </select>
      <p class="hint">{preset.hint}</p>
    </div>

    <div class="field">
      <label for="ep-url">Address <span class="hint">the API's base address, up to and including /v1 (or what the service says)</span></label>
      <input id="ep-url" bind:value={draft.base_url} placeholder="https://api.example.com/v1" autocomplete="off" spellcheck="false" />
      {#if insecure}<p class="small warn">This address isn't encrypted (http, not https) and isn't on your own computer or network, so your key and course work would travel in the open.</p>{/if}
    </div>

    <div class="field">
      <label for="ep-key">API key {#if !preset.key}<span class="hint">usually not needed for this one</span>{/if}</label>
      <div class="row">
        <input id="ep-key" type="password" bind:value={keyInput} autocomplete="off" spellcheck="false" placeholder={view.key_set ? "Saved. Type a new key to replace it." : "Paste the key here"} />
        {#if view.key_set}<button type="button" class="ghost small" onclick={forgetKey} disabled={!!busy}>Remove saved key</button>{/if}
      </div>
      <p class="hint">Kept only on this computer, in the app's own folder, and left out of backups. It's sent only to the address above.</p>
    </div>

    <div class="field">
      <div class="spread">
        <label for="ep-model">Model</label>
        <button type="button" class="ghost small" onclick={loadModels} disabled={!!busy || !draft.base_url}>{busy === "models" ? "Loading…" : "Load the list"}</button>
      </div>
      <input id="ep-model" bind:value={draft.model} list="ep-models" placeholder={`e.g. ${preset.example}`} autocomplete="off" spellcheck="false" />
      <p class="hint">Writes the lessons and the reading list, and does most of the marking. The bigger the model, the better the course.</p>
      {#if modelsMsg}<p class="small muted">{modelsMsg}</p>{/if}
      <datalist id="ep-models">{#each models as m}<option value={m}></option>{/each}</datalist>
    </div>

    <details class="more">
      <summary>Use different models for planning and quick checks</summary>
      <div class="field">
        <label for="ep-strong">Planning and exams <span class="hint">optional, empty uses the model above</span></label>
        <input id="ep-strong" bind:value={draft.strong_model} list="ep-models" autocomplete="off" spellcheck="false" />
      </div>
      <div class="field">
        <label for="ep-quick">Quick checks <span class="hint">optional: a smaller, cheaper model is fine here</span></label>
        <input id="ep-quick" bind:value={draft.quick_model} list="ep-models" autocomplete="off" spellcheck="false" />
      </div>
    </details>

    <label class="radio"><input type="checkbox" bind:checked={draft.sees_images} /> This model can see images <span class="hint">(needed to mark photos of handwritten work)</span></label>

    <div class="row actions">
      <button onclick={check} disabled={!!busy}>{busy === "check" ? "Checking…" : "Check"}</button>
      <button class="ghost" onclick={save} disabled={!!busy || !dirty}>{busy === "save" ? "Saving…" : "Save"}</button>
      {#if saved && !dirty}<span class="small good">Saved.</span>{/if}
    </div>
    <p class="small muted">Check saves, then asks the model one small question (and shows it a tiny picture, if it can see images). It uses a few tokens.</p>
    {#if error}<p class="note bad">{error}</p>{/if}
    {#if view.endpoint.check}
      <p class="small {view.endpoint.check.json_ok ? 'good' : 'bad'}">
        {view.endpoint.check.message}
        <span class="muted">Checked {when(view.endpoint.check.at)}</span>
      </p>
    {/if}

    <h3>How well this can build a course</h3>
    <FitReport fit={view.endpoint_fit} />

    <div class="row actions">
      {#if inUse}
        <button class="ghost" onclick={() => switchTo("claude_code")} disabled={!!busy}>Go back to Claude Code</button>
      {:else}
        <button onclick={() => switchTo("endpoint")} disabled={!!busy || !draft.base_url || !draft.model}>Build with this service</button>
      {/if}
    </div>
    <p class="small muted">
      Switching only changes steps that haven't started. Courses you already have keep everything they were built with.
      Your course requests and work go to this service, under its own terms and privacy policy.
    </p>
  {:else if error}
    <p class="bad">{error}</p>
  {:else}
    <p class="muted">Loading…</p>
  {/if}
</section>

<style>
  h3 {
    margin: 1.25rem 0 0;
    font-size: 1rem;
  }
  .row input {
    flex: 1 1 14rem;
  }
  .more {
    margin: 0 0 1rem;
  }
  .more summary {
    cursor: pointer;
    margin-bottom: 0.75rem;
  }
  .actions {
    margin-top: 0.75rem;
  }
  .good-chip {
    color: var(--good);
    border: 1px solid currentColor;
    border-radius: 999px;
    padding: 0.1rem 0.6rem;
    font-size: 0.85rem;
  }
  .hint {
    margin: 0.25rem 0 0;
  }
</style>
