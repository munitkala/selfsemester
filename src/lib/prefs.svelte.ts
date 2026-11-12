// Display preferences. They only change how the app looks and behaves on this computer, so they live in the window's
// local storage rather than the course database. static/theme-boot.js applies the same attributes before the first
// paint, so a dark theme never flashes white on start; keep the two in step.

export const STORAGE_KEY = "selfsemester.prefs.v1";

export type Mode = "system" | "light" | "dark";
export type LightTheme = "classic" | "paper" | "mint" | "contrast";
export type DarkTheme = "classic" | "slate" | "midnight" | "forest" | "contrast";
export type Accent = "theme" | "blue" | "teal" | "green" | "violet" | "rose" | "coral" | "amber";
export type Effects = "full" | "balanced" | "minimal";

export type Prefs = {
  mode: Mode;
  lightTheme: LightTheme;
  darkTheme: DarkTheme;
  accent: Accent;
  textScale: number;
  density: "compact" | "comfortable" | "spacious";
  corners: "sharp" | "rounded" | "round";
  width: "auto" | "normal" | "wide" | "full";
  sidebar: "full" | "compact";
  startPage: "/" | "/courses" | "/insights";
  effects: Effects;
  motion: "system" | "on" | "off";
  celebrate: boolean;
  trackStudy: boolean;
  dailyGoal: number;
  weekStart: 0 | 1;
  heatmapWeeks: 26 | 52;
  shortcuts: boolean;
};

export const defaults: Prefs = {
  mode: "system",
  lightTheme: "classic",
  darkTheme: "classic",
  accent: "theme",
  textScale: 100,
  density: "comfortable",
  corners: "rounded",
  width: "auto",
  sidebar: "full",
  startPage: "/",
  effects: "balanced",
  motion: "system",
  celebrate: true,
  trackStudy: true,
  dailyGoal: 30,
  weekStart: 1,
  heatmapWeeks: 26,
  shortcuts: true,
};

export const lightThemes: { id: LightTheme; name: string; note: string; swatch: [string, string, string] }[] = [
  { id: "classic", name: "Classic", note: "Warm off-white", swatch: ["#f6f5f1", "#ffffff", "#2c55c9"] },
  { id: "paper", name: "Paper", note: "Sepia, easy on the eyes for long reading", swatch: ["#f4ecd8", "#fbf6ea", "#8a5a1c"] },
  { id: "mint", name: "Mint", note: "Cool and calm, with green", swatch: ["#f1f5f2", "#ffffff", "#1f7a55"] },
  { id: "contrast", name: "High contrast", note: "Black on white, strongest contrast", swatch: ["#ffffff", "#ffffff", "#0039b3"] },
];

export const darkThemes: { id: DarkTheme; name: string; note: string; swatch: [string, string, string] }[] = [
  { id: "classic", name: "Classic", note: "Soft charcoal", swatch: ["#151514", "#1f1f1d", "#84a2ff"] },
  { id: "slate", name: "Slate", note: "Blue-grey, good for evenings", swatch: ["#0f172a", "#162033", "#7cc4f5"] },
  { id: "midnight", name: "Midnight", note: "Deepest dark, with violet", swatch: ["#0b0f19", "#121826", "#b4a4ff"] },
  { id: "forest", name: "Forest", note: "Dark green, low glare", swatch: ["#101814", "#16211b", "#7fd3a4"] },
  { id: "contrast", name: "High contrast", note: "White on black, strongest contrast", swatch: ["#000000", "#0d0d0d", "#9ec1ff"] },
];

/** Hue in OKLCH. "theme" keeps each theme's own accent. */
export const accents: { id: Accent; name: string; hue: number | null }[] = [
  { id: "theme", name: "Theme default", hue: null },
  { id: "blue", name: "Blue", hue: 262 },
  { id: "teal", name: "Teal", hue: 200 },
  { id: "green", name: "Green", hue: 150 },
  { id: "violet", name: "Violet", hue: 295 },
  { id: "rose", name: "Rose", hue: 355 },
  { id: "coral", name: "Coral", hue: 30 },
  { id: "amber", name: "Amber", hue: 70 },
];

function load(): Prefs {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return { ...defaults, ...JSON.parse(raw) };
  } catch {
    /* blocked or broken storage: use the defaults */
  }
  return { ...defaults };
}

export const prefs: Prefs = $state(load());

const darkQuery = typeof matchMedia === "function" ? matchMedia("(prefers-color-scheme: dark)") : null;
const motionQuery = typeof matchMedia === "function" ? matchMedia("(prefers-reduced-motion: reduce)") : null;

export function isDark(p: Prefs = prefs): boolean {
  return p.mode === "dark" || (p.mode === "system" && !!darkQuery?.matches);
}

export function reducedMotion(p: Prefs = prefs): boolean {
  return p.motion === "off" || (p.motion === "system" && !!motionQuery?.matches) || p.effects === "minimal";
}

/** Put the preferences on <html> as data attributes and variables, which app.css reads. */
export function apply(p: Prefs = prefs) {
  const root = document.documentElement;
  const dark = isDark(p);
  root.dataset.scheme = dark ? "dark" : "light";
  root.dataset.theme = dark ? `dark-${p.darkTheme}` : `light-${p.lightTheme}`;
  root.dataset.density = p.density;
  root.dataset.corners = p.corners;
  root.dataset.width = p.width;
  root.dataset.effects = p.effects;
  root.dataset.motion = reducedMotion(p) ? "reduced" : "full";
  const hue = accents.find((a) => a.id === p.accent)?.hue;
  if (hue == null) root.style.removeProperty("--hue");
  else root.style.setProperty("--hue", String(hue));
  root.style.setProperty("--text-scale", String(p.textScale / 100));
}

export function save() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
  } catch {
    /* nothing to do: the choice still applies until the app closes */
  }
  apply();
}

export function reset() {
  Object.assign(prefs, defaults);
  save();
}

/** Follow the system's light/dark and reduced-motion settings while the app is open. */
export function watchSystem(): () => void {
  const on = () => apply();
  darkQuery?.addEventListener("change", on);
  motionQuery?.addEventListener("change", on);
  return () => {
    darkQuery?.removeEventListener("change", on);
    motionQuery?.removeEventListener("change", on);
  };
}
