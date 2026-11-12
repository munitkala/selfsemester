// Figures in lessons. A lesson can contain two kinds of fenced block, each turned into a numbered figure with a
// caption (and an optional credit line):
//
//   ```plot                                   ```diagram
//   caption: y = x² and its tangent at x = 1   caption: Forces on a block on a slope
//   {"x": [-3, 3], "curves": [{"f": "x^2"}]}   credit: Drawn for this course
//   ```                                        <svg viewBox="0 0 480 300">...</svg>
//                                              ```
//
// Plots are drawn here from the maths, so a curve always matches its equation. Diagrams are SVG drawn for the
// lesson, cleaned of anything active (scripts, links, external images) before they're shown. Both use the theme's
// colours through classes, so they read in light and dark themes alike.

import katex from "katex";
import DOMPurify from "dompurify";
import { widgetSlot } from "#lib/widgets.ts";

// ---------- a small, safe expression parser for plots (no eval) ----------

type Fn = (x: number) => number;

const FUNCS: Record<string, (v: number) => number> = {
  sin: Math.sin, cos: Math.cos, tan: Math.tan, asin: Math.asin, acos: Math.acos, atan: Math.atan,
  sinh: Math.sinh, cosh: Math.cosh, tanh: Math.tanh, exp: Math.exp, ln: Math.log, log: Math.log10,
  sqrt: Math.sqrt, abs: Math.abs, floor: Math.floor, ceil: Math.ceil, sign: Math.sign,
};
const CONSTS: Record<string, number> = { pi: Math.PI, e: Math.E };

type Tok = { t: "num"; v: number } | { t: "id"; v: string } | { t: "op"; v: string };

function tokenize(src: string): Tok[] {
  const out: Tok[] = [];
  const s = src.replace(/\*\*/g, "^").replace(/π/g, "pi").replace(/·|×/g, "*").replace(/−/g, "-");
  let i = 0;
  while (i < s.length) {
    const c = s[i];
    if (/\s/.test(c)) {
      i++;
    } else if (/[0-9.]/.test(c)) {
      const m = s.slice(i).match(/^(\d+\.?\d*|\.\d+)(e[+-]?\d+)?/i)!;
      out.push({ t: "num", v: parseFloat(m[0]) });
      i += m[0].length;
    } else if (/[a-z]/i.test(c)) {
      const m = s.slice(i).match(/^[a-z][a-z0-9]*/i)!;
      out.push({ t: "id", v: m[0].toLowerCase() });
      i += m[0].length;
    } else if ("+-*/^(),".includes(c)) {
      out.push({ t: "op", v: c });
      i++;
    } else {
      throw new Error(`unexpected "${c}"`);
    }
  }
  // Implicit multiplication: 2x, 2(x+1), (x)(x), x sin(x)
  const withMul: Tok[] = [];
  for (const tok of out) {
    const prev = withMul[withMul.length - 1];
    const prevEnds = prev && (prev.t === "num" || (prev.t === "id" && !(Object.hasOwn(FUNCS, prev.v))) || (prev.t === "op" && prev.v === ")"));
    const startsTerm = tok.t === "num" || tok.t === "id" || (tok.t === "op" && tok.v === "(");
    if (prevEnds && startsTerm) withMul.push({ t: "op", v: "*" });
    withMul.push(tok);
  }
  return withMul;
}

