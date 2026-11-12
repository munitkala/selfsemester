//! Turning generated units into a dated schedule, and the clock that releases work and enforces deadlines.

use super::seal::Sealer;
use super::types::*;
use crate::paths::AppPaths;
use chrono::{DateTime, Duration, Local, NaiveDate, NaiveTime, TimeZone, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

/// What happens when a deadline passes. Chosen per course at review time.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LatePolicy {
    /// Item locks at the deadline and scores zero.
    LockZero,
    /// Late work accepted for `max_days`, losing `per_day` percentage points per started day.
    Penalty { per_day: f64, max_days: u32 },
}

impl LatePolicy {
    pub fn parse(s: &str) -> Self {
        serde_json::from_str(s).unwrap_or(LatePolicy::LockZero)
    }
    pub fn grace(self) -> Duration {
        match self {
            LatePolicy::LockZero => Duration::zero(),
            LatePolicy::Penalty { max_days, .. } => Duration::days(max_days as i64),
        }
    }
    /// Percentage points lost for work `late_days` late.
    pub fn penalty(self, late_days: i64) -> f64 {
        match self {
            LatePolicy::LockZero => 0.0,
            LatePolicy::Penalty { per_day, .. } => (per_day * late_days.max(0) as f64).min(100.0),
        }
    }
}

/// How strictly a course treats attempts and deadlines. Chosen per course when enrolling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strictness {
    /// One attempt per item, strict marking, the late policy applies.
    Structured,
    /// Resubmit as often as you like with no late penalty. An item closes when you reveal its answer key, and late
    /// work is taken until SANDBOX_GRACE after the course's last deadline.
    Sandbox,
}

impl Strictness {
    pub fn parse(s: &str) -> Self {
        if s == "sandbox" { Strictness::Sandbox } else { Strictness::Structured }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Strictness::Structured => "structured",
            Strictness::Sandbox => "sandbox",
        }
    }
}

/// How long a sandbox course keeps taking work after its last deadline.
pub const SANDBOX_GRACE_DAYS: i64 = 7;

/// When a sandbox course stops taking work: a week after its last deadline.
pub fn sandbox_closes(ends_at: Option<&str>) -> Option<DateTime<Utc>> {
    ends_at.and_then(parse_ts).map(|t| t + Duration::days(SANDBOX_GRACE_DAYS))
}

/// Started days late (0 if on time).
pub fn late_days(due: DateTime<Utc>, at: DateTime<Utc>) -> i64 {
    if at <= due { 0 } else { ((at - due).num_seconds() + 86_399) / 86_400 }
}

