//! Shapes of what each Claude stage returns. These mirror the JSON Schemas in `prompts.rs`; Claude Code validates
//! its answer against the schema, and serde parses it into these.

use serde::{Deserialize, Serialize};

// ---------- 1. Intake ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntakeQuestions {
    pub questions: Vec<IntakeQuestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntakeQuestion {
    pub id: String,
    pub text: String,
    /// text | choice | number | date
    pub kind: String,
    pub options: Vec<String>,
}

// ---------- 2. Scope / plan (includes the readiness report) ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub title: String,
    pub summary: String,
    pub bundle: Bundle,
    /// offline | online
    pub mode: String,
    pub dependencies: Vec<String>,
    pub pacing: Pacing,
    pub units: Vec<PlanUnit>,
    pub weights: Weights,
    pub readiness: Readiness,
    pub domain_family: String,
    /// Inputs the course would need that the app can't capture yet (e.g. audio, camera, code runner).
    pub unsupported_inputs: Vec<String>,
    /// Tools from tools/catalog.json the lessons would use; the review step offers to install missing ones.
    #[serde(default)]
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    pub detected: bool,
    pub parts: Vec<String>,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pacing {
    pub total_weeks: u32,
    pub hours_per_week: f64,
    pub fits_user_hours: bool,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanUnit {
    pub key: String,
    pub title: String,
    pub week: u32,
    pub topics: Vec<String>,
    /// none | midterm | final
    pub exam: String,
}

/// Share of the final grade per assessment kind. Normalised to sum to 1 when the schedule is built.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Weights {
    pub homework: f64,
    pub assignment: f64,
    pub quiz: f64,
    pub test: f64,
    pub exam: f64,
    pub project: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Readiness {
    pub delivery: Vec<ReadinessStep>,
    pub marking: Vec<ReadinessStep>,
    /// Low | Low–medium | Medium | Medium–high | High
    pub overall_delivery: String,
    pub overall_marking: String,
    pub weak_spots: Vec<String>,
    pub basis: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessStep {
    pub step: String,
    pub band: String,
    /// Rough estimate, never 100.
    pub percent: f64,
    pub note: String,
}

// ---------- 3. Sources ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceList {
    pub sources: Vec<SourceEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceEntry {
    pub url: String,
    pub title: String,
    pub licence: String,
    pub attribution: String,
    /// false = paid or copyrighted: "get this yourself"
    pub free: bool,
    pub unit_keys: Vec<String>,
    pub note: String,
}

// ---------- How a course presents itself ----------

/// The details a real course page shows: code, term, instructor, banner colour, a welcome message, learning
/// outcomes and each unit's objectives (by unit key). Written in the course builder; Claude-written courses leave it
/// empty and the app fills in sensible defaults.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CourseMeta {
    /// Short code shown on the course card, e.g. "CHESS 101".
    pub code: String,
    /// e.g. "Fall 2026"
    pub term: String,
    pub instructor: String,
    /// Banner colour name from the app's palette (see src/lib/course.ts).
    pub color: String,
    /// Markdown shown first on the course home page.
    pub welcome: String,
    /// What the learner will be able to do by the end.
    pub outcomes: Vec<String>,
    /// Unit key -> "by the end of this unit you can..." lines.
    pub objectives: std::collections::BTreeMap<String, Vec<String>>,
}

// ---------- 4. Unit content ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitContent {
    pub lesson: Lesson,
    pub items: Vec<GenItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lesson {
    pub title: String,
    /// Markdown with $...$ / $$...$$ LaTeX.
    pub markdown: String,
    pub est_minutes: u32,
    pub sources_used: Vec<String>,
    /// Documents attached in the course builder (empty for courses Claude writes).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<Attachment>,
}

/// A document attached to a lesson or a piece of work in a course written in the course builder.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Attachment {
    /// The name the learner sees, usually the original file name.
    pub name: String,
    /// Where it is, relative to the course folder (always under `files/`).
    pub file: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GenItem {
    pub key: String,
    /// homework | assignment | quiz | test | exam | project
    pub kind: String,
    pub title: String,
    /// written | upload | quiz
    pub module: String,
    pub instructions: String,
    pub questions: Vec<Question>,
    pub rubric: Vec<RubricLine>,
    pub answer_key: Vec<KeyEntry>,
    /// Days between release and the deadline.
    pub due_days: u32,
    /// Documents attached in the course builder (empty for courses Claude writes).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<Attachment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Question {
    pub id: String,
    pub prompt: String,
    /// mcq | numeric | short | long
    #[serde(rename = "type")]
    pub kind: String,
    pub options: Vec<String>,
    pub points: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RubricLine {
    pub criterion: String,
    pub points: f64,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeyEntry {
    pub question_id: String,
    /// For mcq: the exact option text. For numeric: the number as text.
    pub answer: String,
    pub explanation: String,
    /// Allowed absolute difference for numeric answers (0 when not numeric).
    pub tolerance: f64,
}

// ---------- 5. Validation ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Validation {
    pub ok: bool,
    pub issues: Vec<Issue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub item_key: String,
    pub question_id: String,
    pub problem: String,
    /// minor | major
    pub severity: String,
}

// ---------- 6. Marking ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkResult {
    pub lines: Vec<MarkLine>,
    pub overall_comment: String,
    pub revise: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkLine {
    pub criterion: String,
    pub awarded: f64,
    pub possible: f64,
    pub comment: String,
}

/// Public part of an item, written to `items/<id>.json` (or sealed until release for tests and exams).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemPublic {
    pub title: String,
    pub kind: String,
    pub module: String,
    pub instructions: String,
    pub questions: Vec<Question>,
    pub rubric: Vec<RubricLine>,
    #[serde(default)]
    pub files: Vec<Attachment>,
}

/// Private part, always sealed: the answer key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemKey {
    pub answer_key: Vec<KeyEntry>,
}
