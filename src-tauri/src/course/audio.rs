//! Sound in lessons. A lesson can hold ```audio blocks listing clips to hear: words and phrases in the course's
//! language, notes and chords, or other sounds (bird song, a heartbeat). The app fills each clip in the background:
//!
//! 1. a freely licensed recording from Wikimedia Commons, which holds Wiktionary's and Lingua Libre's pronunciation
//!    recordings by native speakers. Only files whose licence allows reuse (public domain, CC0, CC BY, CC BY-SA) are
//!    taken, and their author and licence are shown with the lesson;
//! 2. when no recording exists, the app makes one: notes and chords are synthesised here, and speech is read by one
//!    of the computer's own voices for that language. Those clips are labelled as computer-made.
//!
//! What was found is kept next to the lesson in `items/<key>-lesson.audio.json`, and the files in the course's
//! `audio` folder, so lessons play offline once filled.

use crate::paths::AppPaths;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

pub const DIR: &str = "audio";
/// Recordings longer than this aren't a "clip" (Commons also has whole lectures and songs).
const MAX_SECONDS: f64 = 90.0;
const MAX_BYTES: u64 = 8 * 1024 * 1024;
const MAX_CLIPS_PER_LESSON: usize = 150;
const COMMONS_API: &str = "https://commons.wikimedia.org/w/api.php";
/// Turned off with the setting `audio_online` = "0".
pub const ONLINE_SETTING: &str = "audio_online";

// ---------- what a lesson asks for ----------

