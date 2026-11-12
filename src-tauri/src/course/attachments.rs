//! Documents attached to lessons and graded work in courses written in the course builder: a PDF of the lesson
//! instead of typing it, a worksheet, slides, images. They're copied into the course's own `files` folder, so the
//! originals can be moved or deleted, and opened in the learner's usual app.

use super::types::Attachment;
use crate::paths::AppPaths;
use rusqlite::{Connection, OptionalExtension};
use std::path::{Component, Path, PathBuf};

pub const MAX_BYTES: u64 = 50 * 1024 * 1024;
/// Text files short enough to become a lesson's text.
pub const MAX_TEXT_BYTES: u64 = 1024 * 1024;

/// Documents, slides, spreadsheets, text and images. Nothing that runs.
pub const EXTENSIONS: &[&str] = &[
    "pdf", "doc", "docx", "odt", "rtf", "txt", "md", "markdown", "ppt", "pptx", "odp", "key", "xls", "xlsx", "ods", "csv",
    "png", "jpg", "jpeg", "webp", "gif", "svg", "mp3", "m4a", "wav", "mp4", "webm", "epub",
];

const DIR: &str = "files";

fn clean_name(s: &str) -> String {
    let c: String = s.chars().map(|c| if r#"<>:"/\|?*"#.contains(c) || c.is_control() { '-' } else { c }).collect();
    let c = c.trim().trim_end_matches('.').trim();
    if c.is_empty() { "untitled".into() } else { c.chars().take(80).collect() }
}

/// `dir/name`, or `name (2)`, `(3)`... when that's taken.
fn free_name(dir: &Path, name: &str) -> String {
    if !dir.join(name).exists() {
        return name.to_string();
    }
    let p = Path::new(name);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let ext = p.extension().and_then(|s| s.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
    (2..).map(|n| format!("{stem} ({n}){ext}")).find(|c| !dir.join(c).exists()).expect("some free name")
}

/// Only courses written in the course builder take attachments.
fn manual_course(conn: &Connection, course_id: &str) -> Result<(), String> {
    let origin: Option<String> = conn
        .query_row("SELECT origin FROM courses WHERE id=?1", [course_id], |r| r.get(0))
        .optional()
        .map_err(|e| e.to_string())?;
    match origin.as_deref() {
        Some("manual") => Ok(()),
        Some(_) => Err("Documents can only be attached to courses you write in the course builder.".into()),
        None => Err("No such course.".into()),
    }
}

/// Check one file before anything is copied: its name, type and size.
pub fn check(src: &Path) -> Result<(String, u64), String> {
    let name = src.file_name().and_then(|n| n.to_str()).ok_or("Bad file name.")?.to_string();
    let ext = src.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).unwrap_or_default();
    if !EXTENSIONS.contains(&ext.as_str()) {
        return Err(format!(
            "{name}: attach documents (PDF, Word, slides, spreadsheets), text, images, audio or video. Programs and archives can't be attached."
        ));
    }
    let meta = std::fs::metadata(src).map_err(|e| format!("Couldn't open {name}: {e}"))?;
    if !meta.is_file() {
        return Err(format!("{name} isn't a file."));
    }
    if meta.len() > MAX_BYTES {
        return Err(format!("{name} is {} MB; the limit is {} MB per file.", meta.len() / 1_048_576, MAX_BYTES / 1_048_576));
    }
    Ok((name, meta.len()))
}

/// Copy one already-checked file into the course, under a name that isn't taken yet.
pub fn copy_in(paths: &AppPaths, course_id: &str, src: &Path, name: &str, bytes: u64) -> Result<Attachment, String> {
    let dir = paths.ensure_course(course_id).map_err(|e| e.to_string())?.join(DIR);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stored = free_name(&dir, &clean_name(name));
    std::fs::copy(src, dir.join(&stored)).map_err(|e| format!("Couldn't copy {name}: {e}"))?;
    Ok(Attachment { name: clean_name(name), file: format!("{DIR}/{stored}"), bytes })
}

/// Copy files into a course you're writing. Every file is checked first, so a bad one doesn't leave half the batch.
pub fn add(conn: &Connection, paths: &AppPaths, course_id: &str, files: &[PathBuf]) -> Result<Vec<Attachment>, String> {
    manual_course(conn, course_id)?;
    let checked = files.iter().map(|f| check(f).map(|c| (f, c))).collect::<Result<Vec<_>, _>>()?;
    checked.into_iter().map(|(src, (name, bytes))| copy_in(paths, course_id, src, &name, bytes)).collect()
}

/// The full path of an attachment, refusing anything outside the course's `files` folder.
pub fn path_of(paths: &AppPaths, course_id: &str, file: &str) -> Result<PathBuf, String> {
    let rel = Path::new(file);
    let mut parts = rel.components();
    let inside = matches!(parts.next(), Some(Component::Normal(d)) if d == DIR)
        && parts.clone().count() == 1
        && parts.all(|c| matches!(c, Component::Normal(_)));
    if !inside {
        return Err("That file isn't part of this course.".into());
    }
    let full = paths.course_dir(course_id).join(rel);
    if !full.is_file() {
        return Err("That file is missing from the course folder.".into());
    }
    Ok(full)
}

/// A text or Markdown attachment's contents, to use as a lesson's text.
pub fn read_text(paths: &AppPaths, course_id: &str, file: &str) -> Result<String, String> {
    let full = path_of(paths, course_id, file)?;
    let ext = full.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).unwrap_or_default();
    if !matches!(ext.as_str(), "txt" | "md" | "markdown") {
        return Err("Only text and Markdown files can be used as the lesson text; other documents stay attached.".into());
    }
    if std::fs::metadata(&full).map(|m| m.len()).unwrap_or(0) > MAX_TEXT_BYTES {
        return Err("That file is too long to use as lesson text; it stays attached instead.".into());
    }
    let bytes = std::fs::read(&full).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&bytes);
    Ok(text.trim_start_matches('\u{feff}').to_string())
}

