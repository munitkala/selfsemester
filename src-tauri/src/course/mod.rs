//! Courses: the compile pipeline, schedule, marking, and the read models the UI shows.

pub mod marking;
pub mod pipeline;
pub mod prompts;
pub mod schedule;
pub mod seal;
pub mod types;

use crate::paths::AppPaths;
use rusqlite::{Connection, OptionalExtension, params};
use seal::Sealer;
use serde::Serialize;
use serde_json::Value;

fn io(e: rusqlite::Error) -> String {
    e.to_string()
}

fn json_col(s: Option<String>) -> Value {
    s.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null)
}

#[derive(Debug, Serialize)]
pub struct JobSummary {
    pub queued: i64,
    pub running: i64,
    pub waiting_usage: i64,
    pub failed: i64,
    pub done: i64,
    /// Earliest retry time among usage-limited jobs.
    pub next_attempt_at: Option<String>,
    /// Kind of the job running now, if any.
    pub running_kind: Option<String>,
}

pub fn job_summary(conn: &Connection, course_id: Option<&str>) -> rusqlite::Result<JobSummary> {
    let filter = if course_id.is_some() { "WHERE course_id = ?1" } else { "WHERE ?1 IS NULL OR 1" };
    let sql = format!(
        "SELECT
           sum(status='queued'), sum(status='running'), sum(status='waiting_usage'), sum(status='failed'), sum(status='done'),
           min(CASE WHEN status='waiting_usage' THEN next_attempt_at END),
           max(CASE WHEN status='running' THEN kind END)
         FROM jobs {filter}"
    );
    conn.query_row(&sql, params![course_id], |r| {
        Ok(JobSummary {
            queued: r.get::<_, Option<i64>>(0)?.unwrap_or(0),
            running: r.get::<_, Option<i64>>(1)?.unwrap_or(0),
            waiting_usage: r.get::<_, Option<i64>>(2)?.unwrap_or(0),
            failed: r.get::<_, Option<i64>>(3)?.unwrap_or(0),
            done: r.get::<_, Option<i64>>(4)?.unwrap_or(0),
            next_attempt_at: r.get(5)?,
            running_kind: r.get(6)?,
        })
    })
}

#[derive(Debug, Serialize)]
pub struct CourseCard {
    pub id: String,
    pub title: String,
    pub status: String,
    pub error: Option<String>,
    pub mode: String,
    pub grade: Option<f64>,
    pub final_percent: Option<f64>,
    pub items_total: i64,
    pub items_settled: i64,
    pub next_due: Option<String>,
}

pub fn list(conn: &Connection) -> Result<Vec<CourseCard>, String> {
    let mut st = conn
        .prepare(
            "SELECT c.id, c.title, c.status, c.error, c.mode, c.final_percent,
               (SELECT count(*) FROM items i WHERE i.course_id=c.id AND i.kind!='lesson'),
               (SELECT count(*) FROM items i WHERE i.course_id=c.id AND i.kind!='lesson' AND i.state IN ('marked','missed')),
               (SELECT min(due_at) FROM items i WHERE i.course_id=c.id AND i.state='open' AND i.due_at IS NOT NULL)
             FROM courses c ORDER BY c.status='completed', c.updated_at DESC",
        )
        .map_err(io)?;
    let rows: Vec<CourseCard> = st
        .query_map([], |r| {
            Ok(CourseCard {
                id: r.get(0)?,
                title: r.get(1)?,
                status: r.get(2)?,
                error: r.get(3)?,
                mode: r.get(4)?,
                final_percent: r.get(5)?,
                grade: None,
                items_total: r.get(6)?,
                items_settled: r.get(7)?,
                next_due: r.get(8)?,
            })
        })
        .map_err(io)?
        .collect::<Result<_, _>>()
        .map_err(io)?;
    rows.into_iter()
        .map(|mut c| {
            c.grade = marking::grades(conn, &c.id).map_err(io)?.current;
            Ok(c)
        })
        .collect()
}

#[derive(Debug, Serialize)]
pub struct ItemRow {
    pub id: String,
    pub course_id: String,
    pub course_title: String,
    pub unit_title: Option<String>,
    pub kind: String,
    pub title: String,
    pub module: String,
    pub weight: f64,
    pub release_at: String,
    pub due_at: Option<String>,
    pub state: String,
    pub percent: Option<f64>,
}

