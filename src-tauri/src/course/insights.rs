//! Study time and the raw numbers behind the Insights page.
//!
//! The interface does the charting. This module only records time and hands back plain rows, so the page can bucket
//! them by the learner's own local days and weeks.

use rusqlite::{Connection, params};
use serde::Serialize;

fn io<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// The most one heartbeat can add. The page sends one about every minute, so anything longer is a clock jump or a
/// sleeping laptop, not study.
const MAX_SECONDS_PER_CALL: u32 = 300;

/// How far back the Insights page looks.
const HISTORY_DAYS: i64 = 400;

fn valid_day(day: &str) -> bool {
    chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d").is_ok()
}

/// Add `seconds` of study on `item_id` to the given local day and hour.
pub fn record_study(conn: &Connection, item_id: &str, day: &str, hour: u32, seconds: u32) -> Result<(), String> {
    if !valid_day(day) {
        return Err(format!("not a date: {day}"));
    }
    if hour > 23 {
        return Err(format!("not an hour: {hour}"));
    }
    let seconds = seconds.min(MAX_SECONDS_PER_CALL);
    if seconds == 0 {
        return Ok(());
    }
    let added = conn
        .execute(
            "INSERT INTO study_time(item_id, course_id, day, hour, seconds)
             SELECT id, course_id, ?2, ?3, ?4 FROM items WHERE id = ?1
             ON CONFLICT(item_id, day, hour) DO UPDATE SET seconds = seconds + excluded.seconds",
            params![item_id, day, hour, seconds],
        )
        .map_err(io)?;
    if added == 0 {
        return Err(format!("no such item: {item_id}"));
    }
    Ok(())
}

/// Forget all recorded study time. Marks and submissions are untouched.
pub fn clear_study(conn: &Connection) -> Result<usize, String> {
    conn.execute("DELETE FROM study_time", []).map_err(io)
}

#[derive(Debug, Serialize, PartialEq)]
pub struct StudyRow {
    pub day: String,
    pub hour: u32,
    pub course_id: String,
    pub seconds: u32,
}

#[derive(Debug, Serialize)]
pub struct MarkRow {
    pub marked_at: String,
    pub percent: f64,
    pub kind: String,
    pub course_id: String,
    pub title: String,
    pub late_days: i64,
    pub missed: bool,
}

/// Something the learner did: handed in work or asked about a mark.
#[derive(Debug, Serialize)]
pub struct EventRow {
    pub at: String,
    pub kind: String,
    pub course_id: String,
}

/// Graded work still to come.
#[derive(Debug, Serialize)]
pub struct DueRow {
    pub due_at: String,
    pub kind: String,
    pub course_id: String,
    pub weight: f64,
}

#[derive(Debug, Serialize)]
pub struct Insights {
    pub study: Vec<StudyRow>,
    pub marks: Vec<MarkRow>,
    pub events: Vec<EventRow>,
    pub due: Vec<DueRow>,
}

