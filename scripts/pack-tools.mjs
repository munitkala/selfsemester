// Builds the downloadable tool files listed in tools/catalog.json.
//
//   node scripts/pack-tools.mjs
//
// For each file it fetches the pinned npm package, takes the named file out of it, applies the listed change (if any),
// writes it to tools-dist/<tool>--<file>, and records its size and SHA-256 back in the catalog. The app checks every
// download against those hashes, so run this, commit the catalog, and upload everything in tools-dist/ to the GitHub
// release named in the catalog ("release"). Nothing here runs the packages' own code.

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const catalogPath = join(root, "tools", "catalog.json");
const out = join(root, "tools-dist");
const catalog = JSON.parse(readFileSync(catalogPath, "utf8"));
const work = mkdtempSync(join(tmpdir(), "ss-tools-"));
const npm = process.platform === "win32" ? "npm.cmd" : "npm";
const unpacked = new Map();

function unpack(spec) {
  if (unpacked.has(spec)) return unpacked.get(spec);
  const dir = join(work, spec.replace(/[^a-z0-9.@-]/gi, "_"));
  mkdirSync(dir, { recursive: true });
  const tgz = execFileSync(npm, ["pack", spec, "--silent", "--pack-destination", dir], { encoding: "utf8", shell: process.platform === "win32" }).trim().split("\n").pop();
  // Relative names, so tar never reads a drive letter as a remote host.
  execFileSync("tar", ["-xzf", tgz], { cwd: dir });
  unpacked.set(spec, join(dir, "package"));
  return unpacked.get(spec);
}

/** ES module -> plain script that sets one global (the module's exports are all `export const`). */
function esmGlobal(src, name) {
  if (/^\s*(import|export\s+(default|\{|\*|function|class|let|var))/m.test(src)) throw new Error(`can't wrap this module for ${name}`);
  return `(function () {\n${src.replace(/^export const /gm, "const ")}\nwindow.${name} = ${name};\n})();\n`;
}

function transform(buf, how) {
  if (!how) return buf;
  const [kind, name] = how.split(":");
  const text = buf.toString("utf8");
  if (kind === "json-global") return Buffer.from(`window.${name} = ${JSON.stringify(JSON.parse(text))};\n`);
  if (kind === "esm-global") return Buffer.from(esmGlobal(text, name));
  throw new Error(`unknown transform ${how}`);
}

mkdirSync(out, { recursive: true });
let total = 0;
try {
  for (const tool of catalog.tools) {
    for (const f of tool.files ?? []) {
      const at = f.from.lastIndexOf("@");
      const slash = f.from.indexOf("/", at);
      const pkg = unpack(f.from.slice(0, slash));
      const src = join(pkg, f.from.slice(slash + 1));
      if (!existsSync(src)) throw new Error(`${f.from} isn't in the package`);
      const data = transform(readFileSync(src), f.transform);
      writeFileSync(join(out, `${tool.id}--${f.name}`), data);
      f.sha256 = createHash("sha256").update(data).digest("hex");
      f.bytes = data.length;
      total += data.length;
      console.log(`${tool.id}/${f.name}  ${(data.length / 1024).toFixed(0)} KB`);
    }
  }
  writeFileSync(catalogPath, JSON.stringify(catalog, null, 2) + "\n");
  console.log(`\n${(total / 1024 / 1024).toFixed(1)} MB in ${out}. Upload these to the "${catalog.release}" release.`);
} finally {
  rmSync(work, { recursive: true, force: true });
}