/** Compile an expression in x into a function. Throws on anything it doesn't understand. */
export function compile(expr: string): Fn {
  const toks = tokenize(expr);
  let p = 0;
  const peek = () => toks[p];
  const isOp = (v: string) => peek()?.t === "op" && peek()!.v === v;
  const expect = (v: string) => {
    if (!isOp(v)) throw new Error(`expected "${v}"`);
    p++;
  };
  const sum = (): Fn => {
    let left = product();
    while (isOp("+") || isOp("-")) {
      const op = (toks[p++] as { v: string }).v;
      const a = left, b = product();
      left = op === "+" ? (x) => a(x) + b(x) : (x) => a(x) - b(x);
    }
    return left;
  };
  const product = (): Fn => {
    let left = unary();
    while (isOp("*") || isOp("/")) {
      const op = (toks[p++] as { v: string }).v;
      const a = left, b = unary();
      left = op === "*" ? (x) => a(x) * b(x) : (x) => a(x) / b(x);
    }
    return left;
  };
  const unary = (): Fn => {
    if (isOp("-")) {
      p++;
      const a = unary();
      return (x) => -a(x);
    }
    if (isOp("+")) {
      p++;
      return unary();
    }
    return power();
  };
  const power = (): Fn => {
    const base = atom();
    if (isOp("^")) {
      p++;
      const exp = unary(); // right-associative, and allows 2^-x
      return (x) => Math.pow(base(x), exp(x));
    }
    return base;
  };
  const atom = (): Fn => {
    const tok = toks[p++];
    if (!tok) throw new Error("unexpected end");
    if (tok.t === "num") return () => tok.v;
    if (tok.t === "op" && tok.v === "(") {
      const inner = sum();
      expect(")");
      return inner;
    }
    if (tok.t === "id") {
      if (tok.v === "x" || tok.v === "t") return (x) => x;
      if (Object.hasOwn(CONSTS, tok.v)) return () => CONSTS[tok.v];
      if (Object.hasOwn(FUNCS, tok.v)) {
        const f = FUNCS[tok.v];
        expect("(");
        const arg = sum();
        expect(")");
        return (x) => f(arg(x));
      }
      throw new Error(`unknown name "${tok.v}"`);
    }
    throw new Error(`unexpected "${tok.v}"`);
  };
  const f = sum();
  if (p !== toks.length) throw new Error("unexpected text after the expression");
  return f;
}

// ---------- plots ----------

type Pair = [number, number];
type PlotSpec = {
  x: Pair;
  y?: Pair;
  xlabel?: string;
  ylabel?: string;
  equal?: boolean;
  grid?: boolean;
  curves?: { f: string; label?: string; dashed?: boolean; color?: number; domain?: Pair }[];
  data?: { points: Pair[]; label?: string; line?: boolean; color?: number }[];
  points?: { x: number; y: number; label?: string; color?: number }[];
  vectors?: { from?: Pair; to: Pair; label?: string; color?: number }[];
  fills?: { f: string; g?: string; from: number; to: number; label?: string; color?: number }[];
  lines?: { x?: number; y?: number; label?: string; dashed?: boolean }[];
};

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

function niceStep(span: number, target: number) {
  const raw = span / target;
  const mag = Math.pow(10, Math.floor(Math.log10(raw)));
  const n = raw / mag;
  return (n < 1.5 ? 1 : n < 3 ? 2 : n < 7 ? 5 : 10) * mag;
}

function fmt(v: number) {
  const r = Math.abs(v) < 1e-9 ? 0 : v;
  return Number(r.toPrecision(6)).toString().replace("-", "−");
}

let plotSeq = 0;

type LegendEntry = { label: string; color: number; dashed?: boolean; area?: boolean };

