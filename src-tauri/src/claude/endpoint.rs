//! Another AI service instead of Claude Code: anything that speaks the OpenAI-style chat completions API.
//!
//! That covers most of them: OpenAI, OpenRouter, Google Gemini, Groq, Mistral, DeepSeek, the Anthropic API, and
//! programs that run models on your own computer, like Ollama and LM Studio. Requests go through the system's curl,
//! the same way lesson sounds and tools are downloaded, so the app still needs no HTTP library.
//!
//! What's lost compared with Claude Code: there's no web search, and the model can't open files by itself. Text
//! files are put into the request instead, and pictures are sent along when the model can see images. `fit.rs`
//! turns all of this into a plain report for the learner before anything is built.

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

const TEXT_EXTS: &[&str] = &[
    "txt", "md", "markdown", "csv", "tsv", "json", "xml", "html", "htm", "yaml", "yml", "toml", "ini", "log", "tex",
    "py", "js", "ts", "rs", "c", "h", "cpp", "hpp", "java", "cs", "go", "rb", "php", "sql", "r", "m", "sh", "ipynb",
];
const IMAGE_EXTS: &[(&str, &str)] =
    &[("png", "image/png"), ("jpg", "image/jpeg"), ("jpeg", "image/jpeg"), ("gif", "image/gif"), ("webp", "image/webp")];

/// A 16 x 16 red square, for checking whether the model can see pictures.
const RED_SQUARE_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAIAAACQkWg2AAAAFklEQVR42mO4oKBAEmIY1TCqYfhqAABLfxAQegypMgAAAABJRU5ErkJggg==";

/// The service the learner set up in Settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Endpoint {
    /// Which preset it started from ("openai", "ollama", ...), or "custom". Only used to show the right hints.
    #[serde(default)]
    pub preset: String,
    /// Up to and including the version part, e.g. `https://api.openai.com/v1`.
    pub base_url: String,
    /// Empty for local servers that don't ask for one.
    #[serde(default)]
    pub api_key: String,
    /// Used for lessons, the reading list and most marking.
    pub model: String,
    /// Planning, exams and final-exam marking. Empty means `model`.
    #[serde(default)]
    pub strong_model: String,
    /// Small checks and summaries. Empty means `model`.
    #[serde(default)]
    pub quick_model: String,
    /// The learner says the model takes pictures (photos of handwritten work, diagrams).
    #[serde(default)]
    pub sees_images: bool,
    /// What the last "Check" found. Cleared whenever the address or models change.
    #[serde(default)]
    pub check: Option<Check>,
}

/// Result of a quick real test against the service.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
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
    pub fn model_for(&self, tier: Tier) -> &str {
        let pick = match tier {
            Tier::Fable | Tier::Opus => &self.strong_model,
            Tier::Sonnet => &self.model,
            Tier::Haiku => &self.quick_model,
        };
        if pick.trim().is_empty() { self.model.trim() } else { pick.trim() }
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url.trim().trim_end_matches('/'), path)
    }

    /// Something wrong with the settings that would stop every call, in words for the Settings page.
    pub fn problem(&self) -> Option<String> {
        let url = self.base_url.trim();
        if url.is_empty() {
            return Some("Add the service's address.".into());
        }
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Some("The address should start with https:// (or http:// for a server on your own computer).".into());
        }
        if self.model.trim().is_empty() {
            return Some("Pick a model.".into());
        }
        if bad_key(&self.api_key) {
            return Some("The API key has a character in it that can't be right (a quote, backslash or line break).".into());
        }
        None
    }

    /// Runs on this computer or the local network, so nothing goes out to a company and nothing is billed.
    pub fn is_local(&self) -> bool {
        is_local_host(&host_of(&self.base_url))
    }
}

/// A key curl's config can't carry. Real keys never have these.
pub fn bad_key(key: &str) -> bool {
    key.contains(['"', '\\', '\n', '\r'])
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
    matches!(
        (parts[0], parts[1]),
        (127, _) | (10, _) | (192, 168) | (169, 254)
    ) || (parts[0] == 172 && (16..=31).contains(&parts[1]))
        || (parts[0] == 100 && (64..=127).contains(&parts[1]))
}

