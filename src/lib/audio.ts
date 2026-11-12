// Sound in lessons. A lesson can hold ```audio blocks (see src-tauri/src/course/audio.rs, which fills each clip with a
// freely licensed recording, a computer voice or synthesised notes):
//
//   ```audio
//   caption: The four tones on ma
//   {"lang": "zh-CN", "clips": [{"say": "妈", "romanization": "mā", "label": "1st tone"}]}
//   ```
//
// Blocks become rows of play buttons. Clip ids are b<block>c<clip>, counted the same way as the Rust side.

import { invoke } from "@tauri-apps/api/core";
import { inline } from "#lib/figures.ts";

export type Clip = {
  say?: string;
  romanization?: string;
  lang?: string;
  label?: string;
  notes?: string[];
  chord?: boolean;
  seconds?: number;
  sound?: string;
  commons?: string;
};

export type Found = {
  status: "ok" | "missing";
  file: string;
  source: "commons" | "voice" | "synth" | "";
  credit: string;
  licence: string;
  url: string;
  note: string;
};

const FENCE = /^```audio[^\n]*\n([\s\S]*?)^```[ \t]*$/gm;
const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

function parse(body: string): { caption: string; test: boolean; clips: Clip[] } {
  const lines = body.split("\n");
  let caption = "";
  while (lines.length && /^(caption|credit)\s*:/i.test(lines[0].trim())) {
    const line = lines.shift()!.trim();
    if (/^caption/i.test(line)) caption = line.slice(line.indexOf(":") + 1).trim();
  }
  try {
    const j = JSON.parse(lines.join("\n").trim());
    const list: Clip[] = Array.isArray(j) ? j : Array.isArray(j?.clips) ? j.clips : [];
    const lang = Array.isArray(j) ? "" : String(j?.lang ?? "");
    return { caption, test: !Array.isArray(j) && j?.test === true, clips: list.filter((c) => c && typeof c === "object").map((c) => ({ ...c, lang: c.lang || lang })) };
  } catch {
    return { caption, test: false, clips: [] };
  }
}

function clipButton(id: string, n: number, c: Clip, f: Found | undefined, test: boolean): string {
  const notes = c.notes?.length ? c.notes.join(c.chord ? " + " : " ") : "";
  // A sound clip's label says what it is better than its search words do.
  const soundName = !c.say && !notes && c.label && !test;
  const main = c.say
    ? `<span class="say" lang="${esc(c.lang ?? "")}">${esc(c.say)}</span>`
    : `<span class="say">${soundName ? inline(c.label!) : (test ? `Sound ${n}` : esc(notes || c.sound || "Sound"))}</span>`;
  // In a listening test the learner has to work out what they hear, so only the words or a number show.
  const rom = c.romanization && !test ? `<span class="rom">${esc(c.romanization)}</span>` : "";
  const label = c.label && !test && !soundName ? `<span class="lab">${inline(c.label)}</span>` : "";
  const ready = f?.status === "ok" && !!f.file;
  const state = ready ? "" : f?.status === "missing" ? "missing" : "pending";
  const tag = ready && f!.source === "voice" ? `<span class="tag">computer voice</span>` : "";
  const title = ready
    ? f!.source === "commons" ? `Recording: ${f!.credit} (${f!.licence})` : f!.credit
    : f?.status === "missing" ? f.note || "No recording available" : "Finding a recording…";
  return `<button type="button" class="clip ${state}" data-clip="${id}"${ready ? ` data-file="${esc(f!.file)}"` : " disabled"} title="${esc(title)}">` +
    `<span class="play" aria-hidden="true"></span><span class="words">${main}${rom}${label}</span>${tag}</button>`;
}

/** Pull audio blocks out of a lesson. Returns the Markdown with placeholders and each block's HTML. */
export function extractAudio(md: string, found: Record<string, Found> = {}): { md: string; blocks: string[] } {
  const blocks: string[] = [];
  const out = md.replace(FENCE, (_m, body: string) => {
    const b = blocks.length;
    const { caption, test, clips } = parse(body);
    const buttons = clips.length
      ? clips.map((c, i) => clipButton(`b${b}c${i}`, i + 1, c, found[`b${b}c${i}`], test)).join("")
      : `<p class="figure-error">These sounds couldn't be read.</p>`;
    const cap = caption ? `<figcaption><strong>Listen.</strong> ${inline(caption)}</figcaption>` : "";
    blocks.push(`<figure class="audio-block"><div class="clips">${buttons}</div>${cap}</figure>`);
    return `\n\n@@AUDIO${b}@@\n\n`;
  });
  return { md: out, blocks };
}

export function insertAudio(html: string, blocks: string[]): string {
  return html.replace(/<p>\s*@@AUDIO(\d+)@@\s*<\/p>|@@AUDIO(\d+)@@/g, (_, a, b) => blocks[Number(a ?? b)] ?? "");
}

/** The recordings a lesson uses, once each, for its credits list. */
export function credits(found: Record<string, Found>): Found[] {
  const seen = new Set<string>();
  return Object.values(found).filter((f) => f.status === "ok" && f.source === "commons" && !seen.has(f.file) && seen.add(f.file));
}

const MIME: Record<string, string> = { ogg: "audio/ogg", oga: "audio/ogg", opus: "audio/ogg", wav: "audio/wav", mp3: "audio/mpeg", flac: "audio/flac", m4a: "audio/mp4", webm: "audio/webm" };
const urls = new Map<string, string>();
let current: { audio: HTMLAudioElement; button: HTMLElement } | null = null;

async function urlFor(courseId: string, file: string): Promise<string> {
  const key = `${courseId}/${file}`;
  let u = urls.get(key);
  if (!u) {
    const bytes = await invoke<ArrayBuffer>("course_audio", { courseId, file });
    const ext = file.split(".").pop()?.toLowerCase() ?? "";
    u = URL.createObjectURL(new Blob([bytes], { type: MIME[ext] ?? "application/octet-stream" }));
    urls.set(key, u);
  }
  return u;
}

function stop() {
  if (!current) return;
  current.audio.pause();
  current.button.classList.remove("playing");
  current = null;
}

/** Svelte action: play clips when their buttons in `node` are pressed. Pressing a playing clip stops it. */
export function audioPlayer(node: HTMLElement, courseId: string) {
  let course = courseId;
  const onClick = async (e: MouseEvent) => {
    const button = (e.target as HTMLElement).closest<HTMLElement>("button.clip[data-file]");
    if (!button || !node.contains(button)) return;
    const again = current?.button === button;
    stop();
    if (again) return;
    try {
      const audio = new Audio(await urlFor(course, button.dataset.file!));
      current = { audio, button };
      button.classList.add("playing");
      audio.onended = () => current?.button === button && stop();
      await audio.play();
    } catch (err) {
      stop();
      button.title = `This sound couldn't play: ${err}`;
      button.classList.add("missing");
    }
  };
  node.addEventListener("click", onClick);
  return {
    update(id: string) {
      course = id;
    },
    destroy() {
      node.removeEventListener("click", onClick);
      stop();
    },
  };
}