function renderPlot(spec: PlotSpec): { svg: string; legend: LegendEntry[] } {
  if (!Array.isArray(spec.x) || spec.x.length !== 2 || !(spec.x[1] > spec.x[0])) throw new Error('"x" must be [min, max]');
  const [x0, x1] = spec.x;
  const curves = (spec.curves ?? []).map((c, i) => ({ ...c, fn: compile(c.f), color: c.color ?? i + 1 }));
  const fills = (spec.fills ?? []).map((c) => ({ ...c, fn: compile(c.f), gn: c.g ? compile(c.g) : () => 0 }));

  // Sample curves once; also used for an automatic y range.
  const N = 400;
  const samples = curves.map((c) => {
    const [a, b] = c.domain ?? [x0, x1];
    const pts: (Pair | null)[] = [];
    for (let i = 0; i <= N; i++) {
      const x = a + ((b - a) * i) / N;
      const y = c.fn(x);
      pts.push(Number.isFinite(y) ? [x, y] : null);
    }
    return pts;
  });

  let [y0, y1] = spec.y ?? [Infinity, -Infinity];
  if (!spec.y) {
    const ys: number[] = [];
    for (const s of samples) for (const p of s) if (p) ys.push(p[1]);
    for (const d of spec.data ?? []) for (const p of d.points) ys.push(p[1]);
    for (const p of spec.points ?? []) ys.push(p.y);
    for (const v of spec.vectors ?? []) ys.push(v.to[1], (v.from ?? [0, 0])[1]);
    ys.sort((a, b) => a - b);
    // Ignore the extreme 2% so asymptotes don't flatten everything else.
    const lo = ys[Math.floor(ys.length * 0.02)] ?? -1;
    const hi = ys[Math.ceil(ys.length * 0.98) - 1] ?? 1;
    const pad = (hi - lo || 1) * 0.1;
    y0 = lo - pad;
    y1 = hi + pad;
  }
  if (!(y1 > y0)) throw new Error('"y" must be [min, max]');

  const L = 46, R = 14, T = 12, B = spec.xlabel ? 42 : 30;
  const W = 560;
  const plotW = W - L - R;
  let plotH = 280;
  if (spec.equal) plotH = Math.max(160, Math.min(420, (plotW * (y1 - y0)) / (x1 - x0)));
  const H = T + plotH + B;
  const sx = (x: number) => L + ((x - x0) / (x1 - x0)) * plotW;
  const sy = (y: number) => T + (1 - (y - y0) / (y1 - y0)) * plotH;
  const id = `plotclip${++plotSeq}`;

  const parts: string[] = [];
  parts.push(`<defs><clipPath id="${id}"><rect x="${L}" y="${T}" width="${plotW}" height="${plotH}"/></clipPath>`);
  for (let c = 1; c <= 6; c++) {
    parts.push(`<marker id="${id}a${c}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" class="f${c}"/></marker>`);
  }
  parts.push("</defs>");

  // Grid and ticks
  const xs = niceStep(x1 - x0, 8);
  const ysStep = niceStep(y1 - y0, 6);
  if (spec.grid !== false) {
    for (let v = Math.ceil(x0 / xs) * xs; v <= x1 + 1e-9; v += xs) parts.push(`<line x1="${sx(v)}" x2="${sx(v)}" y1="${T}" y2="${T + plotH}" class="grid"/>`);
    for (let v = Math.ceil(y0 / ysStep) * ysStep; v <= y1 + 1e-9; v += ysStep) parts.push(`<line x1="${L}" x2="${L + plotW}" y1="${sy(v)}" y2="${sy(v)}" class="grid"/>`);
  }
  // Axes at zero when zero is in range, else on the edge.
  const ax = y0 <= 0 && y1 >= 0 ? sy(0) : T + plotH;
  const ay = x0 <= 0 && x1 >= 0 ? sx(0) : L;
  parts.push(`<line x1="${L}" x2="${L + plotW}" y1="${ax}" y2="${ax}" class="axis"/>`);
  parts.push(`<line x1="${ay}" x2="${ay}" y1="${T}" y2="${T + plotH}" class="axis"/>`);
  for (let v = Math.ceil(x0 / xs) * xs; v <= x1 + 1e-9; v += xs) {
    parts.push(`<text x="${sx(v)}" y="${T + plotH + 16}" class="tick" text-anchor="middle">${fmt(v)}</text>`);
  }
  for (let v = Math.ceil(y0 / ysStep) * ysStep; v <= y1 + 1e-9; v += ysStep) {
    parts.push(`<text x="${L - 6}" y="${sy(v) + 4}" class="tick" text-anchor="end">${fmt(v)}</text>`);
  }
  if (spec.xlabel) parts.push(`<text x="${L + plotW / 2}" y="${H - 6}" class="label" text-anchor="middle">${esc(spec.xlabel)}</text>`);
  if (spec.ylabel) parts.push(`<text x="12" y="${T + plotH / 2}" class="label" text-anchor="middle" transform="rotate(-90 12 ${T + plotH / 2})">${esc(spec.ylabel)}</text>`);

  const body: string[] = [];
  // Shaded areas
  for (const f of fills) {
    const top: string[] = [];
    const bot: string[] = [];
    for (let i = 0; i <= 200; i++) {
      const x = f.from + ((f.to - f.from) * i) / 200;
      top.push(`${sx(x).toFixed(1)},${sy(f.fn(x)).toFixed(1)}`);
      bot.unshift(`${sx(x).toFixed(1)},${sy(f.gn(x)).toFixed(1)}`);
    }
    body.push(`<polygon points="${top.concat(bot).join(" ")}" class="area a${f.color ?? 1}"/>`);
  }
  // Reference lines
  for (const l of spec.lines ?? []) {
    if (l.x != null) body.push(`<line x1="${sx(l.x)}" x2="${sx(l.x)}" y1="${T}" y2="${T + plotH}" class="ref${l.dashed === false ? "" : " dash"}"/>`);
    if (l.y != null) body.push(`<line x1="${L}" x2="${L + plotW}" y1="${sy(l.y)}" y2="${sy(l.y)}" class="ref${l.dashed === false ? "" : " dash"}"/>`);
    if (l.label) {
      const tx = l.x != null ? sx(l.x) + 4 : L + plotW - 4;
      const ty = l.y != null ? sy(l.y) - 5 : T + 12;
      body.push(`<text x="${tx}" y="${ty}" class="note"${l.x == null ? ' text-anchor="end"' : ""}>${esc(l.label)}</text>`);
    }
  }
  // Curves, broken where the function is undefined or jumps off the chart.
  curves.forEach((c, i) => {
    const segs: string[] = [];
    let seg: string[] = [];
    let prev: Pair | null = null;
    for (const pnt of samples[i]) {
      const jump = pnt && prev && Math.abs(pnt[1] - prev[1]) > (y1 - y0) * 3;
      if (!pnt || jump) {
        if (seg.length > 1) segs.push(seg.join(" "));
        seg = [];
      }
      if (pnt) seg.push(`${sx(pnt[0]).toFixed(1)},${sy(pnt[1]).toFixed(1)}`);
      prev = pnt;
    }
    if (seg.length > 1) segs.push(seg.join(" "));
    for (const s of segs) body.push(`<polyline points="${s}" class="curve c${c.color}${c.dashed ? " dash" : ""}"/>`);
  });
  // Data series
  (spec.data ?? []).forEach((d, i) => {
    const color = d.color ?? curves.length + i + 1;
    if (d.line) body.push(`<polyline points="${d.points.map(([x, y]) => `${sx(x).toFixed(1)},${sy(y).toFixed(1)}`).join(" ")}" class="curve c${color}"/>`);
    for (const [x, y] of d.points) body.push(`<circle cx="${sx(x)}" cy="${sy(y)}" r="3.5" class="dot f${color}"/>`);
  });
  // Vectors
  for (const v of spec.vectors ?? []) {
    const [fx, fy] = v.from ?? [0, 0];
    const c = v.color ?? 1;
    body.push(`<line x1="${sx(fx)}" y1="${sy(fy)}" x2="${sx(v.to[0])}" y2="${sy(v.to[1])}" class="vec c${c}" marker-end="url(#${id}a${c})"/>`);
    if (v.label) body.push(`<text x="${sx(v.to[0]) + 6}" y="${sy(v.to[1]) - 6}" class="note t${c}">${esc(v.label)}</text>`);
  }
  // Labelled points
  for (const pt of spec.points ?? []) {
    body.push(`<circle cx="${sx(pt.x)}" cy="${sy(pt.y)}" r="4" class="dot f${pt.color ?? 1}"/>`);
    if (pt.label) body.push(`<text x="${sx(pt.x) + 7}" y="${sy(pt.y) - 7}" class="note">${esc(pt.label)}</text>`);
  }
  parts.push(`<g clip-path="url(#${id})">${body.join("")}</g>`);

  const legend: LegendEntry[] = [
    ...curves.filter((c) => c.label).map((c) => ({ label: c.label!, color: c.color, dashed: c.dashed })),
    ...(spec.data ?? []).map((d, i) => ({ label: d.label ?? "", color: d.color ?? curves.length + i + 1 })).filter((d) => d.label),
    ...fills.filter((f) => f.label).map((f) => ({ label: f.label!, color: f.color ?? 1, area: true })),
  ];
  const svg = `<svg viewBox="0 0 ${W} ${H}" class="plot" role="img">${parts.join("")}</svg>`;
  return { svg, legend };
}

