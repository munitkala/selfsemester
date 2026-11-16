//! Build levels: which models write and check a course, how hard they think, how many times a unit may be
//! rewritten after its check, and how long the reading list is. The learner picks one on the review step, from a
//! quick build that uses little of their plan to a maximum build that uses whatever it takes.

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::claude::{Effort, StageRequest, Tier};

/// Most rewrites a unit may get after failing its check.
pub const MAX_REWRITES: u8 = 5;
/// Longest reading list the sources step may ask for.
pub const MAX_SOURCES: u8 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Quick,
    Standard,
    Thorough,
    Max,
    Custom,
}

/// How much each lesson covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Detail {
    /// The key ideas and one worked example each.
    Brief,
    #[default]
    Standard,
    /// As detailed as a demanding university course.
    InDepth,
}

/// The kinds of graded work a plan can hold.
pub const WORK_KINDS: [&str; 6] = ["homework", "assignment", "quiz", "test", "exam", "project"];

/// What goes into the course, separate from how hard Claude works on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Content {
    /// Kinds of graded work to leave out (from `WORK_KINDS`). Leaving out all of them makes a lessons-only course.
    pub skip: Vec<String>,
    pub detail: Detail,
    /// Figures, interactive figures and sound clips in lessons. False = text and formulas only.
    pub media: bool,
}

impl Default for Content {
    fn default() -> Self {
        Content { skip: vec![], detail: Detail::Standard, media: true }
    }
}

impl Content {
    fn clean(mut self) -> Self {
        self.skip.retain(|k| WORK_KINDS.contains(&k.as_str()));
        self.skip.sort();
        self.skip.dedup();
        self
    }
}

/// Apply the content choices to a plan before it's built: drop the left-out kinds of work (and exams from units
/// when exams are left out) and spread their weight over the kinds that remain. Returns true when no graded work
/// is left, i.e. a lessons-only course.
pub fn apply_content(plan: &mut super::types::Plan, c: &Content) -> bool {
    let w = &mut plan.weights;
    for k in &c.skip {
        match k.as_str() {
            "homework" => w.homework = 0.0,
            "assignment" => w.assignment = 0.0,
            "quiz" => w.quiz = 0.0,
            "test" => w.test = 0.0,
            "exam" => w.exam = 0.0,
            "project" => w.project = 0.0,
            _ => {}
        }
    }
    if c.skip.iter().any(|k| k == "exam") {
        for u in &mut plan.units {
            u.exam = "none".into();
        }
    }
    let sum = w.homework + w.assignment + w.quiz + w.test + w.exam + w.project;
    if sum <= 0.0 {
        return true;
    }
    for x in [&mut w.homework, &mut w.assignment, &mut w.quiz, &mut w.test, &mut w.exam, &mut w.project] {
        *x /= sum;
    }
    false
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildSettings {
    pub level: Level,
    /// Finds sources and writes lessons, homework and quizzes.
    pub writer: Tier,
    /// Writes units that hold a test or exam.
    pub exam_writer: Tier,
    /// Checks every unit's answer keys and rubrics.
    pub checker: Tier,
    /// None leaves it to Claude Code's default.
    pub effort: Option<Effort>,
    /// Rewrites a unit may get when its check finds major problems.
    pub rewrites: u8,
    /// Longest reading list to ask for.
    pub sources: u8,
    /// What goes into the course. Independent of the level.
    #[serde(default)]
    pub content: Content,
}

impl Default for BuildSettings {
    fn default() -> Self {
        preset(Level::Standard)
    }
}

/// The fixed levels. Standard is what the app did before levels existed.
pub fn preset(level: Level) -> BuildSettings {
    let (writer, exam_writer, checker, effort, rewrites, sources) = match level {
        Level::Quick => (Tier::Sonnet, Tier::Sonnet, Tier::Haiku, Some(Effort::Low), 0, 4),
        Level::Standard | Level::Custom => (Tier::Sonnet, Tier::Opus, Tier::Haiku, None, 1, 10),
        Level::Thorough => (Tier::Opus, Tier::Opus, Tier::Sonnet, Some(Effort::High), 2, 15),
        Level::Max => (Tier::Fable, Tier::Fable, Tier::Opus, Some(Effort::Max), 3, 25),
    };
    BuildSettings { level, writer, exam_writer, checker, effort, rewrites, sources, content: Content::default() }
}

impl BuildSettings {
    /// Keep numbers in range, and make a preset match its definition (so only Custom can differ).
    pub fn clean(mut self) -> Self {
        let content = self.content.clean();
        if self.level != Level::Custom {
            return BuildSettings { content, ..preset(self.level) };
        }
        self.rewrites = self.rewrites.min(MAX_REWRITES);
        self.sources = self.sources.clamp(2, MAX_SOURCES);
        self.content = content;
        self
    }

    fn set(&self, req: &mut StageRequest, tier: Tier) {
        req.tier = tier;
        req.effort = self.effort;
    }

    pub fn for_sources(&self, req: &mut StageRequest) {
        self.set(req, self.writer);
    }

    pub fn for_unit(&self, req: &mut StageRequest, has_exam: bool) {
        self.set(req, if has_exam { self.exam_writer } else { self.writer });
    }

    pub fn for_check(&self, req: &mut StageRequest) {
        self.set(req, self.checker);
    }
}

fn io(e: rusqlite::Error) -> String {
    e.to_string()
}

/// The course's build settings (Standard for courses approved before levels existed).
pub fn of_course(conn: &Connection, id: &str) -> Result<BuildSettings, String> {
    let s: Option<String> = conn
        .query_row("SELECT build_json FROM courses WHERE id=?1", [id], |r| r.get(0))
        .optional()
        .map_err(io)?
        .flatten();
    Ok(s.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default())
}

pub fn save_for_course(conn: &Connection, id: &str, b: &BuildSettings) -> Result<(), String> {
    let json = serde_json::to_string(b).unwrap();
    conn.execute("UPDATE courses SET build_json=?2 WHERE id=?1", params![id, json]).map_err(io)?;
    // Offered first next time.
    crate::db::set_setting(conn, "build_settings", &json).map_err(io)?;
    Ok(())
}

/// The last settings the learner built with, or Standard.
pub fn last_used(conn: &Connection) -> BuildSettings {
    crate::db::get_setting(conn, "build_settings")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str::<BuildSettings>(&s).ok())
        .map(BuildSettings::clean)
        // Which kinds of work to leave out depends on each course's plan, so that isn't carried over.
        .map(|mut b| {
            b.content.skip.clear();
            b
        })
        .unwrap_or_default()
}

