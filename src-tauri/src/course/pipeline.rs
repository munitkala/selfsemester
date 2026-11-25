//! The compile pipeline. Each stage is a queued Claude job; when one finishes, `on_job_done` stores its result
//! and queues the next. Course status:
//!   intake -> answering -> scoping -> review -> compiling -> active -> completed
//! A failed stage leaves the status alone and sets `courses.error`; `retry` re-queues it.

use super::build::BuildSettings;
use super::materials;
use super::prompts;
use super::schedule;
use super::seal::Sealer;
use super::types::*;
use crate::db::{self, jobs};
use crate::paths::AppPaths;
use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

fn io(e: rusqlite::Error) -> String {
    e.to_string()
}

fn set_status(conn: &Connection, id: &str, status: &str) -> Result<(), String> {
    conn.execute("UPDATE courses SET status=?2, error=NULL, updated_at=?3 WHERE id=?1", params![id, status, db::now_utc()])
        .map_err(io)?;
    Ok(())
}

pub fn default_profile(conn: &Connection) -> Result<String, String> {
    if let Some(id) = conn.query_row("SELECT id FROM profiles ORDER BY created_at LIMIT 1", [], |r| r.get(0)).optional().map_err(io)? {
        return Ok(id);
    }
    conn.execute("INSERT INTO profiles(id, name, created_at) VALUES ('me', 'Me', ?1)", [db::now_utc()]).map_err(io)?;
    Ok("me".into())
}

pub(crate) fn course_id_for(subject: &str) -> String {
    let slug: String = subject
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .take(5)
        .collect::<Vec<_>>()
        .join("-");
    format!("{}-{:x}", if slug.is_empty() { "course".into() } else { slug }, chrono::Utc::now().timestamp_millis() & 0xffffff)
}

/// Step 1: the learner names a subject. Claude writes the intake questions.
pub fn create(conn: &Connection, subject: &str, notes: &str) -> Result<String, String> {
    let subject = subject.trim();
    if subject.is_empty() {
        return Err("Name a subject first.".into());
    }
    let profile = default_profile(conn)?;
    let id = course_id_for(subject);
    let now = db::now_utc();
    conn.execute(
        "INSERT INTO courses(id, profile_id, title, subject, notes, status, created_at, updated_at)
         VALUES (?1,?2,?3,?3,?4,'intake',?5,?5)",
        params![id, profile, subject, notes, now],
    )
    .map_err(io)?;
    jobs::enqueue_with(conn, Some(&id), "intake", None, jobs::PRIORITY_INTERACTIVE, &prompts::intake(subject, notes)).map_err(io)?;
    Ok(id)
}

fn course_text(conn: &Connection, id: &str) -> Result<(String, String, Option<String>, Option<String>), String> {
    conn.query_row("SELECT subject, coalesce(notes,''), intake_json, bundle_choice FROM courses WHERE id=?1", [id], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
    })
    .map_err(io)
}

fn queue_plan(conn: &Connection, id: &str) -> Result<(), String> {
    let (subject, notes, answers, bundle) = course_text(conn, id)?;
    let answers: Value = answers.and_then(|a| serde_json::from_str(&a).ok()).unwrap_or(Value::Null);
    set_status(conn, id, "scoping")?;
    let mut req = prompts::plan(&subject, &notes, &answers, bundle.as_deref());
    materials::attach(conn, id, &mut req, materials::Use::Plan)?;
    jobs::enqueue_with(conn, Some(id), "plan", None, jobs::PRIORITY_INTERACTIVE, &req).map_err(io)?;
    Ok(())
}

/// Step 2: the learner answers the intake questions. Claude plans the course and its readiness report.
pub fn answer_intake(conn: &Connection, id: &str, answers: &Value) -> Result<(), String> {
    conn.execute("UPDATE courses SET intake_json=?2 WHERE id=?1", params![id, answers.to_string()]).map_err(io)?;
    queue_plan(conn, id)
}

/// The plan found a bundle of courses; the learner picked how to take it. Re-plan with that choice.
pub fn choose_bundle(conn: &Connection, id: &str, choice: &str) -> Result<(), String> {
    if !matches!(choice, "combined" | "first" | "sequence") {
        return Err("Unknown choice.".into());
    }
    conn.execute("UPDATE courses SET bundle_choice=?2 WHERE id=?1", params![id, choice]).map_err(io)?;
    queue_plan(conn, id)
}

pub fn plan_of(conn: &Connection, id: &str) -> Result<Plan, String> {
    let s: Option<String> = conn.query_row("SELECT plan_json FROM courses WHERE id=?1", [id], |r| r.get(0)).map_err(io)?;
    serde_json::from_str(&s.ok_or("No plan yet.")?).map_err(|e| e.to_string())
}

/// Step 3: the learner accepted the readiness report. Compile everything: sources, then units.
pub fn approve(
    conn: &Connection,
    id: &str,
    start: NaiveDate,
    late_policy: &schedule::LatePolicy,
    strictness: schedule::Strictness,
    on_transcript: bool,
    build: &BuildSettings,
) -> Result<(), String> {
    let mut plan = plan_of(conn, id)?;
    let build = build.clone().clean();
    super::build::save_for_course(conn, id, &build)?;
    // What to include: drop left-out kinds of work, and the tools text-only lessons won't use.
    let lessons_only = super::build::apply_content(&mut plan, &build.content);
    if !build.content.media {
        plan.tools.clear();
    }
    // A course with nothing graded has no final mark to put on a transcript.
    let on_transcript = on_transcript && !lessons_only;
    conn.execute("UPDATE courses SET plan_json=?2 WHERE id=?1", params![id, serde_json::to_string(&plan).unwrap()]).map_err(io)?;
    conn.execute(
        "UPDATE courses SET title=?2, mode=?3, late_policy=?4, on_transcript=?5, readiness_json=?6, starts_at=?7, strictness=?8
         WHERE id=?1",
        params![
            id,
            plan.title,
            if plan.mode == "online" { "online" } else { "offline" },
            serde_json::to_string(late_policy).unwrap(),
            on_transcript,
            serde_json::to_string(&plan.readiness).unwrap(),
            start.to_string(),
            strictness.as_str()
        ],
    )
    .map_err(io)?;
    conn.execute("DELETE FROM units WHERE course_id=?1", [id]).map_err(io)?;
    for (i, u) in plan.units.iter().enumerate() {
        conn.execute(
            "INSERT INTO units(id, course_id, seq, title, summary, unit_key, week) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![format!("{id}:{}", u.key), id, i as i64, u.title, u.topics.join(", "), u.key, u.week],
        )
        .map_err(io)?;
    }
    set_status(conn, id, "compiling")?;
    let mut req = prompts::sources_up_to(&plan, build.sources);
    build.for_sources(&mut req);
    jobs::enqueue_with(conn, Some(id), "sources", None, jobs::PRIORITY_COMPILE, &req).map_err(io)?;
    Ok(())
}

fn sources_of(conn: &Connection, id: &str) -> Result<Vec<SourceEntry>, String> {
    let mut st = conn.prepare("SELECT url, title, licence, attribution, free, text_ref FROM sources WHERE course_id=?1").map_err(io)?;
    let rows = st
        .query_map([id], |r| {
            Ok(SourceEntry {
                url: r.get(0)?,
                title: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                licence: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                attribution: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                free: r.get(4)?,
                unit_keys: r.get::<_, Option<String>>(5)?.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default(),
                note: String::new(),
            })
        })
        .map_err(io)?;
    rows.collect::<Result<_, _>>().map_err(io)
}

/// How many unit drafts have been queued for this unit (to allow one regeneration after validation).
fn unit_attempts(conn: &Connection, id: &str, key: &str) -> Result<i64, String> {
    conn.query_row("SELECT count(*) FROM jobs WHERE course_id=?1 AND kind='unit' AND unit_key=?2", params![id, key], |r| r.get(0))
        .map_err(io)
}

