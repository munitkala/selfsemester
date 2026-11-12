<p align="center"><img src="assets/logo.svg" width="112" alt="SelfSemester logo"></p>

# SelfSemester

A Windows desktop app that turns any subject into a full university-style course: lessons, homework, quizzes,
assignments and exams, released on a schedule with hard deadlines and real marks.

You name a subject. Claude asks a few questions about your goals and level, plans a syllabus, and tells you how well
it can teach and mark that subject before you commit. Once you approve, it writes the whole course up front, with
proper maths, exact graphs and original diagrams where the subject needs them. After that the app runs the course on
its own: it releases material day by day, enforces deadlines, auto-marks quizzes, and sends written work and photos
of handwritten working back to Claude for marking against a rubric. Final marks go on a transcript.

You can also write a course yourself in the **course builder**, with no Claude at all, and run it the same way.

> **Status: early (0.7.0).** It works end to end but has had little real-world use. Expect rough edges.

<p align="center"><img src="assets/screenshots/04-today.png" width="860" alt="The Today page in a SelfSemester window"></p>

## What it does

- **Courses written for you**, planned around your goals, with a readiness report before anything is built
- **Lessons with figures**: maths in LaTeX, graphs the app draws exactly from the equations, and original diagrams
  with numbered captions and credits
- **Lessons you can hear**: language, music and other courses where sound matters play every word, tone, phrase or
  chord right where it's taught, using free recordings by native speakers from Wikimedia Commons, with the computer's
  own voice or synthesised notes when no recording exists
- **Figures you can try**: sliders, small simulations and self-checking drills inside the lesson, each sealed off
  from the internet, your files and the rest of the app
- **Tools you choose**: geometry, sheet music, 3D molecules and world maps for the subjects you study, picked after
  installing or in Settings; a course that needs something more asks first
- **Write your own course from documents you already have**: attach PDFs, Word files or slides to any lesson or
  piece of work, start with just one lesson, and add weeks, quizzes and exams while the course runs
- **A classroom for every course**: home, weekly modules, syllabus, gradebook and your own materials
- **Real deadlines and marking**: quizzes marked instantly, written and handwritten work marked against a rubric,
  and a review thread to ask why
- **Insights**: streaks, study time against a daily goal, an activity calendar and mark trends
- **Your look**: light, dark or system, nine themes, any accent colour, and effects you can turn down
- **Fits any window**: panels rearrange as you resize
- **Private**: everything stays on your computer; the only thing that goes online is your own Claude Code

| | | |
| --- | --- | --- |
| ![A calculus lesson with secant and tangent lines plotted](assets/screenshots/samples/calculus.png) | ![A statistics lesson with shaded areas under the normal curve](assets/screenshots/samples/statistics.png) | ![A marked result with a score ring and rubric bars](assets/screenshots/08-result.png) |
| Calculus lesson with a plot | Statistics lesson with shaded areas | A marked result |

**More pictures, and what each screen does:** [docs/SCREENSHOTS.md](docs/SCREENSHOTS.md), including example lessons written
by the app and all the themes.

## What's new

Version 0.7.0 makes lessons something you can hear and try: language and music lessons play every word, tone and
chord, lessons can have interactive figures, and you choose the tools for what you study (geometry, sheet music, 3D
molecules, world maps), with a course asking before it installs anything else. Version 0.6.0 lets you build a course
from your own documents.
Version 0.5.0 added figures in lessons, themes, an Insights page, clearer feedback and layouts that fit any window. The full list is in
[CHANGELOG.md](CHANGELOG.md), and what's planned is in [ROADMAP.md](ROADMAP.md).


## Requirements

