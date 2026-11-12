# Third-party software

SelfSemester's own code is MIT licensed (see `LICENSE`). It contains no code copied from other projects. The
interface icons and the colour themes were made for SelfSemester. The libraries below are pulled in by Cargo and npm
at build time and keep their own licences; none of them are GPL or AGPL.

## Rust crates used directly

| Crate | Version | Licence | Used for |
| --- | --- | --- | --- |
| tauri | 2.12 | MIT OR Apache-2.0 | App shell (Windows WebView2) |
| tauri-plugin-opener | 2.7 | MIT OR Apache-2.0 | Opening links and folders |
| tauri-plugin-notification | 2.8 | MIT OR Apache-2.0 | Windows notifications |
| tauri-plugin-dialog | 2.8 | MIT OR Apache-2.0 | File picker for uploads |
| rusqlite (bundled SQLite) | 0.41 | MIT (SQLite itself is public domain) | Database and full-text search |
| aes-gcm | 0.10 | MIT OR Apache-2.0 | Sealing answer keys and exams on disk |
| sha2 | 0.10 | MIT OR Apache-2.0 | Checking downloaded tools |
| serde, serde_json | 1.0 | MIT OR Apache-2.0 | JSON |
| chrono | 0.4 | MIT OR Apache-2.0 | Dates and deadlines |
| dirs | 6 | MIT OR Apache-2.0 | Finding AppData and Documents |
| thiserror | 2 | MIT OR Apache-2.0 | Error types |

All 279 crates in the Windows build are under permissive licences (MIT, Apache-2.0, BSD, ISC, Zlib, Unicode-3.0,
Unlicense, CC0) except five that are **MPL-2.0**: `cssparser`, `cssparser-macros`, `selectors`, `dtoa-short`
(pulled in by Tauri) and `option-ext` (pulled in by `dirs`). MPL-2.0 is file-level copyleft: it applies to those
crates' own files, which are used unmodified, and doesn't affect the rest of the app.

## npm packages shipped in the app

| Package | Version | Licence | Used for |
| --- | --- | --- | --- |
| @tauri-apps/api, plugin-dialog, plugin-opener | 2.x | MIT OR Apache-2.0 | Talking to the Rust side |
| @sveltejs/kit (client runtime) | 3.1 | MIT | Screens and navigation |
| svelte (runtime) | 5 | MIT | Screens and navigation |
| marked | 18.1 | MIT | Markdown rendering |
| katex (including its fonts) | 0.19 | MIT | Maths rendering |
| dompurify | 3.4 | MPL-2.0 OR Apache-2.0 (used under Apache-2.0) | Sanitising rendered HTML |

## Optional tools (downloaded only if you install them)

These aren't part of the app. They're published unmodified on this project's GitHub release (`tools-1`), each with
its licence file, and downloaded only when you install the tool (see `tools/catalog.json`). The chess.js file is
wrapped so it loads as a plain script; nothing else is changed.

| Tool | Package | Version | Licence |
| --- | --- | --- | --- |
| Geometry and graphing | jsxgraph | 1.10.1 | MIT (dual-licensed MIT OR LGPL-3.0-or-later; used under MIT) |
| Sheet music | abcjs | 6.4.4 | MIT |
| 3D molecules | 3dmol | 2.4.2 | BSD-3-Clause |
| World maps | d3 | 7.9.0 | ISC |
| World maps | topojson-client | 3.1.0 | ISC |
| World maps | world-atlas (data from Natural Earth, public domain) | 2.0.2 | ISC |
| Chess rules | chess.js | 0.13.4 | BSD-2-Clause |

Build-only tools (the Svelte and SvelteKit compilers, the static adapter, Vite, TypeScript, svelte-check) are MIT
licensed and aren't shipped. `deepmerge` (MIT), pulled in by the Svelte build plugin, is build-only too.

## Pictures and examples in this repository

- **Screenshots** in `assets/screenshots` show the app with made-up demo courses, taken for this repository.
- **Example lessons** shown in [docs/SCREENSHOTS.md](docs/SCREENSHOTS.md) were written by the app's own lesson prompt
  through Claude Code. They name free textbooks (OpenStax, Open Data Structures, the LibreOffice documentation) as
  reading only; no text or figures from those books are reproduced. Their graphs are drawn by SelfSemester's own
  plotting code and their diagrams were drawn for the lessons.
- **Plots and diagrams** in lessons are drawn by code written for SelfSemester (`src/lib/figures.ts`); no charting
  or diagram library is used.

## Not included

- **Claude Code** is Anthropic's software. SelfSemester doesn't bundle, modify or redistribute it. Each user
  installs it themselves and signs in with their own Claude account, under Anthropic's terms.
- **Course material.** The repository contains no textbooks or downloaded content. Courses are generated on the
  user's own computer, for their personal study, and link to their sources with each source's licence.
