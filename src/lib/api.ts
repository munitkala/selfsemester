import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type JobSummary = {
  queued: number;
  running: number;
  waiting_usage: number;
  failed: number;
  done: number;
  next_attempt_at: string | null;
  running_kind: string | null;
};

export type Progress = { stage: string; total_secs: number; remaining_secs: number; as_of: string; paused: boolean };

export type ClaudeStatus = {
  installed: boolean;
  path: string | null;
  version: string | null;
  logged_in: boolean;
  auth_method: string | null;
  on_subscription: boolean;
  problem: string | null;
};

export type AppStatus = { claude: ClaudeStatus; data_dir: string; user_dir: string; schema_version: number; jobs: JobSummary };

export type CourseMeta = {
  code: string;
  term: string;
  instructor: string;
  color: string;
  welcome: string;
  outcomes: string[];
  objectives: Record<string, string[]>;
};

export type CourseCard = {
  id: string;
  title: string;
  status: string;
  error: string | null;
  mode: string;
  grade: number | null;
  final_percent: number | null;
  items_total: number;
  items_settled: number;
  next_due: string | null;
  origin: "claude" | "manual";
  strictness: Strictness;
  meta: CourseMeta;
};

export type ItemRow = {
  id: string;
  course_id: string;
  course_title: string;
  unit_title: string | null;
  kind: string;
  title: string;
  module: string;
  weight: number;
  release_at: string;
  due_at: string | null;
  state: string;
  percent: number | null;
};

export type IntakeQuestion = { id: string; text: string; kind: "text" | "choice" | "number" | "date"; options: string[] };

export type ReadinessStep = { step: string; band: string; percent: number; note: string };
export type Plan = {
  title: string;
  summary: string;
  bundle: { detected: boolean; parts: string[]; recommendation: string };
  mode: string;
  dependencies: string[];
  pacing: { total_weeks: number; hours_per_week: number; fits_user_hours: boolean; note: string };
  units: { key: string; title: string; week: number; topics: string[]; exam: string }[];
  weights: Record<string, number>;
  readiness: {
    delivery: ReadinessStep[];
    marking: ReadinessStep[];
    overall_delivery: string;
    overall_marking: string;
    weak_spots: string[];
    basis: string;
  };
  domain_family: string;
  unsupported_inputs: string[];
};

export type LatePolicy = { kind: "lock_zero" } | { kind: "penalty"; per_day: number; max_days: number };
export type Strictness = "structured" | "sandbox";

export type CourseView = {
  id: string;
  title: string;
  subject: string;
  status: string;
  error: string | null;
  mode: string;
  late_policy: LatePolicy;
  on_transcript: boolean;
  questions: { questions: IntakeQuestion[] } | null;
  answers: Record<string, string> | null;
  plan: Plan | null;
  bundle_choice: string | null;
  starts_at: string | null;
  ends_at: string | null;
  final_percent: number | null;
  grades: { current: number | null; decided: number; complete: boolean };
  jobs: JobSummary;
  units: { key: string; title: string; week: number; state: string; warnings: string[] | null; items: ItemRow[] }[];
  sources: { url: string; title: string | null; licence: string | null; attribution: string | null; free: boolean }[];
  archive_dir: string | null;
  progress: Progress | null;
  strictness: Strictness;
  origin: "claude" | "manual";
  meta: CourseMeta;
};

export type Material = { id: string; kind: "file" | "image" | "note"; name: string; path: string; bytes: number; added_at: string };

export type Question = { id: string; prompt: string; type: "mcq" | "numeric" | "short" | "long"; options: string[]; points: number };
export type RubricLine = { criterion: string; points: number; description: string };
export type MarkLine = { criterion: string; awarded: number; possible: number; comment: string };

export type ItemView = {
  item: ItemRow;
  points: number;
  late_policy: LatePolicy;
  lesson: string | null;
  content: { title: string; kind: string; module: string; instructions: string; questions: Question[]; rubric: RubricLine[] } | null;
  submission: {
    id: string;
    submitted_at: string;
    text_answer: string | null;
    answers: Record<string, string> | null;
    files: string[] | null;
    late_days: number;
    instant: { lines: MarkLine[]; awarded: number; possible: number } | null;
  } | null;
  mark: {
    raw_percent: number;
    percent: number;
    late_penalty: number;
    breakdown: { lines?: MarkLine[]; overall_comment?: string; revise?: string[] };
    marker: string;
    marked_at: string;
  } | null;
  answer_key: { question_id: string; answer: string; explanation: string }[] | null;
  marking_status: string | null;
  marking_progress: Progress | null;
  strictness: Strictness;
  origin: "claude" | "manual";
  attempts: number;
  key_revealed: boolean;
  can_resubmit: boolean;
  self_mark: { criterion: string; points: number; auto: boolean }[] | null;
  review: ReviewThread | null;
};

