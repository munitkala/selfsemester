export function when(t: string | null | undefined): string {
  if (!t) return "";
  const d = new Date(t);
  return d.toLocaleString(undefined, { weekday: "short", month: "short", day: "numeric", hour: "numeric", minute: "2-digit" });
}

export function day(t: string | null | undefined): string {
  if (!t) return "";
  return new Date(t).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

/** "in 3 days", "in 5 hours", "2 days ago" */
export function relative(t: string | null | undefined): string {
  if (!t) return "";
  const ms = new Date(t).getTime() - Date.now();
  const abs = Math.abs(ms);
  const h = abs / 36e5;
  const text = h < 1 ? `${Math.max(1, Math.round(abs / 6e4))} min` : h < 48 ? `${Math.round(h)} h` : `${Math.round(h / 24)} days`;
  return ms >= 0 ? `in ${text}` : `${text} ago`;
}

export function pct(p: number | null | undefined, digits = 1): string {
  return p == null ? "-" : `${p.toFixed(digits)}%`;
}

export function letter(p: number): string {
  const bands: [number, string][] = [
    [90, "A+"], [85, "A"], [80, "A-"], [77, "B+"], [73, "B"], [70, "B-"], [67, "C+"], [63, "C"], [60, "C-"], [57, "D+"], [53, "D"], [50, "D-"],
  ];
  return bands.find(([m]) => p >= m)?.[1] ?? "F";
}

/** Colour band for a percentage: good from 70, warn from 50, bad below. */
export function band(p: number | null | undefined): "good" | "warn" | "bad" {
  if (p == null) return "warn";
  return p >= 70 ? "good" : p >= 50 ? "warn" : "bad";
}

/** A few plain words to go with a mark. */
export function headline(p: number): string {
  return p >= 85 ? "Excellent work" : p >= 70 ? "Good work" : p >= 50 ? "A pass, with room to improve" : "Worth another look";
}

export const kindLabel: Record<string, string> = {
  lesson: "Lesson",
  homework: "Homework",
  assignment: "Assignment",
  quiz: "Quiz",
  test: "Test",
  exam: "Exam",
  project: "Project",
};

export const stateLabel: Record<string, string> = {
  locked: "Locked",
  open: "Open",
  done: "Read",
  submitted: "Submitted",
  marked: "Marked",
  missed: "Missed",
};

export const statusLabel: Record<string, string> = {
  draft: "Draft in the course builder",
  intake: "Writing questions",
  answering: "Answer questions",
  scoping: "Planning",
  review: "Review plan",
  compiling: "Building course",
  active: "In progress",
  completed: "Completed",
  withdrawn: "Withdrawn",
};

export const stageLabel: Record<string, string> = {
  intake: "writing intake questions",
  plan: "planning the course",
  sources: "finding free sources",
  unit: "writing a unit",
  validate: "checking a unit",
  mark: "marking",
  test: "test call",
};

export function todayIso(offsetDays = 0): string {
  const d = new Date();
  d.setDate(d.getDate() + offsetDays);
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const dd = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}-${m}-${dd}`;
}
