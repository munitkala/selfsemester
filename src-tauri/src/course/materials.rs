//! The learner's own materials for a course: reference files, images and typed notes.
//!
//! Everything is copied into `courses/<id>/materials/`. When the course is planned, written or marked, the stage gets
//! read access to that folder and a list of what's in it, so Claude can use the learner's notes and files.

use crate::claude::StageRequest;
use crate::db;
use crate::paths::AppPaths;
use rusqlite::{Connection, params};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Big enough for a scanned chapter or a phone photo, small enough to keep prompts and the course folder sane.
pub const MAX_BYTES: u64 = 30 * 1024 * 1024;
pub const MAX_NOTE_CHARS: usize = 100_000;

pub const IMAGE_EXT: &[&str] = &["png", "jpg", "jpeg", "webp", "gif"];
/// Things Claude Code's Read tool can open: PDFs and plain text (notes, data, code).
pub const FILE_EXT: &[&str] = &[
    "pdf", "txt", "md", "markdown", "csv", "tsv", "json", "tex", "bib", "html", "xml", "yaml", "yml", "py", "rs", "js",
    "ts", "c", "h", "cpp", "hpp", "cs", "java", "go", "gd", "lua", "sql", "sh", "ps1",
];

#[derive(Debug, Serialize, Clone)]
pub struct Material {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub path: String,
    pub bytes: i64,
    pub added_at: String,
}

fn io(e: rusqlite::Error) -> String {
    e.to_string()
}

fn new_id() -> String {
    let mut b = [0u8; 2];
    let _ = aes_gcm::aead::rand_core::RngCore::try_fill_bytes(&mut aes_gcm::aead::OsRng, &mut b);
    format!("m{:x}{:04x}", chrono::Utc::now().timestamp_micros(), u16::from_le_bytes(b))
}

fn kind_of(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if IMAGE_EXT.contains(&ext.as_str()) {
        Some("image")
    } else if FILE_EXT.contains(&ext.as_str()) {
        Some("file")
    } else {
        None
    }
}

fn clean_name(s: &str) -> String {
    let cleaned: String = s.chars().map(|c| if r#"<>:"/\|?*"#.contains(c) || c.is_control() { '-' } else { c }).collect();
    let cleaned = cleaned.trim().trim_end_matches('.').trim();
    if cleaned.is_empty() { "untitled".into() } else { cleaned.chars().take(80).collect() }
}

/// `dir/name`, or `dir/name (2)`, `(3)`... when that's taken.
fn free_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let p = Path::new(name);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let ext = p.extension().and_then(|s| s.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|c| !c.exists())
        .expect("some free name")
}

fn course_exists(conn: &Connection, course_id: &str) -> Result<(), String> {
    let n: i64 = conn.query_row("SELECT count(*) FROM courses WHERE id=?1", [course_id], |r| r.get(0)).map_err(io)?;
    if n == 0 { Err("No such course.".into()) } else { Ok(()) }
}

fn insert(conn: &Connection, course_id: &str, kind: &str, name: &str, path: &Path, bytes: u64) -> Result<(), String> {
    conn.execute(
        "INSERT INTO materials(id, course_id, kind, name, path, bytes, added_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![new_id(), course_id, kind, name, path.display().to_string(), bytes as i64, db::now_utc()],
    )
    .map_err(io)?;
    Ok(())
}

/// Copy files into the course. Checks every file first, so a bad one doesn't leave half the batch added.
pub fn add_files(conn: &Connection, paths: &AppPaths, course_id: &str, files: &[PathBuf]) -> Result<usize, String> {
    course_exists(conn, course_id)?;
    let mut checked = Vec::new();
    for f in files {
        let name = f.file_name().and_then(|n| n.to_str()).ok_or("Bad file name.")?;
        let kind = kind_of(f).ok_or_else(|| {
            format!("{name}: SelfSemester can take PDFs, images (PNG, JPG, WebP, GIF) and text files such as notes or code.")
        })?;
        let meta = std::fs::metadata(f).map_err(|e| format!("Couldn't open {name}: {e}"))?;
        if !meta.is_file() {
            return Err(format!("{name} isn't a file."));
        }
        if meta.len() > MAX_BYTES {
            return Err(format!("{name} is {} MB; the limit is {} MB per file.", meta.len() / 1_048_576, MAX_BYTES / 1_048_576));
        }
        checked.push((f, clean_name(name), kind, meta.len()));
    }
    let dir = paths.ensure_course(course_id).map_err(|e| e.to_string())?.join("materials");
    for (src, name, kind, bytes) in &checked {
        let dest = free_path(&dir, name);
        std::fs::copy(src, &dest).map_err(|e| format!("Couldn't copy {name}: {e}"))?;
        insert(conn, course_id, kind, name, &dest, *bytes)?;
    }
    Ok(checked.len())
}

