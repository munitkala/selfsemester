//! Demo builds: an installer that comes with a finished course in it, so someone can look through a whole course
//! without building one first.
//!
//! `export` copies one built course (its database rows, folder and the tools it uses) into a folder. A demo
//! installer ships that folder as `demo\` next to the program. On the first launch of such a build, when this
//! computer has no SelfSemester data yet, `seed` copies it in and opens the whole course at once: every lesson and
//! piece of work is released, and the deadlines move so the course starts on the day it's installed.
//! Ordinary builds have no `demo\` folder, so none of this runs.

use crate::db;
use crate::paths::AppPaths;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};

fn io(e: rusqlite::Error) -> String {
    e.to_string()
}

/// Settings a demo keeps. Everything else (the terms notice, display choices, the last build level) is the new
/// owner's to set.
const KEEP_SETTINGS: &[&str] = &["seal_key", "app_version", crate::tools::SETUP_SETTING];

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let dest = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &dest)?;
        } else {
            std::fs::copy(e.path(), dest)?;
        }
    }
    Ok(())
}

/// Copy course `id` into `out` (made empty first): `app.db` with only that course and nothing personal in it,
/// `courses/<id>/`, and `tools/` with the installed tools.
pub fn export(paths: &AppPaths, conn: &Connection, id: &str, out: &Path) -> Result<(), String> {
    let status: String = conn.query_row("SELECT status FROM courses WHERE id=?1", [id], |r| r.get(0)).map_err(io)?;
    if status != "active" {
        return Err(format!("Only a built course can go in a demo; this one is {status}."));
    }
    if out.exists() {
        std::fs::remove_dir_all(out).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let db_out = out.join("app.db");
    conn.execute("VACUUM INTO ?1", [db_out.display().to_string()]).map_err(io)?;

    let d = Connection::open(&db_out).map_err(io)?;
    // Other courses go, with everything that hangs off them.
    let others: Vec<String> = {
        let mut st = d.prepare("SELECT id FROM courses WHERE id != ?1").map_err(io)?;
        st.query_map([id], |r| r.get(0)).map_err(io)?.collect::<Result<_, _>>().map_err(io)?
    };
    for o in &others {
        crate::course::pipeline::delete(&d, paths_for_cleanup(), o)?;
    }
    // Build logs hold the prompts, which name folders on this computer; study time and marks are the builder's own.
    d.execute_batch(
        "DELETE FROM jobs; DELETE FROM claude_calls; DELETE FROM study_time; DELETE FROM review_messages;
         DELETE FROM marks; DELETE FROM submissions; DELETE FROM transcript_entries;
         UPDATE courses SET paused_at=NULL, error=NULL;",
    )
    .map_err(io)?;
    let keep = KEEP_SETTINGS.iter().map(|k| format!("'{k}'")).collect::<Vec<_>>().join(",");
    d.execute(&format!("DELETE FROM settings WHERE key NOT IN ({keep})"), []).map_err(io)?;
    // Paths of the course's own files are set again on the new computer; keep just the file names.
    let mats: Vec<(String, String)> = {
        let mut st = d.prepare("SELECT id, path FROM materials").map_err(io)?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).map_err(io)?.collect::<Result<_, _>>().map_err(io)?
    };
    for (mid, p) in mats {
        let name = Path::new(&p).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        d.execute("UPDATE materials SET path=?2 WHERE id=?1", params![mid, name]).map_err(io)?;
    }
    d.execute_batch("UPDATE profiles SET name='Me'; VACUUM;").map_err(io)?;
    drop(d);

    copy_dir(&paths.course_dir(id), &out.join("courses").join(id)).map_err(|e| e.to_string())?;
    let tools = paths.data_dir.join("tools");
    if tools.is_dir() {
        copy_dir(&tools, &out.join("tools")).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// `delete` only needs folders for the courses it removes, and those live in the builder's data folder, which the
/// export must leave alone: point it at a folder that doesn't exist.
fn paths_for_cleanup() -> &'static AppPaths {
    static P: std::sync::OnceLock<AppPaths> = std::sync::OnceLock::new();
    P.get_or_init(|| {
        let nowhere = std::env::temp_dir().join("selfsemester-demo-export-none");
        AppPaths::rooted(&nowhere, &nowhere)
    })
}

/// The demo folder shipped with this build, if any: `demo\` next to the program.
pub fn bundled() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.join("demo");
    dir.join("app.db").is_file().then_some(dir)
}

/// First launch of a demo build: copy the course in and open all of it. Does nothing (returns false) when this
/// computer already has SelfSemester data, so a demo build never touches someone's own courses.
pub fn seed(paths: &AppPaths, demo: &Path, now: DateTime<Utc>) -> Result<bool, String> {
    if paths.db_file().exists() {
        return Ok(false);
    }
    paths.ensure().map_err(|e| e.to_string())?;
    let courses = demo.join("courses");
    if courses.is_dir() {
        copy_dir(&courses, &paths.courses_dir()).map_err(|e| e.to_string())?;
    }
    let tools = demo.join("tools");
    if tools.is_dir() {
        copy_dir(&tools, &paths.data_dir.join("tools")).map_err(|e| e.to_string())?;
    }
    std::fs::copy(demo.join("app.db"), paths.db_file()).map_err(|e| e.to_string())?;
    let conn = db::open(&paths.db_file()).map_err(io)?;
    let mats: Vec<(String, String, String)> = {
        let mut st = conn.prepare("SELECT id, course_id, path FROM materials").map_err(io)?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(io)?.collect::<Result<_, _>>().map_err(io)?
    };
    for (mid, course, name) in mats {
        let p = paths.course_dir(&course).join("materials").join(name);
        conn.execute("UPDATE materials SET path=?2 WHERE id=?1", params![mid, p.display().to_string()]).map_err(io)?;
    }
    open_everything(&conn, now)?;
    Ok(true)
}

