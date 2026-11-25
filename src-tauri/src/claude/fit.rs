//! "How well can this build a course?" in plain words, shown in Settings and before a new course is planned.
//!
//! Claude Code can do every part of a build. Another service loses web search and opening files by itself, and how
//! good the course is depends on the model. The model's size is a guess from its name, which is rough, so the
//! report says so; the real test is the Check button, which asks the model something and looks at the answer.

use super::Tier;
use super::endpoint::{AiChoice, Api, Endpoint, Roles, Target};
use super::styles::Style;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Fit {
    /// "best", "good", "fair", "limited" or "unknown"
    pub overall: String,
    pub headline: String,
    pub areas: Vec<Area>,
    pub cost: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Area {
    pub id: String,
    pub label: String,
    /// "full", "reduced", "none" or "unknown"
    pub level: String,
    pub note: String,
}

fn area(id: &str, label: &str, level: &str, note: impl Into<String>) -> Area {
    Area { id: id.into(), label: label.into(), level: level.into(), note: note.into() }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    Large,
    Medium,
    Small,
    Unknown,
}

/// Guess how capable a model is from its name. Parameter counts win when the name has one ("qwen2.5-72b",
/// "llama-3.1-8b", "8x7b"); otherwise well-known names are sorted by hand.
pub fn guess_size(model: &str) -> Size {
    let m = model.to_lowercase();
    if let Some(b) = billions(&m) {
        return if b >= 65.0 {
            Size::Large
        } else if b >= 20.0 {
            Size::Medium
        } else {
            Size::Small
        };
    }
    // "gemini" has "mini" in it, which would make every Gemini model look small
    let plain = m.replace("gemini", "");
    let has_plain = |words: &[&str]| words.iter().any(|w| plain.contains(w));
    if has_plain(&["nano", "tiny", "lite"]) {
        return Size::Small;
    }
    if has_plain(&["mini", "flash", "haiku", "small", "air", "turbo"]) {
        return Size::Medium;
    }
    let has = |words: &[&str]| words.iter().any(|w| m.contains(w));
    let large = [
        "gpt-5", "gpt-4.1", "gpt-4o", "o1", "o3", "o4", "opus", "sonnet", "fable", "gemini-3", "-pro", "deepseek",
        "kimi-k2", "glm-4.5", "glm-4.6", "glm-5", "grok-3", "grok-4", "mistral-large", "mistral-medium", "command-a",
        "qwen-max", "qwen3-max", "maverick",
    ];
    if has(&large) {
        return Size::Large;
    }
    Size::Unknown
}

/// The parameter count in a model name, in billions. "30b-a3b" (a mixture of experts) counts as 30.
fn billions(m: &str) -> Option<f64> {
    let bytes = m.as_bytes();
    for (i, _) in m.match_indices('b') {
        // the number must sit right before the "b", and the "b" must end a word
        if bytes.get(i + 1).is_some_and(|c| c.is_ascii_alphanumeric()) {
            continue;
        }
        let start = m[..i].rfind(|c: char| !(c.is_ascii_digit() || c == '.' || c == 'x')).map_or(0, |p| p + 1);
        let num = &m[start..i];
        if num.is_empty() || (start > 0 && bytes[start - 1] == b'a' && start >= 2 && bytes[start - 2] == b'-') {
            continue; // the "a3b" half of "30b-a3b" is the active size, not the whole model
        }
        let value = match num.split_once('x') {
            Some((n, each)) => n.parse::<f64>().ok().zip(each.parse::<f64>().ok()).map(|(n, e)| n * e),
            None => num.parse::<f64>().ok(),
        };
        if let Some(v) = value.filter(|v| *v > 0.0) {
            return Some(v);
        }
    }
    None
}

fn writing(size: Size, what: &str) -> (&'static str, String) {
    match size {
        Size::Large => ("full", format!("A large model, so {what} should come out well.")),
        Size::Medium => (
            "reduced",
            format!("A mid-sized model: {what} should work, but expect thinner content, a few more mistakes, and now and then a step that gets stuck repeating itself and has to be retried."),
        ),
        Size::Small => (
            "reduced",
            format!("A small model. It tends to lose track in long, structured work, so expect {what} to be rough and some steps to fail."),
        ),
        Size::Unknown => ("unknown", "Couldn't tell how capable this model is from its name. Press Check, then try a quick course first.".into()),
    }
}

/// Who does one job, as far as the report cares.
enum Who<'a> {
    Claude,
    Service(&'a Endpoint, String),
}

impl Who<'_> {
    fn name(&self) -> String {
        match self {
            Who::Claude => "Claude Code".into(),
            Who::Service(s, m) if m.is_empty() => s.display_name(),
            Who::Service(s, m) => format!("{} ({m})", s.display_name()),
        }
    }
}