pub fn parse_ts(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&Utc))
}
pub fn ts(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// A local wall-clock time on a date, as UTC.
fn local_at(date: NaiveDate, h: u32, m: u32) -> DateTime<Utc> {
    let naive = date.and_time(NaiveTime::from_hms_opt(h, m, 0).unwrap());
    Local.from_local_datetime(&naive).earliest().unwrap_or_else(|| Utc.from_utc_datetime(&naive).with_timezone(&Local)).with_timezone(&Utc)
}

fn item_points(it: &GenItem) -> f64 {
    let q: f64 = it.questions.iter().map(|q| q.points).sum();
    if q > 0.0 { q } else { it.rubric.iter().map(|r| r.points).sum::<f64>().max(1.0) }
}

fn kind_weight(w: &Weights, kind: &str) -> f64 {
    match kind {
        "homework" => w.homework,
        "assignment" => w.assignment,
        "quiz" => w.quiz,
        "test" => w.test,
        "exam" => w.exam,
        "project" => w.project,
        _ => 0.0,
    }
}

/// Build the dated schedule for a fully generated course and write each item's files.
/// Lessons release at 08:00 local on the first day of their week; graded work with them (exams 5 days later);
/// deadlines are at 23:59 local.
pub fn build(
    conn: &Connection,
    paths: &AppPaths,
    sealer: &Sealer,
    course_id: &str,
    plan: &Plan,
    start: NaiveDate,
) -> std::io::Result<usize> {
    let dir = paths.ensure_course(course_id)?;
    let io = |e: rusqlite::Error| std::io::Error::other(e);
    conn.execute("DELETE FROM items WHERE course_id=?1", [course_id]).map_err(io)?;

    // Load every unit's generated content.
    let mut units: Vec<(String, String, u32, UnitContent)> = Vec::new();
    for u in &plan.units {
        let unit_id: String = conn
            .query_row("SELECT id FROM units WHERE course_id=?1 AND unit_key=?2", params![course_id, u.key], |r| r.get(0))
            .map_err(io)?;
        let content: UnitContent = sealer.read_json(&dir.join("units").join(format!("{}.json", u.key)))?;
        units.push((unit_id, u.key.clone(), u.week.max(1), content));
    }

    let mut n = 0;
    for (unit_id, key, week, content) in &units {
        n += schedule_unit(conn, &dir, sealer, course_id, unit_id, key, *week, content, start, None)?;
    }
    reweight(conn, course_id, &plan.weights).map_err(io)?;
    let first = local_at(start, 8, 0);
    let last_due: Option<String> = conn
        .query_row("SELECT max(due_at) FROM items WHERE course_id=?1", [course_id], |r| r.get(0))
        .map_err(io)?;
    conn.execute(
        "UPDATE courses SET starts_at=?2, ends_at=?3, updated_at=?4 WHERE id=?1",
        params![course_id, ts(first), last_due, crate::db::now_utc()],
    )
    .map_err(io)?;
    Ok(n)
}

/// Share of the final grade for each graded item: each kind's weight, split evenly across that kind's items. Kinds
/// with no items drop out and the rest are scaled back up to 100%. Run after items are added.
pub fn reweight(conn: &Connection, course_id: &str, weights: &Weights) -> rusqlite::Result<()> {
    let mut st = conn.prepare("SELECT kind, count(*) FROM items WHERE course_id=?1 AND kind != 'lesson' GROUP BY kind")?;
    let counts: Vec<(String, i64)> = st.query_map([course_id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
    let used: f64 = counts.iter().map(|(k, _)| kind_weight(weights, k)).sum();
    let total: i64 = counts.iter().map(|(_, c)| c).sum();
    for (kind, count) in &counts {
        let w = if used > 0.0 { kind_weight(weights, kind) / used / *count as f64 } else { 1.0 / total.max(1) as f64 };
        conn.execute("UPDATE items SET weight=?3 WHERE course_id=?1 AND kind=?2", params![course_id, kind, w])?;
    }
    Ok(())
}

/// Schedule one unit: its lesson at 08:00 local on the first day of its week, then its graded work. With
/// `not_before`, nothing is released earlier than that (for weeks added to a course that's already running).
/// Weights are set afterwards by `reweight`.
#[allow(clippy::too_many_arguments)]
pub fn schedule_unit(
    conn: &Connection,
    dir: &std::path::Path,
    sealer: &Sealer,
    course_id: &str,
    unit_id: &str,
    key: &str,
    week: u32,
    content: &UnitContent,
    start: NaiveDate,
    not_before: Option<DateTime<Utc>>,
) -> std::io::Result<usize> {
    let io = |e: rusqlite::Error| std::io::Error::other(e);
    let day0 = start + Duration::days(7 * (week.max(1) as i64 - 1));
    let mut release = local_at(day0, 8, 0);
    if let Some(t) = not_before {
        release = release.max(t);
    }

    let lesson_id = format!("{course_id}:{key}:lesson");
    let lesson_ref = format!("items/{key}-lesson.md");
    std::fs::create_dir_all(dir.join("items"))?;
    std::fs::write(dir.join(&lesson_ref), format!("# {}\n\n{}", content.lesson.title, content.lesson.markdown))?;
    if !content.lesson.files.is_empty() {
        std::fs::write(dir.join(lesson_files_ref(key)), serde_json::to_string(&content.lesson.files)?)?;
    }
    conn.execute(
        "INSERT INTO items(id, course_id, unit_id, seq, kind, title, module, release_at, content_ref)
         VALUES (?1,?2,?3,0,'lesson',?4,'none',?5,?6)",
        params![lesson_id, course_id, unit_id, content.lesson.title, ts(release), lesson_ref],
    )
    .map_err(io)?;
    for (i, it) in content.items.iter().enumerate() {
        schedule_item(conn, dir, sealer, course_id, unit_id, key, i + 1, it, release, not_before)?;
    }
    Ok(1 + content.items.len())
}

/// Where a lesson's attached documents are listed, next to the lesson itself.
pub fn lesson_files_ref(key: &str) -> String {
    format!("items/{key}-lesson.files.json")
}

/// Schedule one piece of graded work in a unit released at `unit_release` (exams 5 days later). Its weight is 0
/// until `reweight` runs.
#[allow(clippy::too_many_arguments)]
pub fn schedule_item(
    conn: &Connection,
    dir: &std::path::Path,
    sealer: &Sealer,
    course_id: &str,
    unit_id: &str,
    key: &str,
    index: usize,
    it: &GenItem,
    unit_release: DateTime<Utc>,
    not_before: Option<DateTime<Utc>>,
) -> std::io::Result<()> {
    let io = |e: rusqlite::Error| std::io::Error::other(e);
    let id = format!("{course_id}:{key}:{index}");
    let mut rel = if it.kind == "exam" { unit_release + Duration::days(5) } else { unit_release };
    if let Some(t) = not_before {
        rel = rel.max(t);
    }
    let rel_local = rel.with_timezone(&Local).date_naive();
    let due = local_at(rel_local + Duration::days(it.due_days.clamp(1, 21) as i64), 23, 59);
    let sealed = matches!(it.kind.as_str(), "exam" | "test");
    let public = ItemPublic {
        title: it.title.clone(),
        kind: it.kind.clone(),
        module: it.module.clone(),
        instructions: it.instructions.clone(),
        questions: it.questions.clone(),
        rubric: it.rubric.clone(),
        files: it.files.clone(),
    };
    let content_ref = format!("{}/{key}-{index}.json", if sealed { "sealed" } else { "items" });
    let key_ref = format!("sealed/{key}-{index}.key");
    std::fs::create_dir_all(dir.join("items"))?;
    std::fs::create_dir_all(dir.join("sealed"))?;
    sealer.write_json(&dir.join(&content_ref), &public, sealed)?;
    sealer.write_json(&dir.join(&key_ref), &ItemKey { answer_key: it.answer_key.clone() }, true)?;
    conn.execute(
        "INSERT INTO items(id, course_id, unit_id, seq, kind, title, module, points, weight, release_at, due_at,
                           sealed, content_ref, key_ref)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,0,?9,?10,?11,?12,?13)",
        params![id, course_id, unit_id, index as i64, it.kind, it.title, it.module, item_points(it),
                ts(rel), ts(due), sealed, content_ref, key_ref],
    )
    .map_err(io)?;
    Ok(())
}

/// Something the learner should hear about.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Notice {
    pub title: String,
    pub body: String,
}

