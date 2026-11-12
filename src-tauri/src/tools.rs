//! Tools: the extra pieces lessons can use, listed in `tools/catalog.json` (built into the app).
//!
//! - built-in: part of the app (maths, graphs, sounds, interactive figures); listed so Settings can say what the
//!   app uses, never installed or removed.
//! - common: offered when the app is first set up and always listed in Settings, with Install and Remove.
//! - extra: only for particular courses. A course plan names the ones it needs and the review step asks whether to
//!   install them. Settings lists an extra tool only while it's installed; once removed it's gone from the list.
//!
//! A tool is a few files (JavaScript libraries, data and their licences) downloaded from the project's GitHub
//! release named in the catalog, each checked against the SHA-256 recorded there. Interactive figures load them from
//! the widget scheme (`/t/<tool>/<file>`, see widget.rs). Nothing here depends on which model writes the courses.

use crate::paths::AppPaths;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

const CATALOG_JSON: &str = include_str!("../../tools/catalog.json");
/// Settings key: "1" once the first-run tool picker has been answered.
pub const SETUP_SETTING: &str = "tools_setup_done";
/// Largest file a tool may contain.
const MAX_FILE: u64 = 20 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    pub release: String,
    pub tools: Vec<Tool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub id: String,
    pub name: String,
    /// built-in | common | extra
    pub category: String,
    pub summary: String,
    #[serde(rename = "for", default)]
    pub for_courses: String,
    #[serde(default)]
    pub licence: String,
    #[serde(default)]
    pub credit: String,
    /// How a lesson writer uses it (globals and a short example); goes into the course prompts.
    #[serde(default)]
    pub hint: String,
    #[serde(default)]
    pub scripts: Vec<String>,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default)]
    pub files: Vec<ToolFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFile {
    pub name: String,
    #[serde(default, skip_serializing)]
    pub from: String,
    #[serde(default, skip_serializing)]
    pub transform: Option<String>,
    pub sha256: String,
    pub bytes: u64,
}

impl Tool {
    pub fn bytes(&self) -> u64 {
        self.files.iter().map(|f| f.bytes).sum()
    }
    pub fn installable(&self) -> bool {
        self.category != "built-in" && !self.files.is_empty()
    }
}

pub fn catalog() -> &'static Catalog {
    static C: OnceLock<Catalog> = OnceLock::new();
    C.get_or_init(|| serde_json::from_str(CATALOG_JSON).expect("tools/catalog.json is valid"))
}

pub fn find(id: &str) -> Option<&'static Tool> {
    catalog().tools.iter().find(|t| t.id == id)
}

fn root(paths: &AppPaths) -> PathBuf {
    paths.data_dir.join("tools")
}

fn marker(paths: &AppPaths, id: &str) -> PathBuf {
    root(paths).join(id).join(".installed")
}

pub fn is_installed(paths: &AppPaths, id: &str) -> bool {
    match find(id) {
        Some(t) if t.category == "built-in" => true,
        Some(_) => marker(paths, id).is_file(),
        None => false,
    }
}

/// Installed tools that lessons can load (built-ins aren't loadable files).
pub fn installed(paths: &AppPaths) -> Vec<&'static Tool> {
    catalog().tools.iter().filter(|t| t.installable() && is_installed(paths, &t.id)).collect()
}

/// A file of an installed tool, only if the catalog lists it (so no path can leave the tool's folder).
pub fn file(paths: &AppPaths, id: &str, name: &str) -> Option<PathBuf> {
    let t = find(id)?;
    if !t.installable() || !is_installed(paths, id) || !t.files.iter().any(|f| f.name == name) {
        return None;
    }
    Some(root(paths).join(id).join(name))
}

// ---------- what Settings shows ----------

