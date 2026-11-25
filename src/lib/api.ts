import type { Found } from "#lib/audio.ts";
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
  /** build steps held because their course is paused */
  paused: number;
};

export type Progress = { stage: string; total_secs: number; remaining_secs: number; as_of: string; paused: boolean; by_you?: boolean };

export type ClaudeStatus = {
  /** "endpoint" when services do every job (then `path` is the address and `version` the model), "mixed" when some do */
  provider: "claude_code" | "endpoint" | "mixed";
  installed: boolean;
  path: string | null;
  version: string | null;
  logged_in: boolean;
  auth_method: string | null;
  on_subscription: boolean;
  problem: string | null;
};

export type Startup =
  | { kind: "fresh" }
  | { kind: "unchanged" }
  | { kind: "backed_up"; folder: string }
  | { kind: "restored"; folder: string; created_at: string };
export type StartupInfo = { version: string; result: Startup | null; problem: string | null };
export type BackupInfo = {
  folder: string;
  path: string;
  bytes: number;
  manifest: { created_at: string; reason: string; from_version: string | null; app_version: string; schema_version: number } | null;
};

export type Tool = {
  id: string;
  name: string;
  category: "built-in" | "common" | "extra";
  summary: string;
  for: string;
  licence: string;
  credit: string;
  installed: boolean;
  /** Download size in bytes (0 for built-ins). */
  size: number;
};
export type ToolsView = { tools: Tool[]; locked: string | null; setup_done: boolean; installing: string[] };

export type EndpointCheck = {
  at: string;
  reached: boolean;
  json_ok: boolean;
  format: string;
  images_ok: boolean | null;
  seconds: number;
  message: string;
};
export type ApiKind = "openai_chat" | "openai_responses" | "anthropic" | "gemini" | "ollama" | "custom" | "command";
/** One AI service the learner set up. */
export type Service = {
  id: string;
  name: string;
  api: ApiKind;
  preset: string;
  base_url: string;
  /** always empty coming back from the app; see ServiceView.key_set */
  api_key: string;
  model: string;
  strong_model: string;
  quick_model: string;
  sees_images: boolean;
  web_search: boolean;
  reads_pdfs: boolean;
  auth_header: string;
  auth_prefix: string;
  auth_query: string;
  headers: string;
  query: string;
  extra_body: string;
  max_output: number | null;
  context: number | null;
  timeout_minutes: number | null;
  custom_path: string;
  custom_body: string;
  custom_text_path: string;
  command: string;
  check: EndpointCheck | null;
};
export type FitLevel = "full" | "reduced" | "none" | "unknown";
export type FitArea = { id: string; label: string; level: FitLevel; note: string };
export type Fit = { overall: "best" | "good" | "fair" | "limited" | "unknown"; headline: string; areas: FitArea[]; cost: string };
export type ServiceView = Service & { key_set: boolean; fit: Fit };
/** Who does one job: "claude_code" (or "") or a service id, and the model (empty for the usual one). */
export type Target = { service: string; model: string };
export type Roles = { plan: Target; write: Target; check: Target; research: Target };
export type AiView = { services: ServiceView[]; roles: Roles; fit: Fit; claude_code_fit: Fit };

export type AppStatus = { startup: StartupInfo; claude: ClaudeStatus; data_dir: string; user_dir: string; schema_version: number; jobs: JobSummary };

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
  /** Tools the lessons would use (tools/catalog.json ids); missing ones are offered at review. */
  tools?: string[];
  /** Standards from the learner's reference the course must meet, with the units that teach each. */
  standards?: { id: string; text: string; units: string[] }[];
  /** Weeks that were planned but not built, because building was ended early with the weeks that were ready. */
  unbuilt?: { key: string; title: string; week: number; topics: string[]; exam: string }[];
};