fn notified(conn: &Connection, item: &str, tag: &str) -> rusqlite::Result<bool> {
    let s: String = conn.query_row("SELECT notified FROM items WHERE id=?1", [item], |r| r.get(0))?;
    let mut v: Vec<String> = serde_json::from_str(&s).unwrap_or_default();
    if v.iter().any(|t| t == tag) {
        return Ok(true);
    }
    v.push(tag.into());
    conn.execute("UPDATE items SET notified=?2 WHERE id=?1", params![item, serde_json::to_string(&v).unwrap()])?;
    Ok(false)
}

/// Advance the clock: release due items, settle missed deadlines, send reminders. Safe to call any time;
/// anything that fell due while the app was closed is settled on the next call.
pub fn tick(conn: &Connection, now: DateTime<Utc>) -> rusqlite::Result<Vec<Notice>> {
    let now_s = ts(now);
    let mut out = Vec::new();

    // 1. Releases (only for courses that are running).
    let mut st = conn.prepare(
        "UPDATE items SET state='open'
         WHERE state='locked' AND release_at <= ?1
           AND course_id IN (SELECT id FROM courses WHERE status='active')
         RETURNING id, title, kind, (SELECT title FROM courses c WHERE c.id = items.course_id)",
    )?;
    let released: Vec<(String, String, String, String)> =
        st.query_map([&now_s], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<Result<_, _>>()?;
    for (id, title, kind, course) in released {
        if !notified(conn, &id, "release")? {
            let what = if kind == "lesson" { "New lesson" } else { "New work released" };
            out.push(Notice { title: format!("{what}: {course}"), body: title });
        }
    }

    // 2. Missed deadlines and 3. reminders, per open graded item.
    let mut st = conn.prepare(
        "SELECT i.id, i.title, i.due_at, c.late_policy, c.title, c.strictness, c.ends_at FROM items i
         JOIN courses c ON c.id = i.course_id
         WHERE i.state='open' AND i.due_at IS NOT NULL AND c.status='active'",
    )?;
    #[allow(clippy::type_complexity)]
    let open: Vec<(String, String, String, String, String, String, Option<String>)> = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))?
        .collect::<Result<_, _>>()?;
    for (id, title, due_s, policy, course, strictness, ends_at) in open {
        let Some(due) = parse_ts(&due_s) else { continue };
        // a sandbox item only locks once the whole course stops taking work
        let locks_at = match Strictness::parse(&strictness) {
            Strictness::Structured => due + LatePolicy::parse(&policy).grace(),
            Strictness::Sandbox => sandbox_closes(ends_at.as_deref()).unwrap_or(due).max(due),
        };
        if now > locks_at {
            conn.execute("UPDATE items SET state='missed' WHERE id=?1", [&id])?;
            conn.execute(
                "INSERT INTO marks(id, item_id, raw_percent, percent, breakdown, marker, marked_at)
                 VALUES (?1, ?2, 0, 0, ?3, 'missed', ?4)",
                params![format!("{id}:missed"), id, r#"{"overall_comment":"Not submitted before the deadline."}"#, now_s],
            )?;
            out.push(Notice { title: format!("Deadline missed: {course}"), body: format!("{title} was locked with a mark of 0.") });
        } else if due > now && due - now <= Duration::hours(1) {
            if !notified(conn, &id, "due1")? {
                out.push(Notice { title: format!("Due within the hour: {course}"), body: title });
            }
        } else if due > now && due - now <= Duration::hours(24) && !notified(conn, &id, "due24")? {
            out.push(Notice { title: format!("Due in 24 hours: {course}"), body: title });
        }
    }
    Ok(out)
}

