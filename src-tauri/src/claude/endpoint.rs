//! AI services other than Claude Code, set up by the learner in Settings.
//!
//! A service can be almost anything that writes text: an OpenAI-style API (OpenAI, OpenRouter, Groq, Mistral,
//! DeepSeek, LM Studio...), OpenAI's Responses API, Anthropic's or Google's own APIs, Ollama, a hand-built HTTP
//! request for anything else, or a program on this computer that reads a prompt and prints an answer. HTTP goes
//! through the system's curl, the same way lesson sounds and tools are downloaded, so the app needs no HTTP library.
//!
//! The model can't open files by itself here, so text files, pictures and (where the API takes them) PDFs travel
//! with the request. Web search only happens where the service does it itself. `fit.rs` turns all of this into a
//! plain report for the learner before anything is built.

use super::styles::{self, Answer, Ask, JsonMode, Style};
use super::{ClaudeRunner, ClaudeStatus, RunError, StageOutput, StageRequest, Tier, Usage};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Biggest text file put into a request, and the most for all of them together. Past this a local model's
/// context fills up and the answer gets worse, so the rest is left out and the model is told.
const MAX_TEXT_FILE: usize = 60_000;
const MAX_TEXT_TOTAL: usize = 200_000;
const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_PDF_BYTES: u64 = 20 * 1024 * 1024;

const TEXT_EXTS: &[&str] = &[
    "txt", "md", "markdown", "csv", "tsv", "json", "xml", "html", "htm", "yaml", "yml", "toml", "ini", "log", "tex",
    "py", "js", "ts", "rs", "c", "h", "cpp", "hpp", "java", "cs", "go", "rb", "php", "sql", "r", "m", "sh", "ipynb",
];
const IMAGE_EXTS: &[(&str, &str)] =
    &[("png", "image/png"), ("jpg", "image/jpeg"), ("jpeg", "image/jpeg"), ("gif", "image/gif"), ("webp", "image/webp")];

/// A 16 x 16 red square, for checking whether the model can see pictures.
const RED_SQUARE_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAIAAACQkWg2AAAAFklEQVR42mO4oKBAEmIY1TCqYfhqAABLfxAQegypMgAAAABJRU5ErkJggg==";

/// What kind of thing a service is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Api {
    #[default]
    OpenaiChat,
    OpenaiResponses,
    Anthropic,
    Gemini,
    Ollama,
    /// Any HTTP API: the learner writes the request body and says where the answer is.
    Custom,
    /// A program on this computer. The prompt goes in on stdin and the answer comes out on stdout.
    Command,
}

impl Api {
    pub fn style(self) -> Option<Style> {
        match self {
            Api::OpenaiChat => Some(Style::OpenaiChat),
            Api::OpenaiResponses => Some(Style::OpenaiResponses),
            Api::Anthropic => Some(Style::Anthropic),
            Api::Gemini => Some(Style::Gemini),
            Api::Ollama => Some(Style::Ollama),
            Api::Custom | Api::Command => None,
        }
    }

    /// The first way to ask for JSON. Services that don't take a schema start further down the list.
    fn first_format(self) -> Format {
        match self {
            Api::OpenaiChat | Api::OpenaiResponses | Api::Ollama => Format::Schema,
            Api::Gemini => Format::Json,
            Api::Anthropic | Api::Custom | Api::Command => Format::Prompt,
        }
    }
}

/// One service the learner set up.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Endpoint {
    /// Short id the job assignments point at.
    pub id: String,
    /// The learner's name for it, e.g. "OpenAI" or "Ollama on the desktop".
    pub name: String,
    pub api: Api,
    /// Which preset it started from ("openai", "ollama", ...), or "custom". Only used to show the right hints.
    pub preset: String,
    /// For HTTP services: the base address, e.g. `https://api.openai.com/v1`.
    pub base_url: String,
    /// Empty for services that don't need one.
    pub api_key: String,
    /// The usual model, for anything the job assignments don't name a model for.
    pub model: String,
    /// Older settings files had per-tier models; kept so they still load. Empty means `model`.
    pub strong_model: String,
    pub quick_model: String,
    /// The learner says the model takes pictures (photos of handwritten work, diagrams).
    pub sees_images: bool,
    /// The service searches the web itself (only where the API has a search tool: Responses, Anthropic, Gemini).
    pub web_search: bool,
    /// Send PDFs along (Anthropic and Gemini take them).
    pub reads_pdfs: bool,
    /// Where the key goes. Empty uses the API's usual header.
    pub auth_header: String,
    /// What goes before the key in that header, e.g. "Bearer ". Only used with `auth_header`.
    pub auth_prefix: String,
    /// Send the key as this query parameter instead of a header (e.g. "key").
    pub auth_query: String,
    /// Extra headers, one "Name: value" per line.
    pub headers: String,
    /// Extra query parameters, one "name=value" per line (e.g. Azure's api-version).
    pub query: String,
    /// JSON object merged into every request body, e.g. {"temperature": 0.3}.
    pub extra_body: String,
    /// Most tokens one answer may have. None leaves it to the service.
    pub max_output: Option<u64>,
    /// Context window to ask for, where the API takes one (Ollama).
    pub context: Option<u64>,
    /// How long one step may take before it counts as stuck. None means 20 minutes.
    pub timeout_minutes: Option<u64>,
    /// Custom HTTP: path after the base address (may be empty), the body with {{...}} placeholders, and where the
    /// answer's text is, as a dotted path like `choices.0.message.content`.
    pub custom_path: String,
    pub custom_body: String,
    pub custom_text_path: String,
    /// Command: the program and its arguments, e.g. `ollama run {{model}}`.
    pub command: String,
    /// What the last "Check" found. Cleared whenever the settings it depends on change.
    pub check: Option<Check>,
}

/// Result of a quick real test against the service.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Check {
    pub at: String,
    /// It answered at all.
    pub reached: bool,
    /// It gave back JSON in the shape asked for.
    pub json_ok: bool,
    /// How it was asked for JSON: "schema", "json" or "prompt" (see `Format`).
    pub format: String,
    /// None when the learner hasn't said the model sees images, so it wasn't tried.
    pub images_ok: Option<bool>,
    pub seconds: f64,
    pub message: String,
}

impl Endpoint {
    /// The model for a step when the job assignments don't name one.
    pub fn model_for(&self, tier: Tier) -> &str {
        let pick = match tier {
            Tier::Fable | Tier::Opus => &self.strong_model,
            Tier::Sonnet => &self.model,
            Tier::Haiku => &self.quick_model,
        };
        if pick.trim().is_empty() { self.model.trim() } else { pick.trim() }
    }

    pub fn url(&self, path: &str) -> String {
        let base = self.base_url.trim().trim_end_matches('/');
        let path = path.trim_start_matches('/');
        if path.is_empty() { base.to_string() } else { format!("{base}/{path}") }
    }

    pub fn display_name(&self) -> String {
        if !self.name.trim().is_empty() {
            return self.name.trim().to_string();
        }
        match self.api {
            Api::Command => "A program on this computer".into(),
            _ => host_of(&self.base_url),
        }
    }

    /// Something wrong with the settings that would stop every call, in words for the Settings page.
    pub fn problem(&self) -> Option<String> {
        if self.api == Api::Command {
            if self.command.trim().is_empty() {
                return Some("Add the command to run.".into());
            }
        } else {
            let url = self.base_url.trim();
            if url.is_empty() {
                return Some("Add the service's address.".into());
            }
            if !(url.starts_with("https://") || url.starts_with("http://")) {
                return Some("The address should start with https:// (or http:// for a server on your own network).".into());
            }
        }
        if self.model.trim().is_empty() && self.api != Api::Custom && self.api != Api::Command {
            return Some("Pick a model.".into());
        }
        if self.api == Api::Custom {
            if self.custom_body.trim().is_empty() {
                return Some("Write the request body.".into());
            }
            if self.custom_text_path.trim().is_empty() {
                return Some("Say where the answer's text is in the reply.".into());
            }
        }
        if bad_key(&self.api_key) {
            return Some("The API key has a line break in it, which can't be right.".into());
        }
        for line in lines(&self.headers) {
            if !line.contains(':') {
                return Some(format!("The header line \"{line}\" needs a colon, like \"Name: value\"."));
            }
        }
        for line in lines(&self.query) {
            if !line.contains('=') {
                return Some(format!("The query line \"{line}\" needs an equals sign, like \"name=value\"."));
            }
        }
        if !self.extra_body.trim().is_empty() && !serde_json::from_str::<Value>(&self.extra_body).is_ok_and(|v| v.is_object()) {
            return Some("The extra body fields should be a JSON object, like {\"temperature\": 0.3}.".into());
        }
        None
    }