/// Delete an attachment that was taken off a draft.
pub fn remove(conn: &Connection, paths: &AppPaths, course_id: &str, file: &str) -> Result<(), String> {
    manual_course(conn, course_id)?;
    let full = path_of(paths, course_id, file)?;
    std::fs::remove_file(full).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::course::builder;
    use crate::db;

    fn setup() -> (tempfile::TempDir, AppPaths, Connection, String) {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::rooted(&dir.path().join("data"), &dir.path().join("docs"));
        paths.ensure().unwrap();
        let conn = db::open_in_memory().unwrap();
        let id = builder::create(&conn, "Chess openings").unwrap();
        (dir, paths, conn, id)
    }

    #[test]
    fn copies_files_in_with_free_names_and_reads_text_back() {
        let (dir, paths, conn, id) = setup();
        let notes = dir.path().join("Week 1 notes.md");
        std::fs::write(&notes, "\u{feff}# Openings\n\nControl the centre.").unwrap();
        let a = add(&conn, &paths, &id, &[notes.clone(), notes.clone()]).unwrap();
        assert_eq!(a[0].file, "files/Week 1 notes.md");
        assert_eq!(a[1].file, "files/Week 1 notes (2).md");
        assert_eq!(read_text(&paths, &id, &a[0].file).unwrap(), "# Openings\n\nControl the centre.");
        remove(&conn, &paths, &id, &a[1].file).unwrap();
        assert!(path_of(&paths, &id, &a[1].file).is_err());
    }

    #[test]
    fn refuses_programs_and_paths_outside_the_course() {
        let (dir, paths, conn, id) = setup();
        let exe = dir.path().join("setup.exe");
        std::fs::write(&exe, "MZ").unwrap();
        assert!(add(&conn, &paths, &id, &[exe]).is_err());
        for bad in ["../app.db", "files/../app.db", "C:/Windows/win.ini", "files/a/b.pdf", "units/u01.json"] {
            assert!(path_of(&paths, &id, bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn only_courses_you_write_take_attachments() {
        let (dir, paths, conn, _) = setup();
        let claude = crate::course::pipeline::create(&conn, "Algebra", "").unwrap();
        let f = dir.path().join("a.pdf");
        std::fs::write(&f, "%PDF").unwrap();
        assert!(add(&conn, &paths, &claude, &[f]).is_err());
    }
}