fn who<'a>(choice: &'a AiChoice, target: &Target, tier: Tier) -> Who<'a> {
    match choice.resolve(target, tier) {
        Some((s, m)) => Who::Service(s, m),
        None => Who::Claude,
    }
}

fn size_of(w: &Who) -> Option<Size> {
    match w {
        Who::Claude => None,
        Who::Service(_, m) => Some(guess_size(m)),
    }
}

/// The report for whatever is set up now.
pub fn assess(choice: &AiChoice) -> Fit {
    if !choice.uses_endpoint() {
        return for_claude_code();
    }
    let plan = who(choice, &choice.roles.plan, Tier::Opus);
    let write = who(choice, &choice.roles.write, Tier::Sonnet);
    let research = who(choice, &choice.roles.research, Tier::Sonnet);
    let claude = for_claude_code();
    let claude_area = |id: &str| claude.areas.iter().find(|a| a.id == id).cloned().expect("Claude Code covers every area");

    let job = |id: &str, label: &str, w: &Who, what: &str| match w {
        Who::Claude => claude_area(id),
        Who::Service(_, m) => {
            let (level, note) = writing(guess_size(m), what);
            area(id, label, level, format!("{}: {note}", w.name()))
        }
    };
    let mut areas = vec![
        job("plan", "Planning and exams", &plan, "the syllabus and exams"),
        job("lessons", "Lessons and assignments", &write, "lessons and assignments"),
        format_area(&choice.services_in_use()),
    ];

    let searches = match &research {
        Who::Claude => true,
        Who::Service(s, _) => s.searches_web(),
    };
    areas.push(match &research {
        Who::Claude => claude_area("sources"),
        Who::Service(..) if searches => area("sources", "Reading list", "full", format!("{} searches the web itself for the reading list.", research.name())),
        Who::Service(..) => area(
            "sources",
            "Reading list",
            "reduced",
            format!("{} can't search the web here, so it suggests sources from memory and links aren't checked. Some may be broken.", research.name()),
        ),
    });
    areas.push(match &research {
        Who::Claude => claude_area("research"),
        Who::Service(..) if searches => area("research", "Research while writing", "full", format!("Thorough and Max builds send their research steps to {}, which searches the web.", research.name())),
        Who::Service(..) => area("research", "Research while writing", "none", "Thorough and Max builds can't look anything up; lessons come from what the model already knows."),
    });

    areas.push(match (&write, size_of(&write)) {
        (Who::Claude, _) => claude_area("marking"),
        (_, Some(Size::Large)) => area("marking", "Marking typed work", "full", format!("{} should mark fairly against the rubric. You can still question any mark in Review.", write.name())),
        (_, Some(Size::Unknown)) => area("marking", "Marking typed work", "unknown", "Depends on the model. Check a few marks in Review early on."),
        _ => area("marking", "Marking typed work", "reduced", "Marks may be off now and then. Use Review to question any that look wrong."),
    });

    let (photos, files) = match &write {
        Who::Claude => (claude_area("photos"), claude_area("files")),
        Who::Service(s, _) => service_files(s),
    };
    areas.push(photos);
    areas.push(files);

    let used = choice.services_in_use();
    if used.iter().any(|s| s.is_local() && s.api != Api::Command) {
        areas.push(area(
            "context",
            "Room for long requests",
            "unknown",
            "Local servers often start with a small context window, and course steps send long requests. Set it to at least 32,000 tokens (Ollama: the Context size setting here, or num_ctx; LM Studio: Context Length) or answers get cut off.",
        ));
    }

    let level = |id: &str| areas.iter().find(|a| a.id == id).map(|a| a.level.clone()).unwrap_or_default();
    let sizes: Vec<Size> = [size_of(&plan), size_of(&write)].into_iter().flatten().collect();
    let extras = if searches { "" } else { ", but the reading list isn't checked on the web and there's no research while writing" };
    let (overall, headline) = if level("format") == "none" {
        ("limited", "This setup can't build courses yet. Fix what Check found and try again.".to_string())
    } else if sizes.contains(&Size::Small) {
        ("limited", "It can build a course, but a small model will make a rough one. Use a bigger model for anything you care about.".to_string())
    } else if level("format") == "unknown" {
        ("unknown", "Press Check on each service to see how well this works. Until then, this is a guess from the model names.".to_string())
    } else if sizes.iter().all(|s| *s == Size::Large) {
        ("good", format!("Good. Courses should build well{extras}."))
    } else {
        ("fair", format!("Usable. Expect simpler lessons and exams than with a large model{extras}."))
    };
    Fit { overall: overall.into(), headline, areas, cost: cost(choice) }
}

