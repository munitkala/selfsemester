//! Submissions, marking, grades, the transcript, and the end-of-course archive.

use super::schedule::{LatePolicy, submission_window, ts};
use super::seal::Sealer;
use super::types::*;
use crate::db::jobs;
use crate::paths::AppPaths;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn io(e: rusqlite::Error) -> String {
    e.to_string()
}

fn new_id(prefix: &str) -> String {
    format!("{prefix}{:x}{:04x}", Utc::now().timestamp_micros(), rand_u16())
}
fn rand_u16() -> u16 {
    let mut b = [0u8; 2];
    let _ = aes_gcm::aead::rand_core::RngCore::try_fill_bytes(&mut aes_gcm::aead::OsRng, &mut b);
    u16::from_le_bytes(b)
}

struct ItemRow {
    course_id: String,
    kind: String,
    module: String,
    content_ref: String,
    key_ref: Option<String>,
}

fn item_row(conn: &Connection, id: &str) -> Result<ItemRow, String> {
    conn.query_row(
        "SELECT course_id, kind, module, content_ref, key_ref FROM items WHERE id=?1",
        [id],
        |r| Ok(ItemRow { course_id: r.get(0)?, kind: r.get(1)?, module: r.get(2)?, content_ref: r.get(3)?, key_ref: r.get(4)? }),
    )
    .optional()
    .map_err(io)?
    .ok_or_else(|| "No such item.".to_string())
}

/// Lessons count as done once read.
pub fn mark_read(conn: &Connection, item_id: &str) -> Result<(), String> {
    conn.execute("UPDATE items SET state='done' WHERE id=?1 AND kind='lesson' AND state='open'", [item_id]).map_err(io)?;
    Ok(())
}

/// Questions the app can mark itself.
fn auto_markable(q: &Question) -> bool {
    q.kind == "mcq" || q.kind == "numeric"
}

fn norm(s: &str) -> String {
    s.trim().to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_num(s: &str) -> Option<f64> {
    s.trim().replace(',', "").replace('−', "-").parse().ok()
}

/// Mark mcq/numeric questions against the key. Returns (lines, awarded, possible).
pub fn auto_mark(public: &ItemPublic, key: &ItemKey, answers: &Value) -> (Vec<MarkLine>, f64, f64) {
    let mut lines = Vec::new();
    let (mut got, mut total) = (0.0, 0.0);
    for q in &public.questions {
        let k = key.answer_key.iter().find(|k| k.question_id == q.id);
        let given = answers.get(&q.id).map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())).unwrap_or_default();
        let right = match (q.kind.as_str(), k) {
            ("mcq", Some(k)) => norm(&given) == norm(&k.answer),
            ("numeric", Some(k)) => match (parse_num(&given), parse_num(&k.answer)) {
                (Some(g), Some(a)) => (g - a).abs() <= k.tolerance.max(1e-9 * a.abs().max(1.0)),
                _ => false,
            },
            _ => false,
        };
        let awarded = if right { q.points } else { 0.0 };
        got += awarded;
        total += q.points;
        let comment = match (right, k) {
            (true, _) => "Correct.".to_string(),
            (false, Some(k)) => format!("Correct answer: {}. {}", k.answer, k.explanation),
            (false, None) => "No answer key for this question.".to_string(),
        };
        lines.push(MarkLine { criterion: q.id.clone(), awarded, possible: q.points, comment });
    }
    (lines, got, total)
}

#[derive(Debug, Serialize)]
pub struct SubmitOutcome {
    pub submission_id: String,
    /// "marked" when marked on the spot (quizzes), "queued" when waiting for Claude.
    pub status: String,
    pub late_days: i64,
}