    /// Runs on this computer or the local network, so nothing goes out to a company and nothing is billed.
    pub fn is_local(&self) -> bool {
        self.api == Api::Command || is_local_host(&host_of(&self.base_url))
    }

    /// PDFs only go to APIs that take them, and only when the learner left that on.
    pub fn sends_pdfs(&self) -> bool {
        self.reads_pdfs && self.api.style().is_some_and(|s| s.reads_pdfs())
    }

    /// The service can search the web for itself.
    pub fn searches_web(&self) -> bool {
        self.web_search && matches!(self.api, Api::OpenaiResponses | Api::Anthropic | Api::Gemini)
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(60 * self.timeout_minutes.filter(|m| *m > 0).unwrap_or(20))
    }
}

/// A key curl's config can't carry. Real keys never have line breaks.
pub fn bad_key(key: &str) -> bool {
    key.contains(['\n', '\r'])
}

/// Non-empty lines, without comments.
fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#'))
}

pub fn host_of(url: &str) -> String {
    let rest = url.trim().split("://").nth(1).unwrap_or("");
    let host_port = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host_port = host_port.rsplit('@').next().unwrap_or(host_port);
    if let Some(v6) = host_port.strip_prefix('[') {
        return v6.split(']').next().unwrap_or("").to_lowercase();
    }
    host_port.split(':').next().unwrap_or("").to_lowercase()
}

pub fn is_local_host(host: &str) -> bool {
    if host == "localhost" || host.ends_with(".local") || host == "::1" {
        return true;
    }
    let parts: Vec<u8> = host.split('.').filter_map(|p| p.parse().ok()).collect();
    if parts.len() != 4 {
        return false;
    }
    // loopback, private ranges, and 100.64/10 (Tailscale and other carrier-grade NAT addresses)
    matches!((parts[0], parts[1]), (127, _) | (10, _) | (192, 168) | (169, 254))
        || (parts[0] == 172 && (16..=31).contains(&parts[1]))
        || (parts[0] == 100 && (64..=127).contains(&parts[1]))
}

/// How the request asks for JSON. Services differ: the newest take a JSON schema, many only take "some JSON",
/// and some reject the field outright. Tried in this order, and the one that works is remembered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Schema,
    Json,
    Prompt,
}

impl Format {
    fn name(self) -> &'static str {
        match self {
            Format::Schema => "schema",
            Format::Json => "json",
            Format::Prompt => "prompt",
        }
    }
    fn next(self) -> Option<Format> {
        match self {
            Format::Schema => Some(Format::Json),
            Format::Json => Some(Format::Prompt),
            Format::Prompt => None,
        }
    }
    fn mode(self) -> JsonMode {
        match self {
            Format::Schema => JsonMode::Schema,
            Format::Json => JsonMode::Json,
            Format::Prompt => JsonMode::Prompt,
        }
    }
}

/// One HTTP answer, as curl got it.
struct Reply {
    status: u16,
    body: String,
    retry_after: Option<u64>,
}

/// What goes with the first message: text, plus pictures and PDFs where the service takes them.
#[derive(Debug, Default, PartialEq)]
struct Attached {
    text: String,
    images: Vec<(String, String)>,
    pdfs: Vec<String>,
}

/// The parts of one step that stay the same across its tries.
struct Job<'a> {
    ep: &'a Endpoint,
    model: &'a str,
    system: &'a str,
    schema: &'a Value,
    web: bool,
    limit: Duration,
}

/// Talks to the services. Holds nothing about any one service, so one runner serves all of them.
pub struct EndpointRunner {
    /// Request and answer files go here for the moment curl needs them.
    tmp_dir: PathBuf,
    stop: AtomicBool,
    /// What worked last time for each service and model, so later calls don't repeat the failed tries.
    formats: Mutex<Vec<(String, Format)>>,
}

impl EndpointRunner {
    pub fn new(tmp_dir: PathBuf) -> Self {
        Self { tmp_dir, stop: AtomicBool::new(false), formats: Mutex::new(vec![]) }
    }

    /// A runner tied to one service, for tests and anything else that wants a plain `ClaudeRunner`.
    pub fn single(endpoint: Endpoint, tmp_dir: PathBuf) -> Single {
        Single { runner: EndpointRunner::new(tmp_dir), endpoint }
    }

    pub fn forget_formats(&self) {
        self.formats.lock().unwrap().clear();
    }

    fn format_for(&self, ep: &Endpoint, model: &str) -> Format {
        let key = format!("{}/{model}", ep.id);
        let known = self.formats.lock().unwrap().iter().find(|(m, _)| *m == key).map(|(_, f)| *f);
        known.unwrap_or(ep.api.first_format())
    }

    fn remember_format(&self, ep: &Endpoint, model: &str, f: Format) {
        let key = format!("{}/{model}", ep.id);
        let mut all = self.formats.lock().unwrap();
        all.retain(|(m, _)| *m != key);
        all.push((key, f));
    }

