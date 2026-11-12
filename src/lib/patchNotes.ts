// Short patch notes for Settings → What's new. The full story is in CHANGELOG.md; keep these to a few quick lines
// per version, newest first, and add one whenever the changelog gets a new section.

export type PatchNote = { version: string; date: string; title: string; notes: string[] };

export const patchNotes: PatchNote[] = [
  {
    version: "0.5.0",
    date: "2026-11-11",
    title: "A fresh look",
    notes: [
      "Figures in lessons: exact plots, original diagrams, numbered captions and credits.",
      "Themes: light, dark or system, with sepia, high contrast and more, plus any accent colour.",
      "Text size, density, corners, sidebar and page width options.",
      "Visual effects and animation you can turn down on slower computers.",
      "New Insights page: streaks, study time, activity calendar and mark trends.",
      "Study time counts on lesson and work pages, with a daily goal on Today.",
      "Ctrl+K to jump to any page or course.",
      "Clearer results: a score ring, a bar per rubric line and a revise checklist.",
      "Course pages, lessons, reviews and the transcript in the new style.",
      "Panels rearrange as you resize the window, and the sidebar shrinks to icons when it's small.",
    ],
  },
  {
    version: "0.4.1",
    date: "2026-11-09",
    title: "Security updates",
    notes: ["Newer SvelteKit and its static adapter, clearing the advisories GitHub flagged.", "Nothing changes in the app itself."],
  },
  {
    version: "0.4.0",
    date: "2026-11-08",
    title: "Your work is kept across updates",
    notes: [
      "Updates replace only the program; courses, marks and files stay put.",
      "A backup is made before each new version opens your data.",
      "Missing data is restored from the newest backup automatically.",
      "A ready-made Windows installer with each release.",
    ],
  },
  {
    version: "0.3.0",
    date: "2026-11-08",
    title: "A classroom for every course",
    notes: [
      "Each course has a home page, weekly modules, syllabus, gradebook and materials.",
      "Next and previous buttons on lessons.",
      "Course builder adds course code, term, instructor, colour and outcomes.",
    ],
  },
  {
    version: "0.2.0",
    date: "2026-11-08",
    title: "Modes, feedback and the course builder",
    notes: [
      "Add your own notes, PDFs and images to a course.",
      "Structured and Sandbox modes.",
      "Instant feedback on multiple choice and number questions.",
      "Review a mark with Claude.",
      "Write your own course without Claude.",
    ],
  },
  {
    version: "0.1.0",
    date: "2026-11-04",
    title: "First version",
    notes: [
      "Name a subject and Claude plans and builds the whole course.",
      "Deadlines, reminders, marking, grades and a transcript.",
      "Runs on your own Claude Code and subscription.",
    ],
  },
];