/// How the request asks for JSON. Services differ: the newest take a JSON schema, many only take "some JSON",
/// and a few local servers reject the field outright. Tried in this order, and the one that works is remembered.
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
}

/// One HTTP answer, as curl got it.
struct Reply {
    status: u16,
    body: String,
    retry_after: Option<u64>,
}

pub struct EndpointRunner {
    endpoint: Mutex<Endpoint>,
    /// Request and answer files go here for the moment curl needs them.
    tmp_dir: PathBuf,
    timeout: Duration,
    stop: AtomicBool,
    /// What worked last time for each model, so later calls don't repeat the failed tries.
    formats: Mutex<Vec<(String, Format)>>,
}

impl EndpointRunner {
    pub fn new(endpoint: Endpoint, tmp_dir: PathBuf) -> Self {
        Self {
            endpoint: Mutex::new(endpoint),
            tmp_dir,
            timeout: Duration::from_secs(20 * 60),
            stop: AtomicBool::new(false),
            formats: Mutex::new(vec![]),
        }
    }

    pub fn endpoint(&self) -> Endpoint {
        self.endpoint.lock().unwrap().clone()
    }

    pub fn set_endpoint(&self, e: Endpoint) {
        *self.endpoint.lock().unwrap() = e;
        self.formats.lock().unwrap().clear();
    }

    fn format_for(&self, model: &str) -> Format {
        self.formats.lock().unwrap().iter().find(|(m, _)| m == model).map(|(_, f)| *f).unwrap_or(Format::Schema)
    }

    fn remember_format(&self, model: &str, f: Format) {
        let mut all = self.formats.lock().unwrap();
        all.retain(|(m, _)| m != model);
        all.push((model.to_string(), f));
    }

    /// Send one request through curl. The API key goes to curl on stdin, never on the command line, where other
    /// programs on the computer could read it.
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
            .args(["-X", method, "-A", &format!("SelfSemester/{}", env!("CARGO_PKG_VERSION"))])
            .args(["-H", "Content-Type: application/json", "-K", "-", "-o"])
            .arg(&out_file)
            .arg("-D")
            .arg(&head_file)
            .args(["-w", "%{http_code}"]);
        if let Some(b) = body {
            std::fs::write(&body_file, serde_json::to_vec(b).map_err(|e| RunError::BadOutput(e.to_string()))?)?;
            cmd.arg("--data-binary").arg(format!("@{}", body_file.display()));
        }
        cmd.arg(ep.url(path)).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        crate::course::audio::hidden(&mut cmd);