export type LatePolicy = { kind: "lock_zero" } | { kind: "penalty"; per_day: number; max_days: number };
export type Strictness = "structured" | "sandbox";
export type Model = "haiku" | "sonnet" | "opus" | "fable";
export type Effort = "low" | "medium" | "high" | "xhigh" | "max";
export type BuildLevel = "quick" | "standard" | "thorough" | "max" | "custom";
export type Detail = "brief" | "standard" | "in_depth";
export type BuildContent = { skip: string[]; detail: Detail; media: boolean };
export type BuildSettings = {
  level: BuildLevel;
  writer: Model;
  exam_writer: Model;
  checker: Model;
  /** null leaves it to Claude Code's default */
  effort: Effort | null;
  rewrites: number;
  sources: number;
  content: BuildContent;
  /** Unit writers may search the web and open sources while writing. */
  research: boolean;
};
export type BuildEstimate = { calls_min: number; calls_max: number; usage_vs_standard: number; minutes: number };

export type Readiness = { ready_units: number; ready_weeks: number; total_units: number; total_weeks: number };

/** What carrying on with a course that's being built will still use. */
export type Remaining = {
  level: string;
  units_left: number;
  calls_min: number;
  calls_max: number;
  minutes: number;
  usage_vs_standard: number;
  minutes_per_week_so_far: number | null;
  heavy: boolean;
};

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
  /** building is paused by the learner */
  paused: boolean;
  /** While building: weeks at the start of the course that are ready, so it can be finished with just those. */
  readiness: Readiness | null;
};

export type Material = { id: string; kind: "file" | "image" | "note"; name: string; path: string; bytes: number; added_at: string };

/** A document attached to a lesson or a piece of work in a course written in the course builder. */
export type Attachment = { name: string; file: string; bytes: number };

export type Question = { id: string; prompt: string; type: "mcq" | "numeric" | "short" | "long"; options: string[]; points: number };
export type RubricLine = { criterion: string; points: number; description: string };
export type MarkLine = { criterion: string; awarded: number; possible: number; comment: string };

export type ItemView = {
  item: ItemRow;
  points: number;
  late_policy: LatePolicy;
  lesson: string | null;
  content: { title: string; kind: string; module: string; instructions: string; questions: Question[]; rubric: RubricLine[]; files?: Attachment[] } | null;
  /** Documents attached to a lesson. */
  files: Attachment[];
  /** What the app found for each of the lesson's sound clips, by clip id. Clips not listed are still being filled. */
  audio: Record<string, Found>;
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
  files?: Attachment[];
};