/// Save a typed note as a Markdown file in the course's materials.
pub fn add_note(conn: &Connection, paths: &AppPaths, course_id: &str, title: &str, text: &str) -> Result<(), String> {
    course_exists(conn, course_id)?;
    let text = text.trim();
    if text.is_empty() {
        return Err("The note is empty.".into());
    }
    if text.chars().count() > MAX_NOTE_CHARS {
        return Err("That note is very long. Save it as a text file and add it as a file instead.".into());
    }
    let title = if title.trim().is_empty() { "Note".to_string() } else { title.trim().chars().take(80).collect() };
    let dir = paths.ensure_course(course_id).map_err(|e| e.to_string())?.join("materials");
    let dest = free_path(&dir, &format!("{}.md", clean_name(&title)));
    let body = format!("# {title}\n\n{text}\n");
    std::fs::write(&dest, &body).map_err(|e| e.to_string())?;
    insert(conn, course_id, "note", &title, &dest, body.len() as u64)
}

pub fn list(conn: &Connection, course_id: &str) -> Result<Vec<Material>, String> {
    let mut st = conn
        .prepare("SELECT id, kind, name, path, bytes, added_at FROM materials WHERE course_id=?1 ORDER BY added_at, id")
        .map_err(io)?;
    let rows = st
        .query_map([course_id], |r| {
            Ok(Material { id: r.get(0)?, kind: r.get(1)?, name: r.get(2)?, path: r.get(3)?, bytes: r.get(4)?, added_at: r.get(5)? })
        })
        .map_err(io)?;
    rows.collect::<Result<_, _>>().map_err(io)
}