// ---------- Estimate ----------

/// Rough share of plan usage per call, relative to Sonnet. Plans don't publish exact rates, so these follow the
/// models' published prices, which is how much heavier one is than another.
fn model_weight(t: Tier) -> f64 {
    match t {
        Tier::Haiku => 0.05,
        Tier::Sonnet => 1.0,
        Tier::Opus => 2.0,
        Tier::Fable => 4.0,
    }
}

/// More effort means more thinking, so more tokens and more time. None is Claude Code's default.
fn effort_weight(e: Option<Effort>) -> f64 {
    match e {
        Some(Effort::Low) => 0.5,
        Some(Effort::Medium) => 0.75,
        None | Some(Effort::High) => 1.0,
        Some(Effort::Xhigh) => 1.4,
        Some(Effort::Max) => 2.0,
    }
}

/// How much slower a model answers than Sonnet.
fn model_time(t: Tier) -> f64 {
    match t {
        Tier::Haiku => 0.5,
        Tier::Sonnet => 1.0,
        Tier::Opus => 1.4,
        Tier::Fable => 2.0,
    }
}

/// How much longer each kind of build step takes on these settings than on Standard's typical step.
/// `exam_share` is the share of units that hold a test or exam.
pub fn time_factor(b: &BuildSettings, kind: &str, exam_share: f64) -> f64 {
    let e = effort_weight(b.effort);
    let s = preset(Level::Standard);
    let rel = |t: Tier, base: Tier| model_time(t) / model_time(base);
    match kind {
        "sources" => e * rel(b.writer, s.writer),
        "unit" => {
            let now = (1.0 - exam_share) * model_time(b.writer) + exam_share * model_time(b.exam_writer);
            let base = (1.0 - exam_share) * model_time(s.writer) + exam_share * model_time(s.exam_writer);
            e * now / base
        }
        "validate" => e * rel(b.checker, s.checker),
        _ => 1.0,
    }
}

/// Share of units whose check finds a major problem each round (seen in test builds), used for expected rewrites.
const REWRITE_RATE: f64 = 0.3;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Estimate {
    /// Claude calls if no unit needs a rewrite.
    pub calls_min: u32,
    /// Claude calls if every unit uses every rewrite.
    pub calls_max: u32,
    /// Expected plan usage compared with a Standard build of the same course (1.0 = the same).
    pub usage_vs_standard: f64,
    /// Expected minutes to build.
    pub minutes: f64,
}