#[derive(Debug, Clone, Serialize)]
pub struct ToolView {
    #[serde(flatten)]
    pub tool: Tool,
    pub installed: bool,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolsView {
    pub tools: Vec<ToolView>,
    /// Why tools can't be installed or removed right now, if they can't.
    pub locked: Option<String>,
    pub setup_done: bool,
    pub installing: Vec<String>,
}

/// Built-in and common tools always; extra tools only while installed.
pub fn view(paths: &AppPaths, conn: &Connection) -> ToolsView {
    let tools = catalog()
        .tools
        .iter()
        .filter(|t| t.category != "extra" || is_installed(paths, &t.id))
        .map(|t| ToolView { tool: t.clone(), installed: is_installed(paths, &t.id), size: t.bytes() })
        .collect();
    ToolsView {
        tools,
        locked: locked(conn),
        setup_done: crate::db::get_setting(conn, SETUP_SETTING).ok().flatten().as_deref() == Some("1"),
        installing: busy().lock().unwrap().iter().cloned().collect(),
    }
}

/// The named tools that can be installed (for a course's review step), whatever their category.
pub fn lookup(paths: &AppPaths, ids: &[String]) -> Vec<ToolView> {
    ids.iter()
        .filter_map(|id| find(id))
        .filter(|t| t.installable())
        .map(|t| ToolView { tool: t.clone(), installed: is_installed(paths, &t.id), size: t.bytes() })
        .collect()
}

/// Tools can't change while a course is being planned or built (its lessons are being written against the installed
/// tools) or while any Claude job is running, or while another install is under way.
pub fn locked(conn: &Connection) -> Option<String> {
    let building: i64 = conn
        .query_row("SELECT count(*) FROM courses WHERE status IN ('scoping', 'compiling')", [], |r| r.get(0))
        .unwrap_or(0);
    if building > 0 {
        return Some("A course is being built. Tools can be changed once it's ready.".into());
    }
    let running: i64 = conn.query_row("SELECT count(*) FROM jobs WHERE status='running'", [], |r| r.get(0)).unwrap_or(0);
    if running > 0 {
        return Some("Claude is working on something. Tools can be changed when it's done.".into());
    }
    if !busy().lock().unwrap().is_empty() {
        return Some("A tool is being installed.".into());
    }
    None
}

fn busy() -> &'static Mutex<HashSet<String>> {
    static B: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    B.get_or_init(Default::default)
}

// ---------- install and remove ----------

/// Where tool files come from.
pub trait Source {
    fn fetch(&self, release: &str, id: &str, file: &ToolFile) -> Result<Vec<u8>, String>;
}

/// The project's GitHub release, through the system's curl (as lesson sounds do).
pub struct Release;

impl Source for Release {
    fn fetch(&self, release: &str, id: &str, file: &ToolFile) -> Result<Vec<u8>, String> {
        let url = format!("https://github.com/munitkala/selfsemester/releases/download/{release}/{id}--{}", file.name);
        let tmp = std::env::temp_dir().join(format!("selfsemester-tool-{}-{id}.part", std::process::id()));
        let mut cmd = std::process::Command::new(crate::course::audio::system_tool(r"System32\curl.exe", "curl"));
        cmd.args(["-sS", "-L", "--fail", "--proto", "=https", "--proto-redir", "=https", "--max-time", "120", "--max-filesize"])
            .arg(MAX_FILE.to_string())
            .args(["-A", &format!("SelfSemester/{} (https://github.com/munitkala/selfsemester; tools)", env!("CARGO_PKG_VERSION")), "-o"])
            .arg(&tmp)
            .arg(&url);
        crate::course::audio::hidden(&mut cmd);
        let out = cmd.output().map_err(|e| format!("couldn't start curl: {e}"))?;
        let body = std::fs::read(&tmp);
        let _ = std::fs::remove_file(&tmp);
        if !out.status.success() {
            return Err(format!("download failed ({})", String::from_utf8_lossy(&out.stderr).trim()));
        }
        body.map_err(|e| e.to_string())
    }
}

/// A folder holding `<tool>--<file>` (what scripts/pack-tools.mjs writes); for testing and offline installs.
pub struct Folder(pub PathBuf);

impl Source for Folder {
    fn fetch(&self, _release: &str, id: &str, file: &ToolFile) -> Result<Vec<u8>, String> {
        std::fs::read(self.0.join(format!("{id}--{}", file.name))).map_err(|e| format!("{}: {e}", file.name))
    }
}

/// Files in a local folder when it has them, otherwise the GitHub release. The folder is
/// `SELFSEMESTER_TOOLS_FROM` if set, else `tool-packs` in the app data folder, so copying the packed files there
/// (from tools-dist or a release) lets a computer without internet install tools. Every file is still checked.
pub struct LocalFirst(pub PathBuf);

