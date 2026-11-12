# SelfSemester screen by screen

Every picture here is SelfSemester in a normal-sized window (1280 × 800, shown at twice the resolution so text stays
sharp). The courses and marks in them are made up for the pictures, except the example lessons, which were
written by the app's own lesson prompt (see [Example lessons](#example-lessons)). Click any picture to see it full
size. The short version of all this is in the [README](../README.md).

- [Example lessons](#example-lessons)
- [Every day](#every-day)
- [Inside a course](#inside-a-course)
- [Feedback and marks](#feedback-and-marks)
- [Planning and building a course](#planning-and-building-a-course)
- [Your record and settings](#your-record-and-settings)
- [Fits any window](#fits-any-window)
- [Themes](#themes)

## Example lessons

These lessons were written by the app's own lesson prompt through Claude Code, then checked by the same second
model that checks every real course, and passed with no major problems. Nothing in them was edited by hand. They
were made with the `live_sample_unit` test (see the [README](../README.md#development)), and more subjects will be
added here.

### Calculus I: Derivatives and tangent lines

| | |
| --- | --- |
| ![Calculus lesson with secant lines closing in on the tangent to y = x squared](../assets/screenshots/samples/calculus.png) | ![Calculus lesson with a cubic, its horizontal tangents and its derivative](../assets/screenshots/samples/calculus-derivative.png) |

A first-year lesson of about 2,000 words with four plots, plus one more inside the homework. Figure 1 shows secant
lines through $(1, 1)$ on $y = x^2$ turning into the tangent as the gap shrinks, with each slope in the legend. Later
figures plot $f(x) = \tfrac13 x^3 - x$ with its horizontal tangents, then its derivative $f'(x) = x^2 - 1$
directly underneath, so you can see the derivative cross zero exactly where the tangent is flat. A last plot shows
the corner of $y = |x|$, where no tangent exists. Every curve is drawn by the app from its equation, so it can't
disagree with the maths, and the text refers to each figure by its number.

### Introductory Statistics: The normal distribution

| | |
| --- | --- |
| ![Statistics lesson with the 68-95-99.7 regions shaded under the normal curve](../assets/screenshots/samples/statistics.png) | ![The same lesson in the Midnight theme, with the area to the left of z = 1.5 shaded](../assets/screenshots/samples/statistics-dark.png) |

Three plots: normal curves with different means and spreads, the regions within one, two and three standard
deviations shaded (the 68-95-99.7 rule), and the area to the left of $z = 1.5$, which the caption gives as
$\Phi(1.5) \approx 0.9332$. The second picture is the same lesson in the Midnight theme: plots follow the theme's
colours, so they read the same in light and dark.

### Introductory Mechanics: Newton's laws and forces on a slope

![Physics lesson plotting friction against applied force, rising to the static limit and dropping to the kinetic value](../assets/screenshots/samples/physics-friction.png)

This lesson has a diagram of a block on a slope, with its weight split into parts along and across the slope. It
also has the plot shown here: friction against applied force for a block on a flat surface. Static friction
matches the push up to its limit of 5 N, then the block slides and friction drops to a steady 3 N. The caption gives
both values, and the text after the figure says what to notice in it.

### Algorithms and Data Structures: Binary search and logarithmic time

| | |
| --- | --- |
| ![Algorithms lesson tracing binary search for 23 across three steps of a sorted array](../assets/screenshots/samples/algorithms.png) | ![Algorithms lesson plotting n against log base 2 of n for n from 1 to 64](../assets/screenshots/samples/algorithms-growth.png) |

Figure 1 is a diagram that traces a search for 23 in a sorted array of ten numbers. Each row shows `lo`, `hi` and
the probe, and the range left to search halves each step until 23 is found at index 5 on the third probe. Figure 2
plots $n$ against $\log_2 n$ for $n$ up to 64, so you can see why doubling the data only adds one probe. The
homework asks you to trace searches by hand, count probes and find the bug in a broken version.

### How figures work

Lessons can contain two kinds of figure. **Plots** are written as the maths (a function, data points, vectors or a
shaded area) and drawn by the app, so graphs are exact. **Diagrams** are original drawings made for the lesson,
for things like forces on a slope, a molecule's shape, a flowchart or a simple outline of an app's window; anything
active is removed before they're shown. Every figure gets a number and a caption, and anything adapted from an
openly licensed source gets a credit line. Claude is told never to copy figures, screenshots, photos or logos; for a
copyrighted figure it draws its own version or says where to find the original. The checking model compares every
figure with the maths and checks its caption and numbering.

## Every day

### Today

![Today page with a greeting, the daily goal ring, streak and what's due](../assets/screenshots/04-today.png)

The page SelfSemester opens on. The header greets you and shows three numbers: study time today against your daily
goal (the ring fills as you go), your streak of days with some study or handed-in work, and your average across
active courses. Below it, sections tile into columns: work due this week, lessons released and not yet read, work
waiting for marks, and work marked this week with its score. Each row shows the course, the deadline and how long is
left. When there's nothing due it says so, and when you reach your goal it tells you.

### Insights

![Insights page with streak, study time, notes and an activity calendar](../assets/screenshots/15-insights.png)

How you've been studying. Four tiles give your streak and best streak, study time in the chosen range (14, 30 or 90
days), your average mark and how much work was on time. **What stands out** lists only things the numbers support,
such as the hour you study most or how this week compares with the same days last week. The activity calendar
shades each day by study time and handed-in work, over 6 or 12 months. Further down: study time per day with your
goal as a dashed line, marks over time with a running average, averages by type of work, time per course, the
hours and weekdays you study, and how much graded work is due in each of the next six weeks. Study time counts only
while a lesson or work page is in front and in use, and stays on your computer; it can be turned off or cleared in
Settings.

### Courses

![Courses page with a card for each course](../assets/screenshots/09-courses.png)

A card for each course with its code, term, instructor, banner colour, progress through the graded work, current
mark and next deadline. Finished courses move to their own list underneath. **New course** starts one with Claude;
**Write a course** opens the course builder.

### Quick search (Ctrl+K)

![Quick search box over the Today page](../assets/screenshots/20-palette.png)

Press Ctrl+K anywhere to jump to a page or a course by typing part of its name, or to switch light and dark, change
the effects level or collapse the sidebar. Arrow keys and Enter work as you'd expect, and Esc closes it.

## Inside a course

### Course home

![Course home with a welcome, what's coming up and recent marks](../assets/screenshots/05-course.png)

Every course has the same classroom layout. The banner shows the course code, term, instructor, dates and your
current mark. Home has the welcome message, how the course works (length, release times, the late policy and how
work is marked), what's up next, your progress through each week, the deadlines coming up, and recent marks.

### Modules

![Modules tab with each week's objectives and work, finished items ticked](../assets/screenshots/10-modules.png)

Each week has the same shape: what you'll be able to do by the end of it, the lesson, then the work, with each
item's share of the final grade and its status. Finished items get a tick, and locked items show when they open.
In a wide window the weeks sit two across.

### Syllabus

![Syllabus tab with the description, outcomes and schedule](../assets/screenshots/11-syllabus.png)

The course description, instructor, term and dates, learning outcomes, the week-by-week schedule with every
deadline, how the grade is made up, and the course policies (late work, attempts, academic honesty).

### Grades

![Grades tab with a grade ring and a bar for each category](../assets/screenshots/12-grades.png)

Your mark as a ring with its letter grade, how much of the final grade is already decided, a bar for each category
(homework, quizzes, tests and so on) with its weight and average, and a table of every graded item.

### A lesson

![Lesson with headings, maths and a worked example table](../assets/screenshots/06-lesson.png)

Lessons are full self-study chapters: what you'll learn, explanations, worked examples, figures where the subject
needs them, common mistakes and practice. The header shows the type of item, the week and an estimated reading
time, and a thin bar at the top fills as you read. **Reading mode** hides everything but the text. Maths is
typeset with LaTeX.

### An assignment

![Assignment with multiple-choice options as cards](../assets/screenshots/07-assignment.png)

The deadline, points and share of the final grade sit at the top. Multiple-choice options are cards you click, and
numeric answers have their own boxes. Written answers take Markdown and LaTeX with a preview, and you can add photos
or PDFs of handwritten working. Submitting asks you to confirm, because Structured courses allow one attempt.

## Feedback and marks

### A marked result

![Result with a score ring, a bar per rubric line and a revise checklist](../assets/screenshots/08-result.png)

The mark as a ring with the letter grade and a few words about it, who marked it and when, then one row per rubric
line with a bar for the points and the marker's comment. An overall comment and a short **To revise** checklist
follow. Late penalties show as their own tag, and the answer key opens underneath.

### Instant feedback

![Instant feedback with ticks and crosses, and a waiting-for-marks card](../assets/screenshots/16-instant-feedback.png)

Multiple-choice and numeric answers are checked the moment you submit, each with a tick or a cross and the right
answer where you missed it, while Claude marks the written parts. The card below confirms your submission time is
saved, so marking can never make work late.

### Review with Claude

![Review thread as a conversation about a mark](../assets/screenshots/17-review.png)

Every marked item has a thread for asking why marks were given or lost. Claude sees the question, your work, the
rubric and the mark, and explains; it can't change the mark. While the answer key is still hidden (in Sandbox
courses), it guides you instead of giving answers away.

### Sandbox mode

![Sandbox result with Try again and Show the answer key](../assets/screenshots/14-sandbox.png)

Sandbox courses let you try again as often as you like with no late penalties; your latest attempt counts. The
answer key stays hidden until you choose to see it, which closes that item.

## Planning and building a course

### Plan review

![Plan review with the syllabus and how well Claude can teach and mark it](../assets/screenshots/02-plan-review.png)

Before anything is built, Claude shows the plan: a summary, what you need to know first, pacing, the week-by-week
syllabus and how the grade is weighted, plus a readiness report with honest estimates of how well it can teach and
mark each part of the subject. You approve it, choose a start date, late policy and Structured or Sandbox mode, or
ask for changes.

### Building

![Building a course, unit by unit](../assets/screenshots/03-building.png)

Each unit is written and then checked by a second model, which re-derives answers, checks figures against the maths
and sends problems back for a rewrite. The page shows which units are ready, being checked or waiting, and how long
is left. You can close this page; it carries on in the background.

### Course builder

![Course builder asking only for a name, with course details and grading folded away as optional](../assets/screenshots/13-builder.png)

Write a course yourself, with no Claude involved. It asks only for what a course needs to start: a name and a first
lesson. Course details (code, term, instructor, colour, welcome message, outcomes) and grading are folded away as
optional, and so are extra weeks and graded work. A live card shows how the course will look. Multiple-choice and
numeric questions are marked by the app and you mark written answers against your own key.

### Your own documents

![A week in the course builder with a PDF and slides attached as the lesson](../assets/screenshots/23-builder-documents.png)

Already have the lesson as a PDF, a Word file or slides? Attach it instead of typing, to the lesson or to any piece
of work (a worksheet, a past paper). Text and Markdown files can be turned into the lesson text in one click. Files
are copied into the course, so the originals can move. When you export a course, they're laid out by week:
`01 Week name\lesson files\` and `01 Week name\work files\01 Homework title\`, and importing brings them back.

### Files in a lesson

![A lesson with its slides and worked examples listed above the text](../assets/screenshots/25-lesson-files.png)

Learners see a lesson's documents above it, and a piece of work's documents with its instructions. Each opens in
the learner's usual app.

### Add to a course while it runs

![The Modules tab with Add to this course: a new week, homework, quiz, assignment, test, exam or project](../assets/screenshots/24-add-to-course.png)

A course you wrote doesn't have to be finished before it starts. Its Modules tab has **Add to this course**: a new
week with its lesson, or homework, a quiz, an assignment, a test, an exam or a project in any week. New work opens
with its week, or straight away if the week has started. Grade shares are recalculated, and nothing already
released or marked changes.

## Your record and settings

### Transcript

![Transcript with summary tiles and coloured grades](../assets/screenshots/18-transcript.png)

Completed courses with their dates, final mark, letter grade and GPA points, plus your cumulative average and GPA.
Sandbox courses are labelled. It can be printed or saved as a PDF. It records your own study and isn't a
qualification.

### Settings

![Settings with mode, light and dark themes and accent colours](../assets/screenshots/19-settings.png)

Short patch notes for each version come first. Then appearance (mode, a light and a dark theme, accent colour, text
size, density, corners), layout (sidebar, page width, start page), motion and performance (visual effects level and
animation), study time and goals (tracking, daily goal, week start, calendar range, clearing history), shortcuts,
Claude Code status and a test call, and your data and backups. On a wide window the section list sits on the left.

### First launch

![First-launch terms](../assets/screenshots/01-terms.png)

The key points of the terms, which everyone accepts once before using the app: no warranty, not professional
advice, not a qualification, personal study only, where your data goes, academic integrity and backups. The full
text is in [DISCLAIMER.md](../DISCLAIMER.md).

## Fits any window

| Small window | Wide window |
| --- | --- |
| ![Today in a small window, with an icon sidebar and stacked sections](../assets/screenshots/21-small-window.png) | ![Insights in a wide window, with panels side by side](../assets/screenshots/22-wide-window.png) |

Panels rearrange as you resize. In a small window the sidebar shrinks to icons and sections stack in one column; in
a wide one they spread out side by side. Lessons and forms stay at a comfortable reading width in the middle. The
fixed widths of earlier versions are still available under Settings, Layout, Page width.

## Themes

Choose light, dark or follow Windows, then a theme for each and any accent colour. Every theme was checked for WCAG
AA contrast; dark themes use dark grey rather than pure black, and Paper is a sepia theme for long reading.

| | | |
| --- | --- | --- |
| ![Classic light theme](../assets/screenshots/themes/classic-light.png) | ![Classic dark theme](../assets/screenshots/themes/classic-dark.png) | ![Light theme with a pink accent](../assets/screenshots/themes/pink.png) |
| **Classic** | **Classic dark** | **Pink** (rose accent) |
| ![Slate dark theme in blue](../assets/screenshots/themes/blue.png) | ![Midnight dark theme](../assets/screenshots/themes/midnight.png) | ![High contrast dark theme, white on black](../assets/screenshots/themes/black.png) |
| **Blue** (Slate) | **Midnight** | **Black** (high contrast) |
| ![Paper sepia theme](../assets/screenshots/themes/paper.png) | ![Forest dark theme](../assets/screenshots/themes/forest.png) | ![Mint light theme](../assets/screenshots/themes/mint.png) |
| **Paper** (sepia) | **Forest** | **Mint** |