struct Shape {
    units: u32,
    exam_units: u32,
    /// Size of a unit compared with a Standard one (lesson detail, media, whether there's graded work).
    unit_scale: f64,
}

/// Rough size of a unit for these content choices, compared with the default.
pub fn unit_scale(c: &Content, lessons_only: bool) -> f64 {
    let detail = match c.detail {
        Detail::Brief => 0.6,
        Detail::Standard => 1.0,
        Detail::InDepth => 1.6,
    };
    detail * if c.media { 1.0 } else { 0.85 } * if lessons_only { 0.6 } else { 1.0 }
}

fn cost(b: &BuildSettings, s: &Shape, secs: &dyn Fn(&str) -> f64) -> (f64, f64) {
    let e = effort_weight(b.effort);
    let plain = (s.units - s.exam_units) as f64;
    let exams = s.exam_units as f64;
    // Expected unit drafts per unit: the first, plus rewrites while checks keep failing.
    let drafts: f64 = (0..=b.rewrites as i32).map(|k| REWRITE_RATE.powi(k)).sum();
    // Relative sizes: a reading list (with searches) ~ 1, a unit ~ 3, a check ~ 1.
    let usage = e
        * (model_weight(b.writer)
            + drafts * 3.0 * s.unit_scale * (plain * model_weight(b.writer) + exams * model_weight(b.exam_writer))
            + drafts * (s.units as f64) * model_weight(b.checker));
    let exam_share = if s.units > 0 { exams / s.units as f64 } else { 0.0 };
    let f = |k: &str| time_factor(b, k, exam_share);
    let time = secs("sources") * f("sources")
        + drafts * (s.units as f64) * (secs("unit") * f("unit") * s.unit_scale + secs("validate") * f("validate"));
    (usage, time)
}

/// `s` is the course as it will be built; `planned` is the plan as written, which a Standard build is compared with.
fn estimate_shape(b: &BuildSettings, s: &Shape, planned: &Shape, secs: &dyn Fn(&str) -> f64) -> Estimate {
    let (usage, time) = cost(b, s, secs);
    let (base, _) = cost(&preset(Level::Standard), planned, secs);
    Estimate {
        calls_min: 1 + 2 * s.units,
        calls_max: 1 + 2 * s.units * (1 + b.rewrites as u32),
        usage_vs_standard: if base > 0.0 { usage / base } else { 1.0 },
        minutes: time / 60.0,
    }
}

