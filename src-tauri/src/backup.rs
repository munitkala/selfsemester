//! Keeping your work safe across updates.
//!
//! The installer replaces only the program files, but a new version upgrades the database the first time it runs,
//! and the program folder and the data folder are the same folder (`%LOCALAPPDATA%\SelfSemester`). So:
//!
//! - Before a new version (or a database upgrade) opens your data for the first time, everything that matters is
//!   copied to `Documents\SelfSemester\Backups\<date> before <version>`: the database (a consistent snapshot, taken
//!   with SQLite's `VACUUM INTO`) and every course's files. The newest `KEEP` backups are kept.
//! - If the app starts and finds no database but there is a backup, it restores the newest one before opening.
//!
//! Left out on purpose: `claude-work` (an empty working folder) and `logs`.

use crate::paths::AppPaths;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

/// How many backups to keep; older ones are deleted.
pub const KEEP: usize = 5;
const MANIFEST: &str = "backup.json";
const DB_FILE: &str = "app.db";
/// Folders under the data folder that hold your work.
const FOLDERS: &[&str] = &["courses", "domains"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    pub created_at: String,
    /// Why it was made: "update" (before a new version first ran) or "manual".
    pub reason: String,
    /// The app version that last used the data (unknown for data from before 0.4.0).
    pub from_version: Option<String>,
    /// The app version that made the backup.
    pub app_version: String,
    pub schema_version: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupInfo {
    pub folder: String,
    pub path: String,
    pub bytes: u64,
    pub manifest: Option<Manifest>,
}

/// What happened when the app started.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Startup {
    /// First run on this computer, nothing to protect.
    Fresh,
    /// Same version as last time, nothing to do.
    Unchanged,
    /// A new version: the data was backed up first.
    BackedUp { folder: String },
    /// The data was missing, so the newest backup was put back.
    Restored { folder: String, created_at: String },
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn dir_size(p: &Path) -> u64 {
    match std::fs::metadata(p) {
        Ok(m) if m.is_file() => m.len(),
        Ok(m) if m.is_dir() => std::fs::read_dir(p).map(|rd| rd.flatten().map(|e| dir_size(&e.path())).sum()).unwrap_or(0),
        _ => 0,
    }
}

fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// The version that last used this database, and its schema version. Opens it read-only and changes nothing.
fn read_state(db: &Path) -> io::Result<(Option<String>, u32)> {
    let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(io::Error::other)?;
    let schema: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).map_err(io::Error::other)?;
    let has_settings: bool = conn
        .query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='settings'", [], |r| r.get::<_, i64>(0))
        .map_err(io::Error::other)?
        > 0;
    let version = if has_settings {
        conn.query_row("SELECT value FROM settings WHERE key='app_version'", [], |r| r.get(0)).optional().map_err(io::Error::other)?
    } else {
        None
    };
    Ok((version, schema))
}