| What | Details |
| --- | --- |
| Windows 10 or 11, 64-bit | Uses Microsoft Edge WebView2, which Windows 11 already has (the installer adds it on Windows 10 if needed). |
| [Claude Code](https://claude.com/claude-code) | Installed and signed in **with a Claude subscription** (Pro or Max). SelfSemester runs your own copy of Claude Code; there's no API key to enter. |
| Internet | Needed while a course is being built, while lesson sounds or tools are being downloaded, and whenever work is marked. Reading lessons and doing work happen offline. |

To build from source instead of downloading the installer, you also need Node.js 22.17+, Rust 1.85+ and the Visual
Studio C++ build tools (see [Install](#install)).

## Before you use it

- **Cost.** SelfSemester itself is free. Signed in with a Claude subscription, Claude Code adds no extra charges. If
  Claude Code is signed in with an API key instead, every call is billed to that key; the Settings page tells you
  which one you're using.
- **Usage limits.** Building a course is heavy work: roughly one call per stage plus two per unit, with each unit
  taking a few minutes. A long course can use a large share of your plan's limits. When a limit is reached, the
  app pauses and tries again at the reset time Claude Code reports (or at least every hour). If your limit has
  reset sooner, press **Resume now**. A progress bar shows roughly how long building or marking has left, based on
  how long each step has taken before on your computer.
- **Keep the app open while a course is building.** Building, releases, deadline checks and reminders only run while
  the window is open. Anything that fell due while it was closed is settled the next time you open it.
- **AI teaching and marking can be wrong.** Before you enrol, each course shows a readiness report with honest
  estimates for every step; none is ever 100%. A second model checks each unit, but mistakes can still get through.
  Every mark shows its reasoning and the answer key, so you can check it.
- **Not a qualification.** Transcripts record your own study. They aren't accredited and shouldn't be presented as
  if they were.
- **Sealed files keep honest people honest.** Answer keys and unreleased exams are encrypted on disk, but the key is
  on the same computer. It stops accidental peeking, not someone set on cheating on their own course.
- **Sources and pictures.** Claude prefers free and openly licensed material (OpenStax, MIT OpenCourseWare,
  LibreTexts, official docs) and marks anything paid as "get this yourself". The app never bypasses paywalls. Lessons
  are written in Claude's own words and link to their sources. Figures are drawn for the lesson rather than copied,
  and anything adapted from an openly licensed source is credited in its caption. Lesson sounds are freely licensed
  recordings from Wikimedia Commons, credited with each lesson, or clearly marked computer voices. Keep generated
  courses for your own study.
- **Supported work types today:** typed answers (Markdown and LaTeX), photo or PDF uploads, and auto-marked quizzes.
  Courses that would need audio, camera or code-running checks are planned without those parts for now.
- **Your own materials:** each course page has a "Your materials" section where you can add reference files (PDFs,
  text, notes, code), images and typed notes. Claude reads them when it plans the course, writes each unit and marks
  your work, so add them before you approve a course if you want every unit to use them.

## Privacy

Everything stays on your computer. The app has no server and no account system. Your Claude Code sends course
prompts, your course materials, your submitted work and review questions to Anthropic to generate and mark courses,
under your agreement with Anthropic. The only other thing the app contacts is Wikimedia Commons, to find and download
recordings for lesson sounds: it sends the word, phrase or sound it's looking for and nothing about you. You can turn
that off in Settings, Lesson sounds. When you install a tool, the app downloads its files from this project's GitHub
releases; nothing about you is sent. SelfSemester never reads, stores or sends your Claude sign-in.
Courses you write in the course builder never leave your computer, unless you ask Claude about one in review mode.

| Folder | Contents |
| --- | --- |
| `%LOCALAPPDATA%\SelfSemester` | Database, course files, sealed answer keys, submissions, your course materials |
| `Documents\SelfSemester\Course builder` | Courses you export from the course builder, to edit in a text editor |
| `Documents\SelfSemester\Completed` | Archives of finished courses: summary, lesson notes, your work, feedback, reference list |
| `Documents\SelfSemester\Backups` | Safety copies of your data, made before each update (the last 5 are kept) |

## Legal and responsible use

Read [`DISCLAIMER.md`](DISCLAIMER.md) before using SelfSemester; the app also asks you to accept its key points the
first time it opens. In short:

- **No warranty.** It's free software provided as is. The authors aren't liable for any loss, including wrong marks,
  lost data or usage on your Claude account. You use it at your own risk.
- **Not professional advice.** Courses Claude writes are AI-generated self-study material, and courses you write
  yourself are your own content. For medicine, law, finance, safety or any subject where a mistake could hurt someone,
  check everything with a qualified professional.
- **Personal study only.** Don't publish or sell generated courses or downloaded sources. Follow each source's
  licence, and never use or change the app to get around paywalls, logins or site terms.
- **Your Claude account, your agreement.** Claude Code runs under your own agreement with Anthropic, and your
  requests and submitted work are sent to Anthropic.
- **Academic integrity.** If you're at a school or university, follow its rules on AI tools.
- **Back up your data.** Everything lives on your computer, and nobody else can recover it. SelfSemester copies it
  to `Documents\SelfSemester\Backups` before each update, but keep your own copy somewhere else too.

## Install

### Download the installer (easiest)

1. Go to the [latest release](https://github.com/munitkala/selfsemester/releases/latest) and download
   `SelfSemester_<version>_x64-setup.exe` under **Assets**.
2. Run it. It installs for your Windows account only, so it doesn't need administrator rights.
3. The installer isn't code-signed, so Windows SmartScreen warns about an unknown publisher. Choose
   **More info → Run anyway**. To check the download first, run `Get-FileHash .\SelfSemester_<version>_x64-setup.exe`
   in PowerShell and compare it with the `.sha256` file next to the installer on the release page.
4. Open SelfSemester from the Start menu.

The installers are built by GitHub Actions straight from the tagged source code (see
`.github/workflows/release.yml`), not on anyone's own computer.

**Updating:** download the newer installer and run it. It replaces only the program: your courses, materials,
marks, transcript and course-builder drafts are kept, and the new version backs them up to
`Documents\SelfSemester\Backups` before it first opens them. If your data ever goes missing, SelfSemester restores
the newest backup by itself when it starts. **Settings → Your data and backups** shows the backups and has a
**Back up now** button.

**Uninstalling:** use **Settings → Apps → Installed apps** in Windows. Your data in `%LOCALAPPDATA%\SelfSemester`
and `Documents\SelfSemester` stays until you delete it yourself.

### Or build it yourself

You need Node.js 22.17+, Rust 1.85+ and the Visual Studio C++ build tools.

```powershell
git clone https://github.com/munitkala/selfsemester.git
cd selfsemester
npm install
npm run tauri build -- --bundles nsis
```

The installer is written to `src-tauri\target\release\bundle\nsis\`. Run it the same way as above.

### Then connect Claude Code

Courses you write yourself in the course builder work without this. For courses Claude writes and marks, install
[Claude Code](https://claude.com/claude-code), open a terminal, run `claude` and sign in with your Claude account.
Then in SelfSemester open **Settings** and press **Run test**.

## Development

```powershell
npm install
npm run tauri dev          # run the app with hot reload
npm run check              # type-check the interface
cd src-tauri
cargo test                 # unit tests, including a simulated course from intake to transcript
```

Three tests call the real Claude Code and use plan usage, so they're skipped unless asked for:
`cargo test -- --ignored live_haiku` (one tiny call),
`cargo test --lib -- --ignored live_compile --nocapture` (every build stage on a one-week course, about five minutes)
and `SS_SAMPLE=physics cargo test --lib -- --ignored live_sample_unit --nocapture` (one example lesson, written and
checked the way a real course is; these made the examples in [docs/SCREENSHOTS.md](docs/SCREENSHOTS.md)).

Two environment variables help when testing without touching real courses:

| Variable | Effect |
| --- | --- |
| `SELFSEMESTER_HOME=<folder>` | Keep all data (database, courses, archives) under that folder instead of AppData and Documents |
| `SELFSEMESTER_NO_CLAUDE=1` | Never start Claude jobs; queued work just waits (useful for demos and UI work) |

The pictures in [docs/SCREENSHOTS.md](docs/SCREENSHOTS.md) are the app's interface with made-up demo data, plus the
example lessons above.

### How it fits together

```
src/                      SvelteKit interface (Today, Courses, Insights, course and item pages, builder, Transcript, Settings)
  lib/markdown.ts         lessons: Markdown, LaTeX maths (KaTeX) and figures
  lib/figures.ts          plots drawn from the maths, and cleaned SVG diagrams, with numbered captions
  lib/audio.ts            play buttons for lesson sounds
  lib/widgets.ts          interactive figures: fills each slot with a sandboxed frame
  lib/ToolList.svelte     Settings, Tools; lib/ToolSetup.svelte is the first-run picker
  lib/prefs.svelte.ts     themes and display settings
src-tauri/src/
  claude/                 runs `claude -p` with a JSON schema, reads the result, spots usage limits
  course/
    prompts.rs            the prompt and JSON schema for each stage
    pipeline.rs           intake -> plan -> sources -> units -> check -> schedule
    schedule.rs           release dates, deadlines, late policy, Structured and Sandbox rules
    marking.rs            submissions, auto-marking, instant feedback, self-marking, grades, transcript, archive
    materials.rs          your own reference files, images and notes for a course
    review.rs             review mode: talking a marked item through with Claude
    builder.rs            the course builder: drafts, publishing, folder export and import
    seal.rs               encrypted answer keys and exams
    audio.rs              lesson sounds: free recordings from Wikimedia Commons, computer voices, synthesised notes
  widget.rs               serves interactive figures from their own address with a no-network policy
  tools.rs                optional tools: catalog, checked downloads, install and remove
tools/catalog.json        the tool list; scripts/pack-tools.mjs builds the files for a GitHub release
  db/                     SQLite schema, migrations, job queue
  worker.rs               background thread: runs one Claude job at a time and ticks the clock
                          (a second thread fills lesson sounds)
  backup.rs               backups before each update, and restoring if the data goes missing
```

Every call to Claude is a row in a job queue, so work survives crashes and usage limits. Each call runs in an empty
folder with no tools (unless the stage needs web search or must read an upload or your course materials), no MCP servers and none of the
user's Claude Code settings, which keeps calls small and predictable.

## Not built yet

- Downloading and indexing sources locally (lessons currently cite their sources rather than quoting from them)
- Tray icon and start with Windows, so reminders arrive with the window closed
- Code-running checks (e.g. Godot), audio marking (e.g. piano) and camera capture
- Re-mark requests, a spaced-repetition review deck, and PDF archives (archives are Markdown for now)

## Contributing

Ideas, bug reports and fixes are welcome. [CONTRIBUTING.md](CONTRIBUTING.md) explains how to report something and
how to run the app and its checks.

## Licence

MIT. See `LICENSE`, `DISCLAIMER.md` for the terms of use, and `THIRD_PARTY.md` for the libraries the app uses.

SelfSemester is an independent project. It isn't made by, affiliated with or endorsed by Anthropic. Claude and
Claude Code are trademarks of Anthropic.

## About this project

Written by the SelfSemester authors with help from Claude.