/// The report for one service doing every job, shown while it's being set up.
pub fn for_endpoint(e: &Endpoint) -> Fit {
    let mut e = e.clone();
    if e.id.is_empty() {
        e.id = "this".into();
    }
    let all = Target { service: e.id.clone(), model: String::new() };
    let choice = AiChoice { services: vec![e.clone()], roles: Roles::every(all), ..Default::default() };
    if let Some(p) = e.problem() {
        return Fit { overall: "limited".into(), headline: format!("Not set up yet. {p}"), areas: vec![], cost: cost(&choice) };
    }
    assess(&choice)
}

/// Whether the app gets answers it can use, from each service's last Check. The worst one counts.
fn format_area(used: &[&Endpoint]) -> Area {
    let label = "Answers the app can use";
    let rank = |a: &Area| match a.level.as_str() {
        "none" => 0,
        "unknown" => 1,
        "reduced" => 2,
        _ => 3,
    };
    used.iter()
        .map(|s| {
            let name = s.display_name();
            match &s.check {
                None => area("format", label, "unknown", format!("{name} hasn't been checked yet. Press Check to try it.")),
                Some(c) if !c.reached => area("format", label, "none", format!("Couldn't reach {name}: {}", c.message)),
                Some(c) if !c.json_ok => area("format", label, "none", format!("{name}: {}", c.message)),
                Some(c) if c.format == "prompt" && s.api.style().is_some_and(|st| st != Style::Anthropic) => area(
                    "format",
                    label,
                    "reduced",
                    format!("{name} worked, but ignores the setting that asks for JSON, so the app asks in words. Some steps may need a second try."),
                ),
                Some(c) => area("format", label, "full", format!("{name} worked when checked ({} s for a short answer).", c.seconds)),
            }
        })
        .min_by_key(rank)
        .unwrap_or_else(|| area("format", label, "full", "Claude Code checks every answer against the shape the app needs."))
}

