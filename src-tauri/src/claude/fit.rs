//! "How well can this build a course?" in plain words, shown in Settings and before a new course is planned.
//!
//! Claude Code can do every part of a build. Another service loses web search and opening files by itself, and how
//! good the course is depends on the model. The model's size is a guess from its name, which is rough, so the
//! report says so; the real test is the Check button, which asks the model something and looks at the answer.

use super::endpoint::{AiChoice, Endpoint};
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

/// The report for whatever is set up now.
pub fn assess(choice: &AiChoice) -> Fit {
    if choice.uses_endpoint() { for_endpoint(&choice.endpoint) } else { for_claude_code() }
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

pub fn for_endpoint(e: &Endpoint) -> Fit {
    let strong = guess_size(e.model_for(super::Tier::Opus));
    let main = guess_size(e.model_for(super::Tier::Sonnet));
    let cost = if e.is_local() {
        "Runs on your own computer or network, so there are no charges. Expect it to be slower.".to_string()
    } else {
        "Billed by the service for each use, at its own rates. A whole course can take a lot of tokens, and SelfSemester can't see your balance.".to_string()
    };

    if let Some(p) = e.problem() {
        return Fit { overall: "limited".into(), headline: format!("Not set up yet. {p}"), areas: vec![], cost };
    }

    let (plan_level, plan_note) = writing(strong, "the syllabus and exams");
    let (lesson_level, lesson_note) = writing(main, "lessons and assignments");
    let (mark_level, mark_note) = match main {
        Size::Large => ("full", "Should mark fairly against the rubric. You can still question any mark in Review.".to_string()),
        Size::Unknown => ("unknown", "Depends on the model. Check a few marks in Review early on.".to_string()),
        _ => ("reduced", "Marks may be off now and then. Use Review to question any that look wrong.".to_string()),
    };

    let format = match &e.check {
        None => area("format", "Answers the app can use", "unknown", "Not checked yet. Press Check to try the service."),
        Some(c) if !c.reached => area("format", "Answers the app can use", "none", format!("Couldn't reach it: {}", c.message)),
        Some(c) if !c.json_ok => area("format", "Answers the app can use", "none", c.message.clone()),
        Some(c) if c.format == "prompt" => area(
            "format",
            "Answers the app can use",
            "reduced",
            "It worked, but this service ignores the setting that asks for JSON, so the app asks in words. Some steps may need a second try.",
        ),
        Some(c) => area("format", "Answers the app can use", "full", format!("Worked when checked ({} s for a short answer).", c.seconds)),
    };

    let photos = match (e.sees_images, e.check.as_ref().and_then(|c| c.images_ok)) {
        (false, _) => area("photos", "Marking photos and uploads", "none", "Photos and scans of handwritten work can't be marked. Typed answers still can. Turn on \"This model can see images\" if it can."),
        (true, Some(false)) => area("photos", "Marking photos and uploads", "none", "It couldn't see the test picture, so photos won't be marked."),
        (true, None) => area("photos", "Marking photos and uploads", "unknown", "Pictures will be sent along. Press Check to make sure the model can see them. PDFs and Word files can't be marked."),
        (true, Some(true)) => area("photos", "Marking photos and uploads", "reduced", "Photos and pictures are marked. PDFs and Word files can't be opened, so hand those in as text or photos."),
    };

    let mut areas = vec![
        area("plan", "Planning and exams", plan_level, plan_note),
        area("lessons", "Lessons and assignments", lesson_level, lesson_note),
        format,
        area("sources", "Reading list", "reduced", "There's no web search here, so the model suggests sources from memory and links aren't checked. Some may be broken."),
        area("research", "Research while writing", "none", "Thorough and Max builds can't look anything up; lessons come from what the model already knows."),
        area("marking", "Marking typed work", mark_level, mark_note),
        photos,
        area("files", "Your own materials", "reduced", "Text files (notes, .txt, .md, .csv, code) are sent with each request. PDFs and Word files can't be opened, so paste what you need into a note."),
    ];
    if e.is_local() {
        areas.push(area(
            "context",
            "Room for long requests",
            "unknown",
            "Local servers often start with a small context window, and course steps send long requests. Set it to at least 32,000 tokens (Ollama: num_ctx; LM Studio: Context Length) or answers get cut off.",
        ));
    }

    let format_level = areas.iter().find(|a| a.id == "format").map(|a| a.level.clone()).unwrap_or_default();
    let (overall, headline) = if format_level == "none" {
        ("limited", "This setup can't build courses yet. Fix what Check found and try again.".to_string())
    } else if strong == Size::Small || main == Size::Small {
        ("limited", "It can build a course, but a small model will make a rough one. Use a bigger model for anything you care about.".to_string())
    } else if format_level == "unknown" {
        ("unknown", "Press Check to see how well this works. Until then, this is a guess from the model names.".to_string())
    } else if strong == Size::Large && main == Size::Large {
        ("good", "Good. Courses should build well, but the reading list isn't checked on the web and there's no research while writing.".to_string())
    } else {
        ("fair", "Usable. Expect simpler lessons and exams than with a large model, no web research, and an unchecked reading list.".to_string())
    };
    Fit { overall: overall.into(), headline, areas, cost }
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
        Endpoint { base_url: "https://api.example.com/v1".into(), model: model.into(), ..Default::default() }
    }

    fn checked(mut e: Endpoint, json_ok: bool) -> Endpoint {
        e.check = Some(Check { reached: true, json_ok, format: "schema".into(), seconds: 2.0, ..Default::default() });
        e
    }

    #[test]
    fn claude_code_does_everything() {
        let f = assess(&AiChoice::default());
        assert_eq!(f.overall, "best");
        assert!(f.areas.iter().all(|a| a.level == "full"));
    }

    #[test]
    fn unchecked_endpoint_is_unknown_until_checked() {
        let f = for_endpoint(&ep("gpt-5"));
        assert_eq!(f.overall, "unknown");
        assert_eq!(for_endpoint(&checked(ep("gpt-5"), true)).overall, "good");
        assert_eq!(for_endpoint(&checked(ep("gpt-5-mini"), true)).overall, "fair");
        assert_eq!(for_endpoint(&checked(ep("llama3.2:3b"), true)).overall, "limited");
        assert_eq!(for_endpoint(&checked(ep("gpt-5"), false)).overall, "limited");
    }

    #[test]
    fn always_says_what_is_lost() {
        let f = for_endpoint(&checked(ep("gpt-5"), true));
        let level = |id: &str| f.areas.iter().find(|a| a.id == id).unwrap().level.clone();
        assert_eq!(level("sources"), "reduced");
        assert_eq!(level("research"), "none");
        assert_eq!(level("photos"), "none");
        assert!(f.cost.contains("Billed"));
    }

    #[test]
    fn local_servers_get_the_context_warning_and_no_charges() {
        let mut e = checked(ep("qwen2.5-72b"), true);
        e.base_url = "http://localhost:11434/v1".into();
        let f = for_endpoint(&e);
        assert!(f.areas.iter().any(|a| a.id == "context"));
        assert!(f.cost.contains("no charges"));
    }

    #[test]
    fn unfinished_setup_says_what_is_missing() {
        let f = for_endpoint(&ep(""));
        assert_eq!(f.overall, "limited");
        assert!(f.headline.contains("Pick a model"));
    }
}