    /// Wait for a child process, stopping it if the step is stopped or runs too long.
    fn wait(&self, child: &mut std::process::Child, limit: Duration) -> Result<(), RunError> {
        let start = Instant::now();
        loop {
            if child.try_wait()?.is_some() {
                return Ok(());
            }
            if self.stop.swap(false, Ordering::SeqCst) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RunError::Stopped);
            }
            if start.elapsed() > limit {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RunError::Timeout(limit.as_secs()));
            }
            std::thread::sleep(Duration::from_millis(150));
        }
    }

    /// Send one request through curl. The address, the key and every header go to curl on stdin as its config,
    /// never on the command line, where other programs on the computer could read them.
    fn send(&self, ep: &Endpoint, method: &str, path: &str, body: Option<&Value>, limit: Duration) -> Result<Reply, RunError> {
        std::fs::create_dir_all(&self.tmp_dir)?;
        let stamp = format!("{}-{}", std::process::id(), chrono::Utc::now().timestamp_micros());
        let body_file = self.tmp_dir.join(format!("ask-{stamp}.json"));
        let out_file = self.tmp_dir.join(format!("answer-{stamp}.json"));
        let head_file = self.tmp_dir.join(format!("headers-{stamp}.txt"));
        let _cleanup = Cleanup(vec![body_file.clone(), out_file.clone(), head_file.clone()]);

        let mut cmd = crate::course::audio::system_command(crate::course::audio::system_tool(r"System32\curl.exe", "curl"));
        cmd.args(["-sS", "--proto", "=http,https", "--max-time"])
            .arg(limit.as_secs().max(5).to_string())
            .args(["-X", method, "-A", &format!("SelfSemester/{}", env!("CARGO_PKG_VERSION")), "-K", "-", "-o"])
            .arg(&out_file)
            .arg("-D")
            .arg(&head_file)
            .args(["-w", "%{http_code}"]);
        if let Some(b) = body {
            std::fs::write(&body_file, serde_json::to_vec(b).map_err(|e| RunError::BadOutput(e.to_string()))?)?;
            cmd.arg("--data-binary").arg(format!("@{}", body_file.display()));
        }
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        crate::course::audio::hidden(&mut cmd);

        let mut child = cmd.spawn().map_err(|e| RunError::Io(std::io::Error::other(format!("couldn't start curl: {e}"))))?;
        {
            let mut stdin = child.stdin.take().expect("stdin piped");
            stdin.write_all(curl_config(ep, path, body.is_some()).as_bytes())?;
        }
        let out_t = read_all(child.stdout.take().expect("stdout piped"));
        let err_t = read_all(child.stderr.take().expect("stderr piped"));
        self.wait(&mut child, limit + Duration::from_secs(10))?;
        let code = out_t.join().unwrap_or_default();
        let stderr = err_t.join().unwrap_or_default();
        let status: u16 = code.trim().parse().unwrap_or(0);
        if status == 0 {
            let why = stderr.trim();
            if why.contains("timed out") || why.contains("(28)") {
                return Err(RunError::Timeout(limit.as_secs()));
            }
            return Err(RunError::Io(std::io::Error::other(format!(
                "couldn't reach {}: {}",
                host_of(&ep.base_url),
                if why.is_empty() { "no answer" } else { why }
            ))));
        }
        let headers = std::fs::read_to_string(&head_file).unwrap_or_default();
        Ok(Reply { status, body: std::fs::read_to_string(&out_file).unwrap_or_default(), retry_after: retry_after(&headers) })
    }

    /// Run the learner's program with `input` on stdin and return what it prints.
    fn run_command(&self, ep: &Endpoint, model: &str, input: &str, limit: Duration) -> Result<String, RunError> {
        let mut words = split_command(&ep.command.replace("{{model}}", model));
        if words.is_empty() {
            return Err(RunError::Endpoint("there's no command to run".into()));
        }
        let program = words.remove(0);
        let mut cmd = crate::course::audio::system_command(&program);
        cmd.args(&words)
            .env("SELFSEMESTER_MODEL", model)
            .env("SELFSEMESTER_API_KEY", ep.api_key.trim())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(home) = dirs::home_dir() {
            cmd.current_dir(home);
        }
        crate::course::audio::hidden(&mut cmd);
        let mut child = cmd.spawn().map_err(|e| RunError::Endpoint(format!("couldn't start {program}: {e}")))?;
        let input = input.to_string();
        let mut stdin = child.stdin.take().expect("stdin piped");
        // Write on its own thread: a program that answers before it has read everything mustn't block us.
        let writer = std::thread::spawn(move || {
            let _ = stdin.write_all(input.as_bytes());
        });
        let out_t = read_all(child.stdout.take().expect("stdout piped"));
        let err_t = read_all(child.stderr.take().expect("stderr piped"));
        self.wait(&mut child, limit)?;
        let _ = writer.join();
        let ok = child.wait().map(|s| s.success()).unwrap_or(false);
        let out = out_t.join().unwrap_or_default();
        let err = err_t.join().unwrap_or_default();
        if !ok && out.trim().is_empty() {
            return Err(RunError::Endpoint(format!("{program} stopped with an error: {}", snip(if err.trim().is_empty() { "no message" } else { &err }))));
        }
        Ok(out)
    }

    /// One request and its answer, in whatever form the service takes.
    fn call(&self, job: &Job, turns: &[(String, String)], first: &Attached, format: Format) -> Result<Answer, ChatError> {
        let Job { ep, model, system, schema, web, limit } = *job;
        let mut answer = match ep.api {
            Api::Command => {
                let text = self.run_command(ep, model, &transcript(system, turns), limit).map_err(ChatError::Run)?;
                Answer { text, ..Default::default() }
            }
            Api::Custom => {
                let body = custom_body(ep, model, system, turns, schema).map_err(|e| ChatError::Run(RunError::Endpoint(e)))?;
                let reply = self.send(ep, "POST", &ep.custom_path, Some(&body), limit).map_err(ChatError::Run)?;
                let v = reply_json(&reply)?;
                let text = at_path(&v, &ep.custom_text_path).map(text_of).unwrap_or_default();
                if text.is_empty() {
                    let why = v.get("error").map(error_text).unwrap_or_else(|| format!("nothing at {} in: {}", ep.custom_text_path, snip(&reply.body)));
                    return Err(ChatError::Run(RunError::Endpoint(why)));
                }
                Answer { text, ..Default::default() }
            }
            api => {
                let style = api.style().expect("HTTP style");
                let ask = Ask {
                    model,
                    system,
                    turns,
                    images: &first.images,
                    pdfs: &first.pdfs,
                    schema,
                    json_mode: format.mode(),
                    web_search: web,
                    max_output: ep.max_output,
                    context: ep.context,
                };
                let mut body = styles::body(style, &ask);
                merge(&mut body, &ep.extra_body);
                let reply = self.send(ep, "POST", &style.path(model), Some(&body), limit).map_err(ChatError::Run)?;
                let v = reply_json(&reply)?;
                styles::parse(style, &v).map_err(|e| ChatError::Run(RunError::Endpoint(e)))?
            }
        };
        if answer.model.as_deref().is_none_or(str::is_empty) {
            answer.model = Some(model.to_string());
        }
        Ok(answer)
    }

    /// Ask, falling back to a simpler way of asking for JSON when the service turns one down, and asking once
    /// more when the answer isn't the JSON that was wanted.
    fn ask(&self, job: &Job, first: Attached) -> Result<(StageOutput, Format), RunError> {
        let Job { ep, model, schema, .. } = *job;
        let mut format = self.format_for(ep, model);
        let mut turns = vec![("user".to_string(), first.text.clone())];
        let mut total = Usage::default();
        let mut nudged = false;
        loop {
            let started = Instant::now();
            match self.call(job, &turns, &first, format) {
                Ok(a) => {
                    total.input_tokens += a.input_tokens;
                    total.output_tokens += a.output_tokens;
                    total.cache_tokens += a.cache_tokens;
                    total.web_searches += a.web_searches;
                    total.duration_ms += started.elapsed().as_millis() as u64;
                    let used = a.model.clone().unwrap_or_else(|| model.to_string());
                    match find_json_for(&a.text, schema) {
                        Some(value) => {
                            self.remember_format(ep, model, format);
                            return Ok((StageOutput { value, usage: total, model: used }, format));
                        }
                        // Asking again would only get cut off again.
                        None if a.cut => {
                            return Err(RunError::BadOutput(format!(
                                "{model}'s answer was cut off before it finished. Either its output limit (or a local server's \
                                 context size) is too small for this step, or the model got stuck repeating itself"
                            )));
                        }
                        None if !nudged => {
                            nudged = true;
                            let what = match find_json(&a.text) {
                                Some(v) => format!(
                                    "Your answer left out these required fields: {}. Send the whole JSON object again with every required field filled in.",
                                    missing_fields(schema, &v).join(", ")
                                ),
                                None => "Your answer wasn't valid JSON. Send it again as one JSON object.".to_string(),
                            };
                            turns.push(("assistant".into(), a.text));
                            turns.push((
                                "user".into(),
                                format!(
                                    "{what} Nothing else, no Markdown fences. It must match this JSON Schema:\n{}",
                                    serde_json::to_string(schema).unwrap_or_default()
                                ),
                            ));
                        }
                        None => {
                            return Err(RunError::BadOutput(format!(
                                "{model} didn't answer in the shape this step needs, even when asked twice. A larger model usually fixes this. It said: {}",
                                snip(&a.text)
                            )));
                        }
                    }
                }
                Err(ChatError::FormatRefused(why)) => match format.next() {
                    Some(f) => format = f,
                    None => return Err(RunError::Endpoint(why)),
                },
                Err(ChatError::Run(e)) => return Err(e),
            }
        }
    }

    /// The models a service lists, for the picker in Settings. Not every service has this list.
    pub fn models(&self, ep: &Endpoint) -> Result<Vec<String>, String> {
        let Some(style) = ep.api.style() else {
            return Err("this kind of service has no model list; type the model name instead".into());
        };
        let Some(path) = style.models_path() else {
            return Err("this API has no model list; type the model name instead".into());
        };
        let reply = self.send(ep, "GET", path, None, Duration::from_secs(30)).map_err(|e| e.to_string())?;
        if reply.status != 200 {
            let v = serde_json::from_str::<Value>(&reply.body).ok();
            return Err(match http_error(reply.status, &reply.body, v.as_ref(), None) {
                ChatError::Run(e) => e.to_string(),
                ChatError::FormatRefused(m) => m,
            });
        }
        let v: Value = serde_json::from_str(&reply.body).map_err(|_| "the service's model list isn't JSON".to_string())?;
        Ok(styles::parse_models(style, &v))
    }

    /// Try a service for real: one small structured answer, and a picture if the model is meant to see them.
    pub fn check(&self, ep: &Endpoint, model: &str) -> Check {
        let mut c = Check { at: crate::db::now_utc(), ..Default::default() };
        if let Some(p) = ep.problem() {
            c.message = p;
            return c;
        }
        let model = if model.trim().is_empty() { ep.model.trim().to_string() } else { model.trim().to_string() };
        let schema = json!({
            "type": "object",
            "properties": {
                "ok": { "type": "boolean" },
                "numbers": { "type": "array", "items": {
                    "type": "object",
                    "properties": { "n": { "type": "integer" }, "word": { "type": "string" } },
                    "required": ["n", "word"]
                } }
            },
            "required": ["ok", "numbers"]
        });
        let started = Instant::now();
        let system = system_text(Some("You check that SelfSemester can reach you."), &schema);
        let first = Attached { text: "Set ok to true and list the numbers 1, 2 and 3 with their English words.".into(), ..Default::default() };
        let name = if model.is_empty() { ep.display_name() } else { model.clone() };
        let job = Job { ep, model: &model, system: &system, schema: &schema, web: false, limit: Duration::from_secs(180) };
        match self.ask(&job, first) {
            Ok((out, format)) => {
                c.reached = true;
                c.format = format.name().into();
                let nums: Vec<i64> = out.value["numbers"].as_array().map(|a| a.iter().filter_map(|x| x["n"].as_i64()).collect()).unwrap_or_default();
                c.json_ok = out.value["ok"] == true && nums == [1, 2, 3];
                c.message = if c.json_ok {
                    format!("{name} answered correctly.")
                } else {
                    format!("{name} answered, but got a simple question wrong. Expect mistakes in lessons and marks.")
                };
            }
            Err(e) => {
                c.reached = !matches!(e, RunError::Io(_) | RunError::Timeout(_));
                c.message = e.to_string();
            }
        }
        c.seconds = (started.elapsed().as_secs_f64() * 10.0).round() / 10.0;
        if ep.sees_images && c.reached && ep.api.style().is_some() {
            let schema = json!({ "type": "object", "properties": { "colour": { "type": "string" } }, "required": ["colour"] });
            let first = Attached {
                text: "What colour is this square? One word.".into(),
                images: vec![("image/png".into(), RED_SQUARE_PNG.into())],
                pdfs: vec![],
            };
            let system = system_text(None, &schema);
            let job = Job { ep, model: &model, system: &system, schema: &schema, web: false, limit: Duration::from_secs(120) };
            let saw = self.ask(&job, first);
            c.images_ok = Some(matches!(&saw, Ok((o, _)) if o.value["colour"].as_str().is_some_and(|s| s.to_lowercase().contains("red"))));
            if c.images_ok == Some(false) {
                c.message += " It couldn't see a test picture, so photos of your work won't be marked.";
            }
        }
        c
    }

    /// Run one step on a service with a given model.
    pub fn run_on(&self, ep: &Endpoint, model: &str, req: &StageRequest) -> Result<StageOutput, RunError> {
        if let Some(p) = ep.problem() {
            return Err(RunError::Endpoint(format!("{} isn't set up: {p}", ep.display_name())));
        }
        let wants_web = req.tools.iter().any(|t| t == "WebSearch" || t == "WebFetch");
        let web = wants_web && ep.searches_web();
        let mut prompt = req.prompt.clone();
        if wants_web && !web {
            prompt += NO_WEB_NOTE;
        }
        let first = with_files(prompt, &req.add_dirs, ep.sees_images && ep.api.style().is_some(), ep.sends_pdfs());
        let system = system_text(req.system.as_deref(), &req.schema);
        let limit = super::cli::time_limit(ep.timeout(), req);
        let job = Job { ep, model, system: &system, schema: &req.schema, web, limit };
        self.ask(&job, first).map(|(out, _)| out)
    }

    pub fn stop_current(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Status for one service, in the same shape as Claude Code's.
pub fn status_of(ep: &Endpoint) -> ClaudeStatus {
    let problem = ep.problem();
    ClaudeStatus {
        provider: "endpoint".into(),
        installed: problem.is_none(),
        logged_in: problem.is_none(),
        path: Some(if ep.api == Api::Command { ep.command.clone() } else { ep.base_url.clone() }),
        version: Some(ep.model.clone()),
        auth_method: Some(if ep.is_local() { "local".into() } else { "api key".into() }),
        on_subscription: false,
        problem: problem.map(|p| format!("{} isn't set up yet: {p} See Settings.", ep.display_name())),
    }
}

/// One service as a plain runner.
pub struct Single {
    pub runner: EndpointRunner,
    pub endpoint: Endpoint,
}

impl ClaudeRunner for Single {
    fn status(&self) -> ClaudeStatus {
        status_of(&self.endpoint)
    }
    fn run(&self, req: &StageRequest) -> Result<StageOutput, RunError> {
        self.runner.run_on(&self.endpoint, self.endpoint.model_for(req.tier), req)
    }
    fn stop_current(&self) {
        self.runner.stop_current();
    }
}

const NO_WEB_NOTE: &str = "\n\nNOTE: you can't search the web or open links in this setup. Wherever the task asks you to \
    search or to check a page, use what you already know instead. Only give sources you're sure exist, at addresses \
    that don't change (for example OpenStax, MIT OpenCourseWare, Khan Academy, a Wikipedia article or official \
    documentation), and say in the note that the link wasn't checked.\n";

/// The stage's own rules, plus the schema written out, since some services ignore the schema field.
fn system_text(system: Option<&str>, schema: &Value) -> String {
    let rules = system.unwrap_or("").trim();
    let schema = serde_json::to_string(schema).unwrap_or_default();
    format!(
        "{rules}{}Answer with one JSON object that matches this JSON Schema, and nothing else: no Markdown fences, \
         no comments, no text before or after it.\n{schema}",
        if rules.is_empty() { "" } else { "\n\n" }
    )
}

/// The whole conversation as one block of text, for a program that only takes a prompt.
fn transcript(system: &str, turns: &[(String, String)]) -> String {
    let mut s = format!("{system}\n\n");
    if turns.len() == 1 {
        s += &turns[0].1;
        return s;
    }
    for (role, text) in turns {
        s += &format!("{}:\n{text}\n\n", if role == "assistant" { "YOUR EARLIER ANSWER" } else { "REQUEST" });
    }
    s
}

/// curl's config for one request: the address with any query parameters, and every header.
fn curl_config(ep: &Endpoint, path: &str, json_body: bool) -> String {
    let mut url = ep.url(path);
    let mut query: Vec<(String, String)> =
        lines(&ep.query).filter_map(|l| l.split_once('=')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect();
    let key = ep.api_key.trim();
    let mut headers: Vec<String> = vec![];
    if json_body {
        headers.push("Content-Type: application/json".into());
    }
    if let Some(style) = ep.api.style() {
        headers.extend(style.fixed_headers().iter().map(|(k, v)| format!("{k}: {v}")));
    }
    if !key.is_empty() {
        if !ep.auth_query.trim().is_empty() {
            query.push((ep.auth_query.trim().to_string(), key.to_string()));
        } else if !ep.auth_header.trim().is_empty() {
            headers.push(format!("{}: {}{key}", ep.auth_header.trim(), ep.auth_prefix));
        } else {
            let (name, prefix) = ep.api.style().map(|s| s.key_header()).unwrap_or(("Authorization", "Bearer "));
            headers.push(format!("{name}: {prefix}{key}"));
        }
    }
    headers.extend(lines(&ep.headers).map(str::to_string));
    if !query.is_empty() {
        let q = query.iter().map(|(k, v)| format!("{}={}", encode(k), encode(v))).collect::<Vec<_>>().join("&");
        url += if url.contains('?') { "&" } else { "?" };
        url += &q;
    }
    let mut cfg = format!("url = \"{}\"\n", quote(&url));
    for h in headers {
        cfg += &format!("header = \"{}\"\n", quote(&h));
    }
    cfg
}

/// Escape a value for a double-quoted string in curl's config.
fn quote(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Percent-encode a query value.
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Split a command line into words, keeping "quoted parts" together.
fn split_command(line: &str) -> Vec<String> {
    let mut words = vec![];
    let mut cur = String::new();
    let mut quoted = false;
    let mut started = false;
    for ch in line.trim().chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    words.push(std::mem::take(&mut cur));
                    started = false;
                }
            }
            c => {
                cur.push(c);
                started = true;
            }
        }
    }
    if started {
        words.push(cur);
    }
    words
}