/// One material, but only if its copy really sits in that course's materials folder.
fn owned(conn: &Connection, paths: &AppPaths, id: &str) -> Result<(String, PathBuf), String> {
    let (course, path): (String, String) = conn
        .query_row("SELECT course_id, path FROM materials WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|_| "That material is gone.".to_string())?;
    let path = PathBuf::from(path);
    if !path.starts_with(paths.course_dir(&course).join("materials")) {
        return Err("That file isn't in the course folder.".into());
    }
    Ok((course, path))
}

pub fn remove(conn: &Connection, paths: &AppPaths, id: &str) -> Result<(), String> {
    let (_, path) = owned(conn, paths, id)?;
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    conn.execute("DELETE FROM materials WHERE id=?1", [id]).map_err(io)?;
    Ok(())
}

/// Path of a material's copy, for opening it from the app.
pub fn file_of(conn: &Connection, paths: &AppPaths, id: &str) -> Result<PathBuf, String> {
    let (_, path) = owned(conn, paths, id)?;
    if !path.is_file() {
        return Err("The file is missing from the course folder.".into());
    }
    Ok(path)
}

/// What the stage is for, which changes how Claude should use the materials.
pub enum Use {
    Plan,
    Write,
    Mark,
}

/// Point a stage at the learner's materials: list them in the prompt, allow Read, and give access to the folder.
pub fn attach(conn: &Connection, course_id: &str, req: &mut StageRequest, use_: Use) -> Result<(), String> {
    let items: Vec<Material> = list(conn, course_id)?.into_iter().filter(|m| Path::new(&m.path).is_file()).collect();
    if items.is_empty() {
        return Ok(());
    }
    let how = match use_ {
        Use::Plan => "Read them before planning. Shape the syllabus around them where they fit (for example a set \
                      textbook, a past syllabus or notes on what the learner already knows), and follow any wishes in \
                      the notes as long as they fit a sound course.",
        Use::Write => "Open the ones that matter for this unit with the Read tool. Build on them where they fit: \
                       follow the notes, use the learner's terms and notation, and refer to their files by name. \
                       They're the learner's own, so don't add them to the reading list.",
        Use::Mark => "They are background only (for example the notation or method the learner was taught). Open them \
                      only if they help you mark fairly; never mark anything in them.",
    };
    let list = items
        .iter()
        .map(|m| match m.kind.as_str() {
            "note" => format!("- note \"{}\": {}", m.name, m.path),
            k => format!("- {k} {}: {}", m.name, m.path),
        })
        .collect::<Vec<_>>()
        .join("\n");
    req.prompt += &format!("\n\nTHE LEARNER'S OWN MATERIALS FOR THIS COURSE. {how}\n{list}\n");
    if !req.tools.iter().any(|t| t == "Read") {
        req.tools.push("Read".into());
    }
    for m in &items {
        if let Some(dir) = Path::new(&m.path).parent().map(|p| p.display().to_string())
            && !req.add_dirs.contains(&dir)
        {
            req.add_dirs.push(dir);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claude::Tier;
    use crate::course::pipeline;

    fn setup() -> (tempfile::TempDir, AppPaths, Connection, String) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = AppPaths::rooted(&tmp.path().join("local"), &tmp.path().join("docs"));
        paths.ensure().unwrap();
        let conn = db::open_in_memory().unwrap();
        let id = pipeline::create(&conn, "Linear algebra", "").unwrap();
        (tmp, paths, conn, id)
    }

    fn blank_req() -> StageRequest {
        StageRequest { tier: Tier::Sonnet, system: None, prompt: "Write the unit.".into(), schema: serde_json::json!({}), tools: vec![], add_dirs: vec![] }
    }

    #[test]
    fn adds_lists_and_removes() {
        let (tmp, paths, conn, id) = setup();
        let src = tmp.path().join("week1 notes.md");
        std::fs::write(&src, "eigenvalues first").unwrap();
        let img = tmp.path().join("board.PNG");
        std::fs::write(&img, [137u8, 80, 78, 71]).unwrap();
        assert_eq!(add_files(&conn, &paths, &id, &[src.clone(), img]).unwrap(), 2);
        add_files(&conn, &paths, &id, &[src]).unwrap(); // same name again gets "(2)"
        add_note(&conn, &paths, &id, "What I know", "  I know matrix multiplication.  ").unwrap();

        let all = list(&conn, &id).unwrap();
        assert_eq!(all.iter().map(|m| m.kind.as_str()).collect::<Vec<_>>(), ["file", "image", "file", "note"]);
        assert!(all[2].path.ends_with("week1 notes (2).md"));
        let note = std::fs::read_to_string(&all[3].path).unwrap();
        assert_eq!(note, "# What I know\n\nI know matrix multiplication.\n");
        for m in &all {
            assert!(Path::new(&m.path).starts_with(paths.course_dir(&id).join("materials")));
        }

        remove(&conn, &paths, &all[0].id).unwrap();
        assert!(!Path::new(&all[0].path).exists());
        assert_eq!(list(&conn, &id).unwrap().len(), 3);
        assert!(file_of(&conn, &paths, &all[0].id).is_err());
    }

    #[test]
    fn refuses_bad_files_without_adding_any() {
        let (tmp, paths, conn, id) = setup();
        let ok = tmp.path().join("a.txt");
        std::fs::write(&ok, "fine").unwrap();
        let exe = tmp.path().join("setup.exe");
        std::fs::write(&exe, "MZ").unwrap();
        assert!(add_files(&conn, &paths, &id, &[ok.clone(), exe]).unwrap_err().contains("setup.exe"));
        assert!(add_files(&conn, &paths, &id, &[tmp.path().join("missing.pdf")]).is_err());
        assert!(add_files(&conn, &paths, "nope", &[ok]).is_err());
        assert!(add_note(&conn, &paths, &id, "Empty", "   ").is_err());
        assert!(list(&conn, &id).unwrap().is_empty());
    }

    #[test]
    fn attach_adds_read_access_only_when_there_is_something() {
        let (tmp, paths, conn, id) = setup();
        let mut r = blank_req();
        attach(&conn, &id, &mut r, Use::Write).unwrap();
        assert!(r.tools.is_empty() && r.add_dirs.is_empty() && r.prompt == "Write the unit.");

        let pdf = tmp.path().join("textbook ch1.pdf");
        std::fs::write(&pdf, "%PDF-1.4").unwrap();
        add_files(&conn, &paths, &id, &[pdf]).unwrap();
        add_note(&conn, &paths, &id, "Exam board", "Use the AQA notation.").unwrap();
        let mut r = blank_req();
        r.tools.push("Read".into());
        attach(&conn, &id, &mut r, Use::Mark).unwrap();
        assert_eq!(r.tools, ["Read"]);
        assert_eq!(r.add_dirs, [paths.course_dir(&id).join("materials").display().to_string()]);
        assert!(r.prompt.contains("file textbook ch1.pdf: ") && r.prompt.contains("note \"Exam board\""));
        assert!(r.prompt.contains("never mark anything in them"));
    }

    #[test]
    fn deleting_the_course_drops_its_materials() {
        let (tmp, paths, conn, id) = setup();
        let f = tmp.path().join("n.md");
        std::fs::write(&f, "x").unwrap();
        add_files(&conn, &paths, &id, &[f]).unwrap();
        pipeline::delete(&conn, &paths, &id).unwrap();
        let n: i64 = conn.query_row("SELECT count(*) FROM materials", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
        assert!(!paths.course_dir(&id).exists());
    }
}
