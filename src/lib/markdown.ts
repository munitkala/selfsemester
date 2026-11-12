import { marked } from "marked";
import katex from "katex";
import DOMPurify from "dompurify";
import { extractFigures, insertFigures } from "#lib/figures.ts";

/**
 * Markdown + LaTeX + figures -> safe HTML.
 * Figure blocks (```plot and ```diagram, see figures.ts) are drawn first and kept aside. Maths ($...$, $$...$$,
 * \(...\), \[...\]) is cut out next so Markdown can't mangle it, the Markdown is rendered and sanitised, then KaTeX
 * output and the figures (both built locally, the figures already cleaned) are put back in place.
 */
export function render(md: string): string {
  const { md: withoutFigures, figures } = extractFigures(md);
  const maths: string[] = [];
  const stash = (tex: string, display: boolean) => {
    maths.push(katex.renderToString(tex.trim(), { displayMode: display, throwOnError: false, output: "htmlAndMathml" }));
    return `@@MATH${maths.length - 1}@@`;
  };
  // Leave code spans and fenced code alone.
  const parts = withoutFigures.split(/(```[\s\S]*?```|`[^`\n]*`)/g);
  const prepared = parts
    .map((p, i) => {
      if (i % 2 === 1) return p;
      return p
        .replace(/\$\$([\s\S]+?)\$\$/g, (_, t) => stash(t, true))
        .replace(/\\\[([\s\S]+?)\\\]/g, (_, t) => stash(t, true))
        .replace(/\\\(([\s\S]+?)\\\)/g, (_, t) => stash(t, false))
        .replace(/(^|[^\\$])\$([^\s$](?:[^$\n]*?[^\s$])?)\$(?!\d)/g, (_, pre, t) => pre + stash(t, false));
    })
    .join("");
  const html = DOMPurify.sanitize(marked.parse(prepared, { async: false, gfm: true, breaks: false }) as string, {
    FORBID_TAGS: ["style", "form", "input", "iframe"],
  });
  return insertFigures(html.replace(/@@MATH(\d+)@@/g, (_, n) => maths[Number(n)] ?? ""), figures);
}
