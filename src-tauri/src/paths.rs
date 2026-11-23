//! Where SelfSemester keeps things on disk.
//!
//! App internals (database, sealed exams, downloads, logs) live in the system's local app data folder:
//! `%LOCALAPPDATA%\SelfSemester` on Windows, `~/.local/share/SelfSemester` on Linux.
//! Anything meant for the learner (transcript, completed-course archives) lives in `Documents/SelfSemester`.

use std::io;
use std::path::{Path, PathBuf};

pub const APP_DIR_NAME: &str = "SelfSemester";

#[derive(Debug, Clone, serde::Serialize)]
pub struct AppPaths {
    /// `%LOCALAPPDATA%\SelfSemester` on Windows, `~/.local/share/SelfSemester` on Linux
    pub data_dir: PathBuf,
    /// `Documents/SelfSemester`
    pub user_dir: PathBuf,
}

/// The learner's Documents folder. On Linux, desktops that haven't set up user folders report none, so use
/// `~/Documents` instead of refusing to start.
fn documents_dir() -> Option<PathBuf> {
    pick_documents(dirs::document_dir(), dirs::home_dir())
}

/// `document` when the desktop knows it, otherwise `home/Documents`. Split out so it can be tested.
fn pick_documents(document: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    document.or_else(|| home.map(|home| home.join("Documents")))
}

impl AppPaths {
    /// Standard locations for the system, or everything under `SELFSEMESTER_HOME` when that's set (for a separate
    /// test or demo copy that mustn't touch real courses).
    pub fn standard() -> io::Result<Self> {
        if let Some(home) = std::env::var_os("SELFSEMESTER_HOME").filter(|h| !h.is_empty()) {
            let home = Path::new(&home);
            return Ok(Self::rooted(&home.join("AppData"), &home.join("Documents")));
        }
        let local = dirs::data_local_dir()
            .ok_or_else(|| io::Error::other("could not find the local app data folder"))?;
        let docs = documents_dir()
            .ok_or_else(|| io::Error::other("could not find the Documents folder"))?;
        Ok(Self::rooted(&local, &docs))
    }

    /// Paths under explicit roots (used by tests).
    pub fn rooted(local_root: &Path, docs_root: &Path) -> Self {
        Self {
            data_dir: local_root.join(APP_DIR_NAME),
            user_dir: docs_root.join(APP_DIR_NAME),
        }
    }

    pub fn db_file(&self) -> PathBuf {
        self.data_dir.join("app.db")
    }
    pub fn domains_dir(&self) -> PathBuf {
        self.data_dir.join("domains")
    }
    pub fn courses_dir(&self) -> PathBuf {
        self.data_dir.join("courses")
    }
    pub fn course_dir(&self, course_id: &str) -> PathBuf {
        self.courses_dir().join(course_id)
    }
    /// Empty working folder Claude Code runs in, so it never picks up a project's CLAUDE.md or files.
    pub fn claude_workdir(&self) -> PathBuf {
        self.data_dir.join("claude-work")
    }
    pub fn logs_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }
    pub fn completed_dir(&self) -> PathBuf {
        self.user_dir.join("Completed")
    }
    /// Safety copies of your data, made before each update (see backup.rs). Outside the program folder on purpose.
    pub fn backups_dir(&self) -> PathBuf {
        self.user_dir.join("Backups")
    }

    /// Create every top-level folder. Safe to call on each launch.
    pub fn ensure(&self) -> io::Result<()> {
        for dir in [
            self.data_dir.clone(),
            self.domains_dir(),
            self.courses_dir(),
            self.claude_workdir(),
            self.logs_dir(),
            self.user_dir.clone(),
            self.completed_dir(),
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    /// Per-course folder layout (see docs/RESEARCH.md "Storage").
    pub fn ensure_course(&self, course_id: &str) -> io::Result<PathBuf> {
        let root = self.course_dir(course_id);
        for sub in ["units", "sealed", "sources", "submissions", "materials"] {
            std::fs::create_dir_all(root.join(sub))?;
        }
        Ok(root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_falls_back_to_home() {
        // Linux desktops without user folders configured report no Documents folder.
        let home = PathBuf::from("home").join("learner");
        let chosen = PathBuf::from("custom").join("Docs");
        assert_eq!(pick_documents(None, Some(home.clone())), Some(home.join("Documents")));
        assert_eq!(pick_documents(Some(chosen.clone()), Some(home)), Some(chosen));
        assert_eq!(pick_documents(None, None), None);
    }

    #[test]
    fn creates_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let p = AppPaths::rooted(&tmp.path().join("local"), &tmp.path().join("docs"));
        p.ensure().unwrap();
        let c = p.ensure_course("c1").unwrap();
        assert!(p.db_file().parent().unwrap().is_dir());
        assert!(p.completed_dir().is_dir());
        assert!(p.claude_workdir().is_dir());
        assert!(c.join("sealed").is_dir());
        assert!(c.ends_with("SelfSemester/courses/c1") || c.ends_with("SelfSemester\\courses\\c1"));
    }
}
