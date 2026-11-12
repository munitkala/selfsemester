// Interactive figures in lessons. A ```widget block holds a small HTML page with its own script:
//
//   ```widget
//   caption: Change the angle and watch the forces on the block
//   height: 320
//   tools: geometry          (optional: installed tools to load first, see src-tauri/src/tools.rs)
//   <input id="a" type="range" min="0" max="60"> <svg id="s" viewBox="0 0 400 240"></svg>
//   <script> ... </script>
//   ```
//
// figures.ts numbers it like any other figure and leaves an empty slot; the `widgets` action (on the app's main
// area) fills each slot with a sandboxed frame served by the app (src-tauri/src/widget.rs). The frame can't reach
// the network, the app or your files, and it's given the theme's colours as CSS variables so it matches the page.

import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import type { Tool } from "#lib/api.ts";

const html = new Map<string, string>();

/** Theme colours passed to every widget as CSS variables (the same list widget.rs accepts). */
const THEME_VARS = [
  "panel", "raised", "text", "muted", "line", "accent", "accent-soft", "on-accent", "good", "warn", "bad",
  "chart-1", "chart-2", "chart-3", "chart-4", "chart-5", "chart-6",
];

function key(s: string): string {
  let h1 = 0xdeadbeef, h2 = 0x41c6ce57;
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    h1 = Math.imul(h1 ^ c, 2654435761);
    h2 = Math.imul(h2 ^ c, 1597334677);
  }
  h1 = Math.imul(h1 ^ (h1 >>> 16), 2246822507) ^ Math.imul(h2 ^ (h2 >>> 13), 3266489909);
  h2 = Math.imul(h2 ^ (h2 >>> 16), 2246822507) ^ Math.imul(h1 ^ (h1 >>> 13), 3266489909);
  return (h2 >>> 0).toString(16) + (h1 >>> 0).toString(16);
}

/** The slot a widget block becomes; the action below turns it into a frame. */
export function widgetSlot(body: string, height: number, title: string, tools: string[] = []): string {
  if (!body.trim()) throw new Error("the block is empty");
  const k = key(body);
  html.set(k, body);
  const h = Math.min(Math.max(Math.round(height) || 320, 80), 1200);
  const t = title.replace(/[<>"&]/g, "");
  return `<div class="widget-slot" data-widget="${k}" data-title="${t}" data-tools="${tools.join(",")}" style="height:${h}px"><span class="muted small">Loading the interactive figure…</span></div>`;
}

let paint: CanvasRenderingContext2D | null = null;
/** A colour as #rrggbb (or rgba), which every library understands, rather than oklch() or color-mix(). */
function plain(c: string): string {
  paint ??= Object.assign(document.createElement("canvas"), { width: 1, height: 1 }).getContext("2d", { willReadFrequently: true });
  if (!paint) return c;
  paint.clearRect(0, 0, 1, 1);
  paint.fillStyle = c;
  paint.fillRect(0, 0, 1, 1);
  const [r, g, b, a] = paint.getImageData(0, 0, 1, 1).data;
  const hex = (n: number) => n.toString(16).padStart(2, "0");
  return a === 255 ? `#${hex(r)}${hex(g)}${hex(b)}` : `rgba(${r}, ${g}, ${b}, ${(a / 255).toFixed(2)})`;
}

/** The theme's current colours, resolved to plain values a widget can use. */
function themeColours(): Record<string, string> {
  const probe = document.createElement("span");
  probe.style.display = "none";
  document.body.appendChild(probe);
  const out: Record<string, string> = {};
  for (const v of THEME_VARS) {
    probe.style.color = "";
    probe.style.color = `var(--${v})`;
    const c = getComputedStyle(probe).color;
    if (c) out[v] = plain(c);
  }
  probe.remove();
  return out;
}

const frames = new Set<HTMLIFrameElement>();

async function mount(slot: HTMLElement) {
  slot.dataset.mounted = "1";
  const body = html.get(slot.dataset.widget ?? "");
  try {
    if (!body) throw new Error("its contents went missing; reopen the page");
    const tools = (slot.dataset.tools ?? "").split(",").filter(Boolean);
    if (tools.length) {
      const found = await invoke<Tool[]>("tools_lookup", { ids: tools });
      const missing = tools.filter((id) => !found.some((t) => t.id === id && t.installed));
      if (missing.length) {
        const names = missing.map((id) => found.find((t) => t.id === id)?.name ?? id);
        const common = missing.every((id) => found.find((t) => t.id === id)?.category === "common");
        slot.style.height = "";
        slot.innerHTML = `<p class="figure-error">This figure uses ${names.join(" and ").replace(/[<>&]/g, "")}, which ${missing.length > 1 ? "aren't" : "isn't"} installed.${common ? " You can install it in Settings, Tools." : ""}</p>`;
        return;
      }
    }
    const id = await invoke<string>("widget_put", { html: body, tools });
    const frame = document.createElement("iframe");
    // Scripts only: no same-origin (so no storage or access to this window), no popups, forms or navigation of the app.
    frame.setAttribute("sandbox", "allow-scripts");
    frame.setAttribute("referrerpolicy", "no-referrer");
    frame.setAttribute("allow", "");
    frame.title = slot.dataset.title || "Interactive figure";
    frame.src = `${convertFileSrc("", "widget")}w/${id}?${new URLSearchParams(themeColours())}`;
    slot.replaceChildren(frame);
    frames.add(frame);
  } catch (e) {
    slot.style.height = "";
    slot.innerHTML = `<p class="figure-error">This interactive figure couldn't be shown (${String((e as Error)?.message ?? e).replace(/[<>&]/g, "")}).</p>`;
  }
}

/** Svelte action for the app's main area: fills widget slots as pages show them, sizes frames and keeps their theme in step. */
export function widgets(node: HTMLElement) {
  const scan = () => {
    for (const f of frames) if (!f.isConnected) frames.delete(f);
    node.querySelectorAll<HTMLElement>(".widget-slot:not([data-mounted])").forEach(mount);
  };
  const onMessage = (e: MessageEvent) => {
    const d = e.data;
    if (!d || d.ssWidget !== "height" || typeof d.h !== "number") return;
    for (const f of frames) {
      if (f.contentWindow === e.source) {
        const slot = f.parentElement;
        if (slot) slot.style.height = `${Math.min(Math.max(Math.ceil(d.h), 60), 1600)}px`;
      }
    }
  };
  const retheme = () => {
    if (!frames.size) return;
    const vars = themeColours();
    for (const f of frames) f.contentWindow?.postMessage({ ssWidget: "theme", vars }, "*");
  };
  const content = new MutationObserver(scan);
  content.observe(node, { childList: true, subtree: true });
  const theme = new MutationObserver(retheme);
  theme.observe(document.documentElement, { attributes: true });
  addEventListener("message", onMessage);
  scan();
  return {
    destroy() {
      content.disconnect();
      theme.disconnect();
      removeEventListener("message", onMessage);
    },
  };
}