        let mut child = cmd.spawn().map_err(|e| RunError::Io(std::io::Error::other(format!("couldn't start curl: {e}"))))?;
        {
            let mut stdin = child.stdin.take().expect("stdin piped");
            let key = ep.api_key.trim();
            if !key.is_empty() {
                stdin.write_all(format!("header = \"Authorization: Bearer {key}\"\n").as_bytes())?;
                // Anthropic's usual header too. Its OpenAI-style endpoint takes either, so sending both is safe.
                if host_of(&ep.base_url).ends_with("anthropic.com") {
                    stdin.write_all(format!("header = \"x-api-key: {key}\"\n").as_bytes())?;
                }
            }
        }
        let mut out = child.stdout.take().expect("stdout piped");
        let mut err = child.stderr.take().expect("stderr piped");
        let out_t = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = out.read_to_string(&mut s);
            s
        });
        let err_t = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = err.read_to_string(&mut s);
            s
        });
        let start = Instant::now();
        loop {
            if child.try_wait()?.is_some() {
                break;
            }
            if self.stop.swap(false, Ordering::SeqCst) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RunError::Stopped);
            }
            if start.elapsed() > limit + Duration::from_secs(10) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RunError::Timeout(limit.as_secs()));
            }
            std::thread::sleep(Duration::from_millis(150));
        }
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

    /// One chat request with the given messages. Returns the reply's text, usage, the model that answered, and
    /// whether the reply was cut off at the output limit.
    fn chat(&self, ep: &Endpoint, model: &str, messages: &[Value], schema: &Value, format: Format, limit: Duration) -> Result<(String, Usage, String, bool), ChatError> {
        let mut body = json!({ "model": model, "messages": messages, "stream": false });
        match format {
            Format::Schema => {
                body["response_format"] = json!({ "type": "json_schema", "json_schema": { "name": "answer", "schema": schema, "strict": false } })
            }
            Format::Json => body["response_format"] = json!({ "type": "json_object" }),
            Format::Prompt => {}
        }
        let started = Instant::now();
        let reply = self.send(ep, "POST", "chat/completions", Some(&body), limit).map_err(ChatError::Run)?;
        let parsed: Option<Value> = serde_json::from_str(&reply.body).ok();
        if reply.status != 200 {
            return Err(http_error(reply.status, &reply.body, parsed.as_ref(), reply.retry_after));
        }
        let v = parsed.ok_or_else(|| ChatError::Run(RunError::BadOutput(format!("the service sent back something that isn't JSON: {}", snip(&reply.body)))))?;
        if let Some(e) = v.get("error").filter(|e| !e.is_null()) {
            return Err(ChatError::Run(RunError::Endpoint(error_text(e))));
        }
        let choice = &v["choices"][0];
        let text = message_text(&choice["message"]);
        let cut = choice["finish_reason"].as_str() == Some("length");
        let u = &v["usage"];
        let n = |p: &str| u.pointer(p).and_then(Value::as_u64).unwrap_or(0);
        let usage = Usage {
            input_tokens: n("/prompt_tokens"),
            output_tokens: n("/completion_tokens"),
            cache_tokens: n("/prompt_tokens_details/cached_tokens"),
            web_searches: 0,
            duration_ms: started.elapsed().as_millis() as u64,
        };
        let used_model = v["model"].as_str().filter(|m| !m.is_empty()).unwrap_or(model).to_string();
        Ok((text, usage, used_model, cut))
    }

    /// Ask, falling back to a simpler way of asking for JSON when the service turns the first one down, and asking
    /// once more when the answer isn't the JSON that was wanted.
    fn ask(&self, ep: &Endpoint, model: &str, system: &str, user: Value, schema: &Value, limit: Duration) -> Result<(StageOutput, Format), RunError> {
        let mut format = self.format_for(model);
        let mut messages = vec![json!({ "role": "system", "content": system }), json!({ "role": "user", "content": user })];
        let mut total = Usage::default();
        let mut nudged = false;
        loop {
            match self.chat(ep, model, &messages, schema, format, limit) {
                Ok((text, usage, used_model, cut)) => {
                    add_usage(&mut total, &usage);
                    match find_json_for(&text, schema) {
                        Some(value) => {
                            self.remember_format(model, format);
                            return Ok((StageOutput { value, usage: total, model: used_model }, format));
                        }
                        // Asking again would only get cut off again.
                        None if cut => {
                            return Err(RunError::BadOutput(format!(
                                "{model}'s answer was cut off before it finished. Either its output limit (or a local server's \
                                 context size) is too small for this step, or the model got stuck repeating itself"
                            )));
                        }
                        None if !nudged => {
                            nudged = true;
                            let what = match find_json(&text) {
                                Some(v) => format!(
                                    "Your answer left out these required fields: {}. Send the whole JSON object again with every required field filled in.",
                                    missing_fields(schema, &v).join(", ")
                                ),
                                None => "Your answer wasn't valid JSON. Send it again as one JSON object.".to_string(),
                            };
                            messages.push(json!({ "role": "assistant", "content": text }));
                            messages.push(json!({
                                "role": "user",
                                "content": format!(
                                    "{what} Nothing else, no Markdown fences. It must match this JSON Schema:
{}",
                                    serde_json::to_string(schema).unwrap_or_default()
                                )
                            }));
                        }
                        None => {
                            return Err(RunError::BadOutput(format!(
                                "{model} didn't answer in the shape this step needs, even when asked twice. A larger model usually fixes this. It said: {}",
                                snip(&text)
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

    /// The models the service lists, for the picker in Settings. Not every service has this list.
    pub fn models(&self) -> Result<Vec<String>, String> {
        let ep = self.endpoint();
        let reply = self.send(&ep, "GET", "models", None, Duration::from_secs(30)).map_err(|e| e.to_string())?;
        if reply.status != 200 {
            let v = serde_json::from_str::<Value>(&reply.body).ok();
            return Err(match http_error(reply.status, &reply.body, v.as_ref(), None) {
                ChatError::Run(e) => e.to_string(),
                ChatError::FormatRefused(m) => m,
            });
        }
        let v: Value = serde_json::from_str(&reply.body).map_err(|_| "the service's model list isn't JSON".to_string())?;
        let list = v["data"].as_array().or_else(|| v["models"].as_array()).cloned().unwrap_or_default();
        let mut names: Vec<String> = list
            .iter()
            .filter_map(|m| m["id"].as_str().or_else(|| m["name"].as_str()).or_else(|| m.as_str()))
            .map(|s| s.trim_start_matches("models/").to_string())
            .collect();
        names.sort();
        names.dedup();
        Ok(names)
    }

    /// Try the service for real: one small structured answer, and a picture if the model is meant to see them.
    pub fn check(&self) -> Check {
        let ep = self.endpoint();
        let mut c = Check { at: crate::db::now_utc(), ..Default::default() };
        if let Some(p) = ep.problem() {
            c.message = p;
            return c;
        }
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
        let model = ep.model_for(Tier::Sonnet).to_string();
        match self.ask(&ep, &model, &system, json!("Set ok to true and list the numbers 1, 2 and 3 with their English words."), &schema, Duration::from_secs(180)) {
            Ok((out, format)) => {
                c.reached = true;
                c.format = format.name().into();
                let nums: Vec<i64> = out.value["numbers"].as_array().map(|a| a.iter().filter_map(|x| x["n"].as_i64()).collect()).unwrap_or_default();
                c.json_ok = out.value["ok"] == true && nums == [1, 2, 3];
                c.message = if c.json_ok {
                    format!("{} answered correctly.", out.model)
                } else {
                    format!("{} answered, but got a simple question wrong. Expect mistakes in lessons and marks.", out.model)
                };
            }
            Err(e) => {
                c.reached = !matches!(e, RunError::Io(_) | RunError::Timeout(_));
                c.message = e.to_string();
            }
        }
        c.seconds = (started.elapsed().as_secs_f64() * 10.0).round() / 10.0;
        if ep.sees_images && c.reached {
            let schema = json!({ "type": "object", "properties": { "colour": { "type": "string" } }, "required": ["colour"] });
            let user = json!([
                { "type": "text", "text": "What colour is this square? One word." },
                { "type": "image_url", "image_url": { "url": format!("data:image/png;base64,{RED_SQUARE_PNG}") } }
            ]);
            let saw = self.ask(&ep, &model, &system_text(None, &schema), user, &schema, Duration::from_secs(120));
            c.images_ok = Some(matches!(&saw, Ok((o, _)) if o.value["colour"].as_str().is_some_and(|s| s.to_lowercase().contains("red"))));
            if c.images_ok == Some(false) {
                c.message += " It couldn't see a test picture, so photos of your work won't be marked.";
            }
        }
        c
    }
}

impl ClaudeRunner for EndpointRunner {
    fn status(&self) -> ClaudeStatus {
        let ep = self.endpoint();
        let problem = ep.problem();
        ClaudeStatus {
            provider: "endpoint".into(),
            installed: problem.is_none(),
            logged_in: problem.is_none(),
            path: Some(ep.base_url.clone()),
            version: Some(ep.model.clone()),
            auth_method: Some(if ep.is_local() { "local".into() } else { "api key".into() }),
            on_subscription: false,
            problem: problem.map(|p| format!("The AI service isn't set up yet: {p} See Settings.")),
        }
    }

    fn run(&self, req: &StageRequest) -> Result<StageOutput, RunError> {
        let ep = self.endpoint();
        if let Some(p) = ep.problem() {
            return Err(RunError::Endpoint(format!("the AI service isn't set up: {p}")));
        }
        let model = ep.model_for(req.tier).to_string();
        let mut prompt = req.prompt.clone();
        if req.tools.iter().any(|t| t == "WebSearch" || t == "WebFetch") {
            prompt += NO_WEB_NOTE;
        }
        let user = with_files(prompt, &req.add_dirs, ep.sees_images);
        let system = system_text(req.system.as_deref(), &req.schema);
        let limit = super::cli::time_limit(self.timeout, req);
        self.ask(&ep, &model, &system, user, &req.schema, limit).map(|(out, _)| out)
    }

    fn stop_current(&self) {
        self.stop.store(true, Ordering::SeqCst);
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

/// Put the learner's files into the request. Claude Code opens them with its Read tool; here they have to travel
/// with the prompt. Only files the prompt names are sent, so a folder of uploads doesn't all go every time.
fn with_files(prompt: String, dirs: &[String], images: bool) -> Value {
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
        return json!(prompt);
    }
    files.sort();
    let mut text = String::new();
    let mut pictures: Vec<Value> = vec![];
    let mut skipped: Vec<String> = vec![];
    let mut room = MAX_TEXT_TOTAL;
    for f in &files {
        let name = f.display().to_string();
        let ext = f.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
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
                continue;
            }
            let small = std::fs::metadata(f).map(|m| m.len() <= MAX_IMAGE_BYTES).unwrap_or(false);
            match small.then(|| std::fs::read(f).ok()).flatten() {
                Some(bytes) => {
                    text += &format!("\n(The picture {name} is attached.)\n");
                    pictures.push(json!({ "type": "image_url", "image_url": { "url": format!("data:{mime};base64,{}", base64(&bytes)) } }));
                }
                _ => skipped.push(format!("{name} (a picture too large to send)")),
            }
        } else {
            skipped.push(format!("{name} (a .{ext} file, which can't be opened in this setup; only text files and pictures can)"));
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
    if pictures.is_empty() {
        return json!(full);
    }
    let mut parts = vec![json!({ "type": "text", "text": full })];
    parts.extend(pictures);
    Value::Array(parts)
}

fn cut_at(s: &str, max: usize) -> &str {
    let mut end = max.min(s.len());
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Plain base64, for pictures. Short enough not to need a crate for it.
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
        400 | 422 if lower.contains("response_format") || lower.contains("json_schema") || lower.contains("json_object") || lower.contains("structured") => {
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

/// The reply's text. Usually a plain string; some services send a list of parts.
fn message_text(m: &Value) -> String {
    match &m["content"] {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join(""),
        _ => m["reasoning_content"].as_str().unwrap_or("").to_string(),
    }
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

fn add_usage(total: &mut Usage, u: &Usage) {
    total.input_tokens += u.input_tokens;
    total.output_tokens += u.output_tokens;
    total.cache_tokens += u.cache_tokens;
    total.duration_ms += u.duration_ms;
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

/// Saved choice of what builds courses. Kept in its own file next to the database rather than in it, so the API
/// key isn't copied into the backups in Documents.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AiChoice {
    /// "claude_code" (the default) or "endpoint".
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub endpoint: Endpoint,
}

impl AiChoice {
    pub fn uses_endpoint(&self) -> bool {
        self.provider == "endpoint"
    }

    pub fn load(file: &Path) -> Self {
        std::fs::read_to_string(file).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
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
        Endpoint { base_url: url.into(), model: "m".into(), ..Default::default() }
    }

    #[test]
    fn picks_a_model_per_tier() {
        let mut e = ep("https://x/v1");
        assert_eq!(e.model_for(Tier::Opus), "m");
        assert_eq!(e.model_for(Tier::Haiku), "m");
        e.strong_model = "big".into();
        e.quick_model = " small ".into();
        assert_eq!(e.model_for(Tier::Fable), "big");
        assert_eq!(e.model_for(Tier::Opus), "big");
        assert_eq!(e.model_for(Tier::Sonnet), "m");
        assert_eq!(e.model_for(Tier::Haiku), "small");
        assert_eq!(e.url("chat/completions"), "https://x/v1/chat/completions");
        assert_eq!(ep("https://x/v1/").url("models"), "https://x/v1/models");
    }

    #[test]
    fn spots_bad_settings() {
        assert!(ep("").problem().is_some());
        assert!(ep("api.openai.com/v1").problem().is_some());
        assert!(Endpoint { model: String::new(), ..ep("https://x/v1") }.problem().is_some());
        assert!(Endpoint { api_key: "a\"b".into(), ..ep("https://x/v1") }.problem().is_some());
        assert!(ep("http://localhost:11434/v1").problem().is_none());
    }

    #[test]
    fn knows_local_addresses() {
        for u in ["http://localhost:1234/v1", "http://127.0.0.1:11434/v1", "http://192.168.1.20:8080/v1", "http://100.64.0.5/v1", "http://[::1]:8000/v1", "http://box.local/v1"] {
            assert!(ep(u).is_local(), "{u}");
        }
        for u in ["https://api.openai.com/v1", "https://openrouter.ai/api/v1", "http://8.8.8.8/v1", "http://172.40.0.1/v1"] {
            assert!(!ep(u).is_local(), "{u}");
        }
        assert_eq!(host_of("https://user@Example.COM:443/v1"), "example.com");
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
        assert_eq!(find_json(cut).unwrap()["item_key"], "a"); // what the plain finder would have taken
        let whole = r#"Here: {"ok": false, "issues": [{"item_key": "a"}]}"#;
        assert_eq!(find_json_for(whole, &schema).unwrap()["ok"], false);
    }

    #[test]
    fn lists_missing_fields() {
        let s = json!({"required": ["ok", "message"]});
        assert_eq!(missing_fields(&s, &json!({"ok": true})), ["message"]);
        assert!(missing_fields(&s, &json!({"ok": true, "message": "hi"})).is_empty());
        assert!(missing_fields(&json!({}), &json!({})).is_empty());
    }

    #[test]
    fn encodes_base64() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
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

        let plain = with_files(prompt.clone(), &dirs, false);
        let text = plain.as_str().unwrap();
        assert!(text.contains("Use SI units."));
        assert!(!text.contains("secret"));
        assert!(text.contains("essay.pdf (a .pdf file"));
        assert!(text.contains("isn't set up to see pictures"));

        let with_pics = with_files(prompt, &dirs, true);
        let parts = with_pics.as_array().unwrap();
        assert_eq!(parts.len(), 2);
        assert!(parts[1]["image_url"]["url"].as_str().unwrap().starts_with("data:image/png;base64,iVBORw"));

        assert_eq!(with_files("No files here.".into(), &dirs, true), json!("No files here."));
    }

    #[test]
    fn choice_round_trips_and_defaults_to_claude_code() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ai.json");
        assert!(!AiChoice::load(&file).uses_endpoint());
        let c = AiChoice { provider: "endpoint".into(), endpoint: Endpoint { api_key: "k".into(), ..ep("https://x/v1") } };
        c.save(&file).unwrap();
        assert_eq!(AiChoice::load(&file), c);
        std::fs::write(&file, "not json").unwrap();
        assert_eq!(AiChoice::load(&file), AiChoice::default());
    }

    /// A real call to a server on this computer. Opt in with
    /// `SELFSEMESTER_TEST_ENDPOINT=http://localhost:1234/v1 SELFSEMESTER_TEST_MODEL=<model> cargo test -- --ignored live_endpoint`
    #[test]
    #[ignore]
    fn live_endpoint_check() {
        let url = std::env::var("SELFSEMESTER_TEST_ENDPOINT").expect("set SELFSEMESTER_TEST_ENDPOINT");
        let model = std::env::var("SELFSEMESTER_TEST_MODEL").expect("set SELFSEMESTER_TEST_MODEL");
        let dir = tempfile::tempdir().unwrap();
        let e = Endpoint {
            base_url: url,
            model,
            api_key: std::env::var("SELFSEMESTER_TEST_KEY").unwrap_or_default(),
            sees_images: std::env::var("SELFSEMESTER_TEST_IMAGES").is_ok(),
            ..Default::default()
        };
        let r = EndpointRunner::new(e, dir.path().to_path_buf());
        let c = r.check();
        println!("{c:?}");
        assert!(c.reached && c.json_ok, "{c:?}");
        if r.endpoint().sees_images {
            assert_eq!(c.images_ok, Some(true), "{c:?}");
        }
    }
}