// ---------- diagrams ----------

function cleanSvg(src: string): string {
  const clean = DOMPurify.sanitize(src, {
    USE_PROFILES: { svg: true, svgFilters: false },
    FORBID_TAGS: ["script", "style", "foreignObject", "image", "a", "use", "animate", "set", "animateTransform", "animateMotion"],
    FORBID_ATTR: ["href", "xlink:href", "style"],
  });
  if (!/^\s*<svg[\s>]/i.test(clean)) throw new Error("no <svg> element");
  // Plain black follows the theme's text colour, so drawings stay visible on dark themes.
  const themed = clean.replace(/\s(fill|stroke)="(black|#000|#000000)"/gi, ' $1="currentColor"');
  // Scale with the page rather than at a fixed pixel size.
  return themed.replace(/^(\s*<svg)([^>]*)>/i, (_, open, attrs) => `${open}${attrs.replace(/\s(width|height|class|role)="[^"]*"/gi, "")} class="drawn" role="img">`);
}

// ---------- captions and figure assembly ----------

/** Plain text with $maths$, for captions and labels. */
export function inline(text: string): string {
  return text
    .split(/(\$[^$\n]+\$)/g)
    .map((part, i) =>
      i % 2
        ? katex.renderToString(part.slice(1, -1), { throwOnError: false })
        : esc(part)
            .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
            .replace(/\^(\{([^}]*)\}|\(([^)]*)\)|-?[0-9a-z.]+)/gi, (_m, all, braced, paren) => `<sup>${braced ?? paren ?? all}</sup>`),
    )
    .join("");
}