/// The learner's request body with its placeholders filled in. Each placeholder becomes a JSON value, so the
/// template stays valid JSON: write `"model": {{model}}`, not `"model": "{{model}}"`.
fn custom_body(ep: &Endpoint, model: &str, system: &str, turns: &[(String, String)], schema: &Value) -> Result<Value, String> {
    let prompt = if turns.len() == 1 { turns[0].1.clone() } else { transcript("", turns).trim().to_string() };
    let mut messages = vec![json!({ "role": "system", "content": system })];
    messages.extend(turns.iter().map(|(r, t)| json!({ "role": r, "content": t })));
    let fill = [
        ("{{model}}", json!(model)),
        ("{{system}}", json!(system)),
        ("{{prompt}}", json!(prompt)),
        ("{{full_prompt}}", json!(transcript(system, turns))),
        ("{{messages}}", Value::Array(messages)),
        ("{{schema}}", schema.clone()),
        ("{{max_output}}", json!(ep.max_output.unwrap_or(16000))),
    ];
    let mut text = ep.custom_body.clone();
    for (k, v) in fill {
        text = text.replace(k, &v.to_string());
    }
    let mut body: Value = serde_json::from_str(&text).map_err(|e| format!("the request body isn't valid JSON once filled in ({e})"))?;
    merge(&mut body, &ep.extra_body);
    Ok(body)
}