impl Source for LocalFirst {
    fn fetch(&self, release: &str, id: &str, file: &ToolFile) -> Result<Vec<u8>, String> {
        Folder(self.0.clone()).fetch(release, id, file).or_else(|_| Release.fetch(release, id, file))
    }
}

pub fn default_source(paths: &AppPaths) -> Box<dyn Source + Send> {
    let dir = std::env::var_os("SELFSEMESTER_TOOLS_FROM")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| paths.data_dir.join("tool-packs"));
    Box::new(LocalFirst(dir))
}

/// Marks a tool as being installed until dropped.
pub struct Busy(String);
impl Drop for Busy {
    fn drop(&mut self) {
        busy().lock().unwrap().remove(&self.0);
    }
}

/// Mark a tool as being installed (refuses if tools are locked). The returned guard clears the mark when dropped.
pub fn begin(conn: &Connection, id: &str) -> Result<Busy, String> {
    let t = find(id).ok_or_else(|| format!("unknown tool {id}"))?;
    if !t.installable() {
        return Err(format!("{} is part of the app", t.name));
    }
    if let Some(why) = locked(conn) {
        return Err(why);
    }
    busy().lock().unwrap().insert(id.to_string());
    Ok(Busy(id.to_string()))
}

/// Download, check and put a tool in place. Files go to a temporary folder first, so a failed download leaves
/// nothing behind and an installed tool is always complete.
pub fn install(paths: &AppPaths, id: &str, source: &dyn Source) -> Result<(), String> {
    let t = find(id).ok_or_else(|| format!("unknown tool {id}"))?;
    if !t.installable() {
        return Err(format!("{} is part of the app", t.name));
    }
    let dir = root(paths);
    let part = dir.join(format!(".{id}.partial"));
    let _ = std::fs::remove_dir_all(&part);
    std::fs::create_dir_all(&part).map_err(|e| e.to_string())?;
    let result = (|| {
        for f in &t.files {
            let data = source.fetch(&catalog().release, id, f)?;
            let hash = format!("{:x}", Sha256::digest(&data));
            if data.len() as u64 != f.bytes || hash != f.sha256 {
                return Err(format!("{} didn't match the copy this app expects, so it wasn't installed", f.name));
            }
            std::fs::write(part.join(&f.name), &data).map_err(|e| e.to_string())?;
        }
        std::fs::write(part.join(".installed"), &catalog().release).map_err(|e| e.to_string())?;
        let target = dir.join(id);
        let _ = std::fs::remove_dir_all(&target);
        std::fs::rename(&part, &target).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&part);
    }
    result
}

pub fn remove(paths: &AppPaths, conn: &Connection, id: &str) -> Result<(), String> {
    let t = find(id).ok_or_else(|| format!("unknown tool {id}"))?;
    if !t.installable() {
        return Err(format!("{} is part of the app and can't be removed", t.name));
    }
    if let Some(why) = locked(conn) {
        return Err(why);
    }
    let dir = root(paths).join(id);
    // Drop the marker first: a half-removed tool then counts as not installed.
    let _ = std::fs::remove_file(dir.join(".installed"));
    std::fs::remove_dir_all(&dir).or_else(|e| if dir.exists() { Err(e.to_string()) } else { Ok(()) })
}

// ---------- what the course prompts say ----------

/// The catalog for planning a course: every installable tool, so the plan can name the ones the course needs.
pub fn plan_catalog() -> String {
    catalog()
        .tools
        .iter()
        .filter(|t| t.installable())
        .map(|t| format!("- {}: {} (for {})", t.id, t.summary, t.for_courses))
        .collect::<Vec<_>>()
        .join("\n")
}

/// How lessons can use the installed tools, for the unit prompt.
pub fn lesson_guide(tools: &[&Tool]) -> String {
    if tools.is_empty() {
        return "No tools are installed, so interactive figures use plain JavaScript only.".into();
    }
    let list = tools.iter().map(|t| format!("- {}: {}", t.id, t.hint)).collect::<Vec<_>>().join("\n");
    format!(
        "TOOLS for interactive figures. These libraries are installed. To use one, add a line \"tools: <id>\" (several \
         separated by commas) after the caption; the app loads it before the figure's own script. Use only these, \
         and only where they help:\n{list}"
    )
}