export type ReviewThread = {
  messages: { id: number; role: "learner" | "claude" | "note"; text: string; created_at: string }[];
  pending: string | null;
  next_attempt_at: string | null;
};

export type GenItem = {
  key: string;
  kind: string;
  title: string;
  module: string;
  instructions: string;
  questions: Question[];
  rubric: RubricLine[];
  answer_key: { question_id: string; answer: string; explanation: string; tolerance: number }[];
  due_days: number;
};

export type DraftUnit = {
  title: string;
  week: number;
  topics: string[];
  objectives: string[];
  lesson_title: string;
  lesson: string;
  est_minutes: number;
  items: GenItem[];
};
export type Draft = {
  title: string;
  subject: string;
  summary: string;
  weights: Record<"homework" | "assignment" | "quiz" | "test" | "exam" | "project", number>;
  units: DraftUnit[];
  meta: CourseMeta;
};

export type Today = {
  overdue: ItemRow[];
  due_soon: ItemRow[];
  open: ItemRow[];
  new_lessons: ItemRow[];
  awaiting_marks: ItemRow[];
  recently_marked: ItemRow[];
  upcoming: ItemRow[];
};

export type Transcript = {
  entries: { course_id: string; title: string; started_on: string; finished_on: string; final_percent: number; letter: string; gpa: number; sandbox: boolean }[];
  average: number | null;
  gpa: number | null;
};

export const api = {
  status: () => invoke<AppStatus>("app_status"),
  jobs: () => invoke<JobSummary>("jobs_status"),
  resume: (courseId?: string) => invoke<number>("resume_now", { courseId: courseId ?? null }),
  noticeAccepted: () => invoke<boolean>("notice_accepted"),
  acceptNotice: () => invoke<void>("accept_notice"),
  test: () => invoke<{ job: { status: string; error: string | null; next_attempt_at: string | null }; result: { ok: boolean; message: string } | null }>("run_test_stage"),
  courses: () => invoke<CourseCard[]>("list_courses"),
  course: (id: string) => invoke<CourseView>("get_course", { id }),
  create: (subject: string, notes: string) => invoke<string>("create_course", { subject, notes }),
  answerIntake: (id: string, answers: Record<string, string>) => invoke<void>("answer_intake", { id, answers }),
  chooseBundle: (id: string, choice: string) => invoke<void>("choose_bundle", { id, choice }),
  approve: (id: string, startDate: string, latePolicy: LatePolicy, strictness: Strictness, onTranscript: boolean) =>
    invoke<void>("approve_course", { id, startDate, latePolicy, strictness, onTranscript }),
  retry: (id: string) => invoke<number>("retry_course", { id }),
  remove: (id: string) => invoke<void>("delete_course", { id }),
  materials: (courseId: string) => invoke<Material[]>("list_materials", { courseId }),
  addMaterials: (courseId: string, files: string[]) => invoke<number>("add_materials", { courseId, files }),
  addNote: (courseId: string, title: string, text: string) => invoke<void>("add_material_note", { courseId, title, text }),
  removeMaterial: (id: string) => invoke<void>("remove_material", { id }),
  openMaterial: (id: string) => invoke<void>("open_material", { id }),
  today: () => invoke<Today>("today"),
  item: (id: string) => invoke<ItemView>("get_item", { id }),
  markRead: (id: string) => invoke<void>("mark_lesson_read", { id }),
  submit: (id: string, textAnswer: string, answers: Record<string, string>, files: string[]) =>
    invoke<{ submission_id: string; status: string; late_days: number }>("submit_item", { id, textAnswer, answers, files }),
  selfMark: (id: string, awarded: number[], comment: string) => invoke<void>("self_mark_item", { id, awarded, comment }),
  revealKey: (id: string) => invoke<void>("reveal_answer_key", { id }),
  sendReview: (id: string, text: string) => invoke<void>("send_review_message", { id, text }),
  builderCreate: (title: string) => invoke<string>("builder_create", { title }),
  builderGet: (id: string) => invoke<{ draft: Draft; problems: string[] }>("builder_get", { id }),
  builderSave: (id: string, draft: Draft) => invoke<string[]>("builder_save", { id, draft }),
  builderPublish: (id: string, startDate: string, latePolicy: LatePolicy, strictness: Strictness, onTranscript: boolean) =>
    invoke<void>("builder_publish", { id, startDate, latePolicy, strictness, onTranscript }),
  builderExport: (id: string) => invoke<string>("builder_export", { id }),
  builderImport: (folder: string) => invoke<string>("builder_import", { folder }),
  transcript: () => invoke<Transcript>("get_transcript"),
  openFolder: (path: string) => invoke<void>("open_folder", { path }),
};

/** Re-run `load` now and whenever the backend says data changed. Returns a cleanup function. */
export function live(load: () => void): () => void {
  load();
  const un = listen("data-changed", () => load());
  return () => {
    un.then((f) => f());
  };
}