/// One thing to hear. Exactly one of `say`, `notes` or `sound` says what it is.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Clip {
    /// Words to hear, written in the language's own script (你好, bonjour).
    pub say: String,
    /// The same words romanised (nǐ hǎo), when the script isn't Latin. Helps find recordings.
    pub romanization: String,
    /// BCP 47 language tag (zh-CN, fr-FR). Falls back to the block's `lang`.
    pub lang: String,
    /// What the learner should notice, shown next to the play button.
    pub label: String,
    /// Notes to synthesise in order, like "C4", "F#3", "Bb4", or "rest".
    pub notes: Vec<String>,
    /// Play `notes` together as one chord instead of one after another.
    pub chord: bool,
    /// Seconds per note (default 0.6, chords 1.8).
    pub seconds: f64,
    /// Search words for a recording that isn't speech or notes ("song of the European robin").
    pub sound: String,
    /// A Wikimedia Commons file name, when a known recording fits exactly. It's checked like any other.
    pub commons: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Block {
    pub caption: String,
    pub clips: Vec<Clip>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum BlockJson {
    Full {
        #[serde(default)]
        lang: String,
        /// A listening test: the UI hides romanisations and labels. They're still used to find recordings.
        #[serde(default, rename = "test")]
        _test: bool,
        clips: Vec<Clip>,
    },
    List(Vec<Clip>),
}

/// The ```audio blocks of a lesson, in order. A clip's id is `b<block>c<clip>`; the UI numbers them the same way.
/// Blocks that can't be read are kept (empty) so the numbering still matches.
pub fn blocks(md: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut lines = md.lines();
    while let Some(line) = lines.next() {
        if !line.starts_with("```audio") {
            continue;
        }
        let mut body = Vec::new();
        let mut closed = false;
        for l in lines.by_ref() {
            if l.trim_end() == "```" {
                closed = true;
                break;
            }
            body.push(l);
        }
        if !closed {
            break;
        }
        let mut caption = String::new();
        while let Some(first) = body.first() {
            let t = first.trim();
            let lower = t.to_ascii_lowercase();
            if lower.starts_with("caption:") || lower.starts_with("credit:") {
                if lower.starts_with("caption:") {
                    caption = t["caption:".len()..].trim().to_string();
                }
                body.remove(0);
            } else {
                break;
            }
        }
        let clips = match serde_json::from_str::<BlockJson>(body.join("\n").trim()) {
            Ok(BlockJson::Full { lang, clips, .. }) => clips
                .into_iter()
                .map(|mut c| {
                    if c.lang.trim().is_empty() {
                        c.lang = lang.clone();
                    }
                    c
                })
                .collect(),
            Ok(BlockJson::List(clips)) => clips,
            Err(_) => vec![],
        };
        out.push(Block { caption, clips });
    }
    out
}

/// Every clip with its id.
pub fn clips(md: &str) -> Vec<(String, Clip)> {
    blocks(md)
        .into_iter()
        .enumerate()
        .flat_map(|(b, block)| block.clips.into_iter().enumerate().map(move |(c, clip)| (format!("b{b}c{c}"), clip)))
        .take(MAX_CLIPS_PER_LESSON)
        .collect()
}

// ---------- what was found ----------

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Found {
    /// The clip this was found for, so an edited lesson gets new audio.
    pub sig: String,
    /// ok | missing
    pub status: String,
    /// Relative to the course folder, always under `audio/`.
    pub file: String,
    /// commons | voice | synth
    pub source: String,
    /// Who made the recording, or which computer voice read it.
    pub credit: String,
    pub licence: String,
    /// The recording's page on Wikimedia Commons.
    pub url: String,
    /// Why there's nothing to play, for a missing clip.
    pub note: String,
}

pub fn sidecar_ref(key: &str) -> String {
    format!("items/{key}-lesson.audio.json")
}

/// What was found for a lesson's clips (empty when nothing has been looked up yet).
pub fn load(course_dir: &Path, key: &str) -> BTreeMap<String, Found> {
    std::fs::read_to_string(course_dir.join(sidecar_ref(key))).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save(course_dir: &Path, key: &str, found: &BTreeMap<String, Found>) -> std::io::Result<()> {
    let path = course_dir.join(sidecar_ref(key));
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(found).map_err(std::io::Error::other)?)?;
    std::fs::rename(tmp, path)
}

fn sig(clip: &Clip) -> String {
    serde_json::to_string(clip).unwrap_or_default()
}

/// The full path of a clip's file, refusing anything outside the course's `audio` folder.
pub fn path_of(paths: &AppPaths, course_id: &str, file: &str) -> Result<PathBuf, String> {
    let rel = Path::new(file);
    let mut parts = rel.components();
    let inside = matches!(parts.next(), Some(Component::Normal(d)) if d == DIR)
        && parts.clone().count() == 1
        && parts.all(|c| matches!(c, Component::Normal(_)));
    if !inside {
        return Err("That sound isn't part of this course.".into());
    }
    let full = paths.course_dir(course_id).join(rel);
    if !full.is_file() {
        return Err("That sound is missing from the course folder.".into());
    }
    Ok(full)
}

// ---------- the outside world (traits so tests can stand in) ----------

#[derive(Debug, PartialEq)]
pub enum NetError {
    /// Rate limited or the server is busy: try again later.
    Busy,
    /// Offline, or the request failed.
    Failed(String),
}

pub trait Net: Send + Sync {
    fn get(&self, url: &str) -> Result<Vec<u8>, NetError>;
}

pub trait Voice: Send + Sync {
    /// Read `text` aloud into a WAV file. `Ok(None)` when the computer has no voice for that language.
    fn speak(&self, lang: &str, text: &str, out: &Path) -> Result<Option<String>, String>;
}

pub(crate) fn hidden(cmd: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = cmd;
}

pub(crate) fn system_tool(windows_rel: &str, other: &str) -> PathBuf {
    match std::env::var_os("SystemRoot").map(|root| Path::new(&root).join(windows_rel)) {
        Some(p) if cfg!(windows) && p.is_file() => p,
        _ => PathBuf::from(other),
    }
}

/// Downloads through the system's own curl (part of Windows 10 and later, macOS and Linux), so the app needs no
/// HTTP library. Requests are spaced out, as Wikimedia asks of automated clients.
pub struct Curl {
    pub tmp_dir: PathBuf,
    pub user_agent: String,
}

impl Curl {
    pub fn new(tmp_dir: PathBuf) -> Self {
        Curl {
            tmp_dir,
            user_agent: format!("SelfSemester/{} (https://github.com/munitkala/selfsemester; lesson audio)", env!("CARGO_PKG_VERSION")),
        }
    }
}

impl Net for Curl {
    fn get(&self, url: &str) -> Result<Vec<u8>, NetError> {
        std::thread::sleep(Duration::from_millis(800));
        std::fs::create_dir_all(&self.tmp_dir).map_err(|e| NetError::Failed(e.to_string()))?;
        let tmp = self.tmp_dir.join(format!("audio-{}.part", std::process::id()));
        let mut cmd = std::process::Command::new(system_tool(r"System32\curl.exe", "curl"));
        cmd.args(["-sS", "-L", "--proto", "=https", "--max-time", "60", "--max-filesize"])
            .arg(MAX_BYTES.to_string())
            .args(["-A", &self.user_agent, "-o"])
            .arg(&tmp)
            .args(["-w", "%{http_code}", url]);
        hidden(&mut cmd);
        let out = cmd.output().map_err(|e| NetError::Failed(format!("curl: {e}")))?;
        let code = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let body = std::fs::read(&tmp).unwrap_or_default();
        let _ = std::fs::remove_file(&tmp);
        match code.as_str() {
            "200" => Ok(body),
            "429" | "503" => Err(NetError::Busy),
            _ if !out.status.success() => Err(NetError::Failed(String::from_utf8_lossy(&out.stderr).trim().to_string())),
            other => Err(NetError::Failed(format!("HTTP {other}"))),
        }
    }
}

/// The computer's own text-to-speech voices: Windows' built-in voices, `say` on macOS, espeak-ng elsewhere.
pub struct SystemVoice {
    pub tmp_dir: PathBuf,
}

/// Windows: reads the text with an installed voice for the language (OneCore voices, the same ones Narrator uses).
const WINDOWS_TTS: &str = r#"param([string]$Lang, [string]$TextFile, [string]$Out)
$ErrorActionPreference = 'Stop'
$text = [IO.File]::ReadAllText($TextFile, [Text.Encoding]::UTF8)
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$null = [Windows.Media.SpeechSynthesis.SpeechSynthesizer, Windows.Media.SpeechSynthesis, ContentType = WindowsRuntime]
$null = [Windows.Storage.Streams.DataReader, Windows.Storage.Streams, ContentType = WindowsRuntime]
function Await($op, [Type]$t) {
  $m = [System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object { $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1' } | Select-Object -First 1
  $task = $m.MakeGenericMethod($t).Invoke($null, @($op)); $task.Wait(); $task.Result
}
$voices = [Windows.Media.SpeechSynthesis.SpeechSynthesizer]::AllVoices
$v = $voices | Where-Object { $_.Language -eq $Lang } | Select-Object -First 1
if (-not $v) { $p = $Lang.Split('-')[0]; $v = $voices | Where-Object { $_.Language.Split('-')[0] -eq $p } | Select-Object -First 1 }
if (-not $v) { Write-Output 'NOVOICE'; exit 0 }
$s = New-Object Windows.Media.SpeechSynthesis.SpeechSynthesizer
$s.Voice = $v
$stream = Await ($s.SynthesizeTextToStreamAsync($text)) ([Windows.Media.SpeechSynthesis.SpeechSynthesisStream])
$r = New-Object Windows.Storage.Streams.DataReader($stream.GetInputStreamAt(0))
$n = Await ($r.LoadAsync([uint32]$stream.Size)) ([uint32])
$bytes = New-Object byte[] $n; $r.ReadBytes($bytes)
[IO.File]::WriteAllBytes($Out, $bytes)
Write-Output ('VOICE ' + $v.DisplayName)
"#;

impl Voice for SystemVoice {
    fn speak(&self, lang: &str, text: &str, out: &Path) -> Result<Option<String>, String> {
        std::fs::create_dir_all(&self.tmp_dir).map_err(|e| e.to_string())?;
        if cfg!(windows) {
            let script = self.tmp_dir.join("speak.ps1");
            let text_file = self.tmp_dir.join("speak.txt");
            std::fs::write(&script, WINDOWS_TTS).map_err(|e| e.to_string())?;
            std::fs::write(&text_file, text).map_err(|e| e.to_string())?;
            let mut cmd = std::process::Command::new(system_tool(r"System32\WindowsPowerShell\v1.0\powershell.exe", "powershell"));
            cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
                .arg(&script)
                .args(["-Lang", lang, "-TextFile"])
                .arg(&text_file)
                .arg("-Out")
                .arg(out);
            hidden(&mut cmd);
            let o = cmd.output().map_err(|e| e.to_string())?;
            let said = String::from_utf8_lossy(&o.stdout).to_string();
            if let Some(name) = said.lines().find_map(|l| l.trim().strip_prefix("VOICE ")) {
                return Ok(Some(name.trim().to_string()));
            }
            if said.contains("NOVOICE") {
                return Ok(None);
            }
            return Err(String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or("speech failed").to_string());
        }
        if cfg!(target_os = "macos") {
            let list = std::process::Command::new("say").args(["-v", "?"]).output().map_err(|e| e.to_string())?;
            let want = lang.replace('-', "_").to_ascii_lowercase();
            let prefix = want.split('_').next().unwrap_or("").to_string();
            let voices = String::from_utf8_lossy(&list.stdout).to_string();
            let pick = |exact: bool| {
                voices.lines().find_map(|l| {
                    let (name, rest) = l.split_once("  ")?;
                    let code = rest.split_whitespace().next()?.to_ascii_lowercase();
                    let hit = if exact { code == want } else { code.split('_').next() == Some(prefix.as_str()) };
                    hit.then(|| name.trim().to_string())
                })
            };
            let Some(name) = pick(true).or_else(|| pick(false)) else { return Ok(None) };
            let o = std::process::Command::new("say")
                .args(["-v", &name, "--file-format=WAVE", "--data-format=LEI16@22050", "-o"])
                .arg(out)
                .arg(text)
                .output()
                .map_err(|e| e.to_string())?;
            return if o.status.success() { Ok(Some(name)) } else { Err("speech failed".into()) };
        }
        match std::process::Command::new("espeak-ng").args(["-v", lang, "-w"]).arg(out).arg(text).output() {
            Ok(o) if o.status.success() => Ok(Some(format!("espeak-ng ({lang})"))),
            _ => Ok(None),
        }
    }
}

// ---------- Wikimedia Commons ----------

#[derive(Debug, Clone, PartialEq)]
struct Recording {
    title: String,
    url: String,
    page: String,
    licence: String,
    artist: String,
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ").replace("&amp;", "&").replace("&#039;", "'").replace("&quot;", "\"")
}

/// Lingua Libre's author field reads "Speaker: X Recorder: Y"; the speaker is who you hear.
fn credit_name(artist: &str) -> String {
    match artist.strip_prefix("Speaker:") {
        Some(rest) => {
            let speaker = rest.split(" Recorder:").next().unwrap_or(rest).trim();
            format!("{speaker} (Lingua Libre)")
        }
        None => artist.to_string(),
    }
}

/// Licences that let anyone reuse a recording: public domain, CC0, CC BY and CC BY-SA. Not NC or ND, not fair use.
pub fn free_licence(short: &str) -> bool {
    let l = short.to_ascii_lowercase();
    let words: Vec<&str> = l.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).collect();
    if words.iter().any(|w| matches!(*w, "nc" | "nd" | "noncommercial" | "nonderivative" | "fair")) || l.contains("non-commercial") {
        return false;
    }
    l.starts_with("cc0")
        || l.starts_with("cc by")
        || l.starts_with("cc-by")
        || l.starts_with("public domain")
        || l.starts_with("pd")
        || l.starts_with("attribution")
        || l.starts_with("gfdl")
}

fn api_url(params: &[(&str, &str)]) -> String {
    let mut q = String::from(COMMONS_API);
    q.push_str("?format=json&formatversion=2");
    for (k, v) in params {
        q.push('&');
        q.push_str(k);
        q.push('=');
        q.push_str(&encode(v));
    }
    q
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn json(net: &dyn Net, url: &str) -> Result<Value, NetError> {
    let body = net.get(url)?;
    serde_json::from_slice(&body).map_err(|e| NetError::Failed(format!("bad reply from Commons: {e}")))
}

/// MediaWiki's form of a file title: underscores as spaces, first letter upper case, no "File:".
fn norm_title(t: &str) -> String {
    let t = t.trim().trim_start_matches("File:").trim_start_matches("file:").replace('_', " ");
    let mut c = t.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Look up files by title and keep the usable ones, in the order asked.
fn usable(net: &dyn Net, titles: &[String]) -> Result<Vec<Recording>, NetError> {
    let mut found: BTreeMap<String, Recording> = BTreeMap::new();
    for chunk in titles.chunks(40) {
        let joined = chunk.iter().map(|t| format!("File:{t}")).collect::<Vec<_>>().join("|");
        let v = json(net, &api_url(&[("action", "query"), ("prop", "imageinfo"), ("iiprop", "url|extmetadata|size|mime"), ("titles", &joined)]))?;
        for p in v["query"]["pages"].as_array().into_iter().flatten() {
            let Some(ii) = p["imageinfo"].get(0) else { continue };
            let mime = ii["mime"].as_str().unwrap_or("");
            let seconds = ii["duration"].as_f64().unwrap_or(0.0);
            let bytes = ii["size"].as_u64().unwrap_or(0);
            if !(mime.starts_with("audio/") || mime == "application/ogg") || seconds > MAX_SECONDS || bytes > MAX_BYTES {
                continue;
            }
            let meta = &ii["extmetadata"];
            let field = |k: &str| strip_tags(meta[k]["value"].as_str().unwrap_or(""));
            let licence = field("LicenseShortName");
            if !free_licence(&licence) {
                continue;
            }
            let title = norm_title(p["title"].as_str().unwrap_or(""));
            let artist = credit_name(&field("Artist"));
            found.insert(
                title.clone(),
                Recording {
                    title,
                    url: ii["url"].as_str().unwrap_or("").to_string(),
                    // shown as a link with the credits, so only ever a Commons page
                    page: ii["descriptionurl"].as_str().filter(|u| u.starts_with("https://commons.wikimedia.org/")).unwrap_or("").to_string(),
                    licence,
                    artist: if artist.is_empty() { "Wikimedia Commons contributor".into() } else { artist },
                },
            );
        }
    }
    Ok(titles.iter().filter_map(|t| found.get(&norm_title(t)).cloned()).filter(|r| r.url.starts_with("https://")).collect())
}

fn search(net: &dyn Net, query: &str, limit: usize) -> Result<Vec<String>, NetError> {
    let n = limit.to_string();
    let v = json(net, &api_url(&[("action", "query"), ("list", "search"), ("srnamespace", "6"), ("srlimit", &n), ("srsearch", query)]))?;
    Ok(v["query"]["search"].as_array().into_iter().flatten().filter_map(|x| x["title"].as_str().map(norm_title)).collect())
}

/// ISO 639-3 codes Lingua Libre puts in its file names, for the languages most often studied.
fn iso3(lang: &str) -> Option<&'static str> {
    let l = lang.to_ascii_lowercase();
    let p = l.split(['-', '_']).next().unwrap_or("");
    if p == "yue" || l == "zh-hk" || l.starts_with("zh-yue") {
        return Some("yue");
    }
    Some(match p {
        "zh" | "cmn" => "cmn",
        "ja" => "jpn",
        "ko" => "kor",
        "fr" => "fra",
        "de" => "deu",
        "es" => "spa",
        "it" => "ita",
        "pt" => "por",
        "ru" => "rus",
        "ar" => "ara",
        "hi" => "hin",
        "en" => "eng",
        "nl" => "nld",
        "pl" => "pol",
        "tr" => "tur",
        "vi" => "vie",
        "sv" => "swe",
        "el" => "ell",
        "he" => "heb",
        "id" => "ind",
        "th" => "tha",
        "uk" => "ukr",
        "cs" => "ces",
        "fi" => "fin",
        "da" => "dan",
        "nb" | "no" => "nob",
        "ca" => "cat",
        "eu" => "eus",
        "bn" => "ben",
        "ta" => "tam",
        "ur" => "urd",
        "fa" => "fas",
        "sw" => "swa",
        "hu" => "hun",
        "ro" => "ron",
        "eo" => "epo",
        "la" => "lat",
        _ => return None,
    })
}

/// File names Wiktionary uses for pronunciations: "Zh-nǐ.ogg", "Fr-bonjour.ogg", "En-us-hello.ogg".
fn conventional_titles(clip: &Clip, lang: &str) -> Vec<String> {
    let lower = lang.to_ascii_lowercase();
    let p = lower.split(['-', '_']).next().unwrap_or("").to_string();
    if p.is_empty() {
        return vec![];
    }
    let cap = norm_title(&p);
    let mut out = Vec::new();
    for t in [clip.say.trim(), clip.romanization.trim()] {
        if t.is_empty() || t.contains(['/', '|', '[', ']', '{', '}', '#', '<', '>']) {
            continue;
        }
        if p == "en" {
            for region in ["us", "uk", "au"] {
                out.push(format!("En-{region}-{t}.ogg"));
            }
        }
        out.push(format!("{cap}-{t}.ogg"));
        out.push(format!("{cap} {t}.ogg"));
        if t.contains(' ') {
            out.push(format!("{cap}-{}.ogg", t.replace(' ', "")));
        }
        out.push(format!("{cap}-{t}.oga"));
    }
    out.dedup();
    out
}

fn find_speech(net: &dyn Net, clip: &Clip, lang: &str) -> Result<Option<Recording>, NetError> {
    let mut titles: Vec<String> = Vec::new();
    if !clip.commons.trim().is_empty() {
        titles.push(norm_title(&clip.commons));
    }
    titles.extend(conventional_titles(clip, lang));
    if let Some(r) = usable(net, &titles)?.into_iter().next() {
        return Ok(Some(r));
    }
    // Lingua Libre: "LL-Q9192 (cmn)-Speaker-你好.wav"
    let (Some(code), say) = (iso3(lang), clip.say.trim()) else { return Ok(None) };
    if say.is_empty() || say.contains('"') {
        return Ok(None);
    }
    let tag = format!("({code})-");
    let hits: Vec<String> = search(net, &format!("intitle:\"{say}\" intitle:LL"), 20)?
        .into_iter()
        .filter(|t| {
            let Some((stem, ext)) = t.rsplit_once('.') else { return false };
            t.starts_with("LL-Q")
                && t.contains(&tag)
                && matches!(ext.to_ascii_lowercase().as_str(), "wav" | "ogg" | "oga" | "flac" | "mp3" | "opus")
                && stem.rsplit_once('-').is_some_and(|(_, word)| word == say)
        })
        .collect();
    let mut hits = hits;
    hits.sort_by_key(|t| variety_rank(t, lang));
    Ok(usable(net, &hits)?.into_iter().next())
}

/// Lingua Libre tags some varieties with their own item, all with the same language code: Taiwanese Mandarin is
/// "LL-Q727694 (cmn)", mainland Mandarin "LL-Q9192 (cmn)". Prefer the variety the course teaches.
fn variety_rank(title: &str, lang: &str) -> u8 {
    let l = lang.to_ascii_lowercase();
    let taiwan = title.starts_with("LL-Q727694 ");
    match iso3(lang) {
        Some("cmn") if l.ends_with("-tw") || l.contains("hant") => u8::from(!taiwan),
        Some("cmn") => u8::from(taiwan),
        _ => 0,
    }
}

fn find_sound(net: &dyn Net, clip: &Clip) -> Result<Option<Recording>, NetError> {
    let mut titles: Vec<String> = Vec::new();
    if !clip.commons.trim().is_empty() {
        titles.push(norm_title(&clip.commons));
    }
    if let Some(r) = usable(net, &titles)?.into_iter().next() {
        return Ok(Some(r));
    }
    let q = clip.sound.replace('"', " ");
    let hits = search(net, &format!("{} filetype:audio", q.trim()), 10)?;
    Ok(usable(net, &hits)?.into_iter().next())
}

// ---------- notes, made here ----------

fn note_hz(name: &str) -> Option<f64> {
    let n = name.trim();
    let mut chars = n.chars();
    let letter = chars.next()?.to_ascii_uppercase();
    let base = match letter {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let rest: String = chars.collect();
    let (shift, octave) = match rest.chars().next() {
        Some('#') | Some('♯') => (1, &rest[rest.chars().next().unwrap().len_utf8()..]),
        Some('b') | Some('♭') => (-1, &rest[rest.chars().next().unwrap().len_utf8()..]),
        _ => (0, rest.as_str()),
    };
    let octave: i32 = octave.parse().ok()?;
    if !(0..=8).contains(&octave) {
        return None;
    }
    let midi = 12 * (octave + 1) + base + shift;
    Some(440.0 * 2f64.powf((midi as f64 - 69.0) / 12.0))
}

/// A soft, piano-like tone (a few decaying harmonics) for each note, or all of them at once for a chord.
pub fn synth_wav(clip: &Clip) -> Result<Vec<u8>, String> {
    const RATE: u32 = 22050;
    let notes: Vec<Option<f64>> = clip
        .notes
        .iter()
        .map(|n| {
            let t = n.trim().to_ascii_lowercase();
            if t == "rest" || t == "-" { Ok(None) } else { note_hz(n).map(Some).ok_or(format!("unknown note \"{n}\"")) }
        })
        .collect::<Result<_, _>>()?;
    if notes.is_empty() || notes.len() > 64 {
        return Err("A clip needs 1 to 64 notes.".into());
    }
    let each = if clip.seconds > 0.05 && clip.seconds <= 8.0 { clip.seconds } else if clip.chord { 1.8 } else { 0.6 };
    let groups: Vec<Vec<f64>> =
        if clip.chord { vec![notes.iter().flatten().copied().collect()] } else { notes.iter().map(|n| n.iter().copied().collect()).collect() };
    let tail = 0.5;
    let total = ((groups.len() as f64 * each + tail) * RATE as f64) as usize;
    let mut buf = vec![0f64; total];
    for (g, freqs) in groups.iter().enumerate() {
        let start = (g as f64 * each * RATE as f64) as usize;
        let len = ((each + tail) * RATE as f64) as usize;
        for &f in freqs {
            for i in 0..len {
                let t = i as f64 / RATE as f64;
                let attack = (t / 0.008).min(1.0);
                let release = if t > each { (1.0 - (t - each) / tail).max(0.0) } else { 1.0 };
                let env = attack * release * (-3.0 * t).exp();
                let mut s = 0.0;
                for h in 1..=6 {
                    let hf = f * h as f64;
                    if hf < RATE as f64 / 2.0 {
                        s += (2.0 * std::f64::consts::PI * hf * t).sin() * (-(h as f64 - 1.0) * 0.9 - t * h as f64 * 0.8).exp();
                    }
                }
                if let Some(x) = buf.get_mut(start + i) {
                    *x += s * env;
                }
            }
        }
    }
    let peak = buf.iter().fold(0f64, |m, x| m.max(x.abs())).max(1e-9);
    let samples: Vec<i16> = buf.iter().map(|x| (x / peak * 0.8 * i16::MAX as f64) as i16).collect();
    Ok(wav(&samples, RATE))
}

fn wav(samples: &[i16], rate: u32) -> Vec<u8> {
    let data = samples.len() as u32 * 2;
    let mut v = Vec::with_capacity(44 + data as usize);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes()); // PCM
    v.extend_from_slice(&1u16.to_le_bytes()); // mono
    v.extend_from_slice(&rate.to_le_bytes());
    v.extend_from_slice(&(rate * 2).to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        v.extend_from_slice(&s.to_le_bytes());
    }
    v
}

// ---------- filling a course's lessons ----------

/// A short, stable name for a made clip's file.
fn hash(s: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn safe_file_name(title: &str) -> String {
    let c: String = title.chars().map(|c| if r#"<>:"/\|?*"#.contains(c) || c.is_control() { '-' } else { c }).collect();
    let c = c.trim().trim_end_matches('.').to_string();
    if c.chars().count() > 100 {
        let ext = Path::new(&c).extension().and_then(|e| e.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
        format!("{}{ext}", hash(&c))
    } else {
        c
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct Pass {
    /// Some lesson got new audio (the UI should refresh).
    pub changed: bool,
    /// Commons asked us to slow down, or the network is down: stop and try later.
    pub stalled: bool,
}

/// Fill one clip. `Err` means "couldn't tell this time" (offline, busy), so nothing is recorded and it's retried.
fn fill(course_dir: &Path, clip: &Clip, net: Option<&dyn Net>, voice: &dyn Voice) -> Result<Found, NetError> {
    let mut f = Found { sig: sig(clip), status: "ok".into(), ..Default::default() };
    let audio_dir = course_dir.join(DIR);
    std::fs::create_dir_all(&audio_dir).map_err(|e| NetError::Failed(e.to_string()))?;
    let lang = clip.lang.trim();
    if !clip.notes.is_empty() {
        let name = format!("notes-{}.wav", hash(&f.sig));
        match synth_wav(clip) {
            Ok(bytes) => {
                std::fs::write(audio_dir.join(&name), bytes).map_err(|e| NetError::Failed(e.to_string()))?;
                f.file = format!("{DIR}/{name}");
                f.source = "synth".into();
                f.credit = "Synthesised by SelfSemester".into();
            }
            Err(e) => {
                f.status = "missing".into();
                f.note = e;
            }
        }
        return Ok(f);
    }
    let speech = !clip.say.trim().is_empty();
    if !speech && clip.sound.trim().is_empty() {
        f.status = "missing".into();
        f.note = "The lesson didn't say what to play.".into();
        return Ok(f);
    }
    let mut net_problem: Option<NetError> = None;
    if let Some(net) = net {
        let found = if speech { find_speech(net, clip, lang) } else { find_sound(net, clip) };
        match found {
            Ok(Some(r)) => {
                let name = safe_file_name(&r.title);
                let path = audio_dir.join(&name);
                if !path.is_file() {
                    let bytes = net.get(&r.url)?;
                    std::fs::write(&path, bytes).map_err(|e| NetError::Failed(e.to_string()))?;
                }
                f.file = format!("{DIR}/{name}");
                f.source = "commons".into();
                f.credit = r.artist;
                f.licence = r.licence;
                f.url = r.page;
                return Ok(f);
            }
            Ok(None) => {}
            Err(e) => net_problem = Some(e),
        }
    }
    if speech && !lang.is_empty() {
        let name = format!("voice-{}.wav", hash(&f.sig));
        let path = audio_dir.join(&name);
        match voice.speak(lang, clip.say.trim(), &path) {
            Ok(Some(v)) if path.is_file() => {
                f.file = format!("{DIR}/{name}");
                f.source = "voice".into();
                f.credit = format!("Computer voice: {v}");
                return Ok(f);
            }
            Ok(_) => {}
            Err(e) => eprintln!("speech for audio clip failed: {e}"),
        }
    }
    // Couldn't reach Commons: leave the clip for a later pass rather than record it as missing.
    if let Some(e) = net_problem {
        return Err(e);
    }
    if net.is_none() {
        return Err(NetError::Failed("online lookups are off".into()));
    }
    f.status = "missing".into();
    f.note = if speech {
        format!("No free recording was found, and this computer has no voice for {lang}. Windows can add one in Settings, Time and language, Speech.")
    } else {
        "No freely licensed recording was found.".into()
    };
    Ok(f)
}

/// Fill the clips of one lesson that aren't filled yet. Saves after each clip so players appear as they're found.
pub fn fill_lesson(course_dir: &Path, key: &str, net: Option<&dyn Net>, voice: &dyn Voice) -> Pass {
    let mut pass = Pass::default();
    let Ok(md) = std::fs::read_to_string(course_dir.join(format!("items/{key}-lesson.md"))) else { return pass };
    let wanted = clips(&md);
    let mut found = load(course_dir, key);
    let before = found.len();
    found.retain(|id, _| wanted.iter().any(|(w, _)| w == id));
    let mut dirty = found.len() != before;
    for (id, clip) in &wanted {
        if let Some(f) = found.get(id) {
            let file_ok = f.status != "ok" || course_dir.join(&f.file).is_file();
            if f.sig == sig(clip) && file_ok {
                continue;
            }
        }
        match fill(course_dir, clip, net, voice) {
            Ok(f) => {
                found.insert(id.clone(), f);
                dirty = true;
                pass.changed = true;
                let _ = save(course_dir, key, &found);
            }
            Err(NetError::Busy) => {
                pass.stalled = true;
                break;
            }
            Err(NetError::Failed(e)) => {
                if net.is_some() {
                    eprintln!("lesson audio: {e}");
                    pass.stalled = true;
                    break;
                }
            }
        }
    }
    if dirty {
        let _ = save(course_dir, key, &found);
    }
    pass
}

/// Fill every lesson of every course, oldest course folder first.
pub fn fill_all(paths: &AppPaths, net: Option<&dyn Net>, voice: &dyn Voice) -> Pass {
    let mut pass = Pass::default();
    let Ok(courses) = std::fs::read_dir(paths.courses_dir()) else { return pass };
    let mut dirs: Vec<PathBuf> = courses.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    for dir in dirs {
        let Ok(items) = std::fs::read_dir(dir.join("items")) else { continue };
        let mut keys: Vec<String> = items
            .flatten()
            .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix("-lesson.md")).map(str::to_string))
            .collect();
        keys.sort();
        for key in keys {
            let p = fill_lesson(&dir, &key, net, voice);
            pass.changed |= p.changed;
            if p.stalled {
                pass.stalled = true;
                return pass;
            }
        }
    }
    pass
}

/// Background thread that keeps lesson audio filled. `online` is read before each pass.
pub fn spawn(paths: AppPaths, online: impl Fn() -> bool + Send + 'static, changed: impl Fn() + Send + 'static) {
    let tmp = paths.claude_workdir().join("audio-tmp");
    std::thread::Builder::new()
        .name("selfsemester-audio".into())
        .spawn(move || {
            let net = Curl::new(tmp.clone());
            let voice = SystemVoice { tmp_dir: tmp };
            std::thread::sleep(Duration::from_secs(5));
            loop {
                let on = online();
                let pass = fill_all(&paths, if on { Some(&net as &dyn Net) } else { None }, &voice);
                if pass.changed {
                    changed();
                }
                std::thread::sleep(if pass.stalled { Duration::from_secs(600) } else { Duration::from_secs(60) });
            }
        })
        .expect("start audio thread");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    const LESSON: &str = "# Tones\n\nSay them.\n\n```audio\ncaption: The four tones on ma\n{\"lang\": \"zh-CN\", \"clips\": [\
        {\"say\": \"妈\", \"romanization\": \"mā\", \"label\": \"1st\"}, {\"say\": \"马\", \"romanization\": \"mǎ\"}]}\n```\n\n\
        Text.\n\n```audio\n{\"test\": true, \"lang\": \"zh-CN\", \"clips\": [{\"say\": \"马\", \"romanization\": \"mǎ\"}]}\n```\n\n\
        ```audio\n[{\"notes\": [\"C4\", \"E4\", \"G4\"], \"chord\": true, \"label\": \"C major\"}]\n```\n\n\
        ```audio\nnot json\n```\n\n```audio\n[{\"sound\": \"European robin song\"}]\n```\n";

    #[test]
    fn reads_blocks_and_numbers_clips_like_the_ui() {
        let b = blocks(LESSON);
        assert_eq!(b.len(), 5);
        assert_eq!(b[0].caption, "The four tones on ma");
        assert_eq!(b[0].clips[1].lang, "zh-CN", "the block's language reaches every clip");
        assert_eq!(b[1].clips[0].lang, "zh-CN", "a listening test block reads like any other");
        assert!(b[3].clips.is_empty(), "a broken block keeps its place");
        let ids: Vec<String> = clips(LESSON).into_iter().map(|(id, _)| id).collect();
        assert_eq!(ids, ["b0c0", "b0c1", "b1c0", "b2c0", "b4c0"]);
    }

    #[test]
    fn only_free_licences_pass() {
        for ok in ["CC0", "CC BY 2.0 fr", "CC BY-SA 3.0 us", "CC BY-SA 4.0", "Public domain", "PD-self", "GFDL"] {
            assert!(free_licence(ok), "{ok}");
        }
        for no in ["CC BY-NC 4.0", "CC BY-ND 2.0", "CC BY-NC-SA 3.0", "Fair use", "", "All rights reserved"] {
            assert!(!free_licence(no), "{no}");
        }
    }

    #[test]
    fn wiktionary_style_names_come_first() {
        let c = Clip { say: "你好".into(), romanization: "nǐ hǎo".into(), ..Default::default() };
        let t = conventional_titles(&c, "zh-CN");
        assert!(t.contains(&"Zh-nǐ hǎo.ogg".to_string()));
        assert!(t.contains(&"Zh nǐ hǎo.ogg".to_string()));
        assert!(t.contains(&"Zh-nǐhǎo.ogg".to_string()));
        let e = conventional_titles(&Clip { say: "water".into(), ..Default::default() }, "en-GB");
        assert_eq!(e[0], "En-us-water.ogg");
        assert_eq!(iso3("zh-CN"), Some("cmn"));
        assert_eq!(iso3("zh-HK"), Some("yue"));
        assert!(variety_rank("LL-Q9192 (cmn)-A-鸡.wav", "zh-CN") < variety_rank("LL-Q727694 (cmn)-B-鸡.wav", "zh-CN"));
        assert!(variety_rank("LL-Q727694 (cmn)-B-鸡.wav", "zh-TW") < variety_rank("LL-Q9192 (cmn)-A-鸡.wav", "zh-TW"));
        assert_eq!(credit_name("Speaker: SpeakerA Recorder: SpeakerA"), "SpeakerA (Lingua Libre)");
        assert_eq!(credit_name("Sample Author"), "Sample Author");
    }

    #[test]
    fn notes_become_a_wav_file() {
        assert!((note_hz("A4").unwrap() - 440.0).abs() < 1e-9);
        assert!((note_hz("C4").unwrap() - 261.6256).abs() < 1e-3);
        assert!((note_hz("Bb3").unwrap() - note_hz("A#3").unwrap()).abs() < 1e-9);
        let w = synth_wav(&Clip { notes: vec!["C4".into(), "rest".into(), "G4".into()], ..Default::default() }).unwrap();
        assert_eq!(&w[0..4], b"RIFF");
        assert_eq!(&w[8..12], b"WAVE");
        let seconds = (w.len() - 44) as f64 / 2.0 / 22050.0;
        assert!((seconds - 2.3).abs() < 0.01, "{seconds}");
        assert!(synth_wav(&Clip { notes: vec!["H9".into()], ..Default::default() }).is_err());
    }

    #[test]
    fn audio_paths_stay_in_the_audio_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = AppPaths::rooted(tmp.path(), tmp.path());
        let dir = paths.course_dir("c");
        std::fs::create_dir_all(dir.join(DIR)).unwrap();
        std::fs::write(dir.join("audio/a.ogg"), b"x").unwrap();
        assert!(path_of(&paths, "c", "audio/a.ogg").is_ok());
        for bad in ["../app.db", "audio/../units/u01.json", "files/a.ogg", "audio/x/a.ogg", "C:/Windows/win.ini"] {
            assert!(path_of(&paths, "c", bad).is_err(), "{bad}");
        }
    }

    /// Answers like Commons: one Wiktionary-style recording exists, Lingua Libre has 你好, robins sing.
    struct FakeCommons {
        calls: Mutex<Vec<String>>,
        busy: bool,
    }

    fn page(title: &str, licence: &str) -> Value {
        serde_json::json!({"title": format!("File:{title}"), "imageinfo": [{
            "url": format!("https://upload.wikimedia.org/x/{}", encode(title)), "descriptionurl": format!("https://commons.wikimedia.org/wiki/File:{}", encode(title)),
            "mime": "application/ogg", "size": 9000, "duration": 1.1,
            "extmetadata": {"LicenseShortName": {"value": licence}, "Artist": {"value": "<a href=\"x\">Sample Author</a>"}}}]})
    }

    impl Net for FakeCommons {
        fn get(&self, url: &str) -> Result<Vec<u8>, NetError> {
            self.calls.lock().unwrap().push(url.to_string());
            if self.busy {
                return Err(NetError::Busy);
            }
            if url.starts_with("https://upload.wikimedia.org/") {
                return Ok(b"OggS fake".to_vec());
            }
            let has = |t: &str| url.contains(&encode(&format!("File:{t}")));
            let mut pages = vec![];
            if has("Zh-mǎ.ogg") {
                pages.push(page("Zh-mǎ.ogg", "CC BY 2.0 fr"));
            }
            if has("Zh-mā.ogg") {
                pages.push(page("Zh-mā.ogg", "CC BY-NC 4.0")); // not free: must be skipped
            }
            if has("LL-Q9192 (cmn)-Speaker-妈.wav") {
                pages.push(page("LL-Q9192 (cmn)-Speaker-妈.wav", "CC0"));
            }
            if has("Robin.ogg") {
                pages.push(page("Robin.ogg", "CC BY-SA 4.0"));
            }
            if url.contains("list=search") {
                let titles: Vec<&str> = if url.contains(&encode("妈")) {
                    vec!["LL-Q9186 (yue)-Other-妈.wav", "LL-Q9192 (cmn)-Speaker-妈妈.wav", "LL-Q9192 (cmn)-Speaker-妈.wav"]
                } else if url.contains("robin") {
                    vec!["Robin.ogg"]
                } else {
                    vec![]
                };
                let s: Vec<Value> = titles.iter().map(|t| serde_json::json!({"title": format!("File:{t}")})).collect();
                return Ok(serde_json::to_vec(&serde_json::json!({"query": {"search": s}})).unwrap());
            }
            Ok(serde_json::to_vec(&serde_json::json!({"query": {"pages": pages}})).unwrap())
        }
    }

    struct FakeVoice(Option<&'static str>);
    impl Voice for FakeVoice {
        fn speak(&self, _lang: &str, _text: &str, out: &Path) -> Result<Option<String>, String> {
            match self.0 {
                Some(v) => {
                    std::fs::write(out, wav(&[0, 1, 0], 22050)).unwrap();
                    Ok(Some(v.into()))
                }
                None => Ok(None),
            }
        }
    }

    fn course_with(lesson: &str) -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("courses").join("c");
        std::fs::create_dir_all(dir.join("items")).unwrap();
        std::fs::write(dir.join("items/u01-lesson.md"), lesson).unwrap();
        (tmp, dir)
    }

    #[test]
    fn fills_clips_from_commons_then_the_computer_voice() {
        let md = "```audio\n{\"lang\": \"zh-CN\", \"clips\": [{\"say\": \"马\", \"romanization\": \"mǎ\"}, {\"say\": \"妈\", \"romanization\": \"mā\"}, \
                  {\"say\": \"骂\", \"romanization\": \"mà\"}, {\"notes\": [\"A4\"]}, {\"sound\": \"European robin song\"}]}\n```\n";
        let (_t, dir) = course_with(md);
        let net = FakeCommons { calls: Mutex::new(vec![]), busy: false };
        let pass = fill_lesson(&dir, "u01", Some(&net), &FakeVoice(Some("Test Voice")));
        assert!(pass.changed && !pass.stalled);
        let f = load(&dir, "u01");
        assert_eq!(f["b0c0"].source, "commons");
        assert_eq!(f["b0c0"].credit, "Sample Author", "HTML is stripped from the author");
        assert_eq!(f["b0c0"].file, "audio/Zh-mǎ.ogg");
        assert!(dir.join("audio/Zh-mǎ.ogg").is_file());
        assert_eq!(f["b0c1"].file, "audio/LL-Q9192 (cmn)-Speaker-妈.wav", "the NC file is skipped for a Lingua Libre one");
        assert_eq!(f["b0c2"].source, "voice", "nothing on Commons, so the computer reads it");
        assert!(f["b0c2"].credit.contains("Test Voice"));
        assert_eq!(f["b0c3"].source, "synth");
        assert_eq!(f["b0c4"].licence, "CC BY-SA 4.0");

        // A second pass has nothing to do and asks Commons nothing.
        let n = net.calls.lock().unwrap().len();
        assert!(!fill_lesson(&dir, "u01", Some(&net), &FakeVoice(None)).changed);
        assert_eq!(net.calls.lock().unwrap().len(), n);

        // Editing a clip refills only that clip; removing one drops its entry.
        std::fs::write(dir.join("items/u01-lesson.md"), md.replace("[\"A4\"]", "[\"C5\"]").replace(", {\"sound\": \"European robin song\"}", "")).unwrap();
        assert!(fill_lesson(&dir, "u01", Some(&net), &FakeVoice(None)).changed);
        let g = load(&dir, "u01");
        assert_ne!(g["b0c3"].file, f["b0c3"].file);
        assert!(!g.contains_key("b0c4"));
        assert_eq!(g["b0c0"], f["b0c0"]);
    }

    /// Live: fills a real lesson from Wikimedia Commons and this computer's voices.
    /// `SS_AUDIO_LESSON=<lesson.md> cargo test --lib -- --ignored live_audio --nocapture` (SS_AUDIO_OUT keeps the files).
    #[test]
    #[ignore]
    fn live_audio() {
        let lesson = std::env::var("SS_AUDIO_LESSON").expect("set SS_AUDIO_LESSON to a lesson .md file");
        let (tmp, dir) = course_with(&std::fs::read_to_string(lesson).unwrap());
        let work = tmp.path().join("work");
        let (net, voice) = (Curl::new(work.clone()), SystemVoice { tmp_dir: work });
        let mut pass = fill_lesson(&dir, "u01", Some(&net), &voice);
        for _ in 0..3 {
            if !pass.stalled {
                break;
            }
            eprintln!("Commons is busy, waiting a minute");
            std::thread::sleep(Duration::from_secs(60));
            pass = fill_lesson(&dir, "u01", Some(&net), &voice);
        }
        let md = std::fs::read_to_string(dir.join("items/u01-lesson.md")).unwrap();
        let found = load(&dir, "u01");
        for (id, clip) in clips(&md) {
            let f = found.get(&id);
            println!(
                "{id:6} {:10} {:12} -> {:8} {} | {} | {}",
                clip.say,
                clip.romanization,
                f.map_or("unfilled", |f| if f.status == "ok" { f.source.as_str() } else { "MISSING" }),
                f.map_or("", |f| f.file.as_str()),
                f.map_or("", |f| f.credit.as_str()),
                f.map_or("", |f| f.licence.as_str())
            );
        }
        if let Ok(out) = std::env::var("SS_AUDIO_OUT") {
            let out = PathBuf::from(out);
            std::fs::create_dir_all(out.join(DIR)).unwrap();
            for e in std::fs::read_dir(dir.join(DIR)).unwrap().flatten() {
                std::fs::copy(e.path(), out.join(DIR).join(e.file_name())).unwrap();
            }
            std::fs::copy(dir.join(sidecar_ref("u01")), out.join("u01-lesson.audio.json")).unwrap();
        }
        assert!(!pass.stalled, "Commons stayed busy");
    }

    #[test]
    fn busy_or_offline_leaves_clips_for_later_and_missing_says_why() {
        let md = "```audio\n{\"lang\": \"zh-CN\", \"clips\": [{\"say\": \"骂\"}]}\n```\n";
        let (_t, dir) = course_with(md);
        let busy = FakeCommons { calls: Mutex::new(vec![]), busy: true };
        let p = fill_lesson(&dir, "u01", Some(&busy), &FakeVoice(None));
        assert!(p.stalled && !p.changed);
        assert!(load(&dir, "u01").is_empty(), "nothing is recorded while Commons is busy");

        // Online lookups off and no voice: still nothing recorded, so it's tried again once they're on.
        assert!(!fill_lesson(&dir, "u01", None, &FakeVoice(None)).changed);
        // Offline but a voice exists: the computer reads it.
        assert_eq!(fill(&dir, &Clip { say: "骂".into(), lang: "zh-CN".into(), ..Default::default() }, None, &FakeVoice(Some("V"))).unwrap().source, "voice");

        let net = FakeCommons { calls: Mutex::new(vec![]), busy: false };
        fill_lesson(&dir, "u01", Some(&net), &FakeVoice(None));
        let f = load(&dir, "u01");
        assert_eq!(f["b0c0"].status, "missing");
        assert!(f["b0c0"].note.contains("no voice for zh-CN"));
    }
}