#[cfg(test)]
pub(crate) fn test_paths(dir: &std::path::Path) -> AppPaths {
    AppPaths::rooted(dir, dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(Vec<u8>);
    impl Source for Fake {
        fn fetch(&self, _r: &str, _id: &str, f: &ToolFile) -> Result<Vec<u8>, String> {
            Ok(if f.name.ends_with(".js") { self.0.clone() } else { vec![] })
        }
    }

    #[test]
    fn catalog_is_complete() {
        let c = catalog();
        assert!(!c.release.is_empty());
        let mut ids = HashSet::new();
        for t in &c.tools {
            assert!(ids.insert(&t.id), "duplicate tool {}", t.id);
            assert!(["built-in", "common", "extra"].contains(&t.category.as_str()), "{}", t.id);
            assert!(!t.summary.is_empty() && !t.name.is_empty());
            if t.category != "built-in" {
                assert!(!t.files.is_empty() && !t.hint.is_empty() && !t.licence.is_empty(), "{}", t.id);
                for f in &t.files {
                    assert_eq!(f.sha256.len(), 64, "{}/{} has no hash: run scripts/pack-tools.mjs", t.id, f.name);
                    assert!(f.bytes > 0 && !f.name.contains(['/', '\\']) && !f.name.starts_with('.'));
                }
                for s in t.scripts.iter().chain(&t.styles) {
                    assert!(t.files.iter().any(|f| &f.name == s), "{} loads {s}, which it doesn't ship", t.id);
                }
                assert!(t.files.iter().any(|f| f.name.to_lowercase().contains("licen")), "{} ships no licence", t.id);
            }
        }
        assert!(c.tools.iter().any(|t| t.category == "common") && c.tools.iter().any(|t| t.category == "extra"));
    }

    #[test]
    fn bad_downloads_leave_nothing_installed() {
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(dir.path());
        assert!(install(&paths, "chess", &Fake(b"not the real file".to_vec())).unwrap_err().contains("didn't match"));
        assert!(!is_installed(&paths, "chess"));
        assert!(std::fs::read_dir(root(&paths)).unwrap().next().is_none(), "the partial folder is cleaned up");
        assert!(install(&paths, "maths", &Fake(vec![])).is_err(), "built-ins aren't installed");
        assert!(file(&paths, "chess", "chess.js").is_none());
    }

    #[test]
    fn installs_from_the_packed_files_then_removes() {
        let packed = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tools-dist");
        if !packed.join("chess--chess.js").is_file() {
            eprintln!("skipped: run scripts/pack-tools.mjs first");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(dir.path());
        let conn = crate::db::open_in_memory().unwrap();
        install(&paths, "chess", &Folder(packed)).unwrap();
        assert!(is_installed(&paths, "chess"));
        assert!(file(&paths, "chess", "chess.js").unwrap().is_file());
        assert!(file(&paths, "chess", "../.installed").is_none() && file(&paths, "chess", ".installed").is_none());
        let v = view(&paths, &conn);
        assert!(v.tools.iter().any(|t| t.tool.id == "chess" && t.installed), "an installed extra tool is listed");
        assert_eq!(lookup(&paths, &["chess".into(), "maths".into(), "nope".into()]).len(), 1, "only installable tools");
        remove(&paths, &conn, "chess").unwrap();
        let v = view(&paths, &conn);
        assert!(!v.tools.iter().any(|t| t.tool.id == "chess"), "a removed extra tool leaves the list");
        assert!(v.tools.iter().any(|t| t.tool.id == "geometry" && !t.installed), "common tools stay listed");
        assert!(lesson_guide(&installed(&paths)).contains("No tools"));
    }

    #[test]
    fn building_a_course_locks_tools() {
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(dir.path());
        let conn = crate::db::open_in_memory().unwrap();
        assert!(locked(&conn).is_none());
        conn.execute("INSERT INTO profiles(id, name, created_at) VALUES ('me', 'Me', 'x')", []).unwrap();
        conn.execute(
            "INSERT INTO courses(id, profile_id, title, subject, status, created_at, updated_at) VALUES ('c1', 'me', 'T', 'S', 'compiling', 'x', 'x')",
            [],
        )
        .unwrap();
        assert!(locked(&conn).is_some());
        assert!(remove(&paths, &conn, "geometry").unwrap_err().contains("being built"));
        assert!(begin(&conn, "geometry").is_err());
    }
}