/// Estimate for building this planned course with these settings. Step times come from past builds on this
/// computer (they were measured mostly on Standard), scaled for the models and effort chosen.
pub fn estimate(conn: &Connection, id: &str, b: &BuildSettings) -> Result<Estimate, String> {
    let b = b.clone().clean();
    let mut plan = super::pipeline::plan_of(conn, id)?;
    let shape_of = |p: &super::types::Plan, scale: f64| Shape {
        units: p.units.len() as u32,
        exam_units: p.units.iter().filter(|u| u.exam != "none").count() as u32,
        unit_scale: scale,
    };
    let planned = shape_of(&plan, 1.0);
    let lessons_only = apply_content(&mut plan, &b.content);
    let shape = shape_of(&plan, unit_scale(&b.content, lessons_only));
    let secs = |k: &str| super::progress::typical_secs(conn, k).unwrap_or(60.0);
    Ok(estimate_shape(&b, &shape, &planned, &secs))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(_: &str) -> f64 {
        60.0
    }

    #[test]
    fn standard_matches_the_old_tiers() {
        let b = preset(Level::Standard);
        assert_eq!((b.writer, b.exam_writer, b.checker, b.effort, b.rewrites, b.sources), (Tier::Sonnet, Tier::Opus, Tier::Haiku, None, 1, 10));
    }

    #[test]
    fn levels_climb_in_usage() {
        let s = Shape { units: 6, exam_units: 2, unit_scale: 1.0 };
        let u: Vec<f64> = [Level::Quick, Level::Standard, Level::Thorough, Level::Max]
            .iter()
            .map(|l| estimate_shape(&preset(*l), &s, &s, &secs).usage_vs_standard)
            .collect();
        assert!(u.windows(2).all(|w| w[0] < w[1]), "{u:?}");
        assert!((u[1] - 1.0).abs() < 1e-9);
        let e = estimate_shape(&preset(Level::Quick), &s, &s, &secs);
        assert_eq!((e.calls_min, e.calls_max), (13, 13));
        let e = estimate_shape(&preset(Level::Max), &s, &s, &secs);
        assert_eq!((e.calls_min, e.calls_max), (13, 49));
    }

    #[test]
    fn time_factor_is_one_on_standard() {
        let s = preset(Level::Standard);
        for k in ["sources", "unit", "validate"] {
            assert!((time_factor(&s, k, 0.5) - 1.0).abs() < 1e-9, "{k}");
        }
        assert!(time_factor(&preset(Level::Max), "unit", 0.5) > 1.0);
        assert!(time_factor(&preset(Level::Quick), "unit", 0.5) < 1.0);
    }

    #[test]
    fn leaving_out_work_reweights_and_drops_exams() {
        let mut plan: crate::course::types::Plan = serde_json::from_value(crate::course::prompts::tests::sample_plan()).unwrap();
        let c = Content { skip: vec!["exam".into()], ..Content::default() };
        assert!(!apply_content(&mut plan, &c));
        let w = &plan.weights;
        assert_eq!(w.exam, 0.0);
        assert!((w.homework - 2.0 / 3.0).abs() < 1e-9 && (w.quiz - 1.0 / 3.0).abs() < 1e-9);
        assert!(plan.units.iter().all(|u| u.exam == "none"));
        let all = Content { skip: WORK_KINDS.iter().map(|k| k.to_string()).collect(), ..Content::default() };
        assert!(apply_content(&mut plan, &all));
    }

    #[test]
    fn content_survives_level_changes_but_skips_are_per_course() {
        let mut b = preset(Level::Thorough);
        b.content = Content { skip: vec!["quiz".into(), "bogus".into()], detail: Detail::Brief, media: false };
        let c = b.clone().clean();
        assert_eq!(c.content, Content { skip: vec!["quiz".into()], detail: Detail::Brief, media: false });
        let conn = crate::db::open_in_memory().unwrap();
        crate::db::set_setting(&conn, "build_settings", &serde_json::to_string(&c).unwrap()).unwrap();
        let last = last_used(&conn);
        assert!(last.content.skip.is_empty() && last.content.detail == Detail::Brief && !last.content.media);
        // Settings saved before content options existed still load.
        let old: BuildSettings = serde_json::from_str(r#"{"level":"quick","writer":"sonnet","exam_writer":"sonnet","checker":"haiku","effort":"low","rewrites":0,"sources":4}"#).unwrap();
        assert_eq!(old.content, Content::default());
    }

    #[test]
    fn clean_resets_presets_and_clamps_custom() {
        let mut b = preset(Level::Quick);
        b.writer = Tier::Fable;
        assert_eq!(b.clean(), preset(Level::Quick));
        let c = BuildSettings { level: Level::Custom, rewrites: 99, sources: 0, ..preset(Level::Standard) }.clean();
        assert_eq!((c.rewrites, c.sources), (MAX_REWRITES, 2));
    }

    #[test]
    fn applies_models_and_effort() {
        let b = preset(Level::Max);
        let mut r = StageRequest {
            tier: Tier::Haiku,
            system: None,
            prompt: "p".into(),
            schema: serde_json::json!({}),
            tools: vec![],
            add_dirs: vec![],
            effort: None,
        };
        b.for_check(&mut r);
        assert_eq!((r.tier, r.effort), (Tier::Opus, Some(Effort::Max)));
        b.for_unit(&mut r, true);
        assert_eq!(r.tier, Tier::Fable);
        let args = crate::claude::cli::CliRunner::stage_args(&r).unwrap();
        let at = args.iter().position(|a| a == "--effort").unwrap();
        assert_eq!(args[at + 1], "max");
        assert_eq!(args[args.iter().position(|a| a == "--model").unwrap() + 1], "fable");
        assert_eq!(args[args.iter().position(|a| a == "--fallback-model").unwrap() + 1], "opus");
        // Default requests stay exactly as before: no effort or fallback flags.
        preset(Level::Standard).for_sources(&mut r);
        let args = crate::claude::cli::CliRunner::stage_args(&r).unwrap();
        assert!(!args.iter().any(|a| a == "--effort" || a == "--fallback-model"));
    }
}