/// Copy the database and the course files to a new backup folder. Returns the folder.
pub fn create(paths: &AppPaths, reason: &str, from_version: Option<&str>, app_version: &str) -> io::Result<PathBuf> {
    let db = paths.db_file();
    let (_, schema) = read_state(&db)?;
    let root = paths.backups_dir();
    std::fs::create_dir_all(&root)?;
    let stamp = chrono::Local::now().format("%Y-%m-%d %H%M%S");
    let label = if reason == "update" { format!("before {app_version}") } else { "manual".to_string() };
    let base = format!("{stamp} {label}");
    let dir = (1..).map(|n| if n == 1 { root.join(&base) } else { root.join(format!("{base} ({n})")) }).find(|p| !p.exists()).unwrap();
    // build it under a temporary name, so a half-made backup is never mistaken for a good one
    let tmp = root.join(format!(".partial-{}", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)?;
    let result = (|| -> io::Result<()> {
        let conn = Connection::open_with_flags(&db, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(io::Error::other)?;
        conn.execute("VACUUM INTO ?1", [tmp.join(DB_FILE).to_string_lossy()]).map_err(io::Error::other)?;
        drop(conn);
        for f in FOLDERS {
            let src = paths.data_dir.join(f);
            if src.is_dir() {
                copy_dir(&src, &tmp.join(f))?;
            }
        }
        let manifest = Manifest {
            created_at: now(),
            reason: reason.into(),
            from_version: from_version.map(str::to_string),
            app_version: app_version.into(),
            schema_version: schema,
        };
        std::fs::write(tmp.join(MANIFEST), serde_json::to_string_pretty(&manifest).unwrap())?;
        Ok(())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, &dir)?;
    prune(paths)?;
    Ok(dir)
}

/// Finished backups, newest first.
pub fn list(paths: &AppPaths) -> Vec<BackupInfo> {
    let mut out: Vec<BackupInfo> = std::fs::read_dir(paths.backups_dir())
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().join(DB_FILE).is_file() && !e.file_name().to_string_lossy().starts_with('.'))
                .map(|e| {
                    let p = e.path();
                    BackupInfo {
                        folder: e.file_name().to_string_lossy().into_owned(),
                        path: p.display().to_string(),
                        bytes: dir_size(&p),
                        manifest: std::fs::read_to_string(p.join(MANIFEST)).ok().and_then(|s| serde_json::from_str(&s).ok()),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| b.folder.cmp(&a.folder)); // folders start with the date and time
    out
}

fn prune(paths: &AppPaths) -> io::Result<()> {
    for old in list(paths).into_iter().skip(KEEP) {
        std::fs::remove_dir_all(old.path)?;
    }
    Ok(())
}

/// Put the newest backup back into an empty data folder. Never overwrites an existing database.
fn restore_latest(paths: &AppPaths) -> io::Result<Option<BackupInfo>> {
    let Some(latest) = list(paths).into_iter().next() else { return Ok(None) };
    let from = PathBuf::from(&latest.path);
    if paths.db_file().exists() {
        return Err(io::Error::other("there is already a database; not restoring over it"));
    }
    std::fs::copy(from.join(DB_FILE), paths.db_file())?;
    for f in FOLDERS {
        if from.join(f).is_dir() {
            copy_dir(&from.join(f), &paths.data_dir.join(f))?;
        }
    }
    Ok(Some(latest))
}

/// Run before the database is opened (and upgraded) at start-up.
pub fn before_open(paths: &AppPaths, app_version: &str, latest_schema: u32) -> io::Result<Startup> {
    if !paths.db_file().exists() {
        return Ok(match restore_latest(paths)? {
            Some(b) => Startup::Restored { created_at: b.manifest.map(|m| m.created_at).unwrap_or_default(), folder: b.folder },
            None => Startup::Fresh,
        });
    }
    let (last, schema) = read_state(&paths.db_file())?;
    if last.as_deref() == Some(app_version) && schema >= latest_schema {
        return Ok(Startup::Unchanged);
    }
    let dir = create(paths, "update", last.as_deref(), app_version)?;
    Ok(Startup::BackedUp { folder: dir.file_name().unwrap().to_string_lossy().into_owned() })
}

/// Remember which version last used the data, after it's open.
pub fn mark_version(conn: &Connection, app_version: &str) -> rusqlite::Result<()> {
    crate::db::set_setting(conn, "app_version", app_version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn setup() -> (tempfile::TempDir, AppPaths) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = AppPaths::rooted(&tmp.path().join("local"), &tmp.path().join("docs"));
        paths.ensure().unwrap();
        (tmp, paths)
    }

    /// A database with one course and a file in its folder, last used by `version`.
    fn some_work(paths: &AppPaths, version: Option<&str>) {
        let conn = db::open(&paths.db_file()).unwrap();
        crate::course::pipeline::create(&conn, "Linear algebra", "").unwrap();
        if let Some(v) = version {
            mark_version(&conn, v).unwrap();
        }
        let dir = paths.ensure_course("c1").unwrap();
        std::fs::write(dir.join("materials").join("notes.md"), "my notes").unwrap();
    }

    fn latest() -> u32 {
        5
    }

    #[test]
    fn first_run_has_nothing_to_back_up() {
        let (_t, paths) = setup();
        assert_eq!(before_open(&paths, "0.4.0", latest()).unwrap(), Startup::Fresh);
        assert!(list(&paths).is_empty());
    }

    #[test]
    fn a_new_version_backs_up_first_and_the_same_version_does_not() {
        let (_t, paths) = setup();
        some_work(&paths, None); // data from before versions were recorded
        let s = before_open(&paths, "0.4.0", latest()).unwrap();
        assert!(matches!(s, Startup::BackedUp { .. }), "{s:?}");
        let b = &list(&paths)[0];
        assert_eq!(b.manifest.as_ref().unwrap().from_version, None);
        assert_eq!(b.manifest.as_ref().unwrap().reason, "update");
        assert!(Path::new(&b.path).join("courses/c1/materials/notes.md").is_file());
        // the copy is a working database with the course in it
        let copy = Connection::open(Path::new(&b.path).join("app.db")).unwrap();
        let n: i64 = copy.query_row("SELECT count(*) FROM courses", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);

        let conn = db::open(&paths.db_file()).unwrap();
        mark_version(&conn, "0.4.0").unwrap();
        drop(conn);
        assert_eq!(before_open(&paths, "0.4.0", latest()).unwrap(), Startup::Unchanged);
        assert!(matches!(before_open(&paths, "0.5.0", latest()).unwrap(), Startup::BackedUp { .. }));
        assert_eq!(list(&paths)[0].manifest.as_ref().unwrap().from_version.as_deref(), Some("0.4.0"));
    }

    #[test]
    fn missing_data_comes_back_from_the_newest_backup() {
        let (_t, paths) = setup();
        some_work(&paths, Some("0.4.0"));
        create(&paths, "manual", Some("0.4.0"), "0.4.0").unwrap();
        // the data folder is wiped (say, by hand), but Documents isn't
        std::fs::remove_dir_all(&paths.data_dir).unwrap();
        paths.ensure().unwrap();
        let s = before_open(&paths, "0.4.0", latest()).unwrap();
        assert!(matches!(s, Startup::Restored { .. }), "{s:?}");
        let conn = db::open(&paths.db_file()).unwrap();
        let n: i64 = conn.query_row("SELECT count(*) FROM courses", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
        assert_eq!(std::fs::read_to_string(paths.course_dir("c1").join("materials/notes.md")).unwrap(), "my notes");
    }

    #[test]
    fn keeps_only_the_newest_backups() {
        let (_t, paths) = setup();
        some_work(&paths, Some("0.4.0"));
        for _ in 0..(KEEP + 2) {
            create(&paths, "manual", Some("0.4.0"), "0.4.0").unwrap();
        }
        assert_eq!(list(&paths).len(), KEEP);
        let leftovers = std::fs::read_dir(paths.backups_dir()).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with('.')).count();
        assert_eq!(leftovers, 0, "no half-made backups left behind");
    }
}