/// Follow a dotted path like `choices.0.message.content` into a value.
fn at_path<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    path.trim().split('.').filter(|p| !p.is_empty()).try_fold(v, |v, part| match part.parse::<usize>() {
        Ok(i) if v.is_array() => v.get(i),
        _ => v.get(part),
    })
}

fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts.iter().map(|p| p.as_str().or_else(|| p["text"].as_str()).unwrap_or("")).collect(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Fields from the learner's "extra body" go over the top of the request, objects merged key by key.
fn merge(body: &mut Value, extra: &str) {
    if let Ok(Value::Object(extra)) = serde_json::from_str::<Value>(extra) {
        merge_values(body, Value::Object(extra));
    }
}

fn merge_values(into: &mut Value, from: Value) {
    match (into, from) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in b {
                merge_values(a.entry(k).or_insert(Value::Null), v);
            }
        }
        (slot, v) => *slot = v,
    }
}

fn reply_json(reply: &Reply) -> Result<Value, ChatError> {
    let parsed: Option<Value> = serde_json::from_str(&reply.body).ok();
    if !(200..300).contains(&reply.status) {
        return Err(http_error(reply.status, &reply.body, parsed.as_ref(), reply.retry_after));
    }
    parsed.ok_or_else(|| ChatError::Run(RunError::BadOutput(format!("the service sent back something that isn't JSON: {}", snip(&reply.body)))))
}

fn read_all<R: Read + Send + 'static>(mut r: R) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let mut s = String::new();
        let _ = r.read_to_string(&mut s);
        s
    })
}

/// Put the learner's files into the request. Claude Code opens them with its Read tool; here they have to travel
/// with the prompt. Only files the prompt names are sent, so a folder of uploads doesn't all go every time.
fn with_files(prompt: String, dirs: &[String], images: bool, pdfs: bool) -> Attached {
    let mut files: Vec<PathBuf> = vec![];
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(d) else { continue };
        for f in rd.flatten().map(|e| e.path()).filter(|p| p.is_file()) {
            if prompt.contains(&f.display().to_string()) && !files.contains(&f) {
                files.push(f);
            }
        }
    }
    if files.is_empty() {
        return Attached { text: prompt, ..Default::default() };
    }
    files.sort();
    let mut out = Attached::default();
    let mut text = String::new();
    let mut skipped: Vec<String> = vec![];
    let mut room = MAX_TEXT_TOTAL;
    for f in &files {
        let name = f.display().to_string();
        let ext = f.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        let size = std::fs::metadata(f).map(|m| m.len()).unwrap_or(u64::MAX);
        if TEXT_EXTS.contains(&ext.as_str()) {
            let Ok(body) = std::fs::read_to_string(f) else {
                skipped.push(format!("{name} (couldn't be read as text)"));
                continue;
            };
            if room < 200 {
                skipped.push(format!("{name} (left out: the request was already long)"));
                continue;
            }
            let cap = MAX_TEXT_FILE.min(room);
            let (part, cut) = if body.len() > cap { (cut_at(&body, cap), true) } else { (body.as_str(), false) };
            room -= part.len();
            text += &format!("\n----- FILE {name} -----\n{part}\n{}----- END OF {name} -----\n", if cut { "[... the rest was left out ...]\n" } else { "" });
        } else if let Some((_, mime)) = IMAGE_EXTS.iter().find(|(e, _)| *e == ext) {
            if !images {
                skipped.push(format!("{name} (a picture, and this model isn't set up to see pictures)"));
            } else if let Some(bytes) = (size <= MAX_IMAGE_BYTES).then(|| std::fs::read(f).ok()).flatten() {
                text += &format!("\n(The picture {name} is attached.)\n");
                out.images.push((mime.to_string(), base64(&bytes)));
            } else {
                skipped.push(format!("{name} (a picture too large to send)"));
            }
        } else if ext == "pdf" && pdfs {
            match (size <= MAX_PDF_BYTES).then(|| std::fs::read(f).ok()).flatten() {
                Some(bytes) => {
                    text += &format!("\n(The PDF {name} is attached.)\n");
                    out.pdfs.push(base64(&bytes));
                }
                None => skipped.push(format!("{name} (a PDF too large to send)")),
            }
        } else {
            skipped.push(format!("{name} (a .{ext} file, which can't be opened in this setup)"));
        }
    }
    let mut full = prompt;
    full += "\n\nFILES: there is no Read tool here. The files named above are included below instead.";
    if !skipped.is_empty() {
        full += &format!(
            " These couldn't be included, so work without them and say so where it matters:\n{}",
            skipped.iter().map(|s| format!("- {s}")).collect::<Vec<_>>().join("\n")
        );
    }
    full += "\n";
    full += &text;
    out.text = full;
    out
}