/// When work submitted now would count, and how late it is. Err with a reason if it can't be accepted.
pub fn submission_window(conn: &Connection, item_id: &str, now: DateTime<Utc>) -> Result<i64, String> {
    #[allow(clippy::type_complexity)]
    let row: Option<(String, Option<String>, String, String, String, Option<String>, bool)> = conn
        .query_row(
            "SELECT i.state, i.due_at, c.late_policy, c.status, c.strictness, c.ends_at, i.key_revealed
             FROM items i JOIN courses c ON c.id=i.course_id WHERE i.id=?1",
            [item_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let (state, due, policy, course_status, strictness, ends_at, revealed) = row.ok_or("No such item.")?;
    if course_status != "active" {
        return Err("This course isn't running, so it can't take submissions.".into());
    }
    if Strictness::parse(&strictness) == Strictness::Sandbox {
        match state.as_str() {
            "open" => {}
            "marked" if revealed => return Err("You revealed the answer key, so this item is closed.".into()),
            "marked" => {}
            "submitted" => return Err("Wait until your last attempt is marked, then try again.".into()),
            "locked" => return Err("This item hasn't been released yet.".into()),
            _ => return Err("The course has stopped taking work for this item.".into()),
        }
        if sandbox_closes(ends_at.as_deref()).is_some_and(|close| now > close) {
            return Err("This sandbox course stopped taking work a week after its last deadline.".into());
        }
        // late days are still recorded, they just cost nothing
        return Ok(due.as_deref().and_then(parse_ts).map_or(0, |d| late_days(d, now)));
    }
    if state != "open" {
        return Err(match state.as_str() {
            "locked" => "This item hasn't been released yet.",
            "missed" => "The deadline has passed and this item is locked.",
            "submitted" | "marked" => "You've already submitted this item.",
            _ => "This item can't take a submission.",
        }
        .into());
    }
    let Some(due) = due.as_deref().and_then(parse_ts) else { return Ok(0) };
    let policy = LatePolicy::parse(&policy);
    if now > due + policy.grace() {
        return Err("The deadline has passed and this item is locked.".into());
    }
    Ok(late_days(due, now))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn late_policy_math() {
        let p = LatePolicy::Penalty { per_day: 10.0, max_days: 3 };
        assert_eq!(p.penalty(2), 20.0);
        assert_eq!(p.grace(), Duration::days(3));
        assert_eq!(LatePolicy::parse("garbage"), LatePolicy::LockZero);
        assert_eq!(LatePolicy::parse(r#"{"kind":"penalty","per_day":5,"max_days":2}"#), LatePolicy::Penalty { per_day: 5.0, max_days: 2 });
        let due = Utc.timestamp_opt(1_000_000, 0).unwrap();
        assert_eq!(late_days(due, due), 0);
        assert_eq!(late_days(due, due + Duration::minutes(1)), 1);
        assert_eq!(late_days(due, due + Duration::hours(25)), 2);
    }
}