fn compiling(conn: &Connection, id: &str) -> Result<bool, String> {
    let s: Option<String> = conn.query_row("SELECT status FROM courses WHERE id=?1", [id], |r| r.get(0)).optional().map_err(io)?;
    Ok(s.as_deref() == Some("compiling"))
}

/// Queue a draft of one unit (a rewrite when `feedback` holds the check's major problems).
fn queue_unit(conn: &Connection, paths: &AppPaths, id: &str, key: &str, feedback: Option<&str>) -> Result<(), String> {
    let plan = plan_of(conn, id)?;
    let srcs = sources_of(conn, id)?;
    let build = super::build::of_course(conn, id)?;
    let tool_guide = crate::tools::lesson_guide(&crate::tools::installed(paths));
    let mut req = prompts::unit_styled(&plan, &srcs, key, feedback, &tool_guide, &build.content);
    build.for_unit(&mut req, plan.units.iter().any(|u| u.key == key && u.exam != "none"));
    materials::attach(conn, id, &mut req, materials::Use::Write)?;
    jobs::enqueue_with(conn, Some(id), "unit", Some(key), jobs::PRIORITY_COMPILE, &req).map_err(io)?;
    Ok(())
}

/// Courses are built one week at a time, in order, so that whatever is finished is a usable start of the course
/// (see `finish_with_ready`). Queue the first unit that isn't written yet, unless a unit or check of this course
/// is already waiting or running (courses started by an older version queued every unit at once).
fn queue_next_unit(conn: &Connection, paths: &AppPaths, id: &str) -> Result<(), String> {
    let busy: i64 = conn
        .query_row(
            "SELECT count(*) FROM jobs WHERE course_id=?1 AND kind IN ('unit','validate') AND status IN ('queued','running','waiting_usage')",
            [id],
            |r| r.get(0),
        )
        .map_err(io)?;
    if busy > 0 {
        return Ok(());
    }
    let next: Option<String> = conn
        .query_row("SELECT unit_key FROM units WHERE course_id=?1 AND state='pending' ORDER BY week, seq LIMIT 1", [id], |r| r.get(0))
        .optional()
        .map_err(io)?;
    match next {
        Some(key) => queue_unit(conn, paths, id, &key, None),
        None => Ok(()),
    }
}

/// How far a course being built has got: the weeks at the start of the course that are written and checked (so
/// it could be finished with just those), and how many weeks are planned.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct Readiness {
    /// Units ready to use, counted from the first one up to the first that isn't.
    pub ready_units: usize,
    /// Last week those cover (0 when none is ready).
    pub ready_weeks: u32,
    pub total_units: usize,
    pub total_weeks: u32,
}

/// Keys of the leading units (in course order) that are written and checked.
fn ready_prefix(conn: &Connection, id: &str) -> Result<Vec<(String, u32)>, String> {
    let mut st = conn.prepare("SELECT unit_key, week, state FROM units WHERE course_id=?1 ORDER BY week, seq").map_err(io)?;
    let rows: Vec<(String, u32, String)> =
        st.query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(io)?.collect::<Result<_, _>>().map_err(io)?;
    Ok(rows.into_iter().take_while(|(_, _, s)| s == "validated").map(|(k, w, _)| (k, w)).collect())
}

