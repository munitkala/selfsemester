//! The course builder: write a course yourself, with no Claude involved.
//!
//! A draft lives in `courses.draft_json` while the course's status is 'draft'. Publishing turns it into the same
//! plan and unit files a Claude-written course has, then schedules it with the same code. Written work in these
//! courses is self-marked (see `marking::self_mark`); multiple choice and numeric questions are marked by the app.
//!
//! A draft can be exported to a plain folder (course.json, plus lesson.md and work.json per unit, and any attached
//! documents in "lesson files" and "work files" folders) and imported back, so it can be edited in any text editor.
//!
//! A course doesn't have to be finished before it's published: a title and a first lesson are enough. Weeks and
//! graded work can be added to a published course later (`add_week`, `add_work`), without touching anything already
//! released or marked.

use super::attachments;
use super::pipeline;
use super::schedule::{self, LatePolicy, Strictness};
use super::seal::Sealer;
use super::types::*;
use crate::db;
use crate::paths::AppPaths;
use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

fn io(e: rusqlite::Error) -> String {
    e.to_string()
}

pub const KINDS: &[&str] = &["homework", "assignment", "quiz", "test", "exam", "project"];
pub const QUESTION_TYPES: &[&str] = &["mcq", "numeric", "short", "long"];
const FORMAT: &str = "selfsemester-course";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Draft {
    pub title: String,
    pub subject: String,
    pub summary: String,
    pub weights: Weights,
    pub units: Vec<DraftUnit>,
    /// Code, term, instructor, colour, welcome and outcomes. `objectives` here is unused: each unit keeps its own.
    #[serde(default)]
    pub meta: CourseMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DraftUnit {
    pub title: String,
    pub week: u32,
    pub topics: Vec<String>,
    /// "By the end of this unit you can..."
    #[serde(default)]
    pub objectives: Vec<String>,
    pub lesson_title: String,
    /// Markdown, with $...$ LaTeX.
    pub lesson: String,
    /// Documents that make up or go with the lesson (a PDF of it, slides, worksheets).
    #[serde(default)]
    pub lesson_files: Vec<Attachment>,
    pub est_minutes: u32,
    /// Graded work. `key` and `module` are filled in when the course is published.
    pub items: Vec<GenItem>,
}

impl Draft {
    pub fn new(title: &str) -> Self {
        Draft {
            title: title.to_string(),
            subject: title.to_string(),
            summary: String::new(),
            weights: Weights { homework: 0.2, assignment: 0.2, quiz: 0.1, test: 0.2, exam: 0.3, project: 0.0 },
            units: vec![DraftUnit::new(1)],
            meta: CourseMeta { instructor: "Me".into(), color: "indigo".into(), ..Default::default() },
        }
    }
}

impl DraftUnit {
    pub fn new(week: u32) -> Self {
        DraftUnit {
            title: format!("Week {week}"),
            week,
            topics: vec![],
            objectives: vec![],
            lesson_title: String::new(),
            lesson: String::new(),
            lesson_files: vec![],
            est_minutes: 30,
            items: vec![],
        }
    }
}

/// Start a new course in the builder.
pub fn create(conn: &Connection, title: &str) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("Give the course a title first.".into());
    }
    insert_draft(conn, &Draft::new(title))
}

fn insert_draft(conn: &Connection, draft: &Draft) -> Result<String, String> {
    let profile = pipeline::default_profile(conn)?;
    let id = pipeline::course_id_for(&draft.title);
    let now = db::now_utc();
    conn.execute(
        "INSERT INTO courses(id, profile_id, title, subject, status, origin, draft_json, created_at, updated_at)
         VALUES (?1,?2,?3,?4,'draft','manual',?5,?6,?6)",
        params![id, profile, draft.title.trim(), draft.subject.trim(), serde_json::to_string(draft).unwrap(), now],
    )
    .map_err(io)?;
    Ok(id)
}