/// Accept a submission: copy files in, then auto-mark it or queue it for Claude.
pub fn submit(
    conn: &Connection,
    paths: &AppPaths,
    sealer: &Sealer,
    item_id: &str,
    text_answer: &str,
    answers: &Value,
    files: &[PathBuf],
    now: DateTime<Utc>,
) -> Result<SubmitOutcome, String> {
    let late = submission_window(conn, item_id, now)?;
    let it = item_row(conn, item_id)?;
    if it.kind == "lesson" {
        return Err("Lessons don't take submissions.".into());
    }
    let dir = paths.course_dir(&it.course_id);
    let public: ItemPublic = sealer.read_json(&dir.join(&it.content_ref)).map_err(|e| e.to_string())?;
    let sub_id = new_id("s");

    // Copy uploads into the course's submissions folder, so they stay with the course.
    let sub_dir = dir.join("submissions").join(&sub_id);
    let mut stored = Vec::new();
    for f in files {
        let name = f.file_name().ok_or("Bad file name.")?;
        std::fs::create_dir_all(&sub_dir).map_err(|e| e.to_string())?;
        let dest = sub_dir.join(name);
        std::fs::copy(f, &dest).map_err(|e| format!("Couldn't copy {}: {e}", f.display()))?;
        stored.push(dest.display().to_string());
    }
    if text_answer.trim().is_empty() && stored.is_empty() && answers.as_object().is_none_or(|o| o.is_empty()) {
        return Err("Nothing to submit yet.".into());
    }

    conn.execute(
        "INSERT INTO submissions(id, item_id, submitted_at, text_answer, answers_json, files_json, late_days)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![sub_id, item_id, ts(now), text_answer, answers.to_string(), json!(stored).to_string(), late],
    )
    .map_err(io)?;
    conn.execute("UPDATE items SET state='submitted' WHERE id=?1", [item_id]).map_err(io)?;

    let key: ItemKey = sealer
        .read_json(&dir.join(it.key_ref.as_deref().ok_or("Item has no answer key.")?))
        .map_err(|e| e.to_string())?;
    let all_auto = !public.questions.is_empty() && public.questions.iter().all(auto_markable) && stored.is_empty();
    if it.module == "quiz" && all_auto {
        let (lines, got, total) = auto_mark(&public, &key, answers);
        let raw = if total > 0.0 { got / total * 100.0 } else { 0.0 };
        let result = MarkResult { lines, overall_comment: format!("{got} / {total} correct."), revise: vec![] };
        apply_mark(conn, &sub_id, item_id, raw, &result, "auto", late)?;
        return Ok(SubmitOutcome { submission_id: sub_id, status: "marked".into(), late_days: late });
    }

    let course_title: String = conn.query_row("SELECT title FROM courses WHERE id=?1", [&it.course_id], |r| r.get(0)).map_err(io)?;
    let final_exam = it.kind == "exam" && public.title.to_lowercase().contains("final");
    let req = super::prompts::mark(&course_title, &public, &key, text_answer, answers, &stored, final_exam);
    jobs::enqueue_with(conn, Some(&it.course_id), "mark", Some(&sub_id), jobs::PRIORITY_MARKING, &req).map_err(io)?;
    Ok(SubmitOutcome { submission_id: sub_id, status: "queued".into(), late_days: late })
}

/// Store a mark, applying the course's late penalty.
pub fn apply_mark(conn: &Connection, sub_id: &str, item_id: &str, raw: f64, result: &MarkResult, marker: &str, late: i64) -> Result<(), String> {
    let policy: String = conn
        .query_row("SELECT c.late_policy FROM items i JOIN courses c ON c.id=i.course_id WHERE i.id=?1", [item_id], |r| r.get(0))
        .map_err(io)?;
    let penalty = LatePolicy::parse(&policy).penalty(late);
    let raw = raw.clamp(0.0, 100.0);
    let pct = (raw - penalty).max(0.0);
    conn.execute(
        "INSERT INTO marks(id, submission_id, item_id, raw_percent, percent, late_penalty, breakdown, marker, marked_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![format!("{sub_id}:m"), sub_id, item_id, raw, pct, penalty, serde_json::to_string(result).unwrap(), marker, crate::db::now_utc()],
    )
    .map_err(io)?;
    conn.execute("UPDATE items SET state='marked' WHERE id=?1", [item_id]).map_err(io)?;
    Ok(())
}

