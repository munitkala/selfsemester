# Changelog

What changed in each version of SelfSemester, newest first. Changes that haven't been given a version number yet
are listed under **Unreleased**.

## Unreleased

The database moves to schema version 6, which adds a table for study time. Your data is backed up before the
upgrade, as with every new schema.

### Themes and display options

**Why:** the app looked the same for everyone and only followed Windows' light or dark setting.

- Settings has a new **Appearance** section. Pick Light, Dark or System, then a light theme (Classic, Paper,
  Mint, High contrast) and a dark theme (Classic, Slate, Midnight, Forest, High contrast) separately, so System
  mode switches between the two you like.
- Every theme was checked against WCAG AA contrast on its own colours. Dark themes use dark grey instead of pure
  black and softer accents, which is easier on the eyes for long sessions. Paper is a sepia theme for reading.
- Choose an accent colour (blue, teal, green, violet, rose, coral, amber, or the theme's own), text size, density
  (compact, comfortable, spacious) and corner style.
- **Layout** options: an icon-only sidebar (also the arrow at the bottom of the sidebar), wider pages for big
  monitors, and which page the app opens on.
- The chosen theme is applied before the window draws, so a dark theme no longer flashes white on start.

### Motion and performance

**Why:** extra visual effects shouldn't slow down an older laptop or drain its battery.

- **Visual effects** has three levels. Full adds blur behind dialogs and a slowly moving glow on Today, which
  costs the most graphics work. Balanced (the default) keeps soft shadows and small animations but no blur.
  Minimal is flat and still.
- **Animation** follows Windows' animation setting, or can be turned on or off.
- Pages reload less: a burst of background changes (a unit being written, marks arriving) now causes one reload
  instead of one per change, and a minimised window waits until it's shown again before reloading.
- Long sections of the Insights page skip drawing until you scroll to them.

### Insights

**Why:** to see how you're actually studying, not just your marks.

- A new **Insights** page shows your streak, study time, average mark and on-time rate, a calendar of activity
  for the last 6 or 12 months, study time per day against your goal, marks over time with a running average,
  averages by type of work, time per course, when in the day and week you study, and how much graded work is due
  in each of the next six weeks.
- A short **What stands out** list points out things the numbers back up, such as your busiest day, whether this
  week is ahead of last week, or which type of work is lowest.
- Study time is counted on lesson and work pages while the window is in front and you've used the mouse or
  keyboard in the last two minutes, so a page left open doesn't count. It stays on this computer. Turn it off,
  set a daily goal, or clear the history under **Study time and goals** in Settings.

### Today

- Today opens with a greeting, today's study time against your goal, your streak and your current average. When
  you reach your goal it says so, with a small flourish you can turn off.

### Getting around

- Press **Ctrl+K** anywhere to jump to a page or course, switch light and dark, or change the effects level. It
  can be turned off under Shortcuts.
- The sidebar has icons and an Insights link, and shows what Claude is doing as a small status chip.

### Course pages and feedback

**Why:** marks and feedback are the part of a course you look at most, and they were a plain table.

- A marked item opens with a score ring, the letter grade and a few words about it, then one row per rubric line
  with a bar for the points and Claude's comment underneath. **To revise** is a short checklist, and a late
  penalty shows as its own tag.
- Instant feedback marks each answer with a tick or a cross and shows the running score.
- While work waits to be marked, a clear card says it's submitted and that the time is already recorded.
- Multiple-choice answers are larger cards that show which one you picked, and submitting asks for a clear
  confirmation.
- Lessons show an estimated reading time and a thin progress bar at the top as you read.
- The answer key labels questions "Question 1", "Question 2" like the rest of the page.
- The review thread is a conversation, with your messages on one side and Claude's on the other.
- The classroom tabs have icons, Home shows progress as a ring, and Grades shows your mark as a ring with a bar
  per category. Finished items in Modules get a tick.
- The transcript has summary tiles (courses, average, GPA) and coloured grades. New course shows the four steps
  ahead, and the course builder lists drafts as course cards.
- While Claude plans or builds a course, the page shows that it's working and how many units are done.
- Fixed: some numbered badges in the course builder used white text on a light accent in dark themes.
- The Insights summary tiles stay on one row at the app's normal window size.
- New screenshots in the README for every screen and nine of the themes.

### What's new

- Settings opens with short patch notes for each version, with a link to this changelog for the details.

## 0.4.1 (2026-11-09)

### Security updates

**Why:** GitHub's Dependabot flagged five advisories in the app's dependencies.

- Updated SvelteKit from 2.70 to 3.1 and the static adapter from 3.0 to 4.0. This fixes two SvelteKit advisories
  (a redirect through enhanced form submission, and script injection through a cache-control header in prerendered
  pages) and one in `cookie`, which SvelteKit 2 depended on. None of them could be triggered in SelfSemester, which
  has no server, forms that post to one, or prerendered pages, but the fixed versions are what ships now.
- Moving to SvelteKit 3 meant a few code changes: its settings moved from `svelte.config.js` into
  `vite.config.js`, `$lib` imports became `#lib` (set up in `package.json` under `imports`), `tsconfig.json`
  extends `$app/tsconfig`, and switching tabs in a classroom uses the new navigation options. Nothing you see in
  the app changed.
- Building from source now needs Node.js 22.17 or newer (SvelteKit 3's minimum). The installer is unaffected.
- Two advisories have no fix to install and don't affect SelfSemester:
  - `glib` (Rust) is part of Tauri's Linux window code. It isn't built into the Windows app at all, and a fix needs a
    newer GTK version than Tauri 2 uses.
  - `deepmerge` (npm) is used by the Svelte build plugin to merge its own settings while building. It never runs
    in the app and never sees user input, and no patched version has been published.

## 0.4.0 (2026-11-08)

### Your work is kept across updates

**Why:** installing a new version should never cost you a course, a mark or a file you added.

- Installing a new version replaces only the program. Your courses, materials, submissions, marks and transcript
  stay where they are (`%LOCALAPPDATA%\SelfSemester` and `Documents\SelfSemester`), and so does a draft in the course
  builder. Uninstalling leaves them too.
- Before a new version opens your data for the first time (and upgrades the database), it now copies the database
  and every course's files to `Documents\SelfSemester\Backups`, outside the program folder. The last 5 backups are
  kept.
- If SelfSemester starts and finds your data missing but a backup exists, it restores the newest one by itself and
  says so.
- **Settings → Your data and backups** lists the backups, shows what happened at start-up, and has a
  **Back up now** button.

### A ready-made installer

- Each release now has a Windows installer to download, built by GitHub Actions from the tagged source
  (`.github/workflows/release.yml`), with a SHA-256 checksum next to it. The README explains downloading it as well
  as building it yourself.

### About the version number

This is 0.4.0, not 1.0.0. Having an installer doesn't make the app finished: 1.0.0 will mean it has had real use,
its data format is settled and updates from then on keep working without surprises.

## 0.3.0 (2026-11-08)

The database moves to schema version 5. Existing courses keep everything and get a course code and colour picked
for them.

### A classroom for every course

**Why:** a course should feel like being enrolled somewhere: one place that says where to start, what's due, how
you're doing and what the rules are. The layout follows common guidance for online courses (a clear "start here",
learning outcomes, a syllabus, the same structure every week, progress per module, deadlines first, readable and
keyboard-friendly), not any particular product's design.

- Every running course opens on a classroom with a coloured banner (course code, term, instructor, dates, current
  mark) and the same five tabs:
  - **Home**: a welcome and how the course works, what to do next, a progress bar for each week, what's coming up
    over the next three weeks, and recent marks.
  - **Modules**: each week with its objectives, then its lesson and work, each with a clear status (to read, due
    soon, overdue, being marked, the mark).
  - **Syllabus**: the course description, learning outcomes, a schedule, how grades are weighted, the letter scale,
    and the policies (deadlines, attempts, marking, honesty, accessibility).
  - **Grades**: the current mark, how much of the grade is decided, averages by category and every graded item.
  - **Materials**: your materials, the reading list and every lesson.
- Lessons and work show which course they belong to, link back to its tabs, and have **Previous** and **Next**
  buttons that follow the course order.
- **Courses** is now a dashboard of course cards, with finished courses listed separately.

### Course builder: the details of a real course

- The builder is laid out in four steps (course details, grading, weeks and work, publish), with an outline that
  ticks each step off and a live preview of the course card.
- New course details: course code, term, instructor, banner colour, a welcome message and learning outcomes. Each
  week gets objectives ("by the end of this week you can..."). They show up on the classroom pages and travel with
  folder export and import.
- Grade weights are entered as percentages, with the share each kind will really get once unused kinds drop out.

### Smaller things

- Marks and instant feedback name lines "Question 1", "Question 2" instead of the question's internal id.
- The terms (DISCLAIMER.md and the first-launch notice) now cover courses you write yourself: they're your own
  content, and only material you have the right to use should go into them. Because the terms changed, the notice is
  shown once more.
- New screenshots in the README.

## 0.2.0 (2026-11-08)

The database moves to schema version 4. Existing courses are kept as they are and run in Structured mode, which is
how every course worked before.

### Your own materials

**Why:** to give Claude your textbook, past notes or what your teacher wants, so the course is built around them.

- Each course page has a **Your materials** section. Add PDFs, images and text files (notes, data, code), up to
  30 MB each, or write a note in the app. They're copied into the course folder, so the originals can be moved or
  deleted.
- Claude reads them when it plans the course, writes each unit and marks your work. Anything added after a unit is
  written is used for marking only.

### Structured and Sandbox modes

**Why:** some courses should feel like the real thing; others are for practice, where trying again matters more
than the first mark.

- Each course picks a mode when you enrol (or publish one you wrote yourself):
  - **Structured**: one attempt per assessment, marked strictly, and the late policy applies.
  - **Sandbox**: resubmit as often as you like, with no late penalties. Your latest attempt counts. The answer key
    stays hidden until you choose to see it, which closes that item. Late work is taken until a week after the
    course's last deadline. Claude's marking points at what to rethink rather than giving the answers away.
- Sandbox courses are marked as such on the transcript.

### Instant feedback

**Why:** waiting for Claude's marks shouldn't mean waiting to find out whether the quick answers were right.

- When you submit, the multiple-choice and numeric questions are checked straight away and shown as right or
  wrong while Claude marks the rest against the rubric.

### Review mode

**Why:** a mark and a comment don't always explain enough.

- Every marked item has a **Review with Claude** thread. Ask about the questions, the feedback or why marks were
  given or lost; Claude answers with the item, your work, the rubric and the mark in front of it. It can't change a
  mark. In a Sandbox course whose key is still hidden, it guides rather than giving answers.
- Each message uses a little of your plan, and goes through the same queue as everything else, so a usage limit
  pauses it rather than losing it. A reply that fails says so in the thread.

### Course builder (no Claude)

**Why:** to run a course you write yourself, such as one a teacher set or one Claude can't do well, with the same
schedule, deadlines and transcript.

- **Course builder** in the sidebar: write the syllabus, units, lessons (Markdown and LaTeX), graded work,
  questions (multiple choice, number, short or long answer), answer keys and rubrics. It saves as you type and
  lists anything that stops the course from being published.
- Publishing schedules it like any other course. Multiple-choice and numeric questions are marked by the app;
  written answers are marked by you, line by line, against your own answer key. Claude is never called.
- **Export to a folder** writes the draft to Documents\SelfSemester\Course builder as `course.json` plus a
  `lesson.md` and `work.json` for each unit, to edit in any text editor. **Import a folder** reads it back as a new
  draft.

### Fixes

- When two marks or submissions for one item share the same second (quick Sandbox retries), the newest one is now
  always the one shown and counted.

## 0.1.0 (2026-11-04)

The first version.

- Name a subject; Claude asks a few questions, plans a syllabus with a readiness report (how well it can teach and
  mark the subject, step by step), and writes the whole course once you approve: lessons, homework, quizzes,
  assignments, tests, exams, rubrics and sealed answer keys.
- The app runs the course on its own schedule: releases, deadlines with a late policy, reminders, auto-marked
  quizzes, and written or photographed work sent to Claude for marking against the rubric.
- Grades, a transcript, and an archive of each finished course in Documents.
- Runs on your own Claude Code and subscription, with no API key. Every Claude call is a queued job that pauses at a
  usage limit and carries on later, with a **Resume now** button and progress bars.
- A first-launch notice with the terms of use, and a warning for high-stakes subjects.