fn cut_at(s: &str, max: usize) -> &str {
    let mut end = max.min(s.len());
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Plain base64, for pictures and PDFs. Short enough not to need a crate for it.
fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(ABC[(n >> 18) as usize & 63] as char);
        out.push(ABC[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { ABC[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { ABC[n as usize & 63] as char } else { '=' });
    }
    out
}

enum ChatError {
    /// The service didn't accept the way JSON was asked for; try a simpler way.
    FormatRefused(String),
    Run(RunError),
}

fn http_error(status: u16, body: &str, v: Option<&Value>, retry_after: Option<u64>) -> ChatError {
    let msg = v.and_then(|v| v.get("error")).map(error_text).unwrap_or_else(|| snip(body));
    let lower = msg.to_lowercase();
    match status {
        400 | 422
            if lower.contains("response_format")
                || lower.contains("json_schema")
                || lower.contains("json_object")
                || lower.contains("structured")
                || lower.contains("responsemimetype")
                || lower.contains("format") =>
        {
            ChatError::FormatRefused(msg)
        }
        401 | 403 => ChatError::Run(RunError::Endpoint(format!("the service turned down the API key (HTTP {status}): {msg}. Check the key in Settings"))),
        404 => ChatError::Run(RunError::Endpoint(format!("the service doesn't know this address or model (HTTP 404): {msg}. Check both in Settings"))),
        402 => ChatError::Run(RunError::UsageLimited {
            retry_at: None,
            message: format!("the service says the account is out of credit (HTTP 402): {msg}"),
        }),
        429 => ChatError::Run(RunError::UsageLimited {
            retry_at: retry_after.map(|s| chrono::Utc::now() + chrono::Duration::seconds(s as i64)),
            message: format!("the service's rate limit was hit: {msg}"),
        }),
        500..=599 => ChatError::Run(RunError::Io(std::io::Error::other(format!("the service had a problem (HTTP {status}): {msg}")))),
        _ => ChatError::Run(RunError::Endpoint(format!("HTTP {status}: {msg}"))),
    }
}

fn error_text(e: &Value) -> String {
    let m = e.get("message").and_then(Value::as_str).or_else(|| e.as_str()).map(str::to_string).unwrap_or_else(|| e.to_string());
    snip(&m)
}

/// `Retry-After` in seconds, from the last set of headers (curl writes one set per redirect).
fn retry_after(headers: &str) -> Option<u64> {
    headers
        .lines()
        .rev()
        .find_map(|l| l.split_once(':').filter(|(k, _)| k.trim().eq_ignore_ascii_case("retry-after")).map(|(_, v)| v.trim().to_string()))
        .and_then(|v| v.parse::<f64>().ok())
        .map(|s| s.ceil() as u64)
}

/// The answer object in a reply: the first one that has every field the schema requires. Without that check, a
/// reply that broke off halfway could hand back one of the small objects nested inside it.
pub fn find_json_for(text: &str, schema: &Value) -> Option<Value> {
    json_objects(text).into_iter().find(|v| missing_fields(schema, v).is_empty())
}

/// Dig the JSON object out of a reply. Models wrap it in Markdown fences, put thinking before it in
/// `<think>` tags, or add a sentence after it, even when asked not to.
pub fn find_json(text: &str) -> Option<Value> {
    json_objects(text).into_iter().next()
}

/// Every JSON object in the text that parses, outermost first.
fn json_objects(text: &str) -> Vec<Value> {
    let mut t = text.trim();
    if let Some(end) = t.rfind("</think>") {
        t = t[end + "</think>".len()..].trim();
    }
    if let Ok(v) = serde_json::from_str::<Value>(t)
        && v.is_object()
    {
        return vec![v];
    }
    // Try every `{` from the left with serde's streaming reader, which stops at the end of the first whole value.
    let mut found = vec![];
    for (i, _) in t.match_indices('{') {
        let mut it = serde_json::Deserializer::from_str(&t[i..]).into_iter::<Value>();
        if let Some(Ok(v)) = it.next()
            && v.is_object()
        {
            found.push(v);
        }
    }
    found
}

/// Top-level fields the schema requires that the answer doesn't have. The next step checks the rest.
fn missing_fields(schema: &Value, v: &Value) -> Vec<String> {
    schema["required"]
        .as_array()
        .map(|r| r.iter().filter_map(Value::as_str).filter(|k| v.get(*k).is_none()).map(str::to_string).collect())
        .unwrap_or_default()
}

fn snip(s: &str) -> String {
    let s = s.trim();
    if s.chars().count() > 300 { format!("{}…", s.chars().take(300).collect::<String>()) } else { s.to_string() }
}

struct Cleanup(Vec<PathBuf>);
impl Drop for Cleanup {
    fn drop(&mut self) {
        for p in &self.0 {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// Who does which job. Each one is Claude Code or one of the learner's services.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Roles {
    /// Planning the syllabus, exams and final-exam marking.
    pub plan: Target,
    /// Lessons, assignments and most marking.
    pub write: Target,
    /// Quick checks and summaries.
    pub check: Target,
    /// Steps that search the web: the reading list, and research while writing on Thorough and Max builds.
    pub research: Target,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Target {
    /// "" or "claude_code" for Claude Code, otherwise a service's id.
    pub service: String,
    /// The model to use. Empty uses the service's usual model.
    pub model: String,
}

impl Target {
    pub fn is_claude_code(&self) -> bool {
        self.service.is_empty() || self.service == "claude_code"
    }
}

impl Roles {
    pub fn all(&self) -> [(&'static str, &Target); 4] {
        [("plan", &self.plan), ("write", &self.write), ("check", &self.check), ("research", &self.research)]
    }

    /// Which job a step belongs to.
    pub fn for_request(&self, req: &StageRequest) -> &Target {
        if req.tools.iter().any(|t| t == "WebSearch" || t == "WebFetch") {
            return &self.research;
        }
        match req.tier {
            Tier::Fable | Tier::Opus => &self.plan,
            Tier::Sonnet => &self.write,
            Tier::Haiku => &self.check,
        }
    }

    pub fn every(target: Target) -> Self {
        Roles { plan: target.clone(), write: target.clone(), check: target.clone(), research: target }
    }
}

/// Saved choice of what builds courses. Kept in its own file next to the database rather than in it, so API keys
/// aren't copied into the backups in Documents.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AiChoice {
    pub services: Vec<Endpoint>,
    pub roles: Roles,
    /// From 0.9.0, which had one service and an on/off switch. Read once and moved into `services` and `roles`.
    #[serde(skip_serializing)]
    pub provider: String,
    #[serde(skip_serializing)]
    pub endpoint: Option<Endpoint>,
}

impl AiChoice {
    /// Any job goes to a service rather than Claude Code.
    pub fn uses_endpoint(&self) -> bool {
        self.roles.all().iter().any(|(_, t)| !t.is_claude_code())
    }

    /// Any job still goes to Claude Code.
    pub fn uses_claude_code(&self) -> bool {
        self.roles.all().iter().any(|(_, t)| t.is_claude_code())
    }

    pub fn service(&self, id: &str) -> Option<&Endpoint> {
        self.services.iter().find(|s| s.id == id)
    }

    /// Services some job points at.
    pub fn services_in_use(&self) -> Vec<&Endpoint> {
        let mut used: Vec<&Endpoint> = vec![];
        for (_, t) in self.roles.all() {
            if let Some(s) = self.service(&t.service)
                && !used.iter().any(|u| u.id == s.id)
            {
                used.push(s);
            }
        }
        used
    }

    /// The service and model for a job, or None for Claude Code (also when the service was deleted).
    pub fn resolve(&self, target: &Target, tier: Tier) -> Option<(&Endpoint, String)> {
        if target.is_claude_code() {
            return None;
        }
        let s = self.service(&target.service)?;
        let model = if target.model.trim().is_empty() { s.model_for(tier).to_string() } else { target.model.trim().to_string() };
        Some((s, model))
    }

    /// A free id for a new service.
    pub fn new_id(&self) -> String {
        (1..).map(|n| format!("s{n}")).find(|id| self.service(id).is_none()).unwrap()
    }

    pub fn load(file: &Path) -> Self {
        let mut c: AiChoice = std::fs::read_to_string(file).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        c.upgrade();
        c
    }

    /// Move a 0.9.0 settings file (one service, on or off) into the current shape.
    fn upgrade(&mut self) {
        if let Some(mut old) = self.endpoint.take()
            && self.services.is_empty()
            && !(old.base_url.is_empty() && old.model.is_empty())
        {
            old.id = "s1".into();
            if old.name.is_empty() {
                old.name = host_of(&old.base_url);
            }
            self.services.push(old);
            if self.provider == "endpoint" {
                self.roles = Roles::every(Target { service: "s1".into(), model: String::new() });
            }
        }
        self.provider.clear();
        // A job pointing at a service that's gone falls back to Claude Code.
        let ids: Vec<String> = self.services.iter().map(|s| s.id.clone()).collect();
        for t in [&mut self.roles.plan, &mut self.roles.write, &mut self.roles.check, &mut self.roles.research] {
            if !t.is_claude_code() && !ids.contains(&t.service) {
                *t = Target::default();
            }
        }
    }

    pub fn save(&self, file: &Path) -> std::io::Result<()> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = file.with_extension("json.part");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
        }
        std::fs::rename(&tmp, file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep(url: &str) -> Endpoint {
        Endpoint { id: "s1".into(), base_url: url.into(), model: "m".into(), ..Default::default() }
    }

    #[test]
    fn picks_a_model_per_tier() {
        let mut e = ep("https://x/v1");
        assert_eq!(e.model_for(Tier::Opus), "m");
        e.strong_model = "big".into();
        e.quick_model = " small ".into();
        assert_eq!(e.model_for(Tier::Fable), "big");
        assert_eq!(e.model_for(Tier::Sonnet), "m");
        assert_eq!(e.model_for(Tier::Haiku), "small");
        assert_eq!(e.url("chat/completions"), "https://x/v1/chat/completions");
        assert_eq!(ep("https://x/v1/").url("/models"), "https://x/v1/models");
        assert_eq!(ep("https://x/run").url(""), "https://x/run");
    }

    #[test]
    fn spots_bad_settings() {
        assert!(ep("").problem().is_some());
        assert!(ep("api.openai.com/v1").problem().is_some());
        assert!(Endpoint { model: String::new(), ..ep("https://x/v1") }.problem().is_some());
        assert!(Endpoint { api_key: "a\nb".into(), ..ep("https://x/v1") }.problem().is_some());
        assert!(Endpoint { headers: "no colon here".into(), ..ep("https://x/v1") }.problem().is_some());
        assert!(Endpoint { query: "nothing".into(), ..ep("https://x/v1") }.problem().is_some());
        assert!(Endpoint { extra_body: "[1]".into(), ..ep("https://x/v1") }.problem().is_some());
        assert!(Endpoint { api: Api::Custom, model: String::new(), ..ep("https://x") }.problem().is_some());
        assert!(Endpoint { api: Api::Command, base_url: String::new(), command: "ollama run {{model}}".into(), ..ep("") }.problem().is_none());
        assert!(ep("http://localhost:11434/v1").problem().is_none());
        // quotes and backslashes are fine now: curl's config escapes them
        assert!(Endpoint { api_key: "a\"b\\c".into(), ..ep("https://x/v1") }.problem().is_none());
    }

    #[test]
    fn knows_local_addresses() {
        for u in ["http://localhost:1234/v1", "http://127.0.0.1:11434/v1", "http://192.168.1.20:8080/v1", "http://100.64.0.5/v1", "http://[::1]:8000/v1", "http://box.local/v1"] {
            assert!(ep(u).is_local(), "{u}");
        }
        for u in ["https://api.openai.com/v1", "https://openrouter.ai/api/v1", "http://8.8.8.8/v1", "http://172.40.0.1/v1"] {
            assert!(!ep(u).is_local(), "{u}");
        }
        assert!(Endpoint { api: Api::Command, ..ep("") }.is_local());
        assert_eq!(host_of("https://user@Example.COM:443/v1"), "example.com");
    }

    #[test]
    fn curl_config_carries_key_headers_and_query() {
        let mut e = ep("https://api.example.com/v1");
        e.api_key = "sk-1".into();
        let c = curl_config(&e, "chat/completions", true);
        assert!(c.contains("url = \"https://api.example.com/v1/chat/completions\""));
        assert!(c.contains("header = \"Authorization: Bearer sk-1\""));
        assert!(c.contains("header = \"Content-Type: application/json\""));

        e.api = Api::Anthropic;
        let c = curl_config(&e, "messages", true);
        assert!(c.contains("header = \"x-api-key: sk-1\"") && c.contains("anthropic-version: 2023-06-01"));

        // Azure-style: own header, no prefix, a version in the query, and an extra header
        e.api = Api::OpenaiChat;
        e.auth_header = "api-key".into();
        e.auth_prefix = String::new();
        e.query = "api-version=2024-10-21\n# a comment".into();
        e.headers = "X-Title: SelfSemester\n\n".into();
        let c = curl_config(&e, "chat/completions", true);
        assert!(c.contains("header = \"api-key: sk-1\""));
        assert!(!c.contains("Bearer"));
        assert!(c.contains("chat/completions?api-version=2024-10-21\""));
        assert!(c.contains("header = \"X-Title: SelfSemester\""));

        // key as a query parameter, with characters that need escaping
        e.auth_query = "key".into();
        e.api_key = "a b\"c".into();
        let c = curl_config(&e, "", false);
        assert!(c.contains("&key=a%20b%22c\""));
        assert!(!c.contains("Content-Type"));
    }

    #[test]
    fn splits_commands() {
        assert_eq!(split_command("ollama run {{model}}"), ["ollama", "run", "{{model}}"]);
        assert_eq!(split_command(r#""C:\Program Files\x\x.exe" -p  "two words" """#), [r"C:\Program Files\x\x.exe", "-p", "two words", ""]);
        assert!(split_command("   ").is_empty());
    }

    #[test]
    fn fills_a_custom_body() {
        let mut e = ep("https://x");
        e.api = Api::Custom;
        e.custom_body = r#"{"model": {{model}}, "input": {{prompt}}, "sys": {{system}}, "msgs": {{messages}}, "schema": {{schema}}}"#.into();
        e.extra_body = r#"{"temperature": 0.2}"#.into();
        let turns = vec![("user".to_string(), "Say \"hi\"\nplease".to_string())];
        let b = custom_body(&e, "tiny", "rules", &turns, &json!({"type": "object"})).unwrap();
        assert_eq!(b["model"], "tiny");
        assert_eq!(b["input"], "Say \"hi\"\nplease");
        assert_eq!(b["msgs"][1]["content"], "Say \"hi\"\nplease");
        assert_eq!(b["schema"]["type"], "object");
        assert_eq!(b["temperature"], 0.2);
        e.custom_body = r#"{"model": "{{model}}"}"#.into();
        assert!(custom_body(&e, "tiny", "", &turns, &json!({})).is_err());
    }

    #[test]
    fn follows_answer_paths() {
        let v = json!({"choices": [{"message": {"content": "hi"}}], "parts": [{"text": "a"}, {"text": "b"}]});
        assert_eq!(text_of(at_path(&v, "choices.0.message.content").unwrap()), "hi");
        assert_eq!(text_of(at_path(&v, "parts").unwrap()), "ab");
        assert!(at_path(&v, "choices.3.message").is_none());
    }

    #[test]
    fn merges_extra_body() {
        let mut b = json!({"model": "m", "options": {"num_ctx": 4096, "seed": 1}});
        merge(&mut b, r#"{"options": {"num_ctx": 32768}, "temperature": 0}"#);
        assert_eq!(b, json!({"model": "m", "options": {"num_ctx": 32768, "seed": 1}, "temperature": 0}));
        merge(&mut b, "not json");
        assert_eq!(b["temperature"], 0);
    }

    #[test]
    fn digs_json_out_of_chatty_replies() {
        assert_eq!(find_json(r#"{"a":1}"#).unwrap()["a"], 1);
        assert_eq!(find_json("```json\n{\"a\": 2}\n```").unwrap()["a"], 2);
        assert_eq!(find_json("<think>maybe {not this}</think>\nHere you go: {\"a\": 3} Hope that helps!").unwrap()["a"], 3);
        assert_eq!(find_json("Sure! {\"a\": {\"b\": [1, 2]}} and {\"c\": 1}").unwrap()["a"]["b"][1], 2);
        assert!(find_json("no json here").is_none());
        assert!(find_json("[1, 2]").is_none());
    }

    #[test]
    fn a_cut_off_reply_doesnt_pass_off_an_inner_object() {
        let schema = json!({"required": ["ok", "issues"]});
        let cut = r#"{"issues": [{"item_key": "a", "problem": "x"}, {"item_key": "b", "prob"#;
        assert!(find_json_for(cut, &schema).is_none());
        assert_eq!(find_json(cut).unwrap()["item_key"], "a");
        let whole = r#"Here: {"ok": false, "issues": [{"item_key": "a"}]}"#;
        assert_eq!(find_json_for(whole, &schema).unwrap()["ok"], false);
    }

    #[test]
    fn lists_missing_fields() {
        let s = json!({"required": ["ok", "message"]});
        assert_eq!(missing_fields(&s, &json!({"ok": true})), ["message"]);
        assert!(missing_fields(&s, &json!({"ok": true, "message": "hi"})).is_empty());
    }

    #[test]
    fn encodes_base64() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn reads_retry_after() {
        assert_eq!(retry_after("HTTP/1.1 429 Too Many Requests\r\nRetry-After: 30\r\n"), Some(30));
        assert_eq!(retry_after("HTTP/2 429\r\nretry-after: 1.5\r\n"), Some(2));
        assert_eq!(retry_after("HTTP/2 200\r\n"), None);
    }

    #[test]
    fn sorts_out_http_errors() {
        let refused = json!({"error": {"message": "response_format json_schema is not supported"}});
        assert!(matches!(http_error(400, "", Some(&refused), None), ChatError::FormatRefused(_)));
        assert!(matches!(http_error(401, "nope", None, None), ChatError::Run(RunError::Endpoint(_))));
        assert!(matches!(http_error(429, "", None, Some(20)), ChatError::Run(RunError::UsageLimited { retry_at: Some(_), .. })));
        match http_error(503, "busy", None, None) {
            ChatError::Run(e) => assert!(e.is_transient()),
            _ => panic!("503 should be a run error"),
        }
    }

    #[test]
    fn sends_only_the_files_the_prompt_names() {
        let dir = tempfile::tempdir().unwrap();
        let notes = dir.path().join("notes.md");
        let photo = dir.path().join("page1.png");
        let essay = dir.path().join("essay.pdf");
        let other = dir.path().join("unrelated.txt");
        std::fs::write(&notes, "Use SI units.").unwrap();
        std::fs::write(&photo, [137u8, 80, 78, 71]).unwrap();
        std::fs::write(&essay, b"%PDF").unwrap();
        std::fs::write(&other, "secret").unwrap();
        let prompt = format!("Mark it.\n- {}\n- {}\n- {}", notes.display(), photo.display(), essay.display());
        let dirs = vec![dir.path().display().to_string()];

        let plain = with_files(prompt.clone(), &dirs, false, false);
        assert!(plain.text.contains("Use SI units."));
        assert!(!plain.text.contains("secret"));
        assert!(plain.text.contains("essay.pdf (a .pdf file"));
        assert!(plain.text.contains("isn't set up to see pictures"));
        assert!(plain.images.is_empty() && plain.pdfs.is_empty());

        let all = with_files(prompt, &dirs, true, true);
        assert_eq!(all.images, [("image/png".to_string(), "iVBORw==".to_string())]);
        assert_eq!(all.pdfs, ["JVBERg==".to_string()]);
        assert!(all.text.contains("(The PDF"));

        assert_eq!(with_files("No files here.".into(), &dirs, true, true).text, "No files here.");
    }

    #[test]
    fn roles_route_steps() {
        let r = Roles {
            plan: Target { service: "a".into(), model: String::new() },
            write: Target::default(),
            check: Target { service: "b".into(), model: "tiny".into() },
            research: Target { service: "claude_code".into(), model: String::new() },
        };
        let mut req = StageRequest {
            tier: Tier::Opus,
            system: None,
            prompt: String::new(),
            schema: json!({}),
            tools: vec![],
            add_dirs: vec![],
            effort: None,
        };
        assert_eq!(r.for_request(&req).service, "a");
        req.tier = Tier::Haiku;
        assert_eq!(r.for_request(&req).model, "tiny");
        req.tools = vec!["WebSearch".into()];
        assert!(r.for_request(&req).is_claude_code());
        req.tools.clear();
        req.tier = Tier::Sonnet;
        assert!(r.for_request(&req).is_claude_code());
    }

    #[test]
    fn choice_round_trips_and_defaults_to_claude_code() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ai.json");
        let empty = AiChoice::load(&file);
        assert!(!empty.uses_endpoint() && empty.uses_claude_code());
        let c = AiChoice {
            services: vec![Endpoint { api_key: "k".into(), ..ep("https://x/v1") }],
            roles: Roles { write: Target { service: "s1".into(), model: "big".into() }, ..Default::default() },
            ..Default::default()
        };
        c.save(&file).unwrap();
        let back = AiChoice::load(&file);
        assert_eq!(back, c);
        assert!(back.uses_endpoint() && back.uses_claude_code());
        assert_eq!(back.resolve(&back.roles.write, Tier::Sonnet).unwrap().1, "big");
        assert!(back.resolve(&back.roles.plan, Tier::Opus).is_none());
        assert_eq!(back.new_id(), "s2");
        std::fs::write(&file, "not json").unwrap();
        assert_eq!(AiChoice::load(&file), AiChoice::default());
    }

    #[test]
    fn reads_a_0_9_0_settings_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ai.json");
        std::fs::write(&file, r#"{"provider":"endpoint","endpoint":{"preset":"lmstudio","base_url":"http://localhost:1234/v1","api_key":"","model":"qwen","strong_model":"","quick_model":"","sees_images":true,"check":null}}"#).unwrap();
        let c = AiChoice::load(&file);
        assert_eq!(c.services.len(), 1);
        assert_eq!((c.services[0].id.as_str(), c.services[0].name.as_str(), c.services[0].api), ("s1", "localhost", Api::OpenaiChat));
        assert!(c.services[0].sees_images);
        assert!(c.roles.all().iter().all(|(_, t)| t.service == "s1"));
        // switched off in 0.9.0: the service is kept, Claude Code still does everything
        std::fs::write(&file, r#"{"provider":"claude_code","endpoint":{"base_url":"https://api.openai.com/v1","model":"gpt-5"}}"#).unwrap();
        let c = AiChoice::load(&file);
        assert_eq!(c.services.len(), 1);
        assert!(!c.uses_endpoint());
    }

    #[test]
    fn a_deleted_service_falls_back_to_claude_code() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ai.json");
        std::fs::write(&file, r#"{"services":[],"roles":{"write":{"service":"s9","model":""}}}"#).unwrap();
        assert!(!AiChoice::load(&file).uses_endpoint());
    }

    /// A program on this computer as the service. Uses the shell's own tools, so it runs anywhere.
    #[test]
    fn runs_a_command_service() {
        let dir = tempfile::tempdir().unwrap();
        let script = format!("\"{}\"", answer_script(dir.path()).display());
        let e = Endpoint { id: "c".into(), api: Api::Command, command: script, model: "any".into(), ..Default::default() };
        let r = EndpointRunner::new(dir.path().join("tmp"));
        let c = r.check(&e, "");
        assert!(c.reached && c.json_ok, "{c:?}");
        assert_eq!(c.format, "prompt");
    }

    const ANSWER: &str = r#"{"ok": true, "numbers": [{"n": 1, "word": "one"}, {"n": 2, "word": "two"}, {"n": 3, "word": "three"}]}"#;

    /// A tiny script that reads the prompt and prints a fixed answer.
    #[cfg(windows)]
    fn answer_script(dir: &Path) -> PathBuf {
        let p = dir.join("answer.cmd");
        std::fs::write(&p, format!("@echo off
more > nul
echo {ANSWER}
")).unwrap();
        p
    }

    #[cfg(not(windows))]
    fn answer_script(dir: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join("answer.sh");
        std::fs::write(&p, format!("#!/bin/sh
cat > /dev/null
echo '{ANSWER}'
")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    /// A real call to a server on this computer. Opt in with
    /// `SELFSEMESTER_TEST_ENDPOINT=http://localhost:1234/v1 SELFSEMESTER_TEST_MODEL=<model> cargo test -- --ignored live_endpoint`
    /// Add SELFSEMESTER_TEST_API=ollama (or anthropic, gemini, openai_responses) for another kind of API.
    #[test]
    #[ignore]
    fn live_endpoint_check() {
        let e = live_endpoint();
        let dir = tempfile::tempdir().unwrap();
        let c = EndpointRunner::new(dir.path().to_path_buf()).check(&e, "");
        println!("{c:?}");
        assert!(c.reached && c.json_ok, "{c:?}");
        if e.sees_images {
            assert_eq!(c.images_ok, Some(true), "{c:?}");
        }
    }

    pub(crate) fn live_endpoint() -> Endpoint {
        let api = match std::env::var("SELFSEMESTER_TEST_API").unwrap_or_default().as_str() {
            "ollama" => Api::Ollama,
            "anthropic" => Api::Anthropic,
            "gemini" => Api::Gemini,
            "openai_responses" => Api::OpenaiResponses,
            _ => Api::OpenaiChat,
        };
        Endpoint {
            id: "live".into(),
            api,
            base_url: std::env::var("SELFSEMESTER_TEST_ENDPOINT").expect("set SELFSEMESTER_TEST_ENDPOINT"),
            model: std::env::var("SELFSEMESTER_TEST_MODEL").expect("set SELFSEMESTER_TEST_MODEL"),
            api_key: std::env::var("SELFSEMESTER_TEST_KEY").unwrap_or_default(),
            sees_images: std::env::var("SELFSEMESTER_TEST_IMAGES").is_ok(),
            context: std::env::var("SELFSEMESTER_TEST_CONTEXT").ok().and_then(|c| c.parse().ok()),
            ..Default::default()
        }
    }
}