/// A marking job finished.
pub fn on_mark_done(conn: &Connection, sub_id: &str, value: &Value, model: &str) -> Result<String, String> {
    let result: MarkResult = serde_json::from_value(value.clone()).map_err(|e| format!("bad mark: {e}"))?;
    let (item_id, late): (String, i64) = conn
        .query_row("SELECT item_id, late_days FROM submissions WHERE id=?1", [sub_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(io)?;
    let possible: f64 = result.lines.iter().map(|l| l.possible).sum();
    let awarded: f64 = result.lines.iter().map(|l| l.awarded.min(l.possible)).sum();
    let raw = if possible > 0.0 { awarded / possible * 100.0 } else { 0.0 };
    apply_mark(conn, sub_id, &item_id, raw, &result, model, late)?;
    Ok(item_id)
}

// ---------- grades ----------

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Grades {
    /// Weighted average over items marked or missed so far (None before anything is graded).
    pub current: Option<f64>,
    /// Share of the final grade already decided, 0..1.
    pub decided: f64,
    /// True when every graded item is marked or missed.
    pub complete: bool,
}

pub fn grades(conn: &Connection, course_id: &str) -> rusqlite::Result<Grades> {
    let mut st = conn.prepare(
        "SELECT i.weight, i.state, (SELECT m.percent FROM marks m WHERE m.item_id = i.id ORDER BY m.marked_at DESC LIMIT 1)
         FROM items i WHERE i.course_id=?1 AND i.kind != 'lesson'",
    )?;
    let rows: Vec<(f64, String, Option<f64>)> = st.query_map([course_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
    let total: f64 = rows.iter().map(|r| r.0).sum();
    let (mut w, mut acc) = (0.0, 0.0);
    for (weight, _, pct) in &rows {
        if let Some(p) = pct {
            w += weight;
            acc += weight * p;
        }
    }
    let complete = !rows.is_empty() && rows.iter().all(|r| r.1 == "marked" || r.1 == "missed");
    Ok(Grades { current: (w > 0.0).then(|| acc / w), decided: if total > 0.0 { w / total } else { 0.0 }, complete })
}

/// Letter grade and 4.0 GPA points (Ontario-style percentage bands).
pub fn letter(pct: f64) -> (&'static str, f64) {
    const BANDS: &[(f64, &str, f64)] = &[
        (90.0, "A+", 4.0), (85.0, "A", 4.0), (80.0, "A-", 3.7), (77.0, "B+", 3.3), (73.0, "B", 3.0), (70.0, "B-", 2.7),
        (67.0, "C+", 2.3), (63.0, "C", 2.0), (60.0, "C-", 1.7), (57.0, "D+", 1.3), (53.0, "D", 1.0), (50.0, "D-", 0.7),
    ];
    BANDS.iter().find(|(min, ..)| pct >= *min).map(|(_, l, g)| (*l, *g)).unwrap_or(("F", 0.0))
}

/// If every graded item is settled, finish the course: final mark, transcript entry, archive folder.
pub fn maybe_complete(conn: &Connection, paths: &AppPaths, sealer: &Sealer, course_id: &str) -> Result<Option<PathBuf>, String> {
    let status: String = conn.query_row("SELECT status FROM courses WHERE id=?1", [course_id], |r| r.get(0)).map_err(io)?;
    if status != "active" {
        return Ok(None);
    }
    let g = grades(conn, course_id).map_err(io)?;
    let unread_or_pending: i64 = conn
        .query_row("SELECT count(*) FROM items WHERE course_id=?1 AND state IN ('locked','submitted')", [course_id], |r| r.get(0))
        .map_err(io)?;
    if !g.complete || unread_or_pending > 0 {
        return Ok(None);
    }
    let final_pct = g.current.unwrap_or(0.0);
    let now = crate::db::now_utc();
    conn.execute(
        "UPDATE courses SET status='completed', final_percent=?2, completed_at=?3, updated_at=?3 WHERE id=?1",
        params![course_id, final_pct, now],
    )
    .map_err(io)?;
    let (title, profile, on_transcript, starts): (String, String, bool, Option<String>) = conn
        .query_row("SELECT title, profile_id, on_transcript, starts_at FROM courses WHERE id=?1", [course_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })
        .map_err(io)?;
    if on_transcript {
        let (l, _) = letter(final_pct);
        let breakdown = kind_breakdown(conn, course_id).map_err(io)?;
        conn.execute(
            "INSERT OR REPLACE INTO transcript_entries(course_id, profile_id, title, started_on, finished_on, final_percent, letter, breakdown)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![course_id, profile, title, starts.unwrap_or_else(|| now.clone()), now, final_pct, l, breakdown.to_string()],
        )
        .map_err(io)?;
    }
    let dir = write_archive(conn, paths, sealer, course_id, &title, final_pct).map_err(|e| e.to_string())?;
    Ok(Some(dir))
}

/// Average mark per assessment kind.
pub fn kind_breakdown(conn: &Connection, course_id: &str) -> rusqlite::Result<Value> {
    let mut st = conn.prepare(
        "SELECT i.kind, sum(i.weight), avg((SELECT m.percent FROM marks m WHERE m.item_id=i.id ORDER BY m.marked_at DESC LIMIT 1))
         FROM items i WHERE i.course_id=?1 AND i.kind != 'lesson' GROUP BY i.kind",
    )?;
    let rows = st.query_map([course_id], |r| {
        Ok(json!({"kind": r.get::<_, String>(0)?, "weight": r.get::<_, f64>(1)?, "average": r.get::<_, Option<f64>>(2)?}))
    })?;
    Ok(Value::Array(rows.collect::<Result<_, _>>()?))
}

fn safe_name(s: &str) -> String {
    let cleaned: String = s.chars().map(|c| if r#"<>:"/\|?*"#.contains(c) || c.is_control() { '-' } else { c }).collect();
    cleaned.trim().trim_end_matches('.').chars().take(80).collect()
}

/// `Documents\SelfSemester\Completed\<course>\`: summary, lesson summaries, work given, my work, feedback, references.
/// Leaves out prompts, sealed files, logs and big downloads.
fn write_archive(conn: &Connection, paths: &AppPaths, sealer: &Sealer, course_id: &str, title: &str, final_pct: f64) -> std::io::Result<PathBuf> {
    let io = |e: rusqlite::Error| std::io::Error::other(e);
    let root = paths.completed_dir().join(safe_name(title));
    for sub in ["01 Lesson summaries", "02 Work given", "03 My work", "04 Feedback and marks"] {
        std::fs::create_dir_all(root.join(sub))?;
    }
    let course_dir = paths.course_dir(course_id);
    let plan: Option<Plan> = conn
        .query_row("SELECT plan_json FROM courses WHERE id=?1", [course_id], |r| r.get::<_, Option<String>>(0))
        .map_err(io)?
        .and_then(|s| serde_json::from_str(&s).ok());
    let (letter_s, gpa) = letter(final_pct);

    let mut summary = format!("# {title}\n\nFinal mark: **{final_pct:.1}%** ({letter_s}, {gpa:.1} GPA)\n\n");
    if let Some(p) = &plan {
        summary += &format!("{}\n\n## Syllabus\n\n", p.summary);
        for u in &p.units {
            summary += &format!("- Week {}: {}: {}\n", u.week, u.title, u.topics.join(", "));
        }
    }
    summary += "\n## Marks by kind\n\n";
    for k in kind_breakdown(conn, course_id).map_err(io)?.as_array().cloned().unwrap_or_default() {
        summary += &format!("- {}: {}\n", k["kind"].as_str().unwrap_or(""), k["average"].as_f64().map(|a| format!("{a:.1}%")).unwrap_or("-".into()));
    }
    std::fs::write(root.join("00 Course summary.md"), summary)?;

    let mut st = conn
        .prepare("SELECT id, kind, title, content_ref, state FROM items WHERE course_id=?1 ORDER BY release_at, seq")
        .map_err(io)?;
    let items: Vec<(String, String, String, String, String)> = st
        .query_map([course_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
        .map_err(io)?
        .collect::<Result<_, _>>()
        .map_err(io)?;
    for (n, (id, kind, ititle, content_ref, state)) in items.iter().enumerate() {
        let stem = format!("{:02} {}", n + 1, safe_name(ititle));
        if kind == "lesson" {
            let md = std::fs::read_to_string(course_dir.join(content_ref)).unwrap_or_default();
            std::fs::write(root.join("01 Lesson summaries").join(format!("{stem}.md")), lesson_summary(&md))?;
            continue;
        }
        if let Ok(p) = sealer.read_json::<ItemPublic>(&course_dir.join(content_ref)) {
            let mut given = format!("# {}\n\n{}\n\n", p.title, p.instructions);
            for q in &p.questions {
                given += &format!("**{}** ({} pts) {}\n\n", q.id, q.points, q.prompt);
                for o in &q.options {
                    given += &format!("- {o}\n");
                }
            }
            std::fs::write(root.join("02 Work given").join(format!("{stem}.md")), given)?;
        }
        let sub: Option<(String, Option<String>, Option<String>, String)> = conn
            .query_row(
                "SELECT id, text_answer, answers_json, files_json FROM submissions WHERE item_id=?1 ORDER BY submitted_at DESC LIMIT 1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()
            .map_err(io)?;
        if let Some((_, text, answers, files)) = sub {
            let mine = root.join("03 My work").join(&stem);
            std::fs::create_dir_all(&mine)?;
            std::fs::write(mine.join("answer.md"), format!("{}\n\n{}", text.unwrap_or_default(), answers.unwrap_or_default()))?;
            for f in serde_json::from_str::<Vec<String>>(&files).unwrap_or_default() {
                let f = Path::new(&f);
                if let (Some(name), Ok(meta)) = (f.file_name(), std::fs::metadata(f)) {
                    if meta.len() <= 50 * 1024 * 1024 {
                        let _ = std::fs::copy(f, mine.join(name));
                    }
                }
            }
        }
        let mark: Option<(f64, f64, String, String)> = conn
            .query_row("SELECT percent, late_penalty, breakdown, marker FROM marks WHERE item_id=?1 ORDER BY marked_at DESC LIMIT 1", [id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .optional()
            .map_err(io)?;
        if let Some((pct, pen, breakdown, marker)) = mark {
            let b: Value = serde_json::from_str(&breakdown).unwrap_or_default();
            let mut fb = format!("# {ititle}\n\nMark: **{pct:.1}%**{}  \nMarked by: {marker}  \nState: {state}\n\n",
                if pen > 0.0 { format!(" (late penalty −{pen:.0})") } else { String::new() });
            for l in b["lines"].as_array().cloned().unwrap_or_default() {
                fb += &format!("- **{}** {}/{}: {}\n", l["criterion"].as_str().unwrap_or(""), l["awarded"], l["possible"], l["comment"].as_str().unwrap_or(""));
            }
            fb += &format!("\n{}\n", b["overall_comment"].as_str().unwrap_or(""));
            std::fs::write(root.join("04 Feedback and marks").join(format!("{stem}.md")), fb)?;
        }
    }

    let mut refs = "# Reference list\n\n".to_string();
    let mut st = conn.prepare("SELECT title, url, licence, attribution, free FROM sources WHERE course_id=?1").map_err(io)?;
    let rows = st
        .query_map([course_id], |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, bool>(4)?)))
        .map_err(io)?;
    for row in rows {
        let (t, url, lic, attr, free) = row.map_err(io)?;
        refs += &format!("- [{}]({url}), {}{}. {}\n", t.unwrap_or(url.clone()), lic.unwrap_or_default(), if free { "" } else { " (not free)" }, attr.unwrap_or_default());
    }
    std::fs::write(root.join("05 Reference list.md"), refs)?;
    Ok(root)
}

/// The lesson's own summary section if it has one, otherwise its opening.
fn lesson_summary(md: &str) -> String {
    let lower = md.to_lowercase();
    if let Some(pos) = lower.find("## summary") {
        let rest = &md[pos..];
        let end = rest[3..].find("\n## ").map(|e| e + 3).unwrap_or(rest.len());
        let title = md.lines().next().unwrap_or("");
        return format!("{title}\n\n{}", &rest[..end]);
    }
    md.chars().take(2000).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quiz() -> (ItemPublic, ItemKey) {
        let q = |id: &str, kind: &str, opts: &[&str]| Question { id: id.into(), prompt: "?".into(), kind: kind.into(), options: opts.iter().map(|s| s.to_string()).collect(), points: 1.0 };
        (
            ItemPublic { title: "Q".into(), kind: "quiz".into(), module: "quiz".into(), instructions: "".into(),
                questions: vec![q("a", "mcq", &["Paris", "Rome"]), q("b", "numeric", &[]), q("c", "numeric", &[])], rubric: vec![] },
            ItemKey { answer_key: vec![
                KeyEntry { question_id: "a".into(), answer: "Paris".into(), explanation: "".into(), tolerance: 0.0 },
                KeyEntry { question_id: "b".into(), answer: "3.14".into(), explanation: "".into(), tolerance: 0.01 },
                KeyEntry { question_id: "c".into(), answer: "10".into(), explanation: "".into(), tolerance: 0.0 },
            ] },
        )
    }

    #[test]
    fn auto_marks_mcq_and_numeric() {
        let (p, k) = quiz();
        let (lines, got, total) = auto_mark(&p, &k, &json!({"a": " paris ", "b": "3.141", "c": "11"}));
        assert_eq!((got, total), (2.0, 3.0));
        assert!(lines[2].comment.starts_with("Correct answer: 10"));
    }

    #[test]
    fn letters() {
        assert_eq!(letter(92.0), ("A+", 4.0));
        assert_eq!(letter(79.9), ("B+", 3.3));
        assert_eq!(letter(49.0), ("F", 0.0));
    }

    #[test]
    fn summary_extraction() {
        let md = "# Waves\n\nIntro\n\n## Summary\n\n- a\n- b\n\n## Exercises\n\nx";
        let s = lesson_summary(md);
        assert!(s.contains("- b") && !s.contains("Exercises"));
    }

    #[test]
    fn safe_names() {
        assert_eq!(safe_name("QM: 1/2?"), "QM- 1-2-");
    }
}