pub fn readiness(conn: &Connection, id: &str) -> Result<Readiness, String> {
    let ready = ready_prefix(conn, id)?;
    let (total_units, total_weeks): (i64, Option<i64>) = conn
        .query_row("SELECT count(*), max(week) FROM units WHERE course_id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(io)?;
    Ok(Readiness {
        ready_units: ready.len(),
        ready_weeks: ready.iter().map(|(_, w)| *w).max().unwrap_or(0),
        total_units: total_units as usize,
        total_weeks: total_weeks.unwrap_or(0).max(0) as u32,
    })
}

/// End building early and start the course with only the weeks that are ready (the leading units that are
/// written and checked). Everything still to build is dropped: queued steps are cancelled, the plan keeps the
/// left-out weeks in `unbuilt` so the syllabus can show them, and grade weights spread over the work that's left.
/// Returns the notice, and whether a step was running (the caller should stop that Claude call).
pub fn finish_with_ready(conn: &Connection, paths: &AppPaths, sealer: &Sealer, id: &str) -> Result<(schedule::Notice, bool), String> {
    if !compiling(conn, id)? {
        return Err("Only a course that's being built can be finished early.".into());
    }
    let ready = ready_prefix(conn, id)?;
    if ready.is_empty() {
        return Err("No week is ready yet. Pause building instead, or wait until the first week has been written and checked.".into());
    }
    let running: i64 = conn
        .query_row(
            &format!("SELECT count(*) FROM jobs WHERE course_id=?1 AND status='running' AND kind IN {}", jobs::BUILD_KINDS),
            [id],
            |r| r.get(0),
        )
        .map_err(io)?;
    conn.execute(
        &format!(
            "UPDATE jobs SET status='cancelled', updated_at=?2 WHERE course_id=?1 AND kind IN {} AND status IN ('queued','running','waiting_usage')",
            jobs::BUILD_KINDS
        ),
        params![id, db::now_utc()],
    )
    .map_err(io)?;
    let keep: Vec<&str> = ready.iter().map(|(k, _)| k.as_str()).collect();
    let mut plan = plan_of(conn, id)?;
    let (kept, dropped): (Vec<PlanUnit>, Vec<PlanUnit>) = plan.units.drain(..).partition(|u| keep.contains(&u.key.as_str()));
    plan.units = kept;
    plan.unbuilt.extend(dropped);
    plan.pacing.total_weeks = plan.units.iter().map(|u| u.week.max(1)).max().unwrap_or(1);
    conn.execute("UPDATE courses SET plan_json=?2 WHERE id=?1", params![id, serde_json::to_string(&plan).unwrap()]).map_err(io)?;
    let marks = keep.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let mut args: Vec<&dyn rusqlite::ToSql> = vec![&id];
    args.extend(keep.iter().map(|k| k as &dyn rusqlite::ToSql));
    conn.execute(&format!("DELETE FROM units WHERE course_id=?1 AND unit_key NOT IN ({marks})"), args.as_slice()).map_err(io)?;
    let notice = seal_and_start(conn, paths, sealer, id)?;
    Ok((notice, running > 0))
}

/// A job finished successfully: store its result and queue what comes next.
/// Returns a notice for the learner when something worth a toast happened.
pub fn on_job_done(conn: &Connection, paths: &AppPaths, sealer: &Sealer, job_id: i64, model: &str) -> Result<Option<schedule::Notice>, String> {
    let (course, kind, unit_key) = jobs::owner(conn, job_id).map_err(io)?;
    let Some(id) = course else { return Ok(None) };
    let value = jobs::result(conn, job_id).map_err(io)?.ok_or("job has no result")?;
    match kind.as_str() {
        "intake" => {
            let q: IntakeQuestions = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
            conn.execute("UPDATE courses SET questions_json=?2 WHERE id=?1", params![id, serde_json::to_string(&q).unwrap()]).map_err(io)?;
            set_status(conn, &id, "answering")?;
        }
        "plan" => {
            let p: Plan = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
            if p.units.is_empty() {
                return Err("The plan had no units.".into());
            }
            conn.execute("UPDATE courses SET plan_json=?2, title=?3 WHERE id=?1", params![id, value.to_string(), p.title]).map_err(io)?;
            set_status(conn, &id, "review")?;
            return Ok(Some(schedule::Notice { title: "Course plan ready".into(), body: format!("{}: review the readiness report.", p.title) }));
        }
        "sources" => {
            let list: SourceList = serde_json::from_value(value).map_err(|e| e.to_string())?;
            let now = db::now_utc();
            conn.execute("DELETE FROM sources WHERE course_id=?1", [&id]).map_err(io)?;
            for (i, s) in list.sources.iter().enumerate() {
                // unit keys ride in text_ref until local download/extraction (stage 5) fills it with a real path.
                conn.execute(
                    "INSERT OR IGNORE INTO sources(id, course_id, url, title, licence, attribution, free, text_ref, fetched_at)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                    params![format!("{id}:src{i}"), id, s.url, s.title, s.licence, s.attribution, s.free, serde_json::to_string(&s.unit_keys).unwrap(), now],
                )
                .map_err(io)?;
            }
            // One week at a time: write the first, and each check that passes queues the next.
            queue_next_unit(conn, paths, &id)?;
        }
        "unit" | "validate" if !compiling(conn, &id)? => {} // ended early or deleted meanwhile: nothing more to do
        "unit" => {
            let key = unit_key.ok_or("unit job without unit")?;
            let content: UnitContent = serde_json::from_value(value).map_err(|e| e.to_string())?;
            let dir = paths.ensure_course(&id).map_err(|e| e.to_string())?;
            // Contains answer keys, so it's sealed.
            sealer.write_json(&dir.join("units").join(format!("{key}.json")), &content, true).map_err(|e| e.to_string())?;
            conn.execute("UPDATE units SET state='generated' WHERE course_id=?1 AND unit_key=?2", params![id, key]).map_err(io)?;
            let plan = plan_of(conn, &id)?;
            let mut req = prompts::validate(&plan, &key, &content);
            super::build::of_course(conn, &id)?.for_check(&mut req);
            jobs::enqueue_with(conn, Some(&id), "validate", Some(&key), jobs::PRIORITY_COMPILE, &req).map_err(io)?;
        }
        "validate" => {
            let key = unit_key.ok_or("validate job without unit")?;
            let v: Validation = serde_json::from_value(value).map_err(|e| e.to_string())?;
            let major: Vec<&Issue> = v.issues.iter().filter(|i| i.severity == "major").collect();
            let build = super::build::of_course(conn, &id)?;
            if !major.is_empty() && unit_attempts(conn, &id, &key)? < 1 + build.rewrites as i64 {
                let fb = major.iter().map(|i| format!("- {} {}: {}", i.item_key, i.question_id, i.problem)).collect::<Vec<_>>().join("\n");
                queue_unit(conn, paths, &id, &key, Some(&fb))?;
                conn.execute("UPDATE units SET state='pending' WHERE course_id=?1 AND unit_key=?2", params![id, key]).map_err(io)?;
                return Ok(None);
            }
            let warnings: Vec<String> = v.issues.iter().map(|i| format!("{} ({}): {}", i.item_key, i.severity, i.problem)).collect();
            conn.execute(
                "UPDATE units SET state='validated', warnings=?3 WHERE course_id=?1 AND unit_key=?2",
                params![id, key, serde_json::to_string(&warnings).unwrap()],
            )
            .map_err(io)?;
            let left: i64 = conn
                .query_row("SELECT count(*) FROM units WHERE course_id=?1 AND state != 'validated'", [&id], |r| r.get(0))
                .map_err(io)?;
            if left == 0 {
                return seal_and_start(conn, paths, sealer, &id).map(Some);
            }
            queue_next_unit(conn, paths, &id)?;
        }
        "review" => {
            let item = unit_key.ok_or("review job without item")?;
            super::review::on_done(conn, &item, &value)?;
        }
        "mark" => {
            let sub = unit_key.ok_or("mark job without submission")?;
            let item = super::marking::on_mark_done(conn, &sub, &value, model)?;
            let title: String = conn.query_row("SELECT title FROM items WHERE id=?1", [&item], |r| r.get(0)).map_err(io)?;
            return Ok(Some(schedule::Notice { title: "Marks returned".into(), body: title }));
        }
        _ => {}
    }
    Ok(None)
}

/// Every unit is generated and checked: build the schedule and start the course.
pub(crate) fn seal_and_start(conn: &Connection, paths: &AppPaths, sealer: &Sealer, id: &str) -> Result<schedule::Notice, String> {
    let plan = plan_of(conn, id)?;
    let start_s: String = conn.query_row("SELECT starts_at FROM courses WHERE id=?1", [id], |r| r.get(0)).map_err(io)?;
    let start = NaiveDate::parse_from_str(&start_s[..10.min(start_s.len())], "%Y-%m-%d").map_err(|e| e.to_string())?;
    let n = schedule::build(conn, paths, sealer, id, &plan, start).map_err(|e| e.to_string())?;
    set_status(conn, id, "active")?;
    unpause(conn, id)?;
    Ok(schedule::Notice { title: "Course ready".into(), body: format!("{}: {n} items scheduled from {start}.", plan.title) })
}

/// A job gave up. Record why on the course (or leave the submission waiting, for marking).
pub fn on_job_failed(conn: &Connection, job_id: i64) -> Result<(), String> {
    let (course, kind, unit_key) = jobs::owner(conn, job_id).map_err(io)?;
    let Some(id) = course else { return Ok(()) };
    let err: Option<String> = conn.query_row("SELECT error FROM jobs WHERE id=?1", [job_id], |r| r.get(0)).map_err(io)?;
    // a review reply that failed only concerns that conversation, not the whole course
    if kind == "review" {
        if let Some(item) = unit_key {
            super::review::on_failed(conn, &item, err.as_deref().unwrap_or("unknown error"))?;
        }
        return Ok(());
    }
    conn.execute(
        "UPDATE courses SET error=?2, updated_at=?3 WHERE id=?1",
        params![id, format!("{kind} step failed: {}", err.unwrap_or_default()), db::now_utc()],
    )
    .map_err(io)?;
    Ok(())
}

/// Pause building a course. Its build jobs stay in the queue but nothing is sent to Claude until `unpause`.
/// Returns true when one of its steps is running now, so the caller should stop that call too.
pub fn pause(conn: &Connection, id: &str) -> Result<bool, String> {
    let status: String = conn.query_row("SELECT status FROM courses WHERE id=?1", [id], |r| r.get(0)).map_err(io)?;
    if !matches!(status.as_str(), "intake" | "scoping" | "compiling") {
        return Err("Only a course that's being built can be paused.".into());
    }
    conn.execute("UPDATE courses SET paused_at=coalesce(paused_at, ?2) WHERE id=?1", params![id, db::now_utc()]).map_err(io)?;
    let running: i64 = conn
        .query_row(
            &format!("SELECT count(*) FROM jobs WHERE course_id=?1 AND status='running' AND kind IN {}", jobs::BUILD_KINDS),
            [id],
            |r| r.get(0),
        )
        .map_err(io)?;
    Ok(running > 0)
}

/// Carry on building a paused course.
pub fn unpause(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("UPDATE courses SET paused_at=NULL WHERE id=?1", [id]).map_err(io)?;
    Ok(())
}

pub fn is_paused(conn: &Connection, id: &str) -> Result<bool, String> {
    conn.query_row("SELECT paused_at IS NOT NULL FROM courses WHERE id=?1", [id], |r| r.get(0)).map_err(io)
}

/// Re-queue a course's failed jobs.
pub fn retry(conn: &Connection, id: &str) -> Result<usize, String> {
    let n = conn
        .execute(
            "UPDATE jobs SET status='queued', attempts=0, error=NULL, next_attempt_at=NULL, updated_at=?2
             WHERE course_id=?1 AND status='failed'",
            params![id, db::now_utc()],
        )
        .map_err(io)?;
    conn.execute("UPDATE courses SET error=NULL WHERE id=?1", [id]).map_err(io)?;
    Ok(n)
}

/// Remove a course and its files. Its finished archive in Documents (if any) is kept.
pub fn delete(conn: &Connection, paths: &AppPaths, id: &str) -> Result<(), String> {
    conn.execute("UPDATE jobs SET status='cancelled' WHERE course_id=?1 AND status IN ('queued','waiting_usage')", [id]).map_err(io)?;
    conn.execute("DELETE FROM transcript_entries WHERE course_id=?1", [id]).map_err(io)?;
    conn.execute("DELETE FROM courses WHERE id=?1", [id]).map_err(io)?;
    let dir = paths.course_dir(id);
    if dir.starts_with(paths.courses_dir()) && dir.is_dir() {
        std::fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::course::marking;
    use crate::course::schedule::{LatePolicy, Strictness, tick};
    use chrono::{Duration, Utc};
    use serde_json::json;

    /// Pretend Claude answered the oldest queued job with `value`.
    fn answer(conn: &Connection, paths: &AppPaths, sealer: &Sealer, value: Value) -> Option<schedule::Notice> {
        let (id, _req) = jobs::claim_next(conn, Utc::now()).unwrap().expect("a queued job");
        let out = crate::claude::StageOutput { value, usage: Default::default(), model: "test-model".into() };
        jobs::finish(conn, id, "test", &Ok(out), Utc::now()).unwrap();
        on_job_done(conn, paths, sealer, id, "test-model").unwrap()
    }

    fn status(conn: &Connection, id: &str) -> String {
        conn.query_row("SELECT status FROM courses WHERE id=?1", [id], |r| r.get(0)).unwrap()
    }

    fn unit_json(key: &str, exam: bool) -> Value {
        let mut items = vec![
            json!({"key": format!("{key}-quiz"), "kind":"quiz","title":"Quiz","module":"quiz","instructions":"Answer.",
                "questions":[{"id":"q1","prompt":"2+2?","type":"numeric","options":[],"points":2},
                             {"id":"q2","prompt":"Capital?","type":"mcq","options":["Paris","Rome"],"points":2}],
                "rubric":[], "answer_key":[{"question_id":"q1","answer":"4","explanation":"","tolerance":0},
                                           {"question_id":"q2","answer":"Paris","explanation":"","tolerance":0}],
                "due_days":3}),
            json!({"key": format!("{key}-hw"), "kind":"homework","title":"Homework","module":"written","instructions":"Explain.",
                "questions":[{"id":"h1","prompt":"Explain waves","type":"long","options":[],"points":10}],
                "rubric":[{"criterion":"clarity","points":10,"description":""}],
                "answer_key":[{"question_id":"h1","answer":"...","explanation":"","tolerance":0}], "due_days":7}),
        ];
        if exam {
            items.push(json!({"key": format!("{key}-exam"), "kind":"exam","title":"Final exam","module":"written","instructions":"Exam.",
                "questions":[{"id":"e1","prompt":"Derive","type":"long","options":[],"points":20}],
                "rubric":[{"criterion":"method","points":20,"description":""}],
                "answer_key":[{"question_id":"e1","answer":"...","explanation":"","tolerance":0}], "due_days":2}));
        }
        json!({"lesson":{"title": format!("Lesson {key}"),"markdown":"Text\n\n## Summary\n\n- point","est_minutes":30,"sources_used":[]},"items":items})
    }

    #[test]
    fn build_level_sets_models_effort_and_rewrites() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = AppPaths::rooted(&tmp.path().join("l"), &tmp.path().join("d"));
        paths.ensure().unwrap();
        let conn = db::open_in_memory().unwrap();
        let sealer = Sealer::load(&conn).unwrap();
        let id = create(&conn, "Quantum mechanics 1", "").unwrap();
        answer(&conn, &paths, &sealer, json!({"questions":[]}));
        answer_intake(&conn, &id, &json!({})).unwrap();
        answer(&conn, &paths, &sealer, crate::course::prompts::tests::sample_plan());
        let quick = crate::course::build::preset(crate::course::build::Level::Quick);
        approve(&conn, &id, Utc::now().date_naive(), &LatePolicy::LockZero, Strictness::Structured, true, &quick).unwrap();
        assert_eq!(crate::course::build::of_course(&conn, &id).unwrap(), quick);
        assert_eq!(crate::course::build::last_used(&conn), quick);
        let queued = |kind: &str| -> Vec<(String, String)> {
            let mut st = conn.prepare("SELECT model, request_json FROM jobs WHERE course_id=?1 AND kind=?2 ORDER BY id").unwrap();
            st.query_map(params![id, kind], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(|r| r.unwrap()).collect()
        };
        let src = queued("sources");
        assert_eq!(src[0].0, "sonnet");
        assert!(src[0].1.contains("\"effort\":\"low\"") && src[0].1.contains("Aim for 2-4 sources"));
        answer(&conn, &paths, &sealer, json!({"sources":[]}));
        // Quick writes even the exam unit with Sonnet.
        assert!(queued("unit").iter().all(|(m, _)| m == "sonnet"));
        assert!(queued("unit")[0].1.contains("runs in a sealed frame")); // figures stay on by default
        answer(&conn, &paths, &sealer, unit_json("u01", false));
        assert!(queued("validate").iter().all(|(m, _)| m == "haiku"));
        // No rewrites on Quick: a major issue is kept as a warning instead.
        answer(&conn, &paths, &sealer, json!({"ok":false,"issues":[{"item_key":"u01-quiz","question_id":"q1","problem":"wrong","severity":"major"}]}));
        answer(&conn, &paths, &sealer, unit_json("u02", true));
        answer(&conn, &paths, &sealer, json!({"ok":true,"issues":[]}));
        assert_eq!(queued("unit").len(), 2);
        assert_eq!(status(&conn, &id), "active");
    }

    #[test]
    fn lessons_only_course_has_no_work_and_finishes_without_a_mark() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = AppPaths::rooted(&tmp.path().join("l"), &tmp.path().join("d"));
        paths.ensure().unwrap();
        let conn = db::open_in_memory().unwrap();
        let sealer = Sealer::load(&conn).unwrap();
        let id = create(&conn, "Quantum mechanics 1", "").unwrap();
        answer(&conn, &paths, &sealer, json!({"questions":[]}));
        answer_intake(&conn, &id, &json!({})).unwrap();
        answer(&conn, &paths, &sealer, crate::course::prompts::tests::sample_plan());
        let mut b = BuildSettings::default();
        b.content.skip = crate::course::build::WORK_KINDS.iter().map(|k| k.to_string()).collect();
        b.content.media = false;
        b.content.detail = crate::course::build::Detail::InDepth;
        let start = (Utc::now() - Duration::days(30)).date_naive();
        approve(&conn, &id, start, &LatePolicy::LockZero, Strictness::Structured, true, &b).unwrap();
        let on_transcript: bool = conn.query_row("SELECT on_transcript FROM courses WHERE id=?1", [&id], |r| r.get(0)).unwrap();
        assert!(!on_transcript);
        let plan = plan_of(&conn, &id).unwrap();
        assert!(plan.units.iter().all(|u| u.exam == "none"));
        answer(&conn, &paths, &sealer, json!({"sources":[]}));
        let prompt: String = conn
            .query_row("SELECT request_json FROM jobs WHERE course_id=?1 AND kind='unit' LIMIT 1", [&id], |r| r.get(0))
            .unwrap();
        assert!(prompt.contains("lessons only") && prompt.contains("TEXT ONLY") && prompt.contains("WRITE IN DEPTH"));
        assert!(!prompt.contains("runs in a sealed frame"));
        let lesson = |k: &str| json!({"lesson":{"title": format!("Lesson {k}"),"markdown":"Text","est_minutes":30,"sources_used":[]},"items":[]});
        answer(&conn, &paths, &sealer, lesson("u01"));
        answer(&conn, &paths, &sealer, json!({"ok":true,"issues":[]}));
        answer(&conn, &paths, &sealer, lesson("u02"));
        answer(&conn, &paths, &sealer, json!({"ok":true,"issues":[]}));
        assert_eq!(status(&conn, &id), "active");
        let graded: i64 = conn.query_row("SELECT count(*) FROM items WHERE course_id=?1 AND kind!='lesson'", [&id], |r| r.get(0)).unwrap();
        assert_eq!(graded, 0);
        tick(&conn, Utc::now()).unwrap();
        let archive = marking::maybe_complete(&conn, &paths, &sealer, &id).unwrap().expect("completed");
        let pct: Option<f64> = conn.query_row("SELECT final_percent FROM courses WHERE id=?1", [&id], |r| r.get(0)).unwrap();
        assert_eq!(pct, None);
        let entries: i64 = conn.query_row("SELECT count(*) FROM transcript_entries", [], |r| r.get(0)).unwrap();
        assert_eq!(entries, 0);
        assert!(std::fs::read_to_string(archive.join("00 Course summary.md")).unwrap().contains("Lessons only"));
    }

    /// A course built all the way to active, in a database file under `dir` (for tests that need a real course).
    pub(crate) fn built_course(dir: &std::path::Path) -> (AppPaths, Connection, String) {
        let paths = AppPaths::rooted(dir, dir);
        paths.ensure().unwrap();
        let conn = db::open(&paths.db_file()).unwrap();
        let sealer = Sealer::load(&conn).unwrap();
        let id = create(&conn, "Quantum mechanics 1", "").unwrap();
        answer(&conn, &paths, &sealer, json!({"questions":[]}));
        answer_intake(&conn, &id, &json!({})).unwrap();
        answer(&conn, &paths, &sealer, crate::course::prompts::tests::sample_plan());
        let start = Utc::now().date_naive() + Duration::days(3);
        approve(&conn, &id, start, &LatePolicy::LockZero, Strictness::Structured, true, &BuildSettings::default()).unwrap();
        answer(&conn, &paths, &sealer, json!({"sources":[]}));
        answer(&conn, &paths, &sealer, unit_json("u01", false));
        answer(&conn, &paths, &sealer, json!({"ok":true,"issues":[]}));
        answer(&conn, &paths, &sealer, unit_json("u02", true));
        answer(&conn, &paths, &sealer, json!({"ok":true,"issues":[]}));
        assert_eq!(status(&conn, &id), "active");
        (paths, conn, id)
    }

    /// A course approved and its reading list found, ready to write units.
    fn compiling_course(paths: &AppPaths, conn: &Connection, sealer: &Sealer, build: &BuildSettings) -> String {
        let id = create(conn, "Quantum mechanics 1", "").unwrap();
        answer(conn, paths, sealer, json!({"questions":[]}));
        answer_intake(conn, &id, &json!({})).unwrap();
        answer(conn, paths, sealer, crate::course::prompts::tests::sample_plan());
        approve(conn, &id, Utc::now().date_naive(), &LatePolicy::LockZero, Strictness::Structured, true, build).unwrap();
        answer(conn, paths, sealer, json!({"sources":[]}));
        id
    }

    #[test]
    fn finish_early_keeps_the_ready_weeks_and_drops_the_rest() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = AppPaths::rooted(&tmp.path().join("l"), &tmp.path().join("d"));
        paths.ensure().unwrap();
        let conn = db::open_in_memory().unwrap();
        let sealer = Sealer::load(&conn).unwrap();
        let id = compiling_course(&paths, &conn, &sealer, &BuildSettings::default());
        // Nothing ready yet: it can't be finished, only paused.
        assert!(finish_with_ready(&conn, &paths, &sealer, &id).unwrap_err().contains("No week is ready"));
        answer(&conn, &paths, &sealer, unit_json("u01", false));
        answer(&conn, &paths, &sealer, json!({"ok":true,"issues":[{"item_key":"u01-hw","question_id":"h1","problem":"minor","severity":"minor"}]}));
        // Week 2 is being written when building is ended.
        let (u02, _) = jobs::claim_next(&conn, Utc::now()).unwrap().unwrap();
        let (_, running) = finish_with_ready(&conn, &paths, &sealer, &id).unwrap();
        assert!(running, "the caller is told to stop the call in progress");
        assert_eq!(status(&conn, &id), "active");
        let plan = plan_of(&conn, &id).unwrap();
        assert_eq!(plan.units.iter().map(|u| u.key.as_str()).collect::<Vec<_>>(), ["u01"]);
        assert_eq!(plan.unbuilt.iter().map(|u| u.key.as_str()).collect::<Vec<_>>(), ["u02"]);
        assert_eq!(plan.pacing.total_weeks, 1);
        let units: i64 = conn.query_row("SELECT count(*) FROM units WHERE course_id=?1", [&id], |r| r.get(0)).unwrap();
        assert_eq!(units, 1);
        // Only week 1's work is scheduled, and the dropped final exam's share moves to what's left.
        let kinds: Vec<String> = {
            let mut st = conn.prepare("SELECT DISTINCT kind FROM items WHERE course_id=?1 ORDER BY kind").unwrap();
            st.query_map([&id], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect()
        };
        assert_eq!(kinds, ["homework", "lesson", "quiz"]);
        let w: f64 = conn.query_row("SELECT sum(weight) FROM items WHERE course_id=?1", [&id], |r| r.get(0)).unwrap();
        assert!((w - 1.0).abs() < 1e-9, "{w}");
        // The stopped week-2 call comes back late: it stays cancelled and nothing more is queued or written.
        let out = crate::claude::StageOutput { value: unit_json("u02", true), usage: Default::default(), model: "m".into() };
        jobs::finish(&conn, u02, "m", &Ok(out), Utc::now()).unwrap();
        let st: String = conn.query_row("SELECT status FROM jobs WHERE id=?1", [u02], |r| r.get(0)).unwrap();
        assert_eq!(st, "cancelled");
        assert!(jobs::claim_next(&conn, Utc::now()).unwrap().is_none());
        // A finished course can't be finished early again.
        assert!(finish_with_ready(&conn, &paths, &sealer, &id).is_err());
    }

    #[test]
    fn pause_after_the_step_keeps_its_result_and_holds_the_next_week() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = AppPaths::rooted(&tmp.path().join("l"), &tmp.path().join("d"));
        paths.ensure().unwrap();
        let conn = db::open_in_memory().unwrap();
        let sealer = Sealer::load(&conn).unwrap();
        let id = compiling_course(&paths, &conn, &sealer, &BuildSettings::default());
        let (job, _) = jobs::claim_next(&conn, Utc::now()).unwrap().unwrap();
        assert!(pause(&conn, &id).unwrap(), "a step of this course is running");
        // Not stopped: the draft finishes and is stored, and its check is queued but held.
        let out = crate::claude::StageOutput { value: unit_json("u01", false), usage: Default::default(), model: "m".into() };
        jobs::finish(&conn, job, "m", &Ok(out), Utc::now()).unwrap();
        on_job_done(&conn, &paths, &sealer, job, "m").unwrap();
        let state: String = conn.query_row("SELECT state FROM units WHERE course_id=?1 AND unit_key='u01'", [&id], |r| r.get(0)).unwrap();
        assert_eq!(state, "generated");
        assert!(jobs::claim_next(&conn, Utc::now()).unwrap().is_none(), "held while paused");
        let left = crate::course::build::remaining(&conn, &id).unwrap();
        assert_eq!((left.units_left, left.calls_min, left.calls_max), (2, 3, 7));
        assert!(!left.heavy);
        unpause(&conn, &id).unwrap();
        answer(&conn, &paths, &sealer, json!({"ok":true,"issues":[]}));
        let next: String = conn
            .query_row("SELECT unit_key FROM jobs WHERE course_id=?1 AND status='queued'", [&id], |r| r.get(0))
            .unwrap();
        assert_eq!(next, "u02");
    }

    #[test]
    fn a_course_from_an_older_version_with_every_unit_queued_gets_no_duplicates() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = AppPaths::rooted(&tmp.path().join("l"), &tmp.path().join("d"));
        paths.ensure().unwrap();
        let conn = db::open_in_memory().unwrap();
        let sealer = Sealer::load(&conn).unwrap();
        let id = compiling_course(&paths, &conn, &sealer, &BuildSettings::default());
        queue_unit(&conn, &paths, &id, "u02", None).unwrap(); // as 0.7 did: every unit queued after sources
        answer(&conn, &paths, &sealer, unit_json("u01", false));
        answer(&conn, &paths, &sealer, unit_json("u02", true));
        answer(&conn, &paths, &sealer, json!({"ok":true,"issues":[]}));
        answer(&conn, &paths, &sealer, json!({"ok":true,"issues":[]}));
        assert_eq!(status(&conn, &id), "active");
        let drafts: i64 = conn.query_row("SELECT count(*) FROM jobs WHERE course_id=?1 AND kind='unit'", [&id], |r| r.get(0)).unwrap();
        assert_eq!(drafts, 2);
    }

    #[test]
    fn whole_course_lifecycle() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = AppPaths::rooted(&tmp.path().join("l"), &tmp.path().join("d"));
        paths.ensure().unwrap();
        let conn = db::open_in_memory().unwrap();
        let sealer = Sealer::load(&conn).unwrap();

        let id = create(&conn, "Quantum mechanics 1", "").unwrap();
        assert_eq!(status(&conn, &id), "intake");
        answer(&conn, &paths, &sealer, json!({"questions":[{"id":"goal","text":"Goal?","kind":"text","options":[]}]}));
        assert_eq!(status(&conn, &id), "answering");

        answer_intake(&conn, &id, &json!({"goal":"learn"})).unwrap();
        assert_eq!(status(&conn, &id), "scoping");
        answer(&conn, &paths, &sealer, crate::course::prompts::tests::sample_plan());
        assert_eq!(status(&conn, &id), "review");

        let start = (Utc::now() - Duration::days(1)).date_naive();
        approve(&conn, &id, start, &LatePolicy::Penalty { per_day: 10.0, max_days: 2 }, Strictness::Structured, true, &BuildSettings::default()).unwrap();
        assert_eq!(status(&conn, &id), "compiling");
        answer(&conn, &paths, &sealer, json!({"sources":[{"url":"https://openstax.org/x","title":"OpenStax","licence":"CC BY 4.0",
            "attribution":"OpenStax","free":true,"unit_keys":["u01","u02"],"note":""}]}));
        // One week at a time: u01, its check (a major issue -> rewritten, checked again), then u02 and its check.
        let queued_units = || -> i64 {
            conn.query_row("SELECT count(*) FROM jobs WHERE course_id=?1 AND kind='unit' AND status='queued'", [&id], |r| r.get(0)).unwrap()
        };
        assert_eq!(queued_units(), 1, "only the first week is queued");
        answer(&conn, &paths, &sealer, unit_json("u01", false));
        assert_eq!(queued_units(), 0, "the next week waits for this one's check");
        answer(&conn, &paths, &sealer, json!({"ok":false,"issues":[{"item_key":"u01-quiz","question_id":"q1","problem":"wrong","severity":"major"}]}));
        answer(&conn, &paths, &sealer, unit_json("u01", false)); // rewritten u01
        assert_eq!(readiness(&conn, &id).unwrap().ready_units, 0);
        answer(&conn, &paths, &sealer, json!({"ok":true,"issues":[]})); // u01 validated
        assert_eq!(readiness(&conn, &id).unwrap(), Readiness { ready_units: 1, ready_weeks: 1, total_units: 2, total_weeks: 2 });
        answer(&conn, &paths, &sealer, unit_json("u02", true));
        let n = answer(&conn, &paths, &sealer, json!({"ok":true,"issues":[]})).expect("course ready notice");
        assert!(n.title.contains("ready"));
        assert_eq!(status(&conn, &id), "active");

        // Weights: homework 0.4, quiz 0.2, exam 0.4 -> sum of item weights is 1.
        let w: f64 = conn.query_row("SELECT sum(weight) FROM items WHERE course_id=?1", [&id], |r| r.get(0)).unwrap();
        assert!((w - 1.0).abs() < 1e-9, "{w}");
        // Exam content is sealed on disk; the quiz isn't.
        let exam_ref: String = conn.query_row("SELECT content_ref FROM items WHERE course_id=?1 AND kind='exam'", [&id], |r| r.get(0)).unwrap();
        assert!(std::fs::read(paths.course_dir(&id).join(&exam_ref)).unwrap().starts_with(b"SSv1"));

        // Week 1 releases now (start was yesterday).
        let notes = tick(&conn, Utc::now()).unwrap();
        assert!(notes.iter().any(|n| n.title.starts_with("New lesson")));
        let quiz: String = conn.query_row("SELECT id FROM items WHERE course_id=?1 AND kind='quiz' AND state='open' LIMIT 1", [&id], |r| r.get(0)).unwrap();
        let out = marking::submit(&conn, &paths, &sealer, &quiz, "", &json!({"q1":"4","q2":"Rome"}), &[], Utc::now()).unwrap();
        assert_eq!(out.status, "marked");
        let pct: f64 = conn.query_row("SELECT percent FROM marks WHERE item_id=?1", [&quiz], |r| r.get(0)).unwrap();
        assert_eq!(pct, 50.0);
        assert!(marking::submit(&conn, &paths, &sealer, &quiz, "", &json!({"q1":"4"}), &[], Utc::now()).is_err()); // only once

        // Written homework goes to Claude; pretend Claude marked it 8/10, submitted on time.
        let hw: String = conn.query_row("SELECT id FROM items WHERE course_id=?1 AND kind='homework' AND state='open' LIMIT 1", [&id], |r| r.get(0)).unwrap();
        let out = marking::submit(&conn, &paths, &sealer, &hw, "Waves wave.", &json!({}), &[], Utc::now()).unwrap();
        assert_eq!(out.status, "queued");
        let n = answer(&conn, &paths, &sealer, json!({"lines":[{"criterion":"clarity","awarded":8,"possible":10,"comment":"ok"}],"overall_comment":"Good","revise":[]}));
        assert_eq!(n.unwrap().title, "Marks returned");
        let g = marking::grades(&conn, &id).unwrap();
        assert!(g.current.is_some() && !g.complete);

        // Far in the future everything is released and every unsubmitted item is missed -> course completes.
        let later = Utc::now() + Duration::days(60);
        tick(&conn, later).unwrap();
        tick(&conn, later).unwrap();
        let archive = marking::maybe_complete(&conn, &paths, &sealer, &id).unwrap().expect("completed");
        assert_eq!(status(&conn, &id), "completed");
        assert!(archive.join("00 Course summary.md").is_file());
        assert!(archive.join("05 Reference list.md").is_file());
        let on_transcript: i64 = conn.query_row("SELECT count(*) FROM transcript_entries WHERE course_id=?1", [&id], |r| r.get(0)).unwrap();
        assert_eq!(on_transcript, 1);
    }

    /// Builds demo data for README screenshots from saved stage outputs (1-intake.json, 2-plan.json,
    /// 3-sources.json, 4-unit.json, as written by `live_compile_stages`). Makes two separate homes for
    /// `SELFSEMESTER_HOME`: `<out>/review` (a plan waiting for approval) and `<out>/active` (a running course).
    /// `SS_DEMO_DATA=<stage outputs> SS_DEMO_OUT=<dir> cargo test --lib -- --ignored seed_demo`
    #[test]
    #[ignore]
    fn seed_demo() {
        let data = std::path::PathBuf::from(std::env::var("SS_DEMO_DATA").expect("SS_DEMO_DATA"));
        let out = std::path::PathBuf::from(std::env::var("SS_DEMO_OUT").expect("SS_DEMO_OUT"));
        let load = |f: &str| -> Value { serde_json::from_str(&std::fs::read_to_string(data.join(f)).unwrap()).unwrap() };
        let (intake, plan, sources, unit) = (load("1-intake.json"), load("2-plan.json"), load("3-sources.json"), load("4-unit.json"));
        let answers = json!({"goal":"Use Git for my own personal projects","level":"Never used either","hours_per_week":"3",
            "finish_date":"2026-11-12","equipment":"Windows","git_installed":"Git is installed","online_offline":"Mostly online",
            "remote_hosting":"Willing to create one"});

        let home = |name: &str| {
            let h = out.join(name);
            let _ = std::fs::remove_dir_all(&h);
            let paths = AppPaths::rooted(&h.join("AppData"), &h.join("Documents"));
            paths.ensure().unwrap();
            let conn = db::open(&paths.db_file()).unwrap();
            let sealer = Sealer::load(&conn).unwrap();
            (paths, conn, sealer)
        };

        // 1. A plan waiting for review: shows the readiness report.
        let (paths, conn, sealer) = home("review");
        let id = create(&conn, "Git basics", "Keep it to one week.").unwrap();
        answer(&conn, &paths, &sealer, intake.clone());
        answer_intake(&conn, &id, &answers).unwrap();
        answer(&conn, &paths, &sealer, plan.clone());
        drop(conn);

        // 2. A course part way through building (first unit written, waiting to be checked). Run the app with
        //    SELFSEMESTER_NO_CLAUDE=1 so it stays there.
        let (paths, conn, sealer) = home("building");
        let id = create(&conn, "Git basics", "Keep it to one week.").unwrap();
        answer(&conn, &paths, &sealer, intake.clone());
        answer_intake(&conn, &id, &answers).unwrap();
        answer(&conn, &paths, &sealer, plan.clone());
        approve(&conn, &id, Utc::now().date_naive(), &LatePolicy::LockZero, Strictness::Structured, true, &BuildSettings::default()).unwrap();
        answer(&conn, &paths, &sealer, sources.clone());
        answer(&conn, &paths, &sealer, unit.clone());
        drop(conn);

        // 3. A running course built from the first unit, with the quiz already taken.
        let (paths, conn, sealer) = home("active");
        let mut one_unit = plan.clone();
        one_unit["units"] = json!([plan["units"][0].clone()]);
        let id = create(&conn, "Git basics", "Keep it to one week.").unwrap();
        answer(&conn, &paths, &sealer, intake);
        answer_intake(&conn, &id, &answers).unwrap();
        answer(&conn, &paths, &sealer, one_unit);
        let start = (Utc::now() - Duration::days(1)).date_naive();
        approve(&conn, &id, start, &LatePolicy::Penalty { per_day: 10.0, max_days: 2 }, Strictness::Structured, true, &BuildSettings::default()).unwrap();
        answer(&conn, &paths, &sealer, sources);
        answer(&conn, &paths, &sealer, unit.clone());
        answer(&conn, &paths, &sealer, json!({"ok": true, "issues": [{"item_key":"u01-assign","question_id":"a2",
            "problem":"Rubric points for the setup steps could be split more clearly.","severity":"minor"}]}));
        tick(&conn, Utc::now()).unwrap();
        // Take the quiz: right answers for the first seven questions, wrong for the rest.
        let quiz = unit["items"].as_array().unwrap().iter().find(|i| i["module"] == "quiz").unwrap();
        let mut given = serde_json::Map::new();
        for (n, k) in quiz["answer_key"].as_array().unwrap().iter().enumerate() {
            let a = if n < 7 { k["answer"].as_str().unwrap().to_string() } else { "0".into() };
            given.insert(k["question_id"].as_str().unwrap().into(), json!(a));
        }
        let quiz_id: String = conn
            .query_row("SELECT id FROM items WHERE course_id=?1 AND module='quiz'", [&id], |r| r.get(0))
            .unwrap();
        marking::submit(&conn, &paths, &sealer, &quiz_id, "", &Value::Object(given), &[], Utc::now()).unwrap();
        println!("demo homes in {}", out.display());
    }

    #[test]
    fn bundle_choice_replans() {
        let conn = db::open_in_memory().unwrap();
        let id = create(&conn, "QM 1, 2 and 3", "").unwrap();
        choose_bundle(&conn, &id, "first").unwrap();
        assert_eq!(status(&conn, &id), "scoping");
        assert!(choose_bundle(&conn, &id, "weird").is_err());
    }

    #[test]
    fn failure_sets_error_and_retry_clears_it() {
        let conn = db::open_in_memory().unwrap();
        let id = create(&conn, "Godot", "").unwrap();
        let (job, _) = jobs::claim_next(&conn, Utc::now()).unwrap().unwrap();
        jobs::finish(&conn, job, "sonnet", &Err(crate::claude::RunError::NotSignedIn), Utc::now()).unwrap();
        on_job_failed(&conn, job).unwrap();
        let err: Option<String> = conn.query_row("SELECT error FROM courses WHERE id=?1", [&id], |r| r.get(0)).unwrap();
        assert!(err.unwrap().contains("intake step failed"));
        assert_eq!(retry(&conn, &id).unwrap(), 1);
        assert!(jobs::claim_next(&conn, Utc::now()).unwrap().is_some());
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;
    use crate::claude::{ClaudeRunner, CliRunner};
    use serde_json::json;

    /// Real calls for every compile stage on a tiny course. Uses plan usage; opt-in:
    /// `cargo test --lib -- --ignored live_compile --nocapture`. Writes results to $SS_LIVE_OUT (default: temp dir).
    /// Goes through Claude Code, or through another service when SELFSEMESTER_TEST_ENDPOINT and
    /// SELFSEMESTER_TEST_MODEL are set (SELFSEMESTER_TEST_KEY too, if it needs one).
    #[test]
    #[ignore]
    fn live_compile_stages() {
        let out_dir = std::env::var("SS_LIVE_OUT").map(std::path::PathBuf::from).unwrap_or_else(|_| std::env::temp_dir().join("ss-live"));
        std::fs::create_dir_all(&out_dir).unwrap();
        let work = tempfile::tempdir().unwrap();
        let r: Box<dyn ClaudeRunner> = match std::env::var("SELFSEMESTER_TEST_ENDPOINT") {
            Ok(url) => Box::new(crate::claude::endpoint::EndpointRunner::single(
                crate::claude::endpoint::Endpoint {
                    base_url: url,
                    model: std::env::var("SELFSEMESTER_TEST_MODEL").expect("set SELFSEMESTER_TEST_MODEL"),
                    api_key: std::env::var("SELFSEMESTER_TEST_KEY").unwrap_or_default(),
                    ..Default::default()
                },
                work.path().to_path_buf(),
            )),
            Err(_) => Box::new(CliRunner::new(work.path().to_path_buf())),
        };
        let save = |name: &str, v: &Value| std::fs::write(out_dir.join(format!("{name}.json")), serde_json::to_string_pretty(v).unwrap()).unwrap();
        let run = |name: &str, req: &crate::claude::StageRequest| {
            let t = std::time::Instant::now();
            let o = r.run(req).unwrap_or_else(|e| panic!("{name}: {e}"));
            println!("{name}: {} in {:.0}s, out {} tokens", o.model, t.elapsed().as_secs_f64(), o.usage.output_tokens);
            save(name, &o.value);
            o.value
        };

        let q: IntakeQuestions = serde_json::from_value(run("1-intake", &prompts::intake("Git basics", "Keep it to one week."))).unwrap();
        assert!((4..=8).contains(&q.questions.len()), "{} questions", q.questions.len());

        let answers = json!({"goal":"use git for my own projects","level":"never used it","hours":"3","finish":"in one week",
                             "equipment":"Windows laptop, Git installed","online":"online"});
        let plan: Plan = serde_json::from_value(run("2-plan", &prompts::plan("Git basics", "Keep it to one week.", &answers, None))).unwrap();
        assert!(!plan.units.is_empty());

        let src: SourceList = serde_json::from_value(run("3-sources", &prompts::sources(&plan))).unwrap();
        assert!(!src.sources.is_empty());

        let key = plan.units[0].key.clone();
        let unit: UnitContent = serde_json::from_value(run("4-unit", &prompts::unit(&plan, &src.sources, &key, None, ""))).unwrap();
        assert!(!unit.items.is_empty());
        let _v: Validation = serde_json::from_value(run("5-validate", &prompts::validate(&plan, &key, &unit))).unwrap();
        println!("saved to {}", out_dir.display());
    }
    /// One unit of a sample course written by real Claude Code calls through the normal lesson and checking prompts,
    /// for the README's example lessons. Uses plan usage; opt-in, one subject per run:
    /// `SS_SAMPLE=calculus SS_LIVE_OUT=<dir> cargo test --lib -- --ignored live_sample_unit --nocapture`
    /// Subjects: calculus, physics, statistics, chemistry, algorithms, spreadsheets.
    #[test]
    #[ignore]
    fn live_sample_unit() {
        let which = std::env::var("SS_SAMPLE").expect("SS_SAMPLE");
        let out_dir = std::env::var("SS_LIVE_OUT").map(std::path::PathBuf::from).unwrap_or_else(|_| std::env::temp_dir().join("ss-live"));
        std::fs::create_dir_all(&out_dir).unwrap();
        let src = |title: &str, url: &str, licence: &str, attribution: &str| {
            json!({"url": url, "title": title, "licence": licence, "attribution": attribution, "free": true, "unit_keys": ["u01"], "note": ""})
        };
        let (title, summary, family, unit, topics, source) = match which.as_str() {
            "calculus" => ("Calculus I", "Limits, derivatives and their uses, for first-year university students.", "theory-maths",
                "Derivatives and tangent lines", vec!["slope of a tangent line", "derivative as a limit", "power rule", "reading a graph of f and f'"],
                src("Calculus Volume 1", "https://openstax.org/details/books/calculus-volume-1", "CC BY-NC-SA 4.0", "OpenStax, Rice University")),
            "physics" => ("Introductory Mechanics", "Forces and motion, the first half of a first-year physics course.", "physics",
                "Newton's laws and forces on a slope", vec!["free-body diagrams", "resolving forces on an incline", "friction", "Newton's second law"],
                src("University Physics Volume 1", "https://openstax.org/details/books/university-physics-volume-1", "CC BY 4.0", "OpenStax, Rice University")),
            "statistics" => ("Introductory Statistics", "Describing data and reasoning with probability distributions.", "statistics",
                "The normal distribution", vec!["the normal curve", "z-scores", "the 68-95-99.7 rule", "areas under the curve"],
                src("Introductory Statistics 2e", "https://openstax.org/details/books/introductory-statistics-2e", "CC BY 4.0", "OpenStax, Rice University")),
            "chemistry" => ("General Chemistry", "Atoms, bonding and molecular structure for a first chemistry course.", "chemistry",
                "Molecular shapes and VSEPR theory", vec!["electron domains", "VSEPR shapes", "bond angles", "lone pairs"],
                src("Chemistry 2e", "https://openstax.org/details/books/chemistry-2e", "CC BY 4.0", "OpenStax, Rice University")),
            "algorithms" => ("Algorithms and Data Structures", "Classic algorithms and how to reason about their cost.", "computing",
                "Binary search and logarithmic time", vec!["searching a sorted list", "binary search step by step", "O(log n) growth", "off-by-one errors"],
                src("Open Data Structures", "https://opendatastructures.org/", "CC BY 2.5", "opendatastructures.org")),
            "spreadsheets" => ("Spreadsheets with LibreOffice Calc", "Practical spreadsheets for budgets and reports, using LibreOffice Calc.", "software-tool",
                "Formulas, cell references and charts", vec!["entering formulas", "relative and absolute references", "SUM and AVERAGE", "making a chart"],
                src("Calc Guide", "https://documentation.libreoffice.org/en/english-documentation/", "GPL-3.0 or CC BY 4.0", "LibreOffice Documentation Team")),
            other => panic!("unknown sample {other}"),
        };
        let plan: Plan = serde_json::from_value(json!({
            "title": title, "summary": summary, "bundle": {"detected": false, "parts": [], "recommendation": ""},
            "mode": "offline", "dependencies": [], "pacing": {"total_weeks": 8, "hours_per_week": 2, "fits_user_hours": true, "note": ""},
            "units": [{"key": "u01", "title": unit, "week": 1, "topics": topics, "exam": "none"}],
            "weights": {"homework": 0.4, "assignment": 0, "quiz": 0.2, "test": 0, "exam": 0.4, "project": 0},
            "readiness": {"delivery": [], "marking": [], "overall_delivery": "High", "overall_marking": "High", "weak_spots": [], "basis": ""},
            "domain_family": family, "unsupported_inputs": []
        }))
        .unwrap();
        let sources: Vec<SourceEntry> = vec![serde_json::from_value(source).unwrap()];
        let work = tempfile::tempdir().unwrap();
        let r = CliRunner::new(work.path().to_path_buf());
        let t = std::time::Instant::now();
        let o = r.run(&prompts::unit(&plan, &sources, "u01", None, "")).unwrap_or_else(|e| panic!("unit: {e}"));
        println!("{which} unit: {} in {:.0}s, out {} tokens", o.model, t.elapsed().as_secs_f64(), o.usage.output_tokens);
        let mut unit_value = o.value;
        let mut unit: UnitContent = serde_json::from_value(unit_value.clone()).unwrap();
        let mut v = r.run(&prompts::validate(&plan, "u01", &unit)).unwrap_or_else(|e| panic!("validate: {e}"));
        // Same rule as a real course: major problems send the unit back once, with the checker's notes.
        let check: Validation = serde_json::from_value(v.value.clone()).unwrap();
        let major: Vec<&Issue> = check.issues.iter().filter(|i| i.severity == "major").collect();
        if !major.is_empty() {
            let fb = major.iter().map(|i| format!("- {} {}: {}", i.item_key, i.question_id, i.problem)).collect::<Vec<_>>().join("\n");
            println!("{which}: {} major issue(s), rewriting", major.len());
            unit_value = r.run(&prompts::unit(&plan, &sources, "u01", Some(&fb), "")).unwrap_or_else(|e| panic!("rewrite: {e}")).value;
            unit = serde_json::from_value(unit_value.clone()).unwrap();
            v = r.run(&prompts::validate(&plan, "u01", &unit)).unwrap_or_else(|e| panic!("validate: {e}"));
        }
        std::fs::write(out_dir.join(format!("{which}-unit.json")), serde_json::to_string_pretty(&unit_value).unwrap()).unwrap();
        std::fs::write(out_dir.join(format!("{which}-validate.json")), serde_json::to_string_pretty(&v.value).unwrap()).unwrap();
        println!("saved {which} to {}", out_dir.display());
    }
}
