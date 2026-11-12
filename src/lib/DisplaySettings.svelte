<script lang="ts">
  import { api } from "#lib/api.ts";
  import { accents, darkThemes, lightThemes, prefs, reset, save, type Prefs } from "#lib/prefs.svelte.ts";
  import Icon from "#lib/Icon.svelte";

  let confirmReset = $state(false);
  let confirmClear = $state(false);
  let cleared = $state<string | null>(null);

  function set<K extends keyof Prefs>(key: K, value: Prefs[K]) {
    prefs[key] = value;
    save();
  }

  async function clearStudy() {
    confirmClear = false;
    try {
      await api.clearStudyTime();
      cleared = "Study history cleared.";
    } catch (e) {
      cleared = String(e);
    }
  }

  const effectLevels: { id: Prefs["effects"]; name: string; note: string }[] = [
    { id: "full", name: "Full", note: "Blur behind dialogs and a slowly moving glow on Today. Uses the most graphics power." },
    { id: "balanced", name: "Balanced", note: "Soft shadows, gradients and small animations, but no blur. Recommended." },
    { id: "minimal", name: "Minimal", note: "Flat and still: no shadows, gradients or animation. Lightest on older or battery-powered computers." },
  ];
</script>

{#snippet seg(key: keyof Prefs, options: { value: string | number; label: string }[], label: string)}
  <div class="segmented" role="group" aria-label={label}>
    {#each options as o}
      <button aria-pressed={prefs[key] === o.value} onclick={() => set(key, o.value as never)}>{o.label}</button>
    {/each}
  </div>
{/snippet}

{#snippet toggle(key: keyof Prefs, label: string, hint: string)}
  <label class="switch">
    <input type="checkbox" checked={!!prefs[key]} onchange={(e) => set(key, e.currentTarget.checked as never)} />
    <span class="track" aria-hidden="true"></span>
    <span class="text">{label}<span class="hint">{hint}</span></span>
  </label>
{/snippet}

<section class="card" id="appearance">
  <h2><Icon name="sun" size={18} /> Appearance</h2>

  <div class="setting">
    <div class="name">Mode<span class="hint">System follows Windows' light or dark setting.</span></div>
    {@render seg("mode", [{ value: "system", label: "System" }, { value: "light", label: "Light" }, { value: "dark", label: "Dark" }], "Mode")}
  </div>

  <div class="setting block">
    <div class="name">Light theme</div>
    <div class="themes" role="radiogroup" aria-label="Light theme">
      {#each lightThemes as t}
        <button class="theme" role="radio" aria-checked={prefs.lightTheme === t.id} onclick={() => set("lightTheme", t.id)}>
          <span class="preview" style="--a: {t.swatch[0]}; --b: {t.swatch[1]}; --c: {t.swatch[2]}" aria-hidden="true"><i></i><i></i><i></i></span>
          <strong>{t.name}</strong>
          <span class="small muted">{t.note}</span>
        </button>
      {/each}
    </div>
  </div>

  <div class="setting block">
    <div class="name">Dark theme</div>
    <div class="themes" role="radiogroup" aria-label="Dark theme">
      {#each darkThemes as t}
        <button class="theme" role="radio" aria-checked={prefs.darkTheme === t.id} onclick={() => set("darkTheme", t.id)}>
          <span class="preview" style="--a: {t.swatch[0]}; --b: {t.swatch[1]}; --c: {t.swatch[2]}" aria-hidden="true"><i></i><i></i><i></i></span>
          <strong>{t.name}</strong>
          <span class="small muted">{t.note}</span>
        </button>
      {/each}
    </div>
  </div>

  <div class="setting">
    <div class="name">Accent colour<span class="hint">Buttons, links, charts and highlights.</span></div>
    <div class="accents" role="radiogroup" aria-label="Accent colour">
      {#each accents as a}
        <button
          class="accent"
          class:theme-default={a.hue == null}
          role="radio"
          aria-checked={prefs.accent === a.id}
          aria-label={a.name}
          title={a.name}
          style={a.hue == null ? "" : `--swatch: oklch(0.6 0.15 ${a.hue})`}
          onclick={() => set("accent", a.id)}
        ></button>
      {/each}
    </div>
  </div>

  <div class="setting">
    <div class="name">Text size<span class="hint">{prefs.textScale}%</span></div>
    {@render seg("textScale", [{ value: 90, label: "Small" }, { value: 100, label: "Default" }, { value: 112, label: "Large" }, { value: 125, label: "Larger" }], "Text size")}
  </div>

  <div class="setting">
    <div class="name">Density<span class="hint">Space between and inside cards.</span></div>
    {@render seg("density", [{ value: "compact", label: "Compact" }, { value: "comfortable", label: "Comfortable" }, { value: "spacious", label: "Spacious" }], "Density")}
  </div>

  <div class="setting">
    <div class="name">Corners</div>
    {@render seg("corners", [{ value: "sharp", label: "Sharp" }, { value: "rounded", label: "Rounded" }, { value: "round", label: "Round" }], "Corners")}
  </div>
</section>

<section class="card" id="layout">
  <h2><Icon name="courses" size={18} /> Layout</h2>
  <div class="setting">
    <div class="name">Sidebar<span class="hint">Compact shows icons only. It also switches to icons by itself in a small window.</span></div>
    {@render seg("sidebar", [{ value: "full", label: "Full" }, { value: "compact", label: "Compact" }], "Sidebar")}
  </div>
  <div class="setting">
    <div class="name">Page width<span class="hint">Auto fills the window and rearranges panels as you resize it.</span></div>
    {@render seg("width", [{ value: "auto", label: "Auto" }, { value: "normal", label: "Narrow" }, { value: "wide", label: "Wide" }, { value: "full", label: "Full" }], "Page width")}
  </div>
  <div class="setting">
    <div class="name">Open on<span class="hint">The page SelfSemester starts on.</span></div>
    {@render seg("startPage", [{ value: "/", label: "Today" }, { value: "/courses", label: "Courses" }, { value: "/insights", label: "Insights" }], "Start page")}
  </div>
</section>

<section class="card" id="performance">
  <h2><Icon name="sparkle" size={18} /> Motion and performance</h2>
  <div class="setting block">
    <div class="name">Visual effects</div>
    <div class="levels" role="radiogroup" aria-label="Visual effects">
      {#each effectLevels as l}
        <button class="level" role="radio" aria-checked={prefs.effects === l.id} onclick={() => set("effects", l.id)}>
          <strong>{l.name}</strong>
          <span class="small muted">{l.note}</span>
        </button>
      {/each}
    </div>
  </div>
  <div class="setting">
    <div class="name">Animation<span class="hint">System follows Windows' "Animation effects" setting. Minimal effects also turns animation off.</span></div>
    {@render seg("motion", [{ value: "system", label: "System" }, { value: "on", label: "On" }, { value: "off", label: "Off" }], "Animation")}
  </div>
  {@render toggle("celebrate", "Celebrate goals", "A small flourish on Today when you reach your daily goal.")}
</section>

<section class="card" id="study">
  <h2><Icon name="clock" size={18} /> Study time and goals</h2>
  {@render toggle("trackStudy", "Count study time", "Counts time on lesson and work pages while the window is in front and you're using it. Stays on this computer.")}
  <div class="setting">
    <div class="name">Daily goal<span class="hint">{prefs.dailyGoal ? `${prefs.dailyGoal} minutes` : "Off"}</span></div>
    {@render seg("dailyGoal", [{ value: 0, label: "Off" }, { value: 15, label: "15 min" }, { value: 30, label: "30 min" }, { value: 60, label: "1 h" }, { value: 120, label: "2 h" }], "Daily goal")}
  </div>
  <div class="setting">
    <div class="name">Week starts on</div>
    {@render seg("weekStart", [{ value: 1, label: "Monday" }, { value: 0, label: "Sunday" }], "Week starts on")}
  </div>
  <div class="setting">
    <div class="name">Activity calendar</div>
    {@render seg("heatmapWeeks", [{ value: 26, label: "6 months" }, { value: 52, label: "12 months" }], "Activity calendar")}
  </div>
  <div class="setting">
    <div class="name">Study history<span class="hint">Removes counted time only. Marks and work stay.</span></div>
    {#if confirmClear}
      <div class="row">
        <button class="danger" onclick={clearStudy}>Clear study history</button>
        <button class="ghost" onclick={() => (confirmClear = false)}>Keep it</button>
      </div>
    {:else}
      <button class="ghost" onclick={() => ((confirmClear = true), (cleared = null))}>Clear…</button>
    {/if}
  </div>
  {#if cleared}<p class="small muted">{cleared}</p>{/if}
</section>

<section class="card" id="shortcuts">
  <h2><Icon name="keyboard" size={18} /> Shortcuts</h2>
  {@render toggle("shortcuts", "Ctrl+K to go anywhere", "Opens a search box to jump to a page or course, or switch light and dark.")}
  <div class="setting">
    <div class="name">Display settings<span class="hint">Puts every option on this page back to how it started.</span></div>
    {#if confirmReset}
      <div class="row">
        <button class="danger" onclick={() => (reset(), (confirmReset = false))}>Reset</button>
        <button class="ghost" onclick={() => (confirmReset = false)}>Cancel</button>
      </div>
    {:else}
      <button class="ghost" onclick={() => (confirmReset = true)}>Reset to defaults…</button>
    {/if}
  </div>
</section>

<style>
  section {
    scroll-margin-top: 1rem;
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 0.45rem;
  }
  h2 :global(.icon) {
    color: var(--accent);
  }
  .setting {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.75rem 1.5rem;
    flex-wrap: wrap;
    padding: 0.7rem 0;
    border-top: 1px solid var(--line);
  }
  .setting.block {
    display: block;
  }
  .setting.block .name {
    margin-bottom: 0.6rem;
  }
  .name {
    font-weight: 600;
  }
  .hint {
    display: block;
    font-weight: 400;
    font-size: 0.84rem;
    color: var(--muted);
  }
  .themes {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(10.5rem, 1fr));
    gap: 0.6rem;
  }
  .theme,
  .level {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    text-align: left;
    background: var(--panel);
    color: var(--text);
    border: 1px solid var(--line);
    padding: 0.6rem;
    border-radius: var(--radius-sm);
  }
  .theme:hover,
  .level:hover {
    border-color: var(--accent-line);
    filter: none;
  }
  .theme[aria-checked="true"],
  .level[aria-checked="true"] {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
    background: var(--accent-soft);
  }
  .preview {
    display: grid;
    grid-template-columns: 2fr 3fr;
    grid-template-rows: 1fr 0.6fr;
    height: 3rem;
    border-radius: 6px;
    overflow: hidden;
    border: 1px solid rgb(127 127 127 / 0.3);
    margin-bottom: 0.3rem;
  }
  .preview i:nth-child(1) {
    background: var(--a);
    grid-row: span 2;
  }
  .preview i:nth-child(2) {
    background: var(--b);
  }
  .preview i:nth-child(3) {
    background: var(--c);
  }
  .levels {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(12rem, 1fr));
    gap: 0.6rem;
  }
  .accents {
    display: flex;
    gap: 0.45rem;
    flex-wrap: wrap;
  }
  .accent {
    width: 1.9rem;
    height: 1.9rem;
    padding: 0;
    border-radius: 50%;
    border: 2px solid var(--panel);
    background: var(--swatch);
    box-shadow: 0 0 0 1px var(--line);
  }
  .accent.theme-default {
    background: conic-gradient(oklch(0.6 0.15 262), oklch(0.6 0.15 150), oklch(0.6 0.15 70), oklch(0.6 0.15 355), oklch(0.6 0.15 262));
  }
  .accent[aria-checked="true"] {
    box-shadow: 0 0 0 2px var(--text);
  }
  .switch {
    display: flex;
    align-items: flex-start;
    gap: 0.75rem;
    padding: 0.7rem 0;
    border-top: 1px solid var(--line);
    margin: 0;
    cursor: pointer;
  }
  .switch input {
    position: absolute;
    opacity: 0;
    width: 1px;
    height: 1px;
  }
  .track {
    flex: none;
    position: relative;
    width: 2.3rem;
    height: 1.3rem;
    margin-top: 0.1rem;
    border-radius: 999px;
    background: var(--line);
    transition: background-color var(--fast);
  }
  .track::after {
    content: "";
    position: absolute;
    top: 0.15rem;
    left: 0.15rem;
    width: 1rem;
    height: 1rem;
    border-radius: 50%;
    background: var(--panel);
    box-shadow: 0 1px 2px rgb(0 0 0 / 0.3);
    transition: transform var(--fast) var(--ease);
  }
  .switch input:checked + .track {
    background: var(--accent);
  }
  .switch input:checked + .track::after {
    transform: translateX(1rem);
  }
  .switch input:focus-visible + .track {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .text {
    font-weight: 600;
  }
</style>