export type DraftUnit = {
  title: string;
  week: number;
  topics: string[];
  objectives: string[];
  lesson_title: string;
  lesson: string;
  lesson_files?: Attachment[];
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

export type InsightsData = {
  study: { day: string; hour: number; course_id: string; seconds: number }[];
  marks: { marked_at: string; percent: number; kind: string; course_id: string; title: string; late_days: number; missed: boolean }[];
  events: { at: string; kind: "submit" | "review"; course_id: string }[];
  due: { due_at: string; kind: string; course_id: string; weight: number }[];
};

export const api = {
  status: () => invoke<AppStatus>("app_status"),
  jobs: () => invoke<JobSummary>("jobs_status"),
  resume: (courseId?: string) => invoke<number>("resume_now", { courseId: courseId ?? null }),
  noticeAccepted: () => invoke<boolean>("notice_accepted"),
  acceptNotice: () => invoke<void>("accept_notice"),
  ai: () => invoke<AiView>("ai_settings"),
  /** id "" adds a new service. key: undefined keeps the saved key, "" removes it */
  saveService: (service: Service, key?: string) => invoke<AiView>("save_service", { service, key: key ?? null }),
  deleteService: (id: string) => invoke<AiView>("delete_service", { id }),
  setRoles: (roles: Roles) => invoke<AiView>("set_roles", { roles }),
  checkService: (id: string, model?: string) => invoke<[EndpointCheck, AiView]>("check_service", { id, model: model ?? null }),
  serviceModels: (service: Service, key?: string) => invoke<string[]>("service_models", { service, key: key ?? null }),
  test: () => invoke<{ job: { status: string; error: string | null; next_attempt_at: string | null }; result: { ok: boolean; message: string } | null }>("run_test_stage"),
  courses: () => invoke<CourseCard[]>("list_courses"),
  course: (id: string) => invoke<CourseView>("get_course", { id }),
  create: (subject: string, notes: string) => invoke<string>("create_course", { subject, notes }),
  answerIntake: (id: string, answers: Record<string, string>) => invoke<void>("answer_intake", { id, answers }),
  chooseBundle: (id: string, choice: string) => invoke<void>("choose_bundle", { id, choice }),
  approve: (id: string, startDate: string, latePolicy: LatePolicy, strictness: Strictness, onTranscript: boolean, build: BuildSettings) =>
    invoke<void>("approve_course", { id, enrol: { startDate, latePolicy, strictness, onTranscript, build } }),
  buildLevels: () => invoke<{ presets: BuildSettings[]; last: BuildSettings }>("build_levels"),
  buildEstimate: (id: string, build: BuildSettings) => invoke<BuildEstimate>("build_estimate", { id, build }),
  /** `now` stops the step in progress (what it used is lost); otherwise it finishes and then building waits. */
  pauseCourse: (id: string, now = false) => invoke<void>("pause_course", { id, now }),
  finishEarly: (id: string) => invoke<void>("finish_course_early", { id }),
  buildRemaining: (id: string) => invoke<Remaining>("build_remaining", { id }),
  continueCourse: (id: string) => invoke<void>("continue_course", { id }),
  retry: (id: string) => invoke<number>("retry_course", { id }),
  remove: (id: string) => invoke<void>("delete_course", { id }),
  materials: (courseId: string) => invoke<Material[]>("list_materials", { courseId }),
  addMaterials: (courseId: string, files: string[]) => invoke<number>("add_materials", { courseId, files }),
  addNote: (courseId: string, title: string, text: string) => invoke<void>("add_material_note", { courseId, title, text }),
  removeMaterial: (id: string) => invoke<void>("remove_material", { id }),
  openMaterial: (id: string) => invoke<void>("open_material", { id }),
  today: () => invoke<Today>("today"),
  item: (id: string) => invoke<ItemView>("get_item", { id }),
  audioOnline: () => invoke<boolean>("audio_online"),
  setAudioOnline: (on: boolean) => invoke<void>("set_audio_online", { on }),
  tools: () => invoke<ToolsView>("tools_view"),
  toolsLookup: (ids: string[]) => invoke<Tool[]>("tools_lookup", { ids }),
  installTool: (id: string) => invoke<void>("tool_install", { id }),
  removeTool: (id: string) => invoke<void>("tool_remove", { id }),
  toolsSetupDone: () => invoke<void>("tools_setup_done"),
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
  builderAttach: (id: string, files: string[]) => invoke<Attachment[]>("builder_attach", { id, files }),
  builderFileText: (id: string, file: string) => invoke<string>("builder_file_text", { id, file }),
  builderRemoveFile: (id: string, file: string) => invoke<void>("builder_remove_file", { id, file }),
  openCourseFile: (courseId: string, file: string) => invoke<void>("open_course_file", { courseId, file }),
  addWeek: (id: string, unit: DraftUnit) => invoke<string>("course_add_week", { id, unit }),
  addWork: (id: string, unitKey: string, item: GenItem) => invoke<void>("course_add_work", { id, unitKey, item }),
  transcript: () => invoke<Transcript>("get_transcript"),
  openFolder: (path: string) => invoke<void>("open_folder", { path }),
  backups: () => invoke<BackupInfo[]>("list_backups"),
  backupNow: () => invoke<string>("backup_now"),
  recordStudy: (itemId: string, day: string, hour: number, seconds: number) => invoke<void>("record_study", { itemId, day, hour, seconds }),
  insights: () => invoke<InsightsData>("get_insights"),
  clearStudyTime: () => invoke<number>("clear_study_time"),
};

/** Re-run `load` now and whenever the backend says data changed. Returns a cleanup function.
 *  A burst of changes (a unit being written, marks arriving) becomes one reload, and a hidden window waits until
 *  it's shown again instead of reloading in the background. */
export function live(load: () => void): () => void {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let stale = false;
  const run = () => {
    timer = null;
    if (document.hidden) {
      stale = true;
      return;
    }
    stale = false;
    load();
  };
  const schedule = () => {
    if (!timer) timer = setTimeout(run, 150);
  };
  const shown = () => {
    if (!document.hidden && stale) schedule();
  };
  load();
  const un = listen("data-changed", schedule);
  document.addEventListener("visibilitychange", shown);
  return () => {
    if (timer) clearTimeout(timer);
    document.removeEventListener("visibilitychange", shown);
    un.then((f) => f());
  };
}