/// Photos and the learner's own files, for the service that writes and marks.
fn service_files(s: &Endpoint) -> (Area, Area) {
    let pdfs = s.sends_pdfs();
    let pdf_note = if pdfs { "PDFs are sent along too; Word files can't be opened." } else { "PDFs and Word files can't be opened, so hand those in as text or photos." };
    let photos = match (s.sees_images, s.check.as_ref().and_then(|c| c.images_ok), s.api) {
        (_, _, Api::Command | Api::Custom) => area("photos", "Marking photos and uploads", "none", "Pictures can't be sent to this kind of service, so photos of your work won't be marked. Typed answers still can."),
        (false, ..) => area("photos", "Marking photos and uploads", "none", "Photos and scans of handwritten work can't be marked. Typed answers still can. Turn on \"This model can see images\" if it can."),
        (true, Some(false), _) => area("photos", "Marking photos and uploads", "none", "It couldn't see the test picture, so photos won't be marked."),
        (true, None, _) => area("photos", "Marking photos and uploads", "unknown", format!("Pictures will be sent along. Press Check to make sure the model can see them. {pdf_note}")),
        (true, Some(true), _) => area("photos", "Marking photos and uploads", "reduced", format!("Photos and pictures are marked. {pdf_note}")),
    };
    let files = if pdfs {
        area("files", "Your own materials", "reduced", "Text files, pictures and PDFs are sent with each request. Word files can't be opened, so paste what you need into a note.")
    } else {
        area("files", "Your own materials", "reduced", "Text files (notes, .txt, .md, .csv, code) are sent with each request. PDFs and Word files can't be opened, so paste what you need into a note.")
    };
    (photos, files)
}

/// What it costs, for Claude Code and each service in use.
fn cost(choice: &AiChoice) -> String {
    let mut parts = vec![];
    if choice.uses_claude_code() {
        parts.push("Claude Code counts toward your Claude plan's usage limits, with no extra charges.".to_string());
    }
    for s in choice.services_in_use() {
        parts.push(if s.is_local() {
            format!("{} runs on your own computer or network, so there are no charges. Expect it to be slower.", s.display_name())
        } else {
            format!("{} bills you for each use, at its own rates. A whole course can take a lot of tokens, and SelfSemester can't see your balance.", s.display_name())
        });
    }
    parts.join(" ")
}