fn shift(s: &str, by: Duration) -> Option<String> {
    if let Some(t) = crate::course::schedule::parse_ts(s) {
        return Some(crate::course::schedule::ts(t + by));
    }
    NaiveDate::parse_from_str(s.get(..10)?, "%Y-%m-%d").ok().map(|d| (d + by).to_string())
}

/// Move every running course so it starts today, and release all of its items now. Deadlines keep their spacing.
pub fn open_everything(conn: &Connection, now: DateTime<Utc>) -> Result<(), String> {
    let courses: Vec<(String, String, Option<String>)> = {
        let mut st = conn.prepare("SELECT id, starts_at, ends_at FROM courses WHERE status='active'").map_err(io)?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(io)?.collect::<Result<_, _>>().map_err(io)?
    };
    let released = crate::course::schedule::ts(now - Duration::minutes(1));
    for (id, starts, ends) in courses {
        let start = NaiveDate::parse_from_str(starts.get(..10).unwrap_or(""), "%Y-%m-%d").map_err(|e| e.to_string())?;
        let by = Duration::days((now.date_naive() - start).num_days());
        conn.execute(
            "UPDATE courses SET starts_at=?2, ends_at=?3 WHERE id=?1",
            params![id, shift(&starts, by), ends.as_deref().and_then(|e| shift(e, by))],
        )
        .map_err(io)?;
        let items: Vec<(String, Option<String>)> = {
            let mut st = conn.prepare("SELECT id, due_at FROM items WHERE course_id=?1").map_err(io)?;
            st.query_map([&id], |r| Ok((r.get(0)?, r.get(1)?))).map_err(io)?.collect::<Result<_, _>>().map_err(io)?
        };
        for (item, due) in items {
            conn.execute(
                "UPDATE items SET release_at=?2, due_at=?3 WHERE id=?1",
                params![item, released, due.as_deref().and_then(|d| shift(d, by))],
            )
            .map_err(io)?;
        }
    }
    crate::course::schedule::tick(conn, now).map_err(io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::course::pipeline::tests::built_course;

    /// Make the demo folder from a real data folder:
    /// `SS_DEMO_HOME=<SELFSEMESTER_HOME> SS_DEMO_COURSE=<id> SS_DEMO_OUT=<folder> cargo test --lib -- --ignored make_demo`
    #[test]
    #[ignore]
    fn make_demo() {
        let home = PathBuf::from(std::env::var("SS_DEMO_HOME").expect("SS_DEMO_HOME"));
        let paths = AppPaths::rooted(&home.join("AppData"), &home.join("Documents"));
        let conn = db::open(&paths.db_file()).unwrap();
        let id = std::env::var("SS_DEMO_COURSE").expect("SS_DEMO_COURSE");
        let out = PathBuf::from(std::env::var("SS_DEMO_OUT").expect("SS_DEMO_OUT"));
        export(&paths, &conn, &id, &out).unwrap();
        eprintln!("demo written to {}", out.display());
    }

    #[test]
    fn export_then_seed_opens_the_whole_course_from_today() {
        let tmp = tempfile::tempdir().unwrap();
        let (paths, conn, id) = built_course(&tmp.path().join("builder"));
        // A second course and some personal bits that mustn't travel.
        crate::course::pipeline::create(&conn, "Another subject", "").unwrap();
        db::set_setting(&conn, "notice_v3_accepted_at", "2026-01-01").unwrap();
        let out = tmp.path().join("demo");
        export(&paths, &conn, &id, &out).unwrap();
        assert!(paths.course_dir(&id).is_dir(), "the builder's own course is left alone");

        let d = Connection::open(out.join("app.db")).unwrap();
        let n = |sql: &str| -> i64 { d.query_row(sql, [], |r| r.get(0)).unwrap() };
        assert_eq!(n("SELECT count(*) FROM courses"), 1);
        assert_eq!(n("SELECT count(*) FROM jobs"), 0);
        assert_eq!(n("SELECT count(*) FROM settings WHERE key='notice_v3_accepted_at'"), 0);
        assert_eq!(n("SELECT count(*) FROM settings WHERE key='seal_key'"), 1);
        drop(d);

        let home = tmp.path().join("new-computer");
        let fresh = AppPaths::rooted(&home, &home);
        let later = Utc::now() + Duration::days(40);
        assert!(seed(&fresh, &out, later).unwrap());
        let c = db::open(&fresh.db_file()).unwrap();
        let start: String = c.query_row("SELECT starts_at FROM courses", [], |r| r.get(0)).unwrap();
        assert_eq!(&start[..10], later.date_naive().to_string());
        let locked: i64 = c.query_row("SELECT count(*) FROM items WHERE state='locked'", [], |r| r.get(0)).unwrap();
        assert_eq!(locked, 0, "everything is released");
        let past_due: i64 = c
            .query_row("SELECT count(*) FROM items WHERE due_at IS NOT NULL AND due_at < ?1", [crate::course::schedule::ts(later)], |r| r.get(0))
            .unwrap();
        assert_eq!(past_due, 0, "deadlines moved with the start");
        assert!(fresh.course_dir(&id).is_dir());
        // Sealed files still open with the copied key.
        let sealer = crate::course::seal::Sealer::load(&c).unwrap();
        let unit = std::fs::read_dir(fresh.course_dir(&id).join("units")).unwrap().next().unwrap().unwrap().path();
        assert!(sealer.open(&std::fs::read(unit).unwrap()).is_ok());
        // A second launch, or a computer that already has data, is left alone.
        assert!(!seed(&fresh, &out, later).unwrap());
    }
}