const FENCE = /^```(plot|diagram|widget)[^\n]*\n([\s\S]*?)^```[ \t]*$/gm;

/** Pull figure blocks out of a lesson. Returns the Markdown with placeholders and the figures' HTML, numbered in order. */
export function extractFigures(md: string): { md: string; figures: string[] } {
  const figures: string[] = [];
  const out = md.replace(FENCE, (_, kind: string, body: string) => {
    const n = figures.length + 1;
    let caption = "";
    let credit = "";
    let height = 320;
    let tools: string[] = [];
    const lines = body.split("\n");
    while (lines.length && /^(caption|credit|source|height|tools)\s*:/i.test(lines[0].trim())) {
      const line = lines.shift()!.trim();
      const value = line.slice(line.indexOf(":") + 1).trim();
      if (/^caption/i.test(line)) caption = value;
      else if (/^height/i.test(line)) height = parseInt(value, 10);
      else if (/^tools/i.test(line)) tools = value.split(/[,\s]+/).filter((t) => /^[a-z0-9-]+$/.test(t));
      else credit = value;
    }
    const content = lines.join("\n").trim();
    let inner = "";
    let legend = "";
    try {
      if (kind === "plot") {
        const r = renderPlot(JSON.parse(content));
        inner = r.svg;
        if (r.legend.length) {
          legend = `<ul class="figure-legend">${r.legend
            .map((l) => `<li><span class="key ${l.area ? `area a${l.color}` : `c${l.color}`}${l.dashed ? " dash" : ""}"></span>${inline(l.label)}</li>`)
            .join("")}</ul>`;
        }
      } else if (kind === "widget") {
        inner = widgetSlot(content, height, caption, tools);
      } else {
        inner = cleanSvg(content);
      }
    } catch (e) {
      inner = `<p class="figure-error">This figure couldn't be drawn (${esc(String((e as Error).message ?? e))}).</p>`;
    }
    const cap = `<figcaption><strong>Figure ${n}.</strong> ${inline(caption)}${credit ? `<span class="credit">${inline(credit)}</span>` : ""}</figcaption>`;
    figures.push(`<figure class="figure ${kind}" id="figure-${n}"><div class="figure-art">${inner}</div>${legend}${cap}</figure>`);
    return `\n\n@@FIG${n - 1}@@\n\n`;
  });
  return { md: out, figures };
}

/** Put figures back where their placeholders ended up in the rendered HTML. */
export function insertFigures(html: string, figures: string[]): string {
  return html.replace(/<p>\s*@@FIG(\d+)@@\s*<\/p>|@@FIG(\d+)@@/g, (_, a, b) => figures[Number(a ?? b)] ?? "");
}
