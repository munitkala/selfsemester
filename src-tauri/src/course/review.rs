//! Review mode: talk a marked item through with Claude (the questions, the feedback, the mark).
//!
//! Each message from the learner queues one Claude call carrying the item, the learner's work, the mark and the
//! conversation so far. Claude can explain but can't change a mark. In a sandbox course whose answer key is still
//! hidden, Claude is told to guide rather than give the answers away.

use super::schedule::Strictness;
use super::seal::Sealer;
use super::types::*;
use crate::claude::{StageRequest, Tier};
use crate::db::{self, jobs};
use crate::paths::AppPaths;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::{Value, json};

/// How much of the conversation goes back to Claude each time (oldest dropped first).
const HISTORY: usize = 20;
pub const MAX_MESSAGE_CHARS: usize = 4000;

fn io(e: rusqlite::Error) -> String {
    e.to_string()
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct Message {
    pub id: i64,
    /// learner | claude | note (a problem getting a reply)
    pub role: String,
    pub text: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct Thread {
    pub messages: Vec<Message>,
    /// Status of the reply being worked on: queued, running or waiting_usage (None when nothing is pending).
    pub pending: Option<String>,
    pub next_attempt_at: Option<String>,
}

pub fn thread(conn: &Connection, item_id: &str) -> Result<Thread, String> {
    let mut st = conn
        .prepare("SELECT id, role, text, created_at FROM review_messages WHERE item_id=?1 ORDER BY id")
        .map_err(io)?;
    let messages = st
        .query_map([item_id], |r| Ok(Message { id: r.get(0)?, role: r.get(1)?, text: r.get(2)?, created_at: r.get(3)? }))
        .map_err(io)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(io)?;
    let (pending, next_attempt_at) = pending_job(conn, item_id)?.map(|(_, s, n)| (Some(s), n)).unwrap_or((None, None));
    Ok(Thread { messages, pending, next_attempt_at })
}

fn pending_job(conn: &Connection, item_id: &str) -> Result<Option<(i64, String, Option<String>)>, String> {
    conn.query_row(
        "SELECT id, status, next_attempt_at FROM jobs WHERE kind='review' AND unit_key=?1
           AND status IN ('queued','running','waiting_usage') ORDER BY id DESC LIMIT 1",
        [item_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .optional()
    .map_err(io)
}

fn add(conn: &Connection, item_id: &str, role: &str, text: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO review_messages(item_id, role, text, created_at) VALUES (?1,?2,?3,?4)",
        params![item_id, role, text, db::now_utc()],
    )
    .map_err(io)?;
    Ok(())
}

pub fn reply_schema() -> Value {
    json!({"type": "object", "properties": {"reply": {"type": "string"}}, "required": ["reply"], "additionalProperties": false})
}

/// The learner asks something about a marked item. Queues Claude's reply.
pub fn send(conn: &Connection, paths: &AppPaths, sealer: &Sealer, item_id: &str, text: &str) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Write a question first.".into());
    }
    if text.chars().count() > MAX_MESSAGE_CHARS {
        return Err(format!("Keep it under {MAX_MESSAGE_CHARS} characters."));
    }
    #[allow(clippy::type_complexity)]
    let (course_id, course_title, state, content_ref, key_ref, revealed, strictness): (String, String, String, String, Option<String>, bool, String) = conn
        .query_row(
            "SELECT i.course_id, c.title, i.state, i.content_ref, i.key_ref, i.key_revealed, c.strictness
             FROM items i JOIN courses c ON c.id=i.course_id WHERE i.id=?1",
            [item_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
        )
        .optional()
        .map_err(io)?
        .ok_or("No such item.")?;
    if state != "marked" && state != "missed" {
        return Err("Review opens once the item has been marked.".into());
    }
    if pending_job(conn, item_id)?.is_some() {
        return Err("Wait for the reply to your last message first.".into());
    }
    let dir = paths.course_dir(&course_id);
    let public: ItemPublic = sealer.read_json(&dir.join(&content_ref)).map_err(|e| e.to_string())?;
    let key_hidden = Strictness::parse(&strictness) == Strictness::Sandbox && state == "marked" && !revealed;
    let key = if key_hidden {
        "(hidden: this is a sandbox course and the learner can still make another attempt)".to_string()
    } else {
        key_ref
            .and_then(|k| sealer.read_json::<ItemKey>(&dir.join(k)).ok())
            .map(|k| serde_json::to_string(&k.answer_key).unwrap_or_default())
            .unwrap_or_else(|| "(none)".into())
    };
    #[allow(clippy::type_complexity)]
    let sub: Option<(Option<String>, Option<String>, String)> = conn
        .query_row(
            "SELECT text_answer, answers_json, files_json FROM submissions WHERE item_id=?1 ORDER BY submitted_at DESC, rowid DESC LIMIT 1",
            [item_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(io)?;
    let mark: Option<(f64, f64, String)> = conn
        .query_row(
            "SELECT percent, late_penalty, breakdown FROM marks WHERE item_id=?1 ORDER BY marked_at DESC, rowid DESC LIMIT 1",
            [item_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(io)?;

    add(conn, item_id, "learner", text)?;
    let history = thread(conn, item_id)?.messages;
    let convo = history[history.len().saturating_sub(HISTORY)..]
        .iter()
        .filter(|m| m.role != "note")
        .map(|m| format!("{}: {}", if m.role == "learner" { "LEARNER" } else { "YOU" }, m.text))
        .collect::<Vec<_>>()
        .join("\n\n");

    let files: Vec<String> = sub.as_ref().and_then(|s| serde_json::from_str(&s.2).ok()).unwrap_or_default();
    let (typed, answers) = sub.map(|s| (s.0.unwrap_or_default(), s.1.unwrap_or_default())).unwrap_or_default();
    let mark_text = mark
        .map(|(pct, pen, b)| format!("{pct:.1}% (late penalty {pen} points). Breakdown: {b}"))
        .unwrap_or_else(|| "(not marked)".into());
    let guide = if key_hidden {
        "The answer key is hidden because the learner may try again: explain concepts, point at mistakes and ask \
         guiding questions, but never give final answers or full worked solutions."
    } else {
        "You may use the answer key to explain the right answers fully."
    };
    let mut prompt = format!(
        "Course: {course_title}\nYou are in review mode with the learner, going over one marked piece of work. Answer \
         their latest message in depth: explain questions, the rubric, the feedback and why marks were given or lost. \
         You can't change the mark. If you think the marking was wrong, say so plainly and explain why, so the learner \
         can note it. {guide} Write Markdown, with LaTeX for maths. Keep to this item and its subject.\n\n\
         ITEM: {}\n\nQUESTIONS + RUBRIC (JSON): {}\n\nANSWER KEY: {key}\n\nLEARNER'S TYPED ANSWER:\n{}\n\n\
         LEARNER'S PER-QUESTION ANSWERS (JSON): {}\n\nMARK: {mark_text}\n\nCONVERSATION SO FAR (latest last):\n{convo}\n",
        public.title,
        json!({"instructions": public.instructions, "questions": public.questions, "rubric": public.rubric}),
        if typed.trim().is_empty() { "(none)" } else { &typed },
        if answers.is_empty() { "{}" } else { &answers },
    );
    let mut tools = vec![];
    let mut add_dirs: Vec<String> = vec![];
    if !files.is_empty() {
        prompt += &format!(
            "\nThe learner's uploaded files (open them with the Read tool if the question is about them):\n{}\n",
            files.iter().map(|f| format!("- {f}")).collect::<Vec<_>>().join("\n")
        );
        tools.push("Read".to_string());
        for f in &files {
            if let Some(d) = std::path::Path::new(f).parent().map(|p| p.display().to_string())
                && !add_dirs.contains(&d)
            {
                add_dirs.push(d);
            }
        }
    }
    let mut req = StageRequest {
        tier: Tier::Sonnet,
        system: Some(super::prompts::HOUSE_RULES.to_string()),
        prompt,
        schema: reply_schema(),
        tools,
        add_dirs,
    };
    super::materials::attach(conn, &course_id, &mut req, super::materials::Use::Mark)?;
    jobs::enqueue_with(conn, Some(&course_id), "review", Some(item_id), jobs::PRIORITY_INTERACTIVE, &req).map_err(io)?;
    Ok(())
}

/// Claude answered.
pub fn on_done(conn: &Connection, item_id: &str, value: &Value) -> Result<(), String> {
    let reply = value.get("reply").and_then(Value::as_str).filter(|r| !r.trim().is_empty()).ok_or("empty reply")?;
    add(conn, item_id, "claude", reply.trim())
}

/// The call gave up. Say so in the thread rather than flagging the whole course.
pub fn on_failed(conn: &Connection, item_id: &str, error: &str) -> Result<(), String> {
    add(conn, item_id, "note", &format!("Couldn't get a reply ({error}). Send your message again to retry."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reply_schema_is_strict() {
        let s = reply_schema();
        assert_eq!(s["additionalProperties"], json!(false));
        assert_eq!(s["required"], json!(["reply"]));
    }
}
