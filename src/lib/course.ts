// Shared bits for showing a course the way a school course page would: banner colours, course codes, who teaches
// it, what each item's status means, and what to do next.

import type { CourseMeta, ItemRow } from "$lib/api";
import { relative, when } from "$lib/format";

/** Banner colours. Each has a deep tone for the banner and a light tint for chips; both work in light and dark. */
export const PALETTE: { name: string; label: string; deep: string; tint: string }[] = [
  { name: "indigo", label: "Indigo", deep: "#3b4cb8", tint: "#e4e8fb" },
  { name: "teal", label: "Teal", deep: "#13756f", tint: "#dcf2ef" },
  { name: "crimson", label: "Crimson", deep: "#a3263d", tint: "#f8e1e5" },
  { name: "amber", label: "Amber", deep: "#9a5a00", tint: "#faecd6" },
  { name: "forest", label: "Forest", deep: "#2e6b34", tint: "#e0f0e1" },
  { name: "plum", label: "Plum", deep: "#73367f", tint: "#f1e3f4" },
  { name: "slate", label: "Slate", deep: "#3f4f63", tint: "#e3e8ee" },
  { name: "ocean", label: "Ocean", deep: "#1d5f99", tint: "#dde9f5" },
];

function hash(s: string) {
  let h = 0;
  for (const c of s) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return h;
}

/** The course's banner colour: the one chosen in the builder, or a steady pick from its id. */
export function colorOf(meta: CourseMeta | null | undefined, id: string) {
  return PALETTE.find((p) => p.name === meta?.color) ?? PALETTE[hash(id) % PALETTE.length];
}

/** "CHESS 101": the chosen code, or one made from the first word of the title. */
export function codeOf(meta: CourseMeta | null | undefined, title: string) {
  if (meta?.code?.trim()) return meta.code.trim();
  const word = (title.match(/[A-Za-z]+/)?.[0] ?? "COURSE").slice(0, 5).toUpperCase();
  return `${word} 101`;
}

export function instructorOf(meta: CourseMeta | null | undefined, origin: string) {
  if (meta?.instructor?.trim()) return meta.instructor.trim();
  return origin === "manual" ? "You" : "Claude";
}

/** CSS variables for a course's colours, for style="...". */
export function colorVars(meta: CourseMeta | null | undefined, id: string) {
  const c = colorOf(meta, id);
  return `--course: ${c.deep}; --course-tint: ${c.tint};`;
}

export type Tone = "done" | "due" | "late" | "locked" | "wait" | "open" | "bad";

/** What an item's state means right now, in a few words, and how to colour it. */
export function statusOf(it: ItemRow): { label: string; tone: Tone } {
  switch (it.state) {
    case "locked":
      return { label: `Opens ${when(it.release_at)}`, tone: "locked" };
    case "done":
      return { label: "Read", tone: "done" };
    case "submitted":
      return { label: "Submitted · being marked", tone: "wait" };
    case "marked":
      return { label: it.percent != null ? `${Math.round(it.percent)}%` : "Marked", tone: "done" };
    case "missed":
      return { label: "Missed", tone: "bad" };
  }
  if (it.kind === "lesson") return { label: "To read", tone: "open" };
  if (!it.due_at) return { label: "Open", tone: "open" };
  const ms = new Date(it.due_at).getTime() - Date.now();
  if (ms < 0) return { label: `Overdue · was due ${relative(it.due_at)}`, tone: "late" };
  if (ms < 48 * 36e5) return { label: `Due ${relative(it.due_at)}`, tone: "due" };
  return { label: `Due ${when(it.due_at)}`, tone: "open" };
}

/** Settled = nothing left to do (lessons read, work marked or missed). */
export function settled(it: ItemRow) {
  return it.state === "done" || it.state === "marked" || it.state === "missed";
}

/** What to do next: overdue work first, then the soonest deadline, then the first unread lesson. */
export function nextUp(items: ItemRow[]): ItemRow | null {
  const open = items.filter((i) => i.state === "open");
  const work = open.filter((i) => i.kind !== "lesson" && i.due_at).sort((a, b) => a.due_at!.localeCompare(b.due_at!));
  const lesson = open.find((i) => i.kind === "lesson");
  // a lesson released alongside the work usually comes first
  if (lesson && work[0] && lesson.release_at <= work[0].release_at) return lesson;
  return work[0] ?? lesson ?? null;
}

/** Open work with a deadline, soonest first, grouped by day. */
export function agenda(items: ItemRow[], days = 21) {
  const until = Date.now() + days * 864e5;
  const due = items
    .filter((i) => i.due_at && (i.state === "open" || i.state === "locked") && new Date(i.due_at).getTime() <= until)
    .sort((a, b) => a.due_at!.localeCompare(b.due_at!));
  const groups: { day: string; items: ItemRow[] }[] = [];
  for (const it of due) {
    const d = new Date(it.due_at!).toLocaleDateString(undefined, { weekday: "long", month: "short", day: "numeric" });
    const g = groups.find((g) => g.day === d);
    if (g) g.items.push(it);
    else groups.push({ day: d, items: [it] });
  }
  return groups;
}

export const markedBy: Record<string, string> = {
  claude: "Written work is marked by Claude against each rubric; multiple-choice and numeric questions are marked by the app straight away.",
  manual: "Multiple-choice and numeric questions are marked by the app. You mark written work yourself, line by line, against your own answer key.",
};