pub fn get(conn: &Connection, id: &str) -> Result<Draft, String> {
    let (status, json): (String, Option<String>) = conn
        .query_row("SELECT status, draft_json FROM courses WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()
        .map_err(io)?
        .ok_or("No such course.")?;
    if status != "draft" {
        return Err("This course has been published, so it can't be edited in the builder any more.".into());
    }
    serde_json::from_str(&json.ok_or("This course has no draft.")?).map_err(|e| e.to_string())
}

pub fn save(conn: &Connection, id: &str, draft: &Draft) -> Result<(), String> {
    get(conn, id)?; // exists and is still a draft
    let title = if draft.title.trim().is_empty() { "Untitled course" } else { draft.title.trim() };
    conn.execute(
        "UPDATE courses SET title=?2, subject=?3, draft_json=?4, updated_at=?5 WHERE id=?1",
        params![id, title, draft.subject.trim(), serde_json::to_string(draft).unwrap(), db::now_utc()],
    )
    .map_err(io)?;
    Ok(())
}

fn auto(q: &Question) -> bool {
    q.kind == "mcq" || q.kind == "numeric"
}

/// Everything that stops a draft from being published, in plain words. Empty = ready.
pub fn problems(d: &Draft) -> Vec<String> {
    let mut out = Vec::new();
    if d.title.trim().is_empty() {
        out.push("The course needs a title.".into());
    }
    if d.units.is_empty() {
        out.push("Add at least one unit.".into());
    }
    let w = &d.weights;
    if [w.homework, w.assignment, w.quiz, w.test, w.exam, w.project].iter().any(|x| !x.is_finite() || *x < 0.0) {
        out.push("Grade weights can't be negative.".into());
    }
    for (ui, u) in d.units.iter().enumerate() {
        let at = format!("Unit {} ({})", ui + 1, if u.title.trim().is_empty() { "untitled" } else { u.title.trim() });
        if u.title.trim().is_empty() {
            out.push(format!("{at}: needs a title."));
        }
        if u.week == 0 || u.week > 104 {
            out.push(format!("{at}: the week must be between 1 and 104."));
        }
        if u.lesson.trim().is_empty() && u.lesson_files.is_empty() {
            out.push(format!("{at}: write the lesson or attach a document for it."));
        }
        for (ii, it) in u.items.iter().enumerate() {
            let at = format!("{at}, work {} ({})", ii + 1, if it.title.trim().is_empty() { "untitled" } else { it.title.trim() });
            if it.title.trim().is_empty() {
                out.push(format!("{at}: needs a title."));
            }
            if !KINDS.contains(&it.kind.as_str()) {
                out.push(format!("{at}: pick what kind of work it is."));
            }
            if it.due_days == 0 || it.due_days > 21 {
                out.push(format!("{at}: the deadline must be 1 to 21 days after release."));
            }
            if it.questions.is_empty() && it.instructions.trim().is_empty() && it.files.is_empty() {
                out.push(format!("{at}: add instructions, a question or a document."));
            }
            let mut ids = std::collections::HashSet::new();
            for (qi, q) in it.questions.iter().enumerate() {
                let at = format!("{at}, question {}", qi + 1);
                if q.id.trim().is_empty() || !ids.insert(q.id.trim()) {
                    out.push(format!("{at}: questions need different ids."));
                }
                if q.prompt.trim().is_empty() {
                    out.push(format!("{at}: the question is empty."));
                }
                if !QUESTION_TYPES.contains(&q.kind.as_str()) {
                    out.push(format!("{at}: pick a question type."));
                }
                if !(q.points.is_finite() && q.points > 0.0) {
                    out.push(format!("{at}: give it some points."));
                }
                let key = it.answer_key.iter().find(|k| k.question_id == q.id);
                match q.kind.as_str() {
                    "mcq" => {
                        let opts: Vec<&str> = q.options.iter().map(|o| o.trim()).filter(|o| !o.is_empty()).collect();
                        if opts.len() < 2 {
                            out.push(format!("{at}: multiple choice needs at least two options."));
                        }
                        match key {
                            Some(k) if opts.contains(&k.answer.trim()) => {}
                            _ => out.push(format!("{at}: choose which option is right.")),
                        }
                    }
                    "numeric" => match key {
                        Some(k) if k.answer.trim().replace(',', "").parse::<f64>().is_ok() => {}
                        _ => out.push(format!("{at}: the right answer must be a number.")),
                    },
                    _ => {}
                }
            }
            for r in &it.rubric {
                if r.criterion.trim().is_empty() || !(r.points.is_finite() && r.points > 0.0) {
                    out.push(format!("{at}: every rubric line needs a name and some points."));
                    break;
                }
            }
        }
    }
    out
}

/// What a draft's course page shows: its meta, with each unit's objectives under the key it's published with.
fn published_meta(d: &Draft) -> CourseMeta {
    let mut meta = d.meta.clone();
    meta.objectives = d
        .units
        .iter()
        .enumerate()
        .map(|(i, u)| (format!("u{:02}", i + 1), u.objectives.iter().map(|o| o.trim().to_string()).filter(|o| !o.is_empty()).collect()))
        .collect();
    meta.outcomes = meta.outcomes.iter().map(|o| o.trim().to_string()).filter(|o| !o.is_empty()).collect();
    meta
}

/// A piece of work as it's published: its key and module filled in, unused options and answers dropped.
fn prepare_item(it: &GenItem, n: usize) -> GenItem {
    let mut it = it.clone();
    it.key = format!("i{n}");
    // fully auto-markable work is a quiz; anything with written answers is written work
    it.module = if !it.questions.is_empty() && it.questions.iter().all(auto) { "quiz" } else { "written" }.into();
    it.answer_key.retain(|k| it.questions.iter().any(|q| q.id == k.question_id));
    for q in &mut it.questions {
        if q.kind != "mcq" {
            q.options.clear();
        } else {
            q.options = q.options.iter().map(|o| o.trim().to_string()).filter(|o| !o.is_empty()).collect();
        }
    }
    it
}

/// The plan and unit contents a published draft becomes, with keys and modules filled in.
fn compile(d: &Draft) -> (Plan, Vec<(String, UnitContent)>) {
    let last = d.units.len().saturating_sub(1);
    let mut units = Vec::new();
    let mut contents = Vec::new();
    for (ui, u) in d.units.iter().enumerate() {
        let key = format!("u{:02}", ui + 1);
        let has_exam = u.items.iter().any(|it| it.kind == "exam");
        units.push(PlanUnit {
            key: key.clone(),
            title: u.title.trim().to_string(),
            week: u.week.max(1),
            topics: u.topics.iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect(),
            exam: if !has_exam { "none" } else if ui == last { "final" } else { "midterm" }.into(),
        });
        let items = u
            .items
            .iter()
            .enumerate()
            .map(|(ii, it)| prepare_item(it, ii + 1))
            .collect();
        let title = if u.lesson_title.trim().is_empty() { u.title.trim() } else { u.lesson_title.trim() };
        contents.push((
            key,
            UnitContent {
                lesson: Lesson {
                    title: title.to_string(),
                    markdown: u.lesson.clone(),
                    est_minutes: u.est_minutes,
                    sources_used: vec![],
                    files: u.lesson_files.clone(),
                },
                items,
            },
        ));
    }
    let weeks = units.iter().map(|u| u.week).max().unwrap_or(1);
    let plan = Plan {
        title: d.title.trim().to_string(),
        summary: d.summary.trim().to_string(),
        bundle: Bundle { detected: false, parts: vec![], recommendation: String::new() },
        mode: "offline".into(),
        dependencies: vec![],
        pacing: Pacing { total_weeks: weeks, hours_per_week: 0.0, fits_user_hours: true, note: String::new() },
        units,
        weights: d.weights.clone(),
        readiness: Readiness {
            delivery: vec![],
            marking: vec![],
            overall_delivery: "Written by you".into(),
            overall_marking: "Self-marked".into(),
            weak_spots: vec![],
            basis: "You wrote this course in the course builder.".into(),
        },
        domain_family: "manual".into(),
        unsupported_inputs: vec![],
        tools: vec![],
    };
    (plan, contents)
}

/// Publish a draft: write its plan and units, build the schedule and start the course.
#[allow(clippy::too_many_arguments)]
pub fn publish(
    conn: &Connection,
    paths: &AppPaths,
    sealer: &Sealer,
    id: &str,
    start: NaiveDate,
    late_policy: &LatePolicy,
    strictness: Strictness,
    on_transcript: bool,
) -> Result<(), String> {
    let draft = get(conn, id)?;
    let issues = problems(&draft);
    if !issues.is_empty() {
        return Err(format!("Fix these first: {}", issues.join(" ")));
    }
    let (plan, contents) = compile(&draft);
    let dir = paths.ensure_course(id).map_err(|e| e.to_string())?;
    let tx = conn.unchecked_transaction().map_err(io)?;
    tx.execute(
        "UPDATE courses SET title=?2, subject=?3, plan_json=?4, late_policy=?5, strictness=?6, on_transcript=?7,
                            starts_at=?8, mode='offline', readiness_json=?9, error=NULL, updated_at=?10, meta_json=?11
         WHERE id=?1",
        params![
            id,
            plan.title,
            draft.subject.trim(),
            serde_json::to_string(&plan).unwrap(),
            serde_json::to_string(late_policy).unwrap(),
            strictness.as_str(),
            on_transcript,
            start.to_string(),
            serde_json::to_string(&plan.readiness).unwrap(),
            db::now_utc(),
            serde_json::to_string(&published_meta(&draft)).unwrap()
        ],
    )
    .map_err(io)?;
    tx.execute("DELETE FROM units WHERE course_id=?1", [id]).map_err(io)?;
    for (i, (u, (key, content))) in plan.units.iter().zip(&contents).enumerate() {
        tx.execute(
            "INSERT INTO units(id, course_id, seq, title, summary, unit_key, week, state) VALUES (?1,?2,?3,?4,?5,?6,?7,'validated')",
            params![format!("{id}:{key}"), id, i as i64, u.title, u.topics.join(", "), key, u.week],
        )
        .map_err(io)?;
        // same as a Claude-written unit: sealed, because it holds the answer keys
        sealer.write_json(&dir.join("units").join(format!("{key}.json")), content, true).map_err(|e| e.to_string())?;
    }
    pipeline::seal_and_start(&tx, paths, sealer, id)?;
    tx.commit().map_err(io)?;
    Ok(())
}

// ---------- folder export / import ----------

#[derive(Serialize, Deserialize)]
struct CourseFile {
    format: String,
    version: u32,
    title: String,
    subject: String,
    summary: String,
    weights: Weights,
    units: Vec<UnitFile>,
    #[serde(default)]
    meta: CourseMeta,
}

#[derive(Serialize, Deserialize)]
struct UnitFile {
    folder: String,
    title: String,
    week: u32,
    topics: Vec<String>,
    #[serde(default)]
    objectives: Vec<String>,
    lesson_title: String,
    est_minutes: u32,
    /// Paths relative to the unit's folder, e.g. "lesson files/Week 1.pdf".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    lesson_files: Vec<Attachment>,
}

fn clean(s: &str) -> String {
    let c: String = s.chars().map(|c| if r#"<>:"/\|?*"#.contains(c) || c.is_control() { '-' } else { c }).collect();
    let c = c.trim().trim_end_matches('.').trim();
    if c.is_empty() { "untitled".into() } else { c.chars().take(60).collect() }
}

const README: &str = "This folder is a SelfSemester course, exported from the course builder.\r\n\r\n\
course.json      the title, summary, grade weights and the list of units\r\n\
NN <unit>/lesson.md   that unit's lesson (Markdown, $...$ for maths)\r\n\
NN <unit>/work.json   that unit's graded work: questions, answer keys, rubrics, deadlines\r\n\
NN <unit>/lesson files/         documents attached to the lesson\r\n\
NN <unit>/work files/NN <work>/ documents attached to each piece of work\r\n\r\n\
Edit anything in a text editor, then use Import folder in the course builder. Importing makes a new draft;\r\n\
this folder isn't changed.\r\n";

/// Write a draft to a new folder under Documents\SelfSemester\Course builder. Returns the folder.
pub fn export(conn: &Connection, paths: &AppPaths, id: &str) -> Result<PathBuf, String> {
    let d = get(conn, id)?;
    let base = paths.user_dir.join("Course builder");
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    let name = clean(&d.title);
    let root = (1..).map(|n| if n == 1 { base.join(&name) } else { base.join(format!("{name} ({n})")) }).find(|p| !p.exists()).unwrap();
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let mut units = Vec::new();
    for (i, u) in d.units.iter().enumerate() {
        let folder = format!("{:02} {}", i + 1, clean(&u.title));
        let ud = root.join(&folder);
        std::fs::create_dir_all(&ud).map_err(|e| e.to_string())?;
        std::fs::write(ud.join("lesson.md"), &u.lesson).map_err(|e| e.to_string())?;
        let lesson_files = export_files(paths, id, &u.lesson_files, &ud, "lesson files")?;
        let mut items = u.items.clone();
        for (ii, it) in items.iter_mut().enumerate() {
            let sub = format!("work files/{:02} {}", ii + 1, clean(&it.title));
            it.files = export_files(paths, id, &it.files, &ud, &sub)?;
        }
        std::fs::write(ud.join("work.json"), serde_json::to_string_pretty(&items).unwrap()).map_err(|e| e.to_string())?;
        units.push(UnitFile {
            folder,
            title: u.title.clone(),
            week: u.week,
            topics: u.topics.clone(),
            objectives: u.objectives.clone(),
            lesson_title: u.lesson_title.clone(),
            est_minutes: u.est_minutes,
            lesson_files,
        });
    }
    let file = CourseFile {
        format: FORMAT.into(),
        version: 1,
        title: d.title,
        subject: d.subject,
        summary: d.summary,
        weights: d.weights,
        units,
        meta: d.meta,
    };
    std::fs::write(root.join("course.json"), serde_json::to_string_pretty(&file).unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(root.join("README.txt"), README).map_err(|e| e.to_string())?;
    Ok(root)
}

/// Copy a draft's attachments into `<unit folder>/<sub>`, returning them with paths relative to the unit folder.
fn export_files(paths: &AppPaths, id: &str, files: &[Attachment], unit_dir: &Path, sub: &str) -> Result<Vec<Attachment>, String> {
    if files.is_empty() {
        return Ok(vec![]);
    }
    let dir = unit_dir.join(sub);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for f in files {
        let src = attachments::path_of(paths, id, &f.file)?;
        let name = Path::new(&f.file).file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();
        std::fs::copy(&src, dir.join(&name)).map_err(|e| format!("Couldn't copy {}: {e}", f.name))?;
        out.push(Attachment { name: f.name.clone(), file: format!("{sub}/{name}"), bytes: f.bytes });
    }
    Ok(out)
}

/// A path written in an exported folder, only if it stays inside that unit's folder.
fn inside(unit_dir: &Path, rel: &str) -> Result<PathBuf, String> {
    let p = Path::new(rel);
    if p.is_absolute() || p.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
        return Err(format!("\"{rel}\" isn't inside the course folder."));
    }
    Ok(unit_dir.join(p))
}

/// Read a folder written by `export` (possibly edited since) into a new draft. Returns the new course's id.
pub fn import(conn: &Connection, paths: &AppPaths, folder: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(folder.join("course.json"))
        .map_err(|_| "That folder has no course.json. Pick a folder exported from the course builder.".to_string())?;
    let file: CourseFile = serde_json::from_str(&text).map_err(|e| format!("course.json couldn't be read: {e}"))?;
    if file.format != FORMAT {
        return Err("That course.json isn't a SelfSemester course.".into());
    }
    let mut units = Vec::new();
    for u in file.units {
        // only folders directly inside the exported one
        if u.folder.contains(['/', '\\']) || u.folder.contains("..") {
            return Err(format!("Unit folder \"{}\" isn't inside the course folder.", u.folder));
        }
        let ud = folder.join(&u.folder);
        let lesson = std::fs::read_to_string(ud.join("lesson.md")).unwrap_or_default();
        let items: Vec<GenItem> = match std::fs::read_to_string(ud.join("work.json")) {
            Ok(t) => serde_json::from_str(&t).map_err(|e| format!("{}/work.json couldn't be read: {e}", u.folder))?,
            Err(_) => vec![],
        };
        // Files are copied in once the draft exists; until then they point into the folder being imported.
        let to_full = |files: &[Attachment]| -> Result<Vec<Attachment>, String> {
            files
                .iter()
                .map(|f| Ok(Attachment { file: inside(&ud, &f.file)?.display().to_string(), ..f.clone() }))
                .collect()
        };
        let mut items = items;
        for it in &mut items {
            it.files = to_full(&it.files)?;
        }
        units.push(DraftUnit {
            title: u.title,
            week: u.week,
            topics: u.topics,
            objectives: u.objectives,
            lesson_title: u.lesson_title,
            lesson,
            lesson_files: to_full(&u.lesson_files)?,
            est_minutes: u.est_minutes,
            items,
        });
    }
    let mut draft = Draft { title: file.title, subject: file.subject, summary: file.summary, weights: file.weights, units, meta: file.meta };
    if draft.title.trim().is_empty() {
        return Err("course.json has no title.".into());
    }
    // Check every attached file before creating anything.
    for u in &draft.units {
        for f in u.lesson_files.iter().chain(u.items.iter().flat_map(|it| it.files.iter())) {
            attachments::check(Path::new(&f.file)).map_err(|e| format!("In the folder being imported: {e}"))?;
        }
    }
    let id = insert_draft(conn, &draft)?;
    let copy = |files: &mut Vec<Attachment>| -> Result<(), String> {
        for f in files.iter_mut() {
            let (_, bytes) = attachments::check(Path::new(&f.file))?;
            *f = attachments::copy_in(paths, &id, Path::new(&f.file), &f.name, bytes)?;
        }
        Ok(())
    };
    for u in &mut draft.units {
        copy(&mut u.lesson_files)?;
        for it in &mut u.items {
            copy(&mut it.files)?;
        }
    }
    save(conn, &id, &draft)?;
    Ok(id)
}

// ---------- growing a published course ----------

/// A published course written in the course builder that's still running.
fn running_manual(conn: &Connection, id: &str) -> Result<(Plan, CourseMeta, NaiveDate), String> {
    let (status, origin, plan, meta, starts): (String, String, Option<String>, Option<String>, Option<String>) = conn
        .query_row("SELECT status, origin, plan_json, meta_json, starts_at FROM courses WHERE id=?1", [id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .optional()
        .map_err(io)?
        .ok_or("No such course.")?;
    if origin != "manual" {
        return Err("Only courses you wrote in the course builder can have weeks or work added.".into());
    }
    if status != "active" {
        return Err("Weeks and work can only be added while the course is running.".into());
    }
    let plan: Plan = serde_json::from_str(&plan.ok_or("This course has no plan.")?).map_err(|e| e.to_string())?;
    let meta: CourseMeta = meta.and_then(|m| serde_json::from_str(&m).ok()).unwrap_or_default();
    let start = starts
        .as_deref()
        .and_then(schedule::parse_ts)
        .map(|t| t.with_timezone(&chrono::Local).date_naive())
        .ok_or("This course has no start date.")?;
    Ok((plan, meta, start))
}

/// After adding to a course: grade shares, the plan, and the end date.
fn finish_growth(conn: &Connection, id: &str, plan: &Plan, meta: &CourseMeta) -> Result<(), String> {
    schedule::reweight(conn, id, &plan.weights).map_err(io)?;
    let last_due: Option<String> =
        conn.query_row("SELECT max(due_at) FROM items WHERE course_id=?1", [id], |r| r.get(0)).map_err(io)?;
    conn.execute(
        "UPDATE courses SET plan_json=?2, meta_json=?3, ends_at=coalesce(?4, ends_at), updated_at=?5 WHERE id=?1",
        params![id, serde_json::to_string(plan).unwrap(), serde_json::to_string(meta).unwrap(), last_due, db::now_utc()],
    )
    .map_err(io)?;
    Ok(())
}

/// Add a week (a lesson, and any graded work with it) to a published course you wrote. It's released at the start
/// of its week, or now if that has already passed.
pub fn add_week(conn: &Connection, paths: &AppPaths, sealer: &Sealer, id: &str, unit: DraftUnit) -> Result<String, String> {
    let (mut plan, mut meta, start) = running_manual(conn, id)?;
    let check = Draft { units: vec![unit.clone()], ..Draft::new("course") };
    let issues = problems(&check);
    if !issues.is_empty() {
        return Err(format!("Fix these first: {}", issues.join(" ").replace("Unit 1 ", "This week ")));
    }
    let n: i64 = conn.query_row("SELECT count(*) FROM units WHERE course_id=?1", [id], |r| r.get(0)).map_err(io)?;
    let n = n as usize;
    let key = (n + 1..).map(|i| format!("u{i:02}")).find(|k| !plan.units.iter().any(|u| &u.key == k)).unwrap();
    let (one, mut contents) = compile(&check);
    let mut pu = one.units.into_iter().next().ok_or("Nothing to add.")?;
    let (_, content) = contents.remove(0);
    pu.key = key.clone();
    pu.exam = if content.items.iter().any(|it| it.kind == "exam") { "midterm" } else { "none" }.into();
    let dir = paths.ensure_course(id).map_err(|e| e.to_string())?;
    let unit_id = format!("{id}:{key}");
    let tx = conn.unchecked_transaction().map_err(io)?;
    tx.execute(
        "INSERT INTO units(id, course_id, seq, title, summary, unit_key, week, state) VALUES (?1,?2,?3,?4,?5,?6,?7,'validated')",
        params![unit_id, id, n as i64, pu.title, pu.topics.join(", "), key, pu.week],
    )
    .map_err(io)?;
    sealer.write_json(&dir.join("units").join(format!("{key}.json")), &content, true).map_err(|e| e.to_string())?;
    schedule::schedule_unit(&tx, &dir, sealer, id, &unit_id, &key, pu.week, &content, start, Some(chrono::Utc::now()))
        .map_err(|e| e.to_string())?;
    let objectives: Vec<String> = unit.objectives.iter().map(|o| o.trim().to_string()).filter(|o| !o.is_empty()).collect();
    meta.objectives.insert(key.clone(), objectives);
    plan.pacing.total_weeks = plan.pacing.total_weeks.max(pu.week);
    plan.units.push(pu);
    finish_growth(&tx, id, &plan, &meta)?;
    tx.commit().map_err(io)?;
    Ok(key)
}

/// Add a piece of graded work to a week of a published course you wrote. It opens with its week, or now if the
/// week has already started.
pub fn add_work(conn: &Connection, paths: &AppPaths, sealer: &Sealer, id: &str, unit_key: &str, item: GenItem) -> Result<(), String> {
    let (plan, meta, _) = running_manual(conn, id)?;
    if !plan.units.iter().any(|u| u.key == unit_key) {
        return Err("That week isn't in this course.".into());
    }
    let mut check = Draft::new("course");
    check.units[0].lesson = "-".into();
    check.units[0].items = vec![item.clone()];
    let issues = problems(&check);
    if !issues.is_empty() {
        return Err(format!("Fix these first: {}", issues.join(" ").replace("Unit 1 (Week 1), work 1 ", "This work ")));
    }
    let dir = paths.ensure_course(id).map_err(|e| e.to_string())?;
    let file = dir.join("units").join(format!("{unit_key}.json"));
    let mut content: UnitContent = sealer.read_json(&file).map_err(|e| e.to_string())?;
    let index = content.items.len() + 1;
    let prepared = prepare_item(&item, index);
    let (unit_id, release): (String, String) = conn
        .query_row(
            "SELECT u.id, i.release_at FROM units u JOIN items i ON i.unit_id = u.id AND i.kind = 'lesson'
             WHERE u.course_id=?1 AND u.unit_key=?2",
            params![id, unit_key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(io)?;
    let release = schedule::parse_ts(&release).ok_or("The week has no release date.")?;
    let tx = conn.unchecked_transaction().map_err(io)?;
    schedule::schedule_item(&tx, &dir, sealer, id, &unit_id, unit_key, index, &prepared, release, Some(chrono::Utc::now()))
        .map_err(|e| e.to_string())?;
    content.items.push(prepared);
    sealer.write_json(&file, &content, true).map_err(|e| e.to_string())?;
    finish_growth(&tx, id, &plan, &meta)?;
    tx.commit().map_err(io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::course::marking;
    use chrono::{Duration, Utc};
    use serde_json::json;

    fn setup() -> (tempfile::TempDir, AppPaths, Connection, Sealer) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = AppPaths::rooted(&tmp.path().join("local"), &tmp.path().join("docs"));
        paths.ensure().unwrap();
        let conn = db::open_in_memory().unwrap();
        let sealer = Sealer::load(&conn).unwrap();
        (tmp, paths, conn, sealer)
    }

    fn q(id: &str, kind: &str, options: &[&str], points: f64) -> Question {
        Question { id: id.into(), prompt: format!("Question {id}"), kind: kind.into(), options: options.iter().map(|s| s.to_string()).collect(), points }
    }
    fn k(id: &str, answer: &str) -> KeyEntry {
        KeyEntry { question_id: id.into(), answer: answer.into(), explanation: String::new(), tolerance: 0.0 }
    }

    /// A small but complete course: a quiz and a mixed homework in week 1.
    fn sample() -> Draft {
        let mut d = Draft::new("Chess openings");
        d.meta.code = "CHESS 101".into();
        d.meta.outcomes = vec!["Play the Italian with a plan".into(), "  ".into()];
        d.units[0].objectives = vec!["Name the main line".into()];
        d.units[0].title = "The Italian Game".into();
        d.units[0].lesson = "# Italian\n\n1. e4 e5 2. Nf3 Nc6 3. Bc4".into();
        d.units[0].items = vec![
            GenItem {
                key: String::new(),
                kind: "quiz".into(),
                title: "Move order".into(),
                module: String::new(),
                instructions: "Answer both.".into(),
                questions: vec![q("q1", "mcq", &["Bc4", "Bb5"], 1.0), q("q2", "numeric", &[], 1.0)],
                rubric: vec![],
                answer_key: vec![k("q1", "Bc4"), k("q2", "3")],
                due_days: 3,
                files: vec![],
            },
            GenItem {
                key: String::new(),
                kind: "homework".into(),
                title: "Plans".into(),
                module: String::new(),
                instructions: "Explain.".into(),
                questions: vec![q("q1", "mcq", &["d3", "d4"], 2.0), q("q2", "long", &[], 8.0)],
                rubric: vec![],
                answer_key: vec![k("q1", "d3"), k("q2", "Slow build-up with c3 and d3.")],
                due_days: 5,
                files: vec![],
            },
        ];
        d
    }

    #[test]
    fn new_draft_lists_what_is_missing() {
        let d = Draft::new("X");
        let p = problems(&d);
        assert!(p.iter().any(|m| m.contains("write the lesson or attach a document")));
        let mut bad = sample();
        bad.units[0].items[0].answer_key[0].answer = "Nf3".into();
        bad.units[0].items[0].answer_key[1].answer = "three".into();
        let p = problems(&bad);
        assert!(p.iter().any(|m| m.contains("choose which option is right")), "{p:?}");
        assert!(p.iter().any(|m| m.contains("must be a number")), "{p:?}");
        assert!(problems(&sample()).is_empty(), "{:?}", problems(&sample()));
    }

    #[test]
    fn publish_schedules_and_self_marks() {
        let (_tmp, paths, conn, sealer) = setup();
        let id = create(&conn, "Chess openings").unwrap();
        save(&conn, &id, &sample()).unwrap();
        assert!(publish(&conn, &paths, &sealer, &id, Utc::now().date_naive() - Duration::days(1), &LatePolicy::LockZero, Strictness::Structured, true).is_ok());
        assert!(get(&conn, &id).is_err(), "published drafts aren't editable");
        let status: String = conn.query_row("SELECT status FROM courses WHERE id=?1", [&id], |r| r.get(0)).unwrap();
        assert_eq!(status, "active");
        let meta = crate::course::view(&conn, &paths, &id).unwrap().meta;
        assert_eq!(meta.code, "CHESS 101");
        assert_eq!(meta.outcomes, ["Play the Italian with a plan"]);
        assert_eq!(meta.objectives["u01"], ["Name the main line"]);
        crate::course::schedule::tick(&conn, Utc::now()).unwrap();

        // the quiz is all multiple choice / numeric, so the app marks it straight away
        let quiz = format!("{id}:u01:1");
        let out = marking::submit(&conn, &paths, &sealer, &quiz, "", &json!({"q1": "Bc4", "q2": "3"}), &[], Utc::now()).unwrap();
        assert_eq!(out.status, "marked");

        // the homework has a written answer: instant feedback for the mcq, then the learner marks the rest
        let hw = format!("{id}:u01:2");
        let out = marking::submit(&conn, &paths, &sealer, &hw, "Build up slowly.", &json!({"q1": "d4"}), &[], Utc::now()).unwrap();
        assert_eq!(out.status, "self");
        let jobs: i64 = conn.query_row("SELECT count(*) FROM jobs", [], |r| r.get(0)).unwrap();
        assert_eq!(jobs, 0, "a hand-written course never calls Claude");
        let v = crate::course::item(&conn, &paths, &sealer, &hw).unwrap();
        assert!(v.answer_key.is_some(), "the key is there to mark against");
        let instant = v.submission.unwrap()["instant"].clone();
        assert_eq!(instant["awarded"], json!(0.0));
        assert_eq!(instant["lines"][0]["comment"], json!("Not right."));
        assert!(marking::self_mark(&conn, &paths, &sealer, &hw, &[0.0, 9.0], "").is_err(), "more than the points");
        marking::self_mark(&conn, &paths, &sealer, &hw, &[2.0, 6.0], "Fair.").unwrap();
        let pct: f64 = conn.query_row("SELECT percent FROM marks WHERE item_id=?1", [&hw], |r| r.get(0)).unwrap();
        assert!((pct - 60.0).abs() < 1e-9, "the mcq was wrong whatever I typed: 6 / 10, got {pct}");
    }

    fn published(strictness: Strictness, start: NaiveDate) -> (tempfile::TempDir, AppPaths, Connection, Sealer, String) {
        let (tmp, paths, conn, sealer) = setup();
        let id = create(&conn, "Chess openings").unwrap();
        save(&conn, &id, &sample()).unwrap();
        let policy = LatePolicy::Penalty { per_day: 10.0, max_days: 2 };
        publish(&conn, &paths, &sealer, &id, start, &policy, strictness, true).unwrap();
        crate::course::schedule::tick(&conn, Utc::now()).unwrap();
        (tmp, paths, conn, sealer, id)
    }

    #[test]
    fn structured_allows_one_attempt() {
        let (_t, paths, conn, sealer, id) = published(Strictness::Structured, Utc::now().date_naive() - Duration::days(1));
        let quiz = format!("{id}:u01:1");
        marking::submit(&conn, &paths, &sealer, &quiz, "", &json!({"q1": "Bb5"}), &[], Utc::now()).unwrap();
        let again = marking::submit(&conn, &paths, &sealer, &quiz, "", &json!({"q1": "Bc4"}), &[], Utc::now());
        assert!(again.unwrap_err().contains("already submitted"));
        assert!(marking::reveal_key(&conn, &quiz).is_err(), "nothing to reveal: the key is already shown");
        let v = crate::course::item(&conn, &paths, &sealer, &quiz).unwrap();
        assert!(v.answer_key.is_some() && !v.can_resubmit);
    }

    #[test]
    fn sandbox_resubmits_without_penalty_until_the_key_is_revealed() {
        // started 10 days ago: the quiz (due 3 days after release) is a week late
        let (_t, paths, conn, sealer, id) = published(Strictness::Sandbox, Utc::now().date_naive() - Duration::days(10));
        let quiz = format!("{id}:u01:1");
        let state: String = conn.query_row("SELECT state FROM items WHERE id=?1", [&quiz], |r| r.get(0)).unwrap();
        assert_eq!(state, "open", "late, but a sandbox item stays open");

        marking::submit(&conn, &paths, &sealer, &quiz, "", &json!({"q1": "Bb5", "q2": "3"}), &[], Utc::now()).unwrap();
        let v = crate::course::item(&conn, &paths, &sealer, &quiz).unwrap();
        assert_eq!(v.mark.as_ref().unwrap()["late_penalty"], json!(0.0));
        assert_eq!(v.mark.as_ref().unwrap()["percent"], json!(50.0));
        assert!(v.answer_key.is_none(), "key stays hidden while another attempt is possible");
        let feedback = v.mark.as_ref().unwrap()["breakdown"].to_string();
        assert!(!feedback.contains("Bc4"), "the feedback doesn't give the answer away: {feedback}");
        assert!(v.can_resubmit);

        marking::submit(&conn, &paths, &sealer, &quiz, "", &json!({"q1": "Bc4", "q2": "3"}), &[], Utc::now()).unwrap();
        let v = crate::course::item(&conn, &paths, &sealer, &quiz).unwrap();
        assert_eq!(v.attempts, 2);
        assert_eq!(v.mark.as_ref().unwrap()["percent"], json!(100.0), "the latest attempt counts");
        assert!(marking::grades(&conn, &id).unwrap().decided > 0.0);

        marking::reveal_key(&conn, &quiz).unwrap();
        let v = crate::course::item(&conn, &paths, &sealer, &quiz).unwrap();
        assert!(v.answer_key.is_some() && !v.can_resubmit);
        let closed = marking::submit(&conn, &paths, &sealer, &quiz, "", &json!({"q1": "Bc4"}), &[], Utc::now());
        assert!(closed.unwrap_err().contains("revealed"));
    }

    #[test]
    fn sandbox_course_closes_a_week_after_the_last_deadline() {
        // last deadline (homework, 5 days) was 15 days ago: past the 7-day sandbox grace
        let (_t, paths, conn, sealer, id) = published(Strictness::Sandbox, Utc::now().date_naive() - Duration::days(20));
        let hw = format!("{id}:u01:2");
        let state: String = conn.query_row("SELECT state FROM items WHERE id=?1", [&hw], |r| r.get(0)).unwrap();
        assert_eq!(state, "missed");
        assert!(marking::submit(&conn, &paths, &sealer, &hw, "late", &json!({}), &[], Utc::now()).is_err());
        let done = marking::maybe_complete(&conn, &paths, &sealer, &id).unwrap();
        assert!(done.is_some(), "everything settled and the window closed: the course finishes");
    }

    #[test]
    fn review_hides_the_key_in_sandbox_and_reports_failures_in_the_thread() {
        let (_t, paths, conn, sealer, id) = published(Strictness::Sandbox, Utc::now().date_naive() - Duration::days(1));
        let hw = format!("{id}:u01:2");
        assert!(crate::course::review::send(&conn, &paths, &sealer, &hw, "Why?").is_err(), "not marked yet");
        marking::submit(&conn, &paths, &sealer, &hw, "Slow build-up.", &json!({"q1": "d3"}), &[], Utc::now()).unwrap();
        marking::self_mark(&conn, &paths, &sealer, &hw, &[0.0, 5.0], "").unwrap();

        crate::course::review::send(&conn, &paths, &sealer, &hw, "What did I miss in question 2?").unwrap();
        let (job, req): (i64, String) =
            conn.query_row("SELECT id, request_json FROM jobs WHERE kind='review'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert!(req.contains("never give final answers"), "sandbox key still hidden");
        assert!(!req.contains("Slow build-up with c3 and d3"), "the key's text isn't sent");
        assert!(crate::course::review::send(&conn, &paths, &sealer, &hw, "again").is_err(), "one question at a time");

        conn.execute("UPDATE jobs SET status='done', result_json=?2 WHERE id=?1", rusqlite::params![job, r#"{"reply":"Think about c3."}"#]).unwrap();
        crate::course::pipeline::on_job_done(&conn, &paths, &sealer, job, "sonnet").unwrap();
        crate::course::review::send(&conn, &paths, &sealer, &hw, "And d4?").unwrap();
        let job2: i64 = conn.query_row("SELECT max(id) FROM jobs WHERE kind='review'", [], |r| r.get(0)).unwrap();
        conn.execute("UPDATE jobs SET status='failed', error='timed out' WHERE id=?1", [job2]).unwrap();
        crate::course::pipeline::on_job_failed(&conn, job2).unwrap();

        let t = crate::course::review::thread(&conn, &hw).unwrap();
        let roles: Vec<&str> = t.messages.iter().map(|m| m.role.as_str()).collect();
        assert_eq!(roles, ["learner", "claude", "learner", "note"]);
        assert!(t.pending.is_none());
        let course_error: Option<String> = conn.query_row("SELECT error FROM courses WHERE id=?1", [&id], |r| r.get(0)).unwrap();
        assert!(course_error.is_none(), "a failed review reply doesn't flag the course");
    }

    #[test]
    fn export_and_import_round_trip() {
        let (_tmp, paths, conn, _) = setup();
        let id = create(&conn, "Chess openings").unwrap();
        save(&conn, &id, &sample()).unwrap();
        let dir = export(&conn, &paths, &id).unwrap();
        assert!(dir.join("course.json").is_file() && dir.join("01 The Italian Game").join("lesson.md").is_file());
        let again = export(&conn, &paths, &id).unwrap();
        assert_ne!(dir, again, "never writes over an earlier export");

        std::fs::write(dir.join("01 The Italian Game").join("lesson.md"), "Edited outside the app.").unwrap();
        let copy = import(&conn, &paths, &dir).unwrap();
        let d = get(&conn, &copy).unwrap();
        assert_eq!(d.units[0].lesson, "Edited outside the app.");
        assert_eq!(d.units[0].items, sample().units[0].items);
        assert_eq!(d.meta, sample().meta, "course details survive the round trip");
        assert_eq!(d.units[0].objectives, ["Name the main line"]);
        assert!(import(&conn, &paths, &paths.user_dir).is_err());
    }

    /// A file on disk to attach, standing in for something the learner picked.
    fn doc(tmp: &tempfile::TempDir, name: &str, text: &str) -> PathBuf {
        let p = tmp.path().join(name);
        std::fs::write(&p, text).unwrap();
        p
    }

    #[test]
    fn a_document_can_be_the_lesson_and_the_work() {
        let (tmp, paths, conn, sealer) = setup();
        let id = create(&conn, "Watercolour basics").unwrap();
        let mut d = get(&conn, &id).unwrap();
        // the least a course needs: a title and a first lesson, here a PDF instead of typed text
        d.units[0].lesson_files = attachments::add(&conn, &paths, &id, &[doc(&tmp, "Week 1 lesson.pdf", "%PDF-1.7")]).unwrap();
        assert!(problems(&d).is_empty(), "{:?}", problems(&d));
        d.units[0].items = vec![GenItem {
            key: String::new(),
            kind: "homework".into(),
            title: "Colour wheel".into(),
            module: String::new(),
            instructions: String::new(),
            questions: vec![],
            rubric: vec![],
            answer_key: vec![],
            due_days: 4,
            files: attachments::add(&conn, &paths, &id, &[doc(&tmp, "worksheet.pdf", "%PDF-1.7")]).unwrap(),
        }];
        assert!(problems(&d).is_empty(), "work with only a document is fine: {:?}", problems(&d));
        save(&conn, &id, &d).unwrap();
        publish(&conn, &paths, &sealer, &id, Utc::now().date_naive() - Duration::days(1), &LatePolicy::LockZero, Strictness::Structured, false).unwrap();
        crate::course::schedule::tick(&conn, Utc::now()).unwrap();

        let lesson = crate::course::item(&conn, &paths, &sealer, &format!("{id}:u01:lesson")).unwrap();
        assert_eq!(lesson.files.len(), 1);
        assert_eq!(lesson.files[0].name, "Week 1 lesson.pdf");
        assert!(attachments::path_of(&paths, &id, &lesson.files[0].file).is_ok());
        let work = crate::course::item(&conn, &paths, &sealer, &format!("{id}:u01:1")).unwrap();
        assert_eq!(work.content.unwrap().files[0].name, "worksheet.pdf");
    }

    #[test]
    fn exported_folders_keep_files_in_readable_places_and_import_them_back() {
        let (tmp, paths, conn, _) = setup();
        let id = create(&conn, "Chess openings").unwrap();
        let mut d = sample();
        d.units[0].lesson_files = attachments::add(&conn, &paths, &id, &[doc(&tmp, "Italian slides.pptx", "slides")]).unwrap();
        d.units[0].items[1].files = attachments::add(&conn, &paths, &id, &[doc(&tmp, "Plans.pdf", "%PDF")]).unwrap();
        save(&conn, &id, &d).unwrap();
        let dir = export(&conn, &paths, &id).unwrap();
        let unit = dir.join("01 The Italian Game");
        assert!(unit.join("lesson files").join("Italian slides.pptx").is_file());
        assert!(unit.join("work files").join("02 Plans").join("Plans.pdf").is_file());

        let copy = import(&conn, &paths, &dir).unwrap();
        let back = get(&conn, &copy).unwrap();
        assert_eq!(back.units[0].lesson_files[0].name, "Italian slides.pptx");
        let f = attachments::path_of(&paths, &copy, &back.units[0].lesson_files[0].file).unwrap();
        assert_eq!(std::fs::read_to_string(f).unwrap(), "slides");
        assert_eq!(back.units[0].items[1].files[0].name, "Plans.pdf");

        // a folder can't point outside itself
        let text = std::fs::read_to_string(dir.join("course.json")).unwrap().replace("lesson files/Italian slides.pptx", "../../app.db");
        std::fs::write(dir.join("course.json"), text).unwrap();
        assert!(import(&conn, &paths, &dir).is_err());
    }

    #[test]
    fn weeks_and_work_can_be_added_to_a_running_course_without_touching_marks() {
        let (tmp, paths, conn, sealer) = setup();
        let id = create(&conn, "Chess openings").unwrap();
        save(&conn, &id, &sample()).unwrap();
        publish(&conn, &paths, &sealer, &id, Utc::now().date_naive() - Duration::days(1), &LatePolicy::LockZero, Strictness::Structured, true).unwrap();
        crate::course::schedule::tick(&conn, Utc::now()).unwrap();
        let quiz = format!("{id}:u01:1");
        marking::submit(&conn, &paths, &sealer, &quiz, "", &json!({"q1": "Bc4", "q2": "3"}), &[], Utc::now()).unwrap();

        // a new week whose lesson is a document, with a test
        let mut week = DraftUnit::new(2);
        week.title = "The Ruy Lopez".into();
        week.lesson_files = attachments::add(&conn, &paths, &id, &[doc(&tmp, "Ruy Lopez.pdf", "%PDF")]).unwrap();
        week.items = vec![GenItem { kind: "test".into(), title: "Ruy Lopez test".into(), instructions: "Annotate the game.".into(), ..sample().units[0].items[1].clone() }];
        let key = add_week(&conn, &paths, &sealer, &id, week).unwrap();
        assert_eq!(key, "u02");
        // more work in week 1, released now because week 1 has started
        let extra = GenItem { title: "Extra drill".into(), ..sample().units[0].items[0].clone() };
        add_work(&conn, &paths, &sealer, &id, "u01", extra).unwrap();

        let c = crate::course::view(&conn, &paths, &id).unwrap();
        assert_eq!(c.units.len(), 2);
        assert_eq!(c.units[0].items.len(), 4, "lesson, two original pieces of work and the new one");
        let total: f64 = c.units.iter().flat_map(|u| &u.items).map(|i| i.weight).sum();
        assert!((total - 1.0).abs() < 1e-9, "grade shares still add up to 100%: {total}");
        let marked: f64 = conn.query_row("SELECT percent FROM marks WHERE item_id=?1", [&quiz], |r| r.get(0)).unwrap();
        assert_eq!(marked, 100.0, "the existing mark is untouched");
        let added = c.units[0].items.iter().find(|i| i.title == "Extra drill").unwrap();
        assert!(schedule::parse_ts(&added.release_at).unwrap() <= Utc::now() + Duration::seconds(5));

        // courses Claude writes, and drafts, can't be grown this way
        let other = create(&conn, "Draft").unwrap();
        assert!(add_week(&conn, &paths, &sealer, &other, DraftUnit::new(1)).is_err());
    }
}