const ITEM_COLS: &str = "i.id, i.course_id, c.title, u.title, i.kind, i.title, i.module, i.weight, i.release_at, i.due_at, i.state,
    (SELECT m.percent FROM marks m WHERE m.item_id=i.id ORDER BY m.marked_at DESC LIMIT 1)";

fn item_rows(conn: &Connection, where_sql: &str, p: &[&dyn rusqlite::ToSql]) -> Result<Vec<ItemRow>, String> {
    let sql = format!(
        "SELECT {ITEM_COLS} FROM items i JOIN courses c ON c.id=i.course_id LEFT JOIN units u ON u.id=i.unit_id
         WHERE {where_sql}"
    );
    let mut st = conn.prepare(&sql).map_err(io)?;
    st.query_map(p, |r| {
        Ok(ItemRow {
            id: r.get(0)?,
            course_id: r.get(1)?,
            course_title: r.get(2)?,
            unit_title: r.get(3)?,
            kind: r.get(4)?,
            title: r.get(5)?,
            module: r.get(6)?,
            weight: r.get(7)?,
            release_at: r.get(8)?,
            due_at: r.get(9)?,
            state: r.get(10)?,
            percent: r.get(11)?,
        })
    })
    .map_err(io)?
    .collect::<Result<_, _>>()
    .map_err(io)
}

#[derive(Debug, Serialize)]
pub struct UnitView {
    pub key: String,
    pub title: String,
    pub week: i64,
    pub state: String,
    pub warnings: Value,
    pub items: Vec<ItemRow>,
}

#[derive(Debug, Serialize)]
pub struct CourseView {
    pub id: String,
    pub title: String,
    pub subject: String,
    pub status: String,
    pub error: Option<String>,
    pub mode: String,
    pub late_policy: Value,
    pub on_transcript: bool,
    pub questions: Value,
    pub answers: Value,
    pub plan: Value,
    pub bundle_choice: Option<String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub final_percent: Option<f64>,
    pub grades: marking::Grades,
    pub jobs: JobSummary,
    pub units: Vec<UnitView>,
    pub sources: Vec<Value>,
    pub archive_dir: Option<String>,
}

