//! Interactive figures (```widget blocks in lessons, see src/lib/widgets.ts).
//!
//! A widget is a small HTML page a course writer made: sliders, a simulation, a board to click on. It runs in a
//! sandboxed frame served from its own address (the `widget` scheme), so the lesson page's rules don't apply to it
//! and its rules are strict:
//! - its own Content-Security-Policy: inline script and style only, no network of any kind, no forms, no plugins;
//! - the frame is sandboxed without same-origin, so it has no storage, cookies or access to the app window;
//! - the lesson page's CSP only lets frames load from this scheme, so a widget can't navigate itself elsewhere.
//!
//! The page sends the widget's HTML here once (`put`), gets an id back, and points the frame at
//! `widget://localhost/w/<id>`. Nothing here depends on which model wrote the lesson.

use std::collections::{hash_map::DefaultHasher, HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

/// Largest widget accepted, in bytes. Lessons are text; anything bigger is a mistake.
pub const MAX_BYTES: usize = 256 * 1024;
/// How many widgets are kept in memory; the oldest go first. A lesson rarely has more than a handful.
const KEEP: usize = 64;

/// The policy each widget page is served with.
/// Scripts and styles may also come from this scheme's own `/t/` path: installed tools (see tools.rs).
pub const CSP: &str = "default-src 'none'; script-src 'unsafe-inline' http://widget.localhost/t/ widget://localhost/t/; \
style-src 'unsafe-inline' http://widget.localhost/t/ widget://localhost/t/; img-src data: blob:; media-src data: blob:; \
font-src data:; form-action 'none'; base-uri 'none'";

/// Theme colours a widget may use, as CSS variables of the same name. Values come from the lesson page.
const THEME_VARS: &[&str] = &[
    "panel", "raised", "text", "muted", "line", "accent", "accent-soft", "on-accent", "good", "warn", "bad",
    "chart-1", "chart-2", "chart-3", "chart-4", "chart-5", "chart-6",
];

/// A widget's HTML and the ids of the tools it loads.
type Kept = (String, Vec<String>);

#[derive(Default)]
pub struct Store {
    inner: Mutex<(HashMap<String, Kept>, VecDeque<String>)>,
}

impl Store {
    /// Keep a widget's HTML and the tools it loads; returns its id (the same widget always gets the same id).
    pub fn put(&self, html: &str, tools: &[String]) -> Result<String, String> {
        if html.len() > MAX_BYTES {
            return Err(format!("this interactive figure is too large ({} KB, the limit is {} KB)", html.len() / 1024, MAX_BYTES / 1024));
        }
        let mut h = DefaultHasher::new();
        html.hash(&mut h);
        tools.hash(&mut h);
        let id = format!("{:016x}", h.finish());
        let mut g = self.inner.lock().unwrap();
        let (map, order) = &mut *g;
        if !map.contains_key(&id) {
            map.insert(id.clone(), (html.to_string(), tools.to_vec()));
            order.push_back(id.clone());
            while order.len() > KEEP {
                if let Some(old) = order.pop_front() {
                    map.remove(&old);
                }
            }
        }
        Ok(id)
    }

    pub fn get(&self, id: &str) -> Option<(String, Vec<String>)> {
        self.inner.lock().unwrap().0.get(id).cloned()
    }
}

/// A colour value safe to place inside a CSS declaration: colour functions and names only, nothing that could
/// close the rule or pull anything in.
fn safe_colour(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 80
        && v.chars().all(|c| c.is_ascii_alphanumeric() || " #(),.%/-".contains(c))
        && !v.to_ascii_lowercase().contains("url")
}

/// The full page for a widget: theme colours, a few base styles so plain controls look at home, the tools it uses,
/// the widget itself, and a small script that reports its height and follows theme changes.
pub fn page(body: &str, theme: &[(String, String)], tools: &[&crate::tools::Tool]) -> String {
    let mut loads = String::new();
    for t in tools {
        for f in &t.styles {
            loads.push_str(&format!(r#"<link rel="stylesheet" href="/t/{}/{}">"#, t.id, f));
        }
        for f in &t.scripts {
            loads.push_str(&format!(r#"<script src="/t/{}/{}"></script>"#, t.id, f));
        }
    }
    let vars: String = theme
        .iter()
        .filter(|(k, v)| THEME_VARS.contains(&k.as_str()) && safe_colour(v))
        .map(|(k, v)| format!("--{k}:{v};"))
        .collect();
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="{CSP}"><style>
:root{{--panel:#fff;--raised:#fbfaf7;--text:#1c1c1a;--muted:#5f5e58;--line:#e2e0d9;--accent:#5b4fc4;--accent-soft:#eeecfa;--on-accent:#fff;--good:#1e7a3c;--warn:#8a5700;--bad:#b3261e;--chart-1:#4f6fd0;--chart-2:#2f9c7a;--chart-3:#c9921f;--chart-4:#c9503f;--chart-5:#8a5cc2;--chart-6:#2f8fb0;{vars}}}
html,body{{margin:0;background:var(--panel);color:var(--text);font:14px/1.5 system-ui,"Segoe UI",sans-serif;overflow:hidden}}
body{{padding:4px;display:flow-root}}
*{{box-sizing:border-box}}
button,select,input{{font:inherit;color:inherit}}
button{{background:var(--raised);border:1px solid var(--line);border-radius:8px;padding:.3rem .8rem;cursor:pointer}}
button:hover{{border-color:var(--accent)}}
input,select{{accent-color:var(--accent)}}
input[type=number],input[type=text],select{{background:var(--panel);border:1px solid var(--line);border-radius:6px;padding:.2rem .4rem}}
label{{display:inline-flex;align-items:center;gap:.4rem}}
svg,canvas{{max-width:100%}}
svg{{color:var(--text)}}
.muted{{color:var(--muted)}}
</style>{loads}</head><body>
{body}
<script>(()=>{{const send=()=>parent.postMessage({{ssWidget:"height",h:Math.ceil(document.body.getBoundingClientRect().height)}},"*");new ResizeObserver(send).observe(document.body);addEventListener("load",send);addEventListener("message",e=>{{if(e.source!==parent||!e.data||e.data.ssWidget!=="theme")return;for(const[k,v]of Object.entries(e.data.vars||{{}}))if(/^[a-z0-9-]+$/.test(k)&&/^[a-z0-9 #(),.%\/-]+$/i.test(v)&&!/url/i.test(v))document.documentElement.style.setProperty("--"+k,v)}})}})();</script>
</body></html>"#
    )
}

/// Answer a request on the widget scheme: `/w/<id>?panel=...&text=...` for a widget page, `/t/<tool>/<file>` for a
/// file of an installed tool.
pub fn respond(store: &Store, paths: &crate::paths::AppPaths, uri: &str) -> tauri::http::Response<Vec<u8>> {
    let url = tauri::Url::parse(uri).or_else(|_| tauri::Url::parse(&format!("widget://localhost{uri}")));
    let found: Option<(&str, Vec<u8>)> = url.ok().and_then(|u| {
        if let Some(rest) = u.path().strip_prefix("/t/") {
            let (id, name) = rest.split_once('/')?;
            let path = crate::tools::file(paths, id, name)?;
            let kind = match path.extension().and_then(|e| e.to_str()) {
                Some("js") => "text/javascript; charset=utf-8",
                Some("css") => "text/css; charset=utf-8",
                _ => "text/plain; charset=utf-8",
            };
            return Some((kind, std::fs::read(path).ok()?));
        }
        let id = u.path().strip_prefix("/w/")?.to_string();
        let (html, wanted) = store.get(&id)?;
        let theme: Vec<(String, String)> = u.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect();
        let tools: Vec<&crate::tools::Tool> = crate::tools::installed(paths).into_iter().filter(|t| wanted.contains(&t.id)).collect();
        Some(("text/html; charset=utf-8", page(&html, &theme, &tools).into_bytes()))
    });
    let builder = tauri::http::Response::builder()
        .header("Content-Security-Policy", CSP)
        .header("X-Content-Type-Options", "nosniff")
        .header("Cache-Control", "no-store");
    match found {
        // Tool files are loaded by sandboxed (opaque-origin) pages, so they're shared with any origin. They're public
        // libraries, and nothing else is served this way.
        Some((kind, body)) => builder.status(200).header("Content-Type", kind).header("Access-Control-Allow-Origin", "*").body(body),
        None => builder.status(404).header("Content-Type", "text/plain").body(b"not found".to_vec()),
    }
    .expect("static response parts")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_widget_same_id_and_old_ones_go() {
        let s = Store::default();
        let a = s.put("<p>a</p>", &[]).unwrap();
        assert_eq!(a, s.put("<p>a</p>", &[]).unwrap());
        assert_ne!(a, s.put("<p>b</p>", &[]).unwrap());
        assert_ne!(a, s.put("<p>a</p>", &["maps".to_string()]).unwrap());
        for i in 0..KEEP {
            s.put(&format!("<p>{i}</p>"), &[]).unwrap();
        }
        assert!(s.get(&a).is_none(), "the oldest widget is dropped once the store is full");
        assert!(s.put(&"x".repeat(MAX_BYTES + 1), &[]).is_err());
    }

    #[test]
    fn serves_with_a_strict_policy_and_only_known_ids() {
        let s = Store::default();
        let dir = tempfile::tempdir().unwrap();
        let paths = crate::tools::test_paths(dir.path());
        let id = s.put("<b>hi</b>", &["chess".to_string()]).unwrap();
        let ok = respond(&s, &paths, &format!("http://widget.localhost/w/{id}?text=%23123456"));
        assert_eq!(ok.status(), 200);
        let csp = ok.headers()["Content-Security-Policy"].to_str().unwrap();
        assert!(csp.starts_with("default-src 'none'") && !csp.contains("connect-src"));
        let body = String::from_utf8(ok.body().clone()).unwrap();
        assert!(body.contains("<b>hi</b>") && body.contains("--text:#123456;"));
        assert!(!body.contains("/t/chess/"), "a tool that isn't installed isn't loaded");
        assert_eq!(respond(&s, &paths, "widget://localhost/w/0000000000000000").status(), 404);
        assert_eq!(respond(&s, &paths, &format!("widget://localhost/other/{id}")).status(), 404);
        assert_eq!(respond(&s, &paths, "widget://localhost/t/chess/chess.js").status(), 404, "not installed");
        assert_eq!(respond(&s, &paths, "widget://localhost/t/chess/..%2F..%2Fapp.db").status(), 404);
    }

    #[test]
    fn theme_values_cannot_break_out_of_css() {
        let theme = vec![
            ("text".to_string(), "red;}</style><script>alert(1)</script>".to_string()),
            ("accent".to_string(), "url(https://example.com/x)".to_string()),
            ("evil".to_string(), "#fff".to_string()),
            ("panel".to_string(), "oklch(0.5 0.1 262 / 50%)".to_string()),
        ];
        let p = page("", &theme, &[]);
        assert!(!p.contains("alert(1)") && !p.contains("example.com") && !p.contains("--evil"));
        assert!(p.contains("--panel:oklch(0.5 0.1 262 / 50%);"));
    }
}
