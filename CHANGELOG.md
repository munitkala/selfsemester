# Changelog

What changed in each version of SelfSemester, newest first. Changes that haven't been given a version number yet
are listed under **Unreleased**.

## Unreleased

Nothing yet.

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
