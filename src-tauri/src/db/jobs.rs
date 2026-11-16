//! The Claude job queue. Every compile stage and marking call is a row here, so work survives crashes,
//! closed laptops, and usage limits: a limited job waits until its `next_attempt_at` instead of retrying hot.

use crate::claude::output::next_attempt;
use crate::claude::{ClaudeRunner, RunError, StageOutput, StageRequest};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

/// Give up on a job after this many non-limit failures (usage-limit waits don't count).
pub const MAX_FAILURES: i64 = 3;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Job {
    pub id: i64,
    pub course_id: Option<String>,
    pub kind: String,
    pub status: String,
    pub attempts: i64,
    pub next_attempt_at: Option<String>,
    pub error: Option<String>,
}

fn ts(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Job priorities: interactive stages the user is waiting on, then marking, then background compile work.
pub const PRIORITY_INTERACTIVE: i64 = 20;
pub const PRIORITY_MARKING: i64 = 10;
pub const PRIORITY_COMPILE: i64 = 0;

/// Jobs that build a course. Pausing a course holds these; marking and replies carry on.
pub const BUILD_KINDS: &str = "('intake','plan','sources','unit','validate')";

/// SQL condition (on a `jobs` row) that is true when the job is held because its course's building is paused.
pub fn held_sql() -> String {
    format!("(kind IN {BUILD_KINDS} AND course_id IN (SELECT id FROM courses WHERE paused_at IS NOT NULL))")
}

pub fn enqueue(conn: &Connection, course_id: Option<&str>, kind: &str, req: &StageRequest) -> rusqlite::Result<i64> {
    enqueue_with(conn, course_id, kind, None, PRIORITY_INTERACTIVE, req)
}

pub fn enqueue_with(
    conn: &Connection,
    course_id: Option<&str>,
    kind: &str,
    unit_key: Option<&str>,
    priority: i64,
    req: &StageRequest,
) -> rusqlite::Result<i64> {
    let now = super::now_utc();
    let body = serde_json::to_string(req).expect("request serialises");
    conn.execute(
        "INSERT INTO jobs(course_id, kind, unit_key, priority, model, request_json, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?7)",
        params![course_id, kind, unit_key, priority, req.tier.alias(), body, now],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Result JSON of a finished job.
pub fn result(conn: &Connection, id: i64) -> rusqlite::Result<Option<serde_json::Value>> {
    let s: Option<String> = conn.query_row("SELECT result_json FROM jobs WHERE id=?1", [id], |r| r.get(0)).optional()?.flatten();
    Ok(s.and_then(|s| serde_json::from_str(&s).ok()))
}

/// Who a job belongs to: (course_id, kind, unit_key).
pub fn owner(conn: &Connection, id: i64) -> rusqlite::Result<(Option<String>, String, Option<String>)> {
    conn.query_row("SELECT course_id, kind, unit_key FROM jobs WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
}

pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<Job>> {
    conn.query_row(
        "SELECT id, course_id, kind, status, attempts, next_attempt_at, error FROM jobs WHERE id = ?1",
        [id],
        |r| {
            Ok(Job {
                id: r.get(0)?,
                course_id: r.get(1)?,
                kind: r.get(2)?,
                status: r.get(3)?,
                attempts: r.get(4)?,
                next_attempt_at: r.get(5)?,
                error: r.get(6)?,
            })
        },
    )
    .optional()
}

/// Atomically take the oldest runnable job and mark it running. Usage waits that are over all go back in the queue
/// first, so the app stops showing them as paused as soon as it starts again. Jobs of a paused course stay put.
pub fn claim_next(conn: &Connection, now: DateTime<Utc>) -> rusqlite::Result<Option<(i64, StageRequest)>> {
    let now_s = ts(now);
    conn.execute(
        "UPDATE jobs SET status = 'queued', next_attempt_at = NULL WHERE status = 'waiting_usage' AND next_attempt_at <= ?1",
        [&now_s],
    )?;
    let sql = format!(
        "UPDATE jobs SET status = 'running', attempts = attempts + 1, updated_at = ?1
         WHERE id = (
           SELECT id FROM jobs WHERE status = 'queued' AND NOT {held}
           ORDER BY priority DESC, id LIMIT 1)
         RETURNING id, request_json",
        held = held_sql()
    );
    let row: Option<(i64, String)> = conn
        .query_row(
            &sql,
            [&now_s],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(row.map(|(id, body)| (id, serde_json::from_str(&body).expect("stored request is valid"))))
}

/// Put jobs paused by a usage limit back in the queue now (all of them, or one course's). Returns how many.
/// If the limit is still in force they simply pause again; a refused call uses no usage.
pub fn resume_waiting(conn: &Connection, course_id: Option<&str>, now: DateTime<Utc>) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE jobs SET status = 'queued', next_attempt_at = NULL, updated_at = ?1
         WHERE status = 'waiting_usage' AND (?2 IS NULL OR course_id = ?2)",
        params![ts(now), course_id],
    )
}

/// Jobs left 'running' by a crash go back in the queue at launch.
pub fn requeue_interrupted(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute("UPDATE jobs SET status = 'queued', updated_at = ?1 WHERE status = 'running'", [super::now_utc()])
}

fn log_call(conn: &Connection, job_id: i64, model: &str, out: Option<&StageOutput>, outcome: &str) -> rusqlite::Result<()> {
    let u = out.map(|o| o.usage.clone()).unwrap_or_default();
    conn.execute(
        "INSERT INTO claude_calls(job_id, model, input_tokens, output_tokens, cache_tokens, duration_ms, outcome, at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            job_id,
            out.map(|o| o.model.as_str()).unwrap_or(model),
            u.input_tokens as i64,
            u.output_tokens as i64,
            u.cache_tokens as i64,
            u.duration_ms as i64,
            outcome,
            super::now_utc()
        ],
    )?;
    Ok(())
}

/// Record the outcome of one attempt.
pub fn finish(conn: &Connection, id: i64, model: &str, result: &Result<StageOutput, RunError>, now: DateTime<Utc>) -> rusqlite::Result<()> {
    let now_s = ts(now);
    match result {
        Ok(out) => {
            log_call(conn, id, model, Some(out), "ok")?;
            conn.execute(
                "UPDATE jobs SET status='done', result_json=?2, error=NULL, next_attempt_at=NULL, updated_at=?3 WHERE id=?1",
                params![id, out.value.to_string(), now_s],
            )?;
        }
        Err(RunError::UsageLimited { retry_at, message }) => {
            log_call(conn, id, model, None, "usage_limited")?;
            let attempts: i64 = conn.query_row("SELECT attempts FROM jobs WHERE id=?1", [id], |r| r.get(0))?;
            let next = next_attempt(*retry_at, attempts as u32, now);
            conn.execute(
                "UPDATE jobs SET status='waiting_usage', error=?2, next_attempt_at=?3, updated_at=?4 WHERE id=?1",
                params![id, message, ts(next), now_s],
            )?;
            // The limit is on the whole plan, so everything else waiting would be refused too: hold it all until then.
            conn.execute(
                "UPDATE jobs SET status='waiting_usage', error=?2, next_attempt_at=?3, updated_at=?4 WHERE status='queued' AND id != ?1",
                params![id, message, ts(next), now_s],
            )?;
        }
        Err(RunError::Stopped) => {
            // Stopped on purpose (the course was paused): back in the queue, and this attempt doesn't count.
            log_call(conn, id, model, None, "stopped")?;
            conn.execute(
                "UPDATE jobs SET status='queued', attempts=max(attempts-1, 0), error=NULL, updated_at=?2 WHERE id=?1",
                params![id, now_s],
            )?;
        }
        Err(e) => {
            log_call(conn, id, model, None, "error")?;
            // Signed-out / not-installed / oversized requests won't fix themselves: fail now so the UI can say why.
            let attempts: i64 = conn.query_row("SELECT attempts FROM jobs WHERE id=?1", [id], |r| r.get(0))?;
            let give_up = !e.is_transient() || attempts >= MAX_FAILURES;
            if give_up {
                conn.execute(
                    "UPDATE jobs SET status='failed', error=?2, updated_at=?3 WHERE id=?1",
                    params![id, e.to_string(), now_s],
                )?;
            } else {
                conn.execute(
                    "UPDATE jobs SET status='queued', error=?2, updated_at=?3 WHERE id=?1",
                    params![id, e.to_string(), now_s],
                )?;
            }
        }
    }
    Ok(())
}

/// Claim and run one job. Returns the job id it ran, or None when nothing is runnable.
/// The connection isn't held while Claude runs, so the UI stays responsive.
pub fn run_one<R: ClaudeRunner + ?Sized>(
    conn: &std::sync::Mutex<Connection>,
    runner: &R,
) -> rusqlite::Result<Option<i64>> {
    run_one_with(conn, runner, |_| {})
}

/// Like `run_one`, calling `started(job_id)` once the job is claimed and before Claude runs.
pub fn run_one_with<R: ClaudeRunner + ?Sized>(
    conn: &std::sync::Mutex<Connection>,
    runner: &R,
    started: impl FnOnce(i64),
) -> rusqlite::Result<Option<i64>> {
    let claimed = claim_next(&conn.lock().unwrap(), Utc::now())?;
    let Some((id, req)) = claimed else { return Ok(None) };
    started(id);
    let result = runner.run(&req);
    finish(&conn.lock().unwrap(), id, req.tier.alias(), &result, Utc::now())?;
    Ok(Some(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claude::{ClaudeStatus, Tier, Usage};
    use serde_json::json;
    use std::sync::Mutex;

    fn req() -> StageRequest {
        StageRequest { tier: Tier::Haiku, system: None, prompt: "p".into(), schema: json!({"type":"object"}), tools: vec![], add_dirs: vec![], effort: None }
    }

    struct Fake(Mutex<Vec<Result<StageOutput, RunError>>>);
    impl ClaudeRunner for Fake {
        fn status(&self) -> ClaudeStatus {
            ClaudeStatus::default()
        }
        fn run(&self, _: &StageRequest) -> Result<StageOutput, RunError> {
            self.0.lock().unwrap().remove(0)
        }
    }
    fn ok() -> Result<StageOutput, RunError> {
        Ok(StageOutput { value: json!({"x":1}), usage: Usage { output_tokens: 9, ..Default::default() }, model: "claude-haiku-5-5".into() })
    }

    #[test]
    fn success_path_logs_usage() {
        let conn = Mutex::new(crate::db::open_in_memory().unwrap());
        let id = enqueue(&conn.lock().unwrap(), None, "test", &req()).unwrap();
        let fake = Fake(Mutex::new(vec![ok()]));
        assert_eq!(run_one(&conn, &fake).unwrap(), Some(id));
        let c = conn.lock().unwrap();
        assert_eq!(get(&c, id).unwrap().unwrap().status, "done");
        let tokens: i64 = c.query_row("SELECT output_tokens FROM claude_calls WHERE job_id=?1", [id], |r| r.get(0)).unwrap();
        assert_eq!(tokens, 9);
        drop(c);
        assert_eq!(run_one(&conn, &fake).unwrap(), None); // queue empty
    }

    #[test]
    fn usage_limit_waits_then_resumes() {
        let conn = crate::db::open_in_memory().unwrap();
        let id = enqueue(&conn, None, "unit", &req()).unwrap();
        let t0 = Utc::now();
        claim_next(&conn, t0).unwrap().unwrap();
        let limited: Result<StageOutput, RunError> = Err(RunError::UsageLimited { retry_at: None, message: "limit".into() });
        finish(&conn, id, "haiku", &limited, t0).unwrap();
        let j = get(&conn, id).unwrap().unwrap();
        assert_eq!(j.status, "waiting_usage");
        // Not runnable before the wait is over...
        assert!(claim_next(&conn, t0 + chrono::Duration::minutes(5)).unwrap().is_none());
        // ...runnable after it.
        assert!(claim_next(&conn, t0 + chrono::Duration::minutes(16)).unwrap().is_some());
    }

    #[test]
    fn permanent_errors_fail_fast_and_transient_ones_retry() {
        let conn = crate::db::open_in_memory().unwrap();
        let a = enqueue(&conn, None, "t", &req()).unwrap();
        claim_next(&conn, Utc::now()).unwrap();
        finish(&conn, a, "haiku", &Err(RunError::NotSignedIn), Utc::now()).unwrap();
        assert_eq!(get(&conn, a).unwrap().unwrap().status, "failed");

        let b = enqueue(&conn, None, "t", &req()).unwrap();
        for i in 1..=MAX_FAILURES {
            claim_next(&conn, Utc::now()).unwrap().unwrap();
            finish(&conn, b, "haiku", &Err(RunError::Timeout(1)), Utc::now()).unwrap();
            let want = if i < MAX_FAILURES { "queued" } else { "failed" };
            assert_eq!(get(&conn, b).unwrap().unwrap().status, want);
        }
    }

    #[test]
    fn resume_makes_waiting_jobs_runnable() {
        let conn = crate::db::open_in_memory().unwrap();
        let course = crate::course::pipeline::create(&conn, "Test", "").unwrap();
        claim_next(&conn, Utc::now()).unwrap().unwrap(); // the intake job create() queued
        let id = enqueue(&conn, Some(&course), "unit", &req()).unwrap();
        let t0 = Utc::now();
        claim_next(&conn, t0).unwrap().unwrap();
        let limited: Result<StageOutput, RunError> = Err(RunError::UsageLimited { retry_at: None, message: "limit".into() });
        finish(&conn, id, "haiku", &limited, t0).unwrap();
        assert!(claim_next(&conn, t0).unwrap().is_none());
        assert_eq!(resume_waiting(&conn, Some("other"), t0).unwrap(), 0);
        assert_eq!(resume_waiting(&conn, Some(&course), t0).unwrap(), 1);
        assert_eq!(claim_next(&conn, t0).unwrap().unwrap().0, id);
    }

    #[test]
    fn a_usage_limit_holds_the_whole_queue_and_resume_releases_it() {
        let conn = crate::db::open_in_memory().unwrap();
        let a = enqueue(&conn, None, "t", &req()).unwrap();
        let b = enqueue(&conn, None, "t", &req()).unwrap();
        let t0 = Utc::now();
        claim_next(&conn, t0).unwrap().unwrap();
        let limited: Result<StageOutput, RunError> = Err(RunError::UsageLimited { retry_at: None, message: "limit".into() });
        finish(&conn, a, "haiku", &limited, t0).unwrap();
        assert_eq!(get(&conn, b).unwrap().unwrap().status, "waiting_usage");
        assert!(claim_next(&conn, t0).unwrap().is_none());
        // Resume puts every waiting job back in the queue at once, so nothing still reads as paused.
        assert_eq!(resume_waiting(&conn, None, t0).unwrap(), 2);
        assert_eq!(get(&conn, a).unwrap().unwrap().status, "queued");
        assert_eq!(get(&conn, b).unwrap().unwrap().status, "queued");
        // Same when the wait runs out on its own.
        claim_next(&conn, t0).unwrap().unwrap();
        finish(&conn, a, "haiku", &limited, t0).unwrap();
        let later = t0 + chrono::Duration::hours(6);
        claim_next(&conn, later).unwrap().unwrap();
        assert_eq!(get(&conn, b).unwrap().unwrap().status, "queued");
    }

    #[test]
    fn paused_courses_keep_their_build_steps_and_stopped_calls_requeue() {
        let conn = crate::db::open_in_memory().unwrap();
        let course = crate::course::pipeline::create(&conn, "Test", "").unwrap();
        conn.execute("UPDATE courses SET status='compiling' WHERE id=?1", [&course]).unwrap();
        let intake = claim_next(&conn, Utc::now()).unwrap().unwrap().0;
        assert!(crate::course::pipeline::pause(&conn, &course).unwrap()); // the intake step is running
        finish(&conn, intake, "haiku", &Err(RunError::Stopped), Utc::now()).unwrap();
        let j = get(&conn, intake).unwrap().unwrap();
        assert_eq!((j.status.as_str(), j.attempts), ("queued", 0));
        let unit = enqueue_with(&conn, Some(&course), "unit", None, PRIORITY_COMPILE, &req()).unwrap();
        let mark = enqueue_with(&conn, Some(&course), "mark", None, PRIORITY_MARKING, &req()).unwrap();
        let s = crate::course::job_summary(&conn, Some(&course)).unwrap();
        assert_eq!((s.queued, s.paused), (1, 2));
        // Marking isn't held; building is.
        assert_eq!(claim_next(&conn, Utc::now()).unwrap().unwrap().0, mark);
        assert!(claim_next(&conn, Utc::now()).unwrap().is_none());
        crate::course::pipeline::unpause(&conn, &course).unwrap();
        assert_eq!(claim_next(&conn, Utc::now()).unwrap().unwrap().0, intake);
        assert_eq!(claim_next(&conn, Utc::now()).unwrap().unwrap().0, unit);
    }

    #[test]
    fn interrupted_jobs_are_requeued() {
        let conn = crate::db::open_in_memory().unwrap();
        let id = enqueue(&conn, None, "t", &req()).unwrap();
        claim_next(&conn, Utc::now()).unwrap();
        assert_eq!(requeue_interrupted(&conn).unwrap(), 1);
        assert_eq!(get(&conn, id).unwrap().unwrap().status, "queued");
    }
}
