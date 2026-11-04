//! The compile pipeline. Each stage is a queued Claude job; when one finishes, `on_job_done` stores its result
//! and queues the next. Course status:
//!   intake -> answering -> scoping -> review -> compiling -> active -> completed
//! A failed stage leaves the status alone and sets `courses.error`; `retry` re-queues it.

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

fn course_id_for(subject: &str) -> String {
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
    jobs::enqueue_with(conn, Some(id), "plan", None, jobs::PRIORITY_INTERACTIVE, &prompts::plan(&subject, &notes, &answers, bundle.as_deref()))
        .map_err(io)?;
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
pub fn approve(conn: &Connection, id: &str, start: NaiveDate, late_policy: &schedule::LatePolicy, on_transcript: bool) -> Result<(), String> {
    let plan = plan_of(conn, id)?;
    conn.execute(
        "UPDATE courses SET title=?2, mode=?3, late_policy=?4, on_transcript=?5, readiness_json=?6, starts_at=?7 WHERE id=?1",
        params![
            id,
            plan.title,
            if plan.mode == "online" { "online" } else { "offline" },
            serde_json::to_string(late_policy).unwrap(),
            on_transcript,
            serde_json::to_string(&plan.readiness).unwrap(),
            start.to_string()
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
    jobs::enqueue_with(conn, Some(id), "sources", None, jobs::PRIORITY_COMPILE, &prompts::sources(&plan)).map_err(io)?;
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
            let plan = plan_of(conn, &id)?;
            let srcs = sources_of(conn, &id)?;
            for u in &plan.units {
                jobs::enqueue_with(conn, Some(&id), "unit", Some(&u.key), jobs::PRIORITY_COMPILE, &prompts::unit(&plan, &srcs, &u.key, None)).map_err(io)?;
            }
        }
        "unit" => {
            let key = unit_key.ok_or("unit job without unit")?;
            let content: UnitContent = serde_json::from_value(value).map_err(|e| e.to_string())?;
            let dir = paths.ensure_course(&id).map_err(|e| e.to_string())?;
            // Contains answer keys, so it's sealed.
            sealer.write_json(&dir.join("units").join(format!("{key}.json")), &content, true).map_err(|e| e.to_string())?;
            conn.execute("UPDATE units SET state='generated' WHERE course_id=?1 AND unit_key=?2", params![id, key]).map_err(io)?;
            let plan = plan_of(conn, &id)?;
            jobs::enqueue_with(conn, Some(&id), "validate", Some(&key), jobs::PRIORITY_COMPILE, &prompts::validate(&plan, &key, &content)).map_err(io)?;
        }
        "validate" => {
            let key = unit_key.ok_or("validate job without unit")?;
            let v: Validation = serde_json::from_value(value).map_err(|e| e.to_string())?;
            let major: Vec<&Issue> = v.issues.iter().filter(|i| i.severity == "major").collect();
            if !major.is_empty() && unit_attempts(conn, &id, &key)? < 2 {
                let plan = plan_of(conn, &id)?;
                let srcs = sources_of(conn, &id)?;
                let fb = major.iter().map(|i| format!("- {} {}: {}", i.item_key, i.question_id, i.problem)).collect::<Vec<_>>().join("\n");
                jobs::enqueue_with(conn, Some(&id), "unit", Some(&key), jobs::PRIORITY_COMPILE, &prompts::unit(&plan, &srcs, &key, Some(&fb)))
                    .map_err(io)?;
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
fn seal_and_start(conn: &Connection, paths: &AppPaths, sealer: &Sealer, id: &str) -> Result<schedule::Notice, String> {
    let plan = plan_of(conn, id)?;
    let start_s: String = conn.query_row("SELECT starts_at FROM courses WHERE id=?1", [id], |r| r.get(0)).map_err(io)?;
    let start = NaiveDate::parse_from_str(&start_s[..10.min(start_s.len())], "%Y-%m-%d").map_err(|e| e.to_string())?;
    let n = schedule::build(conn, paths, sealer, id, &plan, start).map_err(|e| e.to_string())?;
    set_status(conn, id, "active")?;
    Ok(schedule::Notice { title: "Course ready".into(), body: format!("{}: {n} items scheduled from {start}.", plan.title) })
}

/// A job gave up. Record why on the course (or leave the submission waiting, for marking).
pub fn on_job_failed(conn: &Connection, job_id: i64) -> Result<(), String> {
    let (course, kind, _) = jobs::owner(conn, job_id).map_err(io)?;
    let Some(id) = course else { return Ok(()) };
    let err: Option<String> = conn.query_row("SELECT error FROM jobs WHERE id=?1", [job_id], |r| r.get(0)).map_err(io)?;
    conn.execute(
        "UPDATE courses SET error=?2, updated_at=?3 WHERE id=?1",
        params![id, format!("{kind} step failed: {}", err.unwrap_or_default()), db::now_utc()],
    )
    .map_err(io)?;
    Ok(())
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
mod tests {
    use super::*;
    use crate::course::marking;
    use crate::course::schedule::{LatePolicy, tick};
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
        approve(&conn, &id, start, &LatePolicy::Penalty { per_day: 10.0, max_days: 2 }, true).unwrap();
        assert_eq!(status(&conn, &id), "compiling");
        answer(&conn, &paths, &sealer, json!({"sources":[{"url":"https://openstax.org/x","title":"OpenStax","licence":"CC BY 4.0",
            "attribution":"OpenStax","free":true,"unit_keys":["u01","u02"],"note":""}]}));
        // Two unit jobs, then a validation per unit. u01 validation reports a major issue once -> regenerated.
        answer(&conn, &paths, &sealer, unit_json("u01", false));
        answer(&conn, &paths, &sealer, unit_json("u02", true));
        answer(&conn, &paths, &sealer, json!({"ok":false,"issues":[{"item_key":"u01-quiz","question_id":"q1","problem":"wrong","severity":"major"}]}));
        answer(&conn, &paths, &sealer, json!({"ok":true,"issues":[]})); // u02 validated
        answer(&conn, &paths, &sealer, unit_json("u01", false)); // regenerated u01
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

    /// Real Claude Code calls for every compile stage on a tiny course. Uses plan usage; opt-in:
    /// `cargo test --lib -- --ignored live_compile --nocapture`. Writes results to $SS_LIVE_OUT (default: temp dir).
    #[test]
    #[ignore]
    fn live_compile_stages() {
        let out_dir = std::env::var("SS_LIVE_OUT").map(std::path::PathBuf::from).unwrap_or_else(|_| std::env::temp_dir().join("ss-live"));
        std::fs::create_dir_all(&out_dir).unwrap();
        let work = tempfile::tempdir().unwrap();
        let r = CliRunner::new(work.path().to_path_buf());
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
        let unit: UnitContent = serde_json::from_value(run("4-unit", &prompts::unit(&plan, &src.sources, &key, None))).unwrap();
        assert!(!unit.items.is_empty());
        let _v: Validation = serde_json::from_value(run("5-validate", &prompts::validate(&plan, &key, &unit))).unwrap();
        println!("saved to {}", out_dir.display());
    }
}
