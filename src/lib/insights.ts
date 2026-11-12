// Turns the raw rows from `get_insights` into what the Insights page and Today draw: streaks, a calendar heatmap,
// weekly totals, mark trends and a few plain-language observations. Everything is bucketed by local days.

import type { CourseCard, InsightsData } from "#lib/api.ts";
import { kindLabel } from "#lib/format.ts";
import { colorOf } from "#lib/course.ts";

/** A day counts towards the streak with this much study time, or with any work handed in that day. */
export const ACTIVE_MINUTES = 5;

export function localDay(d: Date): string {
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const dd = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}-${m}-${dd}`;
}

function dayDate(day: string): Date {
  const [y, m, d] = day.split("-").map(Number);
  return new Date(y, m - 1, d);
}

function addDays(d: Date, n: number): Date {
  const out = new Date(d);
  out.setDate(out.getDate() + n);
  return out;
}

export type DayStat = { minutes: number; events: number };

/** Minutes studied and things handed in, per local day. */
export function byDay(data: InsightsData): Map<string, DayStat> {
  const days = new Map<string, DayStat>();
  const get = (day: string) => {
    let s = days.get(day);
    if (!s) days.set(day, (s = { minutes: 0, events: 0 }));
    return s;
  };
  for (const r of data.study) get(r.day).minutes += r.seconds / 60;
  for (const e of data.events) get(localDay(new Date(e.at))).events += 1;
  return days;
}

export function isActive(s: DayStat | undefined): boolean {
  return !!s && (s.minutes >= ACTIVE_MINUTES || s.events > 0);
}

/** Current and best run of active days. Today not being active yet doesn't break the current run. */
export function streaks(days: Map<string, DayStat>, today = new Date()): { current: number; best: number } {
  let current = 0;
  let cursor = isActive(days.get(localDay(today))) ? today : addDays(today, -1);
  while (isActive(days.get(localDay(cursor)))) {
    current += 1;
    cursor = addDays(cursor, -1);
  }
  const active = [...days.keys()].filter((d) => isActive(days.get(d))).sort();
  let best = 0;
  let run = 0;
  let prev: Date | null = null;
  for (const d of active) {
    const date = dayDate(d);
    run = prev && localDay(addDays(prev, 1)) === d ? run + 1 : 1;
    best = Math.max(best, run);
    prev = date;
  }
  return { current, best: Math.max(best, current) };
}

export type HeatCell = { day: string; minutes: number; events: number; level: 0 | 1 | 2 | 3 | 4; future: boolean };

/** A calendar grid: one column per week, one row per weekday, ending with the current week. */
export function heatmap(days: Map<string, DayStat>, weeks: number, weekStart: 0 | 1, today = new Date()): HeatCell[][] {
  const offset = (today.getDay() - weekStart + 7) % 7;
  const start = addDays(today, -offset - (weeks - 1) * 7);
  // Levels from the learner's own spread, so a light week still shows shades rather than one flat colour.
  const values = [...days.values()].map((s) => s.minutes + s.events * 10).filter((v) => v > 0).sort((a, b) => a - b);
  const q = (p: number) => values[Math.min(values.length - 1, Math.floor(p * values.length))] ?? 0;
  const cuts = [q(0.25), q(0.5), q(0.75)];
  const todayKey = localDay(today);
  const grid: HeatCell[][] = [];
  for (let w = 0; w < weeks; w++) {
    const col: HeatCell[] = [];
    for (let d = 0; d < 7; d++) {
      const date = addDays(start, w * 7 + d);
      const day = localDay(date);
      const s = days.get(day) ?? { minutes: 0, events: 0 };
      const v = s.minutes + s.events * 10;
      const level = v <= 0 ? 0 : v <= cuts[0] ? 1 : v <= cuts[1] ? 2 : v <= cuts[2] ? 3 : 4;
      col.push({ day, minutes: s.minutes, events: s.events, level, future: day > todayKey });
    }
    grid.push(col);
  }
  return grid;
}

/** Minutes per day for the last `n` days, oldest first. */
export function lastDays(days: Map<string, DayStat>, n: number, today = new Date()): { day: string; minutes: number }[] {
  const out = [];
  for (let i = n - 1; i >= 0; i--) {
    const day = localDay(addDays(today, -i));
    out.push({ day, minutes: days.get(day)?.minutes ?? 0 });
  }
  return out;
}

function sumMinutes(days: Map<string, DayStat>, from: Date, count: number): number {
  let total = 0;
  for (let i = 0; i < count; i++) total += days.get(localDay(addDays(from, i)))?.minutes ?? 0;
  return total;
}

/** This week so far against the same days last week. */
export function weekCompare(days: Map<string, DayStat>, weekStart: 0 | 1, today = new Date()) {
  const offset = (today.getDay() - weekStart + 7) % 7;
  const start = addDays(today, -offset);
  return { thisWeek: sumMinutes(days, start, offset + 1), lastWeek: sumMinutes(days, addDays(start, -7), offset + 1) };
}

export function courseColor(courses: CourseCard[], id: string): string {
  const c = courses.find((c) => c.id === id);
  return colorOf(c?.meta, id).deep;
}

export type MarkPoint = { at: Date; percent: number; label: string; color: string; average: number };

/** Marks in time order, with the average of the last five alongside each. */
export function markTrend(data: InsightsData, courses: CourseCard[]): MarkPoint[] {
  const marks = data.marks.filter((m) => !m.missed);
  return marks.map((m, i) => {
    const recent = marks.slice(Math.max(0, i - 4), i + 1);
    const course = courses.find((c) => c.id === m.course_id)?.title ?? "";
    return {
      at: new Date(m.marked_at),
      percent: m.percent,
      label: `${m.title}${course ? ` · ${course}` : ""}`,
      color: courseColor(courses, m.course_id),
      average: recent.reduce((s, r) => s + r.percent, 0) / recent.length,
    };
  });
}

export function averageByKind(data: InsightsData): { kind: string; label: string; average: number; count: number }[] {
  const groups = new Map<string, number[]>();
  for (const m of data.marks) {
    if (m.missed) continue;
    groups.set(m.kind, [...(groups.get(m.kind) ?? []), m.percent]);
  }
  return [...groups.entries()]
    .map(([kind, ps]) => ({ kind, label: kindLabel[kind] ?? kind, average: ps.reduce((a, b) => a + b, 0) / ps.length, count: ps.length }))
    .sort((a, b) => b.average - a.average);
}

export function onTime(data: InsightsData): { onTime: number; late: number; missed: number; rate: number | null } {
  let onTime = 0;
  let late = 0;
  let missed = 0;
  for (const m of data.marks) {
    if (m.missed) missed += 1;
    else if (m.late_days > 0) late += 1;
    else onTime += 1;
  }
  const total = onTime + late + missed;
  return { onTime, late, missed, rate: total ? (100 * onTime) / total : null };
}

/** Minutes per course over the last `n` days, biggest first. */
export function minutesByCourse(data: InsightsData, courses: CourseCard[], n = 30, today = new Date()) {
  const from = localDay(addDays(today, -(n - 1)));
  const totals = new Map<string, number>();
  for (const r of data.study) if (r.day >= from) totals.set(r.course_id, (totals.get(r.course_id) ?? 0) + r.seconds / 60);
  return [...totals.entries()]
    .map(([id, minutes]) => ({ id, title: courses.find((c) => c.id === id)?.title ?? "Removed course", minutes, color: courseColor(courses, id) }))
    .sort((a, b) => b.minutes - a.minutes);
}

/** Minutes by hour of day across all history. */
export function byHour(data: InsightsData): number[] {
  const hours = Array(24).fill(0);
  for (const r of data.study) hours[r.hour] += r.seconds / 60;
  return hours;
}

/** Minutes by weekday (0 = Sunday) across all history. */
export function byWeekday(data: InsightsData): number[] {
  const days = Array(7).fill(0);
  for (const r of data.study) days[dayDate(r.day).getDay()] += r.seconds / 60;
  return days;
}

/** Graded work due in each of the next `weeks` weeks. */
export function workload(data: InsightsData, weeks: number, weekStart: 0 | 1, today = new Date()) {
  const offset = (today.getDay() - weekStart + 7) % 7;
  const start = addDays(today, -offset);
  start.setHours(0, 0, 0, 0);
  const out = Array.from({ length: weeks }, (_, i) => ({ start: addDays(start, i * 7), count: 0, weight: 0 }));
  for (const d of data.due) {
    const i = Math.floor((new Date(d.due_at).getTime() - start.getTime()) / (7 * 864e5));
    if (i >= 0 && i < weeks) {
      out[i].count += 1;
      out[i].weight += d.weight;
    }
  }
  return out;
}

export const WEEKDAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

export function formatMinutes(min: number): string {
  const m = Math.round(min);
  if (m < 60) return `${m} min`;
  const h = Math.floor(m / 60);
  const rest = m % 60;
  return rest ? `${h} h ${rest} min` : `${h} h`;
}

/** A few short observations, only the ones the numbers actually back up. */
export function observations(data: InsightsData, days: Map<string, DayStat>, weekStart: 0 | 1, today = new Date()): string[] {
  const out: string[] = [];
  const hours = byHour(data);
  const totalHours = hours.reduce((a, b) => a + b, 0);
  if (totalHours >= 60) {
    const best = hours.indexOf(Math.max(...hours));
    const label = new Date(2000, 0, 1, best).toLocaleTimeString(undefined, { hour: "numeric" });
    out.push(`You study most around ${label}.`);
    const wd = byWeekday(data);
    out.push(`${WEEKDAYS[wd.indexOf(Math.max(...wd))]} is your busiest day.`);
  }
  const { thisWeek, lastWeek } = weekCompare(days, weekStart, today);
  if (lastWeek >= 15) {
    const change = Math.round((100 * (thisWeek - lastWeek)) / lastWeek);
    if (Math.abs(change) >= 10) out.push(`${formatMinutes(thisWeek)} so far this week, ${Math.abs(change)}% ${change > 0 ? "more" : "less"} than by this point last week.`);
  }
  const marks = data.marks.filter((m) => !m.missed);
  if (marks.length >= 6) {
    const half = Math.floor(marks.length / 2);
    const avg = (ms: typeof marks) => ms.reduce((s, m) => s + m.percent, 0) / ms.length;
    const diff = avg(marks.slice(half)) - avg(marks.slice(0, half));
    if (Math.abs(diff) >= 3) out.push(`Your recent marks are ${Math.abs(diff).toFixed(0)} points ${diff > 0 ? "higher" : "lower"} than your earlier ones.`);
  }
  const kinds = averageByKind(data).filter((k) => k.count >= 2);
  if (kinds.length >= 2 && kinds[0].average - kinds[kinds.length - 1].average >= 8) {
    const low = kinds[kinds.length - 1];
    out.push(`${low.label} is your lowest type of work (${low.average.toFixed(0)}% average), so it may be worth extra practice.`);
  }
  const t = onTime(data);
  if (t.late + t.missed >= 2 && t.rate != null && t.rate < 80) out.push(`${t.late + t.missed} pieces of work were late or missed. Starting a day earlier could help.`);
  return out;
}
