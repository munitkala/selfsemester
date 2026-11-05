//! Time estimates for long Claude work (building a course, marking), from how long each kind of step has taken
//! before on this computer.

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

/// Typical seconds per step before there's any history (measured on a one-week course).
fn default_secs(kind: &str) -> f64 {
    match kind {
        "intake" => 10.0,
        "plan" => 45.0,
        "sources" => 30.0,
        "unit" => 190.0,
        "validate" => 60.0,
        "mark" => 60.0,
        _ => 30.0,
    }
}

/// Average seconds a successful step of this kind took, from the last 20 calls.
pub fn typical_secs(conn: &Connection, kind: &str) -> rusqlite::Result<f64> {
    let avg: Option<f64> = conn.query_row(
        "SELECT avg(d) FROM (SELECT c.duration_ms / 1000.0 AS d FROM claude_calls c JOIN jobs j ON j.id = c.job_id
         WHERE j.kind = ?1 AND c.outcome = 'ok' AND c.duration_ms > 0 ORDER BY c.id DESC LIMIT 20)",
        [kind],
        |r| r.get(0),
    )?;
    Ok(avg.unwrap_or_else(|| default_secs(kind)))
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Progress {
    /// What's happening, in words.
    pub stage: String,
    pub total_secs: f64,
    pub remaining_secs: f64,
    /// When the estimate was made; the UI counts down from here.
    pub as_of: String,
    /// Paused for the usage limit, so the countdown is on hold.
    pub paused: bool,
}

fn running_elapsed(conn: &Connection, course_id: &str, now: DateTime<Utc>) -> rusqlite::Result<Option<(String, f64)>> {
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT kind, updated_at FROM jobs WHERE course_id=?1 AND status='running' ORDER BY id LIMIT 1",
            [course_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(row.and_then(|(k, at)| super::schedule::parse_ts(&at).map(|t| (k, (now - t).num_milliseconds() as f64 / 1000.0))))
}

fn paused(conn: &Connection, course_id: &str) -> rusqlite::Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT count(*) FROM jobs WHERE course_id=?1 AND status='waiting_usage'",
        [course_id],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// Estimate for a course that's being set up or built. None when nothing is in progress.
pub fn course(conn: &Connection, course_id: &str, now: DateTime<Utc>) -> rusqlite::Result<Option<Progress>> {
    let status: String = conn.query_row("SELECT status FROM courses WHERE id=?1", [course_id], |r| r.get(0))?;
    let running = running_elapsed(conn, course_id, now)?;
    let as_of = super::schedule::ts(now);
    let paused = paused(conn, course_id)?;

    let (stage, total, mut remaining) = match status.as_str() {
        "intake" | "scoping" => {
            let kind = if status == "intake" { "intake" } else { "plan" };
            let t = typical_secs(conn, kind)?;
            let stage = if status == "intake" { "Writing your questions" } else { "Planning the course" };
            (stage.to_string(), t, t)
        }
        "compiling" => {
            let (src, unit, val) = (typical_secs(conn, "sources")?, typical_secs(conn, "unit")?, typical_secs(conn, "validate")?);
            let mut st = conn.prepare("SELECT state FROM units WHERE course_id=?1")?;
            let states: Vec<String> = st.query_map([course_id], |r| r.get(0))?.collect::<Result<_, _>>()?;
            let sources_done: bool = conn
                .query_row(
                    "SELECT count(*) FROM jobs WHERE course_id=?1 AND kind='sources' AND status='done'",
                    [course_id],
                    |r| r.get::<_, i64>(0),
                )
                .map(|n| n > 0)?;
            let n = states.len() as f64;
            let total = src + n * (unit + val);
            let mut remaining = if sources_done { 0.0 } else { src };
            for s in &states {
                remaining += match s.as_str() {
                    "validated" => 0.0,
                    "generated" => val,
                    _ => unit + val,
                };
            }
            let done_units = states.iter().filter(|s| *s == "validated").count();
            (format!("Building the course: {done_units} of {} units ready", states.len()), total, remaining)
        }
        _ => return Ok(None),
    };
    // Credit time already spent on the step that's running now (never more than its typical length).
    if let Some((kind, secs)) = running {
        remaining -= secs.min(typical_secs(conn, &kind)?).max(0.0);
    }
    remaining = remaining.max(0.0);
    Ok(Some(Progress { stage, total_secs: total, remaining_secs: remaining, as_of, paused }))
}

/// Estimate for marking one submission. None when it isn't waiting for Claude.
pub fn marking(conn: &Connection, submission_id: &str, now: DateTime<Utc>) -> rusqlite::Result<Option<Progress>> {
    let job: Option<(i64, String, String, i64)> = conn
        .query_row(
            "SELECT id, status, updated_at, priority FROM jobs WHERE kind='mark' AND unit_key=?1 ORDER BY id DESC LIMIT 1",
            [submission_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((id, status, updated, priority)) = job else { return Ok(None) };
    if !matches!(status.as_str(), "queued" | "running" | "waiting_usage") {
        return Ok(None);
    }
    let mark = typical_secs(conn, "mark")?;
    // Jobs run one at a time: anything with higher priority, or the same priority but older, goes first.
    let ahead: Vec<String> = {
        let mut st = conn.prepare(
            "SELECT kind FROM jobs WHERE status IN ('queued','running') AND id != ?1 AND (priority > ?2 OR (priority = ?2 AND id < ?1) OR status='running')",
        )?;
        st.query_map(params![id, priority], |r| r.get(0))?.collect::<Result<_, _>>()?
    };
    let mut wait = 0.0;
    for k in &ahead {
        wait += typical_secs(conn, k)?;
    }
    let mut remaining = wait + mark;
    if status == "running" {
        if let Some(t) = super::schedule::parse_ts(&updated) {
            remaining -= ((now - t).num_milliseconds() as f64 / 1000.0).min(mark).max(0.0);
        }
    }
    Ok(Some(Progress {
        stage: if status == "running" { "Marking".into() } else { "Waiting to be marked".into() },
        total_secs: wait + mark,
        remaining_secs: remaining.max(0.0),
        as_of: super::schedule::ts(now),
        paused: status == "waiting_usage",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claude::{StageOutput, Usage};
    use crate::db::{self, jobs};

    #[test]
    fn uses_history_then_defaults() {
        let conn = db::open_in_memory().unwrap();
        assert_eq!(typical_secs(&conn, "unit").unwrap(), 190.0);
        let id = crate::course::pipeline::create(&conn, "Test", "").unwrap();
        let (job, _) = jobs::claim_next(&conn, Utc::now()).unwrap().unwrap();
        let out = StageOutput { value: serde_json::json!({}), usage: Usage { duration_ms: 12_000, ..Default::default() }, model: "m".into() };
        jobs::finish(&conn, job, "sonnet", &Ok(out), Utc::now()).unwrap();
        assert_eq!(typical_secs(&conn, "intake").unwrap(), 12.0);
        let _ = id;
    }

    #[test]
    fn intake_progress_counts_down() {
        let conn = db::open_in_memory().unwrap();
        let id = crate::course::pipeline::create(&conn, "Test", "").unwrap();
        let p = course(&conn, &id, Utc::now()).unwrap().unwrap();
        assert_eq!((p.total_secs, p.remaining_secs), (10.0, 10.0));
        jobs::claim_next(&conn, Utc::now()).unwrap();
        let later = Utc::now() + chrono::Duration::seconds(4);
        let p = course(&conn, &id, later).unwrap().unwrap();
        assert!(p.remaining_secs <= 6.5 && p.remaining_secs >= 5.0, "{}", p.remaining_secs);
    }
}