pub fn view(conn: &Connection, paths: &AppPaths, id: &str) -> Result<CourseView, String> {
    let mut v = conn
        .query_row(
            "SELECT id, title, subject, status, error, mode, late_policy, on_transcript, questions_json, intake_json, plan_json,
                    bundle_choice, starts_at, ends_at, final_percent
             FROM courses WHERE id=?1",
            [id],
            |r| {
                Ok(CourseView {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    subject: r.get(2)?,
                    status: r.get(3)?,
                    error: r.get(4)?,
                    mode: r.get(5)?,
                    late_policy: json_col(r.get(6)?),
                    on_transcript: r.get(7)?,
                    questions: json_col(r.get(8)?),
                    answers: json_col(r.get(9)?),
                    plan: json_col(r.get(10)?),
                    bundle_choice: r.get(11)?,
                    starts_at: r.get(12)?,
                    ends_at: r.get(13)?,
                    final_percent: r.get(14)?,
                    grades: marking::Grades { current: None, decided: 0.0, complete: false },
                    jobs: JobSummary { queued: 0, running: 0, waiting_usage: 0, failed: 0, done: 0, next_attempt_at: None, running_kind: None },
                    units: vec![],
                    sources: vec![],
                    archive_dir: None,
                })
            },
        )
        .optional()
        .map_err(io)?
        .ok_or("No such course.")?;
    v.grades = marking::grades(conn, id).map_err(io)?;
    v.jobs = job_summary(conn, Some(id)).map_err(io)?;

    let mut st = conn.prepare("SELECT id, unit_key, title, week, state, warnings FROM units WHERE course_id=?1 ORDER BY seq").map_err(io)?;
    let units: Vec<(String, String, String, i64, String, Option<String>)> = st
        .query_map([id], |r| Ok((r.get(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default(), r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))
        .map_err(io)?
        .collect::<Result<_, _>>()
        .map_err(io)?;
    for (uid, key, title, week, state, warnings) in units {
        let items = item_rows(conn, "i.unit_id = ?1 ORDER BY i.release_at, i.seq", &[&uid])?;
        v.units.push(UnitView { key, title, week, state, warnings: json_col(warnings), items });
    }
    let mut st = conn.prepare("SELECT url, title, licence, attribution, free FROM sources WHERE course_id=?1").map_err(io)?;
    v.sources = st
        .query_map([id], |r| {
            Ok(serde_json::json!({"url": r.get::<_, String>(0)?, "title": r.get::<_, Option<String>>(1)?,
                "licence": r.get::<_, Option<String>>(2)?, "attribution": r.get::<_, Option<String>>(3)?, "free": r.get::<_, bool>(4)?}))
        })
        .map_err(io)?
        .collect::<Result<_, _>>()
        .map_err(io)?;
    if v.status == "completed" {
        let dir = paths.completed_dir().join(marking_safe(&v.title));
        if dir.is_dir() {
            v.archive_dir = Some(dir.display().to_string());
        }
    }
    Ok(v)
}

fn marking_safe(title: &str) -> String {
    let cleaned: String = title.chars().map(|c| if r#"<>:"/\|?*"#.contains(c) || c.is_control() { '-' } else { c }).collect();
    cleaned.trim().trim_end_matches('.').chars().take(80).collect()
}

#[derive(Debug, Serialize)]
pub struct Today {
    pub overdue: Vec<ItemRow>,
    pub due_soon: Vec<ItemRow>,
    pub open: Vec<ItemRow>,
    pub new_lessons: Vec<ItemRow>,
    pub awaiting_marks: Vec<ItemRow>,
    pub recently_marked: Vec<ItemRow>,
    pub upcoming: Vec<ItemRow>,
}

pub fn today(conn: &Connection) -> Result<Today, String> {
    let now = crate::db::now_utc();
    let week = schedule::ts(chrono::Utc::now() + chrono::Duration::days(7));
    let recent = schedule::ts(chrono::Utc::now() - chrono::Duration::days(7));
    let active = "c.status='active'";
    Ok(Today {
        overdue: item_rows(conn, &format!("{active} AND i.state='open' AND i.due_at < ?1 ORDER BY i.due_at"), &[&now])?,
        due_soon: item_rows(conn, &format!("{active} AND i.state='open' AND i.due_at >= ?1 AND i.due_at <= ?2 ORDER BY i.due_at"), &[&now, &week])?,
        open: item_rows(conn, &format!("{active} AND i.state='open' AND i.due_at > ?1 ORDER BY i.due_at"), &[&week])?,
        new_lessons: item_rows(conn, &format!("{active} AND i.kind='lesson' AND i.state='open' ORDER BY i.release_at"), &[])?,
        awaiting_marks: item_rows(conn, &format!("{active} AND i.state='submitted' ORDER BY i.due_at"), &[])?,
        recently_marked: item_rows(
            conn,
            "i.state='marked' AND EXISTS (SELECT 1 FROM marks m WHERE m.item_id=i.id AND m.marked_at >= ?1) ORDER BY i.due_at DESC",
            &[&recent],
        )?,
        upcoming: item_rows(conn, &format!("{active} AND i.state='locked' AND i.release_at <= ?1 ORDER BY i.release_at LIMIT 8"), &[&week])?,
    })
}

#[derive(Debug, Serialize)]
pub struct ItemView {
    pub item: ItemRow,
    pub points: f64,
    pub late_policy: Value,
    /// Lesson Markdown (lessons only, once released).
    pub lesson: Option<String>,
    /// Instructions, questions and rubric (graded items, once released).
    pub content: Option<types::ItemPublic>,
    pub submission: Option<Value>,
    pub mark: Option<Value>,
    /// Shown only after the item is marked.
    pub answer_key: Option<Value>,
    pub marking_status: Option<String>,
}

pub fn item(conn: &Connection, paths: &AppPaths, sealer: &Sealer, id: &str) -> Result<ItemView, String> {
    let row = item_rows(conn, "i.id = ?1", &[&id])?.into_iter().next().ok_or("No such item.")?;
    let (content_ref, key_ref, points, policy): (String, Option<String>, f64, String) = conn
        .query_row(
            "SELECT i.content_ref, i.key_ref, i.points, c.late_policy FROM items i JOIN courses c ON c.id=i.course_id WHERE i.id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(io)?;
    let dir = paths.course_dir(&row.course_id);
    let released = row.state != "locked";
    let (mut lesson, mut content) = (None, None);
    if released {
        if row.kind == "lesson" {
            lesson = Some(std::fs::read_to_string(dir.join(&content_ref)).map_err(|e| e.to_string())?);
        } else {
            content = Some(sealer.read_json(&dir.join(&content_ref)).map_err(|e| e.to_string())?);
        }
    }
    let submission: Option<Value> = conn
        .query_row(
            "SELECT id, submitted_at, text_answer, answers_json, files_json, late_days FROM submissions WHERE item_id=?1 ORDER BY submitted_at DESC LIMIT 1",
            [id],
            |r| {
                Ok(serde_json::json!({"id": r.get::<_, String>(0)?, "submitted_at": r.get::<_, String>(1)?,
                    "text_answer": r.get::<_, Option<String>>(2)?, "answers": json_col(r.get(3)?),
                    "files": json_col(r.get(4)?), "late_days": r.get::<_, i64>(5)?}))
            },
        )
        .optional()
        .map_err(io)?;
    let mark: Option<Value> = conn
        .query_row(
            "SELECT raw_percent, percent, late_penalty, breakdown, marker, marked_at FROM marks WHERE item_id=?1 ORDER BY marked_at DESC LIMIT 1",
            [id],
            |r| {
                Ok(serde_json::json!({"raw_percent": r.get::<_, f64>(0)?, "percent": r.get::<_, f64>(1)?, "late_penalty": r.get::<_, f64>(2)?,
                    "breakdown": json_col(r.get(3)?), "marker": r.get::<_, String>(4)?, "marked_at": r.get::<_, String>(5)?}))
            },
        )
        .optional()
        .map_err(io)?;
    let answer_key = if row.state == "marked" || row.state == "missed" {
        key_ref.and_then(|k| sealer.read_json::<types::ItemKey>(&dir.join(k)).ok()).map(|k| serde_json::to_value(k.answer_key).unwrap())
    } else {
        None
    };
    let marking_status = match (&submission, row.state.as_str()) {
        (Some(s), "submitted") => {
            let sid = s["id"].as_str().unwrap_or_default();
            conn.query_row(
                "SELECT status || coalesce('|' || next_attempt_at, '') || coalesce('|' || error, '') FROM jobs WHERE kind='mark' AND unit_key=?1 ORDER BY id DESC LIMIT 1",
                [sid],
                |r| r.get(0),
            )
            .optional()
            .map_err(io)?
        }
        _ => None,
    };
    Ok(ItemView { item: row, points, late_policy: json_col(Some(policy)), lesson, content, submission, mark, answer_key, marking_status })
}

#[derive(Debug, Serialize)]
pub struct TranscriptView {
    pub entries: Vec<Value>,
    pub average: Option<f64>,
    pub gpa: Option<f64>,
}

pub fn transcript(conn: &Connection) -> Result<TranscriptView, String> {
    let mut st = conn
        .prepare("SELECT course_id, title, started_on, finished_on, final_percent, letter, breakdown FROM transcript_entries ORDER BY finished_on")
        .map_err(io)?;
    let entries: Vec<Value> = st
        .query_map([], |r| {
            let pct: f64 = r.get(4)?;
            Ok(serde_json::json!({"course_id": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)?, "started_on": r.get::<_, String>(2)?,
                "finished_on": r.get::<_, String>(3)?, "final_percent": pct, "letter": r.get::<_, String>(5)?,
                "gpa": marking::letter(pct).1, "breakdown": json_col(r.get(6)?)}))
        })
        .map_err(io)?
        .collect::<Result<_, _>>()
        .map_err(io)?;
    let n = entries.len() as f64;
    let avg = (n > 0.0).then(|| entries.iter().map(|e| e["final_percent"].as_f64().unwrap_or(0.0)).sum::<f64>() / n);
    let gpa = (n > 0.0).then(|| entries.iter().map(|e| e["gpa"].as_f64().unwrap_or(0.0)).sum::<f64>() / n);
    Ok(TranscriptView { entries, average: avg, gpa })
}