pub fn insights(conn: &Connection) -> Result<Insights, String> {
    let since = chrono::Utc::now() - chrono::Duration::days(HISTORY_DAYS);
    let since_day = since.format("%Y-%m-%d").to_string();
    let since_at = since.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let mut st = conn
        .prepare(
            "SELECT day, hour, course_id, sum(seconds) FROM study_time WHERE day >= ?1
             GROUP BY day, hour, course_id ORDER BY day, hour",
        )
        .map_err(io)?;
    let study = st
        .query_map([&since_day], |r| Ok(StudyRow { day: r.get(0)?, hour: r.get(1)?, course_id: r.get(2)?, seconds: r.get(3)? }))
        .map_err(io)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(io)?;

    // Latest mark per item, so a resubmitted sandbox item counts once, at its newest result.
    let mut st = conn
        .prepare(
            "SELECT m.marked_at, m.percent, i.kind, i.course_id, i.title, coalesce(s.late_days, 0), m.marker = 'missed'
             FROM marks m JOIN items i ON i.id = m.item_id LEFT JOIN submissions s ON s.id = m.submission_id
             WHERE m.marked_at >= ?1
               AND m.marked_at = (SELECT max(m2.marked_at) FROM marks m2 WHERE m2.item_id = m.item_id)
             ORDER BY m.marked_at",
        )
        .map_err(io)?;
    let marks = st
        .query_map([&since_at], |r| {
            Ok(MarkRow {
                marked_at: r.get(0)?,
                percent: r.get(1)?,
                kind: r.get(2)?,
                course_id: r.get(3)?,
                title: r.get(4)?,
                late_days: r.get(5)?,
                missed: r.get(6)?,
            })
        })
        .map_err(io)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(io)?;

    let mut st = conn
        .prepare(
            "SELECT s.submitted_at, 'submit', i.course_id FROM submissions s JOIN items i ON i.id = s.item_id
             WHERE s.submitted_at >= ?1
             UNION ALL
             SELECT r.created_at, 'review', i.course_id FROM review_messages r JOIN items i ON i.id = r.item_id
             WHERE r.role = 'learner' AND r.created_at >= ?1
             ORDER BY 1",
        )
        .map_err(io)?;
    let events = st
        .query_map([&since_at], |r| Ok(EventRow { at: r.get(0)?, kind: r.get(1)?, course_id: r.get(2)? }))
        .map_err(io)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(io)?;

    let mut st = conn
        .prepare(
            "SELECT i.due_at, i.kind, i.course_id, i.weight FROM items i JOIN courses c ON c.id = i.course_id
             WHERE i.due_at IS NOT NULL AND i.state IN ('locked', 'open') AND c.status = 'active'
             ORDER BY i.due_at",
        )
        .map_err(io)?;
    let due = st
        .query_map([], |r| Ok(DueRow { due_at: r.get(0)?, kind: r.get(1)?, course_id: r.get(2)?, weight: r.get(3)? }))
        .map_err(io)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(io)?;

    Ok(Insights { study, marks, events, due })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn setup() -> Connection {
        let conn = db::open_in_memory().unwrap();
        let now = db::now_utc();
        conn.execute("INSERT INTO profiles(id, name, created_at) VALUES ('me', 'Me', ?1)", [&now]).unwrap();
        conn.execute(
            "INSERT INTO courses(id, profile_id, title, subject, status, created_at, updated_at)
             VALUES ('c1', 'me', 'Algebra', 'Algebra', 'active', ?1, ?1)",
            [&now],
        )
        .unwrap();
        for (id, kind, due, state) in [("l1", "lesson", None, "done"), ("q1", "quiz", Some("2999-01-01T00:00:00Z"), "open")] {
            conn.execute(
                "INSERT INTO items(id, course_id, kind, title, release_at, due_at, content_ref, state, weight)
                 VALUES (?1, 'c1', ?2, ?1, ?3, ?4, 'x.json', ?5, 0.1)",
                params![id, kind, now, due, state],
            )
            .unwrap();
        }
        conn
    }

    #[test]
    fn study_time_adds_up_per_hour() {
        let conn = setup();
        record_study(&conn, "l1", "2026-11-10", 9, 60).unwrap();
        record_study(&conn, "l1", "2026-11-10", 9, 45).unwrap();
        record_study(&conn, "l1", "2026-11-10", 10, 30).unwrap();
        let total: u32 = conn.query_row("SELECT sum(seconds) FROM study_time WHERE hour = 9", [], |r| r.get(0)).unwrap();
        assert_eq!(total, 105);
        let course: String = conn.query_row("SELECT course_id FROM study_time LIMIT 1", [], |r| r.get(0)).unwrap();
        assert_eq!(course, "c1");
    }

    #[test]
    fn record_study_rejects_bad_input_and_caps_long_gaps() {
        let conn = setup();
        assert!(record_study(&conn, "l1", "10/11/2026", 9, 60).is_err());
        assert!(record_study(&conn, "l1", "2026-11-10", 24, 60).is_err());
        assert!(record_study(&conn, "nope", "2026-11-10", 9, 60).is_err());
        record_study(&conn, "l1", "2026-11-10", 9, 0).unwrap();
        record_study(&conn, "l1", "2026-11-10", 9, 10_000).unwrap();
        let total: u32 = conn.query_row("SELECT sum(seconds) FROM study_time", [], |r| r.get(0)).unwrap();
        assert_eq!(total, MAX_SECONDS_PER_CALL);
    }

    #[test]
    fn insights_returns_recent_rows_and_latest_mark_only() {
        let conn = setup();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        record_study(&conn, "q1", &today, 20, 120).unwrap();
        record_study(&conn, "q1", "2000-01-01", 20, 120).unwrap(); // too old to show
        let now = db::now_utc();
        conn.execute(
            "INSERT INTO submissions(id, item_id, submitted_at, late_days) VALUES ('s1', 'q1', ?1, 1)",
            [&now],
        )
        .unwrap();
        for (id, at, pct) in [("m1", "2026-11-01T10:00:00Z", 55.0), ("m2", "2026-11-02T10:00:00Z", 80.0)] {
            conn.execute(
                "INSERT INTO marks(id, submission_id, item_id, raw_percent, percent, breakdown, marker, marked_at)
                 VALUES (?1, 's1', 'q1', ?3, ?3, '{}', 'auto', ?2)",
                params![id, at, pct],
            )
            .unwrap();
        }
        let got = insights(&conn).unwrap();
        assert_eq!(got.study, vec![StudyRow { day: today, hour: 20, course_id: "c1".into(), seconds: 120 }]);
        assert_eq!(got.marks.len(), 1);
        assert_eq!(got.marks[0].percent, 80.0);
        assert_eq!(got.marks[0].late_days, 1);
        assert!(!got.marks[0].missed);
        assert_eq!(got.events.len(), 1);
        assert_eq!(got.events[0].kind, "submit");
        assert_eq!(got.due.len(), 1);
        assert_eq!(got.due[0].kind, "quiz");
    }

    #[test]
    fn clear_study_only_removes_study_time() {
        let conn = setup();
        record_study(&conn, "l1", "2026-11-10", 9, 60).unwrap();
        assert_eq!(clear_study(&conn).unwrap(), 1);
        let items: u32 = conn.query_row("SELECT count(*) FROM items", [], |r| r.get(0)).unwrap();
        assert_eq!(items, 2);
    }

    #[test]
    fn deleting_a_course_removes_its_study_time() {
        let conn = setup();
        record_study(&conn, "l1", "2026-11-10", 9, 60).unwrap();
        conn.execute("DELETE FROM courses WHERE id = 'c1'", []).unwrap();
        let n: u32 = conn.query_row("SELECT count(*) FROM study_time", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
    }
}
