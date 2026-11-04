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

/// Atomically take the oldest runnable job (queued, or waiting on usage whose wait is over) and mark it running.
pub fn claim_next(conn: &Connection, now: DateTime<Utc>) -> rusqlite::Result<Option<(i64, StageRequest)>> {
    let now_s = ts(now);
    let row: Option<(i64, String)> = conn
        .query_row(
            "UPDATE jobs SET status = 'running', attempts = attempts + 1, updated_at = ?1
             WHERE id = (
               SELECT id FROM jobs
               WHERE status = 'queued' OR (status = 'waiting_usage' AND next_attempt_at <= ?1)
               ORDER BY priority DESC, id LIMIT 1)
             RETURNING id, request_json",
            [&now_s],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(row.map(|(id, body)| (id, serde_json::from_str(&body).expect("stored request is valid"))))
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
        StageRequest { tier: Tier::Haiku, system: None, prompt: "p".into(), schema: json!({"type":"object"}), tools: vec![], add_dirs: vec![] }
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
    fn interrupted_jobs_are_requeued() {
        let conn = crate::db::open_in_memory().unwrap();
        let id = enqueue(&conn, None, "t", &req()).unwrap();
        claim_next(&conn, Utc::now()).unwrap();
        assert_eq!(requeue_interrupted(&conn).unwrap(), 1);
        assert_eq!(get(&conn, id).unwrap().unwrap().status, "queued");
    }
}