pub fn for_claude_code() -> Fit {
    Fit {
        overall: "best".into(),
        headline: "Everything works: Claude searches the web for the reading list, opens your files and marks photos of your work.".into(),
        areas: vec![
            area("plan", "Planning and exams", "full", "Opus plans the syllabus and writes exams; Fable on the Max level."),
            area("lessons", "Lessons and assignments", "full", "Written to the build level you pick."),
            area("format", "Answers the app can use", "full", "Claude Code checks every answer against the shape the app needs."),
            area("sources", "Reading list", "full", "Found with web search. Only pages it actually opened are listed."),
            area("research", "Research while writing", "full", "Thorough and Max builds look things up while writing lessons."),
            area("marking", "Marking typed work", "full", "Sonnet marks most work and Opus marks final exams."),
            area("photos", "Marking photos and uploads", "full", "Reads pictures, PDFs and other files you hand in."),
            area("files", "Your own materials", "full", "Opens your PDFs, notes and other files itself."),
        ],
        cost: "Counts toward your Claude plan's usage limits. No extra charges.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claude::endpoint::Check;

    #[test]
    fn guesses_sizes_from_names() {
        assert_eq!(guess_size("qwen2.5-72b-instruct"), Size::Large);
        assert_eq!(guess_size("meta-llama/llama-3.1-8b-instruct"), Size::Small);
        assert_eq!(guess_size("qwen3:30b-a3b"), Size::Medium);
        assert_eq!(guess_size("qwen3-next-80b-a3b-thinking"), Size::Large);
        assert_eq!(guess_size("gemma-4-26b"), Size::Medium);
        assert_eq!(guess_size("mixtral-8x7b"), Size::Medium);
        assert_eq!(guess_size("llama3.2:3b"), Size::Small);
        assert_eq!(guess_size("gpt-5"), Size::Large);
        assert_eq!(guess_size("gpt-5-mini"), Size::Medium);
        assert_eq!(guess_size("gpt-4.1-nano"), Size::Small);
        assert_eq!(guess_size("gemini-2.5-pro"), Size::Large);
        assert_eq!(guess_size("gemini-2.5-flash-lite"), Size::Small);
        assert_eq!(guess_size("deepseek-chat"), Size::Large);
        assert_eq!(guess_size("my-own-model"), Size::Unknown);
        assert_eq!(guess_size("phi-web"), Size::Unknown); // a "b" that isn't a size
    }

    fn ep(model: &str) -> Endpoint {
        Endpoint { id: "s1".into(), base_url: "https://api.example.com/v1".into(), model: model.into(), ..Default::default() }
    }

    fn checked(mut e: Endpoint, json_ok: bool) -> Endpoint {
        e.check = Some(Check { reached: true, json_ok, format: "schema".into(), seconds: 2.0, ..Default::default() });
        e
    }

    fn level(f: &Fit, id: &str) -> String {
        f.areas.iter().find(|a| a.id == id).map(|a| a.level.clone()).unwrap_or_default()
    }

    #[test]
    fn claude_code_does_everything() {
        let f = assess(&AiChoice::default());
        assert_eq!(f.overall, "best");
        assert!(f.areas.iter().all(|a| a.level == "full"));
    }

    #[test]
    fn unchecked_endpoint_is_unknown_until_checked() {
        assert_eq!(for_endpoint(&ep("gpt-5")).overall, "unknown");
        assert_eq!(for_endpoint(&checked(ep("gpt-5"), true)).overall, "good");
        assert_eq!(for_endpoint(&checked(ep("gpt-5-mini"), true)).overall, "fair");
        assert_eq!(for_endpoint(&checked(ep("llama3.2:3b"), true)).overall, "limited");
        assert_eq!(for_endpoint(&checked(ep("gpt-5"), false)).overall, "limited");
    }

    #[test]
    fn always_says_what_is_lost() {
        let f = for_endpoint(&checked(ep("gpt-5"), true));
        assert_eq!(level(&f, "sources"), "reduced");
        assert_eq!(level(&f, "research"), "none");
        assert_eq!(level(&f, "photos"), "none");
        assert!(f.cost.contains("bills you"));
    }

    #[test]
    fn a_service_that_searches_and_reads_pdfs_gets_credit() {
        let mut e = checked(ep("claude-sonnet-4-5"), true);
        e.api = Api::Anthropic;
        e.web_search = true;
        e.reads_pdfs = true;
        e.sees_images = true;
        e.check.as_mut().unwrap().images_ok = Some(true);
        e.check.as_mut().unwrap().format = "prompt".into();
        let f = for_endpoint(&e);
        assert_eq!(level(&f, "sources"), "full");
        assert_eq!(level(&f, "research"), "full");
        assert_eq!(level(&f, "format"), "full"); // Anthropic always asks in words; that's not a fallback
        assert!(f.areas.iter().find(|a| a.id == "files").unwrap().note.contains("PDFs are sent"));
        // web search on an API without a search tool does nothing
        let mut plain = checked(ep("gpt-5"), true);
        plain.web_search = true;
        assert_eq!(level(&for_endpoint(&plain), "sources"), "reduced");
    }

    #[test]
    fn mixed_jobs_report_each_one() {
        let local = Endpoint { base_url: "http://localhost:11434".into(), api: Api::Ollama, ..checked(ep("qwen2.5-72b"), true) };
        let choice = AiChoice {
            services: vec![local],
            roles: Roles { write: Target { service: "s1".into(), model: String::new() }, ..Default::default() },
            ..Default::default()
        };
        let f = assess(&choice);
        // planning and research stay with Claude Code
        assert!(f.areas.iter().find(|a| a.id == "plan").unwrap().note.contains("Opus"));
        assert_eq!(level(&f, "sources"), "full");
        assert!(f.areas.iter().find(|a| a.id == "lessons").unwrap().note.contains("localhost"));
        assert!(f.areas.iter().any(|a| a.id == "context"));
        assert!(f.cost.contains("Claude Code counts") && f.cost.contains("no charges"));
        assert_eq!(f.overall, "good");
    }

    #[test]
    fn unfinished_setup_says_what_is_missing() {
        let f = for_endpoint(&ep(""));
        assert_eq!(f.overall, "limited");
        assert!(f.headline.contains("Pick a model"));
    }
}
