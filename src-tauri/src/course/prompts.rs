//! Prompts and JSON Schemas for every Claude stage.
//!
//! Prompts put the stable course context first (subject, plan) and the stage-specific part last, so repeated
//! calls for one course share as much leading text as possible.

use super::types::*;
use crate::claude::{StageRequest, Tier};
use serde_json::{Value, json};

// ---------- schema helpers (strict: every property required, no extras) ----------

fn obj(props: &[(&str, Value)]) -> Value {
    let mut p = serde_json::Map::new();
    for (k, v) in props {
        p.insert((*k).to_string(), v.clone());
    }
    let req: Vec<&str> = props.iter().map(|(k, _)| *k).collect();
    json!({"type": "object", "properties": p, "required": req, "additionalProperties": false})
}
fn s() -> Value {
    json!({"type": "string"})
}
fn n() -> Value {
    json!({"type": "number"})
}
fn int() -> Value {
    json!({"type": "integer"})
}
fn b() -> Value {
    json!({"type": "boolean"})
}
fn arr(v: Value) -> Value {
    json!({"type": "array", "items": v})
}
fn one_of(vals: &[&str]) -> Value {
    json!({"type": "string", "enum": vals})
}

const BANDS: &[&str] = &["Low", "Low–medium", "Medium", "Medium–high", "High"];

/// Rules shared by every course-writing stage.
pub const HOUSE_RULES: &str = "\
You work inside SelfSemester, a desktop app that turns a subject into a complete, university-style course with hard \
deadlines and marks. The learner studies alone; you are not available while they study, so everything you write must \
stand on its own. Rules: prefer free and openly licensed sources (OpenStax, MIT OpenCourseWare, LibreTexts, arXiv, \
official documentation); only mention paid or copyrighted material when nothing free covers a topic, and mark it \
\"get this yourself\"; never suggest bypassing paywalls. Explain in your own words, with short quotes at most. Use \
Markdown, and LaTeX for maths ($inline$, $$display$$). Be accurate; when unsure, say so rather than guess. \
Answer only with JSON matching the given schema.";

/// The app can currently capture these kinds of work.
pub const SUPPORTED_MODULES: &str = "written answers (typed text, Markdown, LaTeX), photo or PDF uploads \
(e.g. handwritten working), and auto-marked quizzes (multiple choice, numeric). Not yet supported: audio recording, \
camera capture, running code or projects locally, external tool checks.";

fn req(tier: Tier, system: &str, prompt: String, schema: Value, tools: &[&str]) -> StageRequest {
    StageRequest {
        tier,
        system: Some(system.to_string()),
        prompt,
        schema,
        tools: tools.iter().map(|t| t.to_string()).collect(),
        add_dirs: vec![],
    }
}

// ---------- 1. Intake ----------

pub fn intake_schema() -> Value {
    obj(&[(
        "questions",
        arr(obj(&[
            ("id", s()),
            ("text", s()),
            ("kind", one_of(&["text", "choice", "number", "date"])),
            ("options", arr(s())),
        ])),
    )])
}

pub fn intake(subject: &str, notes: &str) -> StageRequest {
    let prompt = format!(
        "A learner wants a course on:\n\n{subject}\n\nTheir extra notes: {notes}\n\n\
         Write 4 to 8 short questions you need answered before you can plan the course. Always cover: their goal, \
         current level, hours per week they can study, the date they want to finish by, equipment or software they \
         have, and whether they will study offline or online. Add subject-specific questions only if they change the \
         plan. Use kind 'choice' with 2-5 options where sensible (options empty otherwise), 'number' for hours, \
         'date' for dates (YYYY-MM-DD). Ids are short snake_case."
    );
    req(Tier::Sonnet, HOUSE_RULES, prompt, intake_schema(), &[])
}

// ---------- 2. Scope / plan ----------

pub fn plan_schema() -> Value {
    let step = obj(&[("step", s()), ("band", one_of(BANDS)), ("percent", n()), ("note", s())]);
    obj(&[
        ("title", s()),
        ("summary", s()),
        ("bundle", obj(&[("detected", b()), ("parts", arr(s())), ("recommendation", s())])),
        ("mode", one_of(&["offline", "online"])),
        ("dependencies", arr(s())),
        (
            "pacing",
            obj(&[("total_weeks", int()), ("hours_per_week", n()), ("fits_user_hours", b()), ("note", s())]),
        ),
        (
            "units",
            arr(obj(&[
                ("key", s()),
                ("title", s()),
                ("week", int()),
                ("topics", arr(s())),
                ("exam", one_of(&["none", "midterm", "final"])),
            ])),
        ),
        (
            "weights",
            obj(&[
                ("homework", n()),
                ("assignment", n()),
                ("quiz", n()),
                ("test", n()),
                ("exam", n()),
                ("project", n()),
            ]),
        ),
        (
            "readiness",
            obj(&[
                ("delivery", arr(step.clone())),
                ("marking", arr(step)),
                ("overall_delivery", one_of(BANDS)),
                ("overall_marking", one_of(BANDS)),
                ("weak_spots", arr(s())),
                ("basis", s()),
            ]),
        ),
        ("domain_family", s()),
        ("unsupported_inputs", arr(s())),
    ])
}

pub fn plan(subject: &str, notes: &str, answers: &Value, bundle_choice: Option<&str>) -> StageRequest {
    let bundle = match bundle_choice {
        Some("combined") => "The learner chose ONE combined course covering every part.",
        Some("first") => "The learner chose to take ONLY THE FIRST part now; plan just that.",
        Some("sequence") => "The learner chose separate courses in sequence; plan ONLY THE FIRST course now.",
        _ => "If the request bundles several courses (e.g. 'QM 1, 2 and 3'), set bundle.detected=true, list the parts, \
              and recommend combined vs one at a time (one major topic at a time when the load is heavy). Still plan \
              your recommended option.",
    };
    let prompt = format!(
        "Subject requested: {subject}\nLearner notes: {notes}\nIntake answers (JSON): {answers}\n\n\
         Plan the whole course like a university syllabus.\n\
         - {bundle}\n\
         - Pace it to fit the subject and the learner's hours and finish date (a small tool course may be 1-2 weeks; \
           a university subject 8-16 weeks). Set fits_user_hours=false and explain in pacing.note if their hours can't \
           fit.\n\
         - Units: one per week or so, keys u01, u02, ...; week = the course week it starts (1-based). Mark a midterm \
           around the middle of longer courses and a final in the last unit.\n\
         - Weights: share of the final grade per kind, summing to 1 (use 0 for kinds you won't use).\n\
         - The app can capture: {SUPPORTED_MODULES} List anything this course needs beyond that in \
           unsupported_inputs and plan a reduced version without it.\n\
         - mode 'online' if the learner needs internet, accounts or software during the course; list each in \
           dependencies.\n\
         - Readiness report: rough estimates of how well you can DELIVER this course (lesson accuracy, coverage by \
           free sources) and how accurately you can MARK it (per assessment type). Never 100%. Bands: Low, \
           Low–medium, Medium, Medium–high, High. Name weak spots plainly. basis = one sentence on where the \
           numbers come from (they are starting estimates, not measurements)."
    );
    req(Tier::Opus, HOUSE_RULES, prompt, plan_schema(), &[])
}

// ---------- 3. Sources ----------

pub fn sources_schema() -> Value {
    obj(&[(
        "sources",
        arr(obj(&[
            ("url", s()),
            ("title", s()),
            ("licence", s()),
            ("attribution", s()),
            ("free", b()),
            ("unit_keys", arr(s())),
            ("note", s()),
        ])),
    )])
}

pub fn sources(plan: &Plan) -> StageRequest {
    let units: Vec<String> = plan.units.iter().map(|u| format!("{} {}: {}", u.key, u.title, u.topics.join(", "))).collect();
    let prompt = format!(
        "Course: {}\n{}\nUnits:\n{}\n\n\
         Find a reading list for this course using web search. Free and openly licensed first; for each source give \
         the exact URL, title, licence (e.g. 'CC BY 4.0', 'CC BY-NC-SA 4.0', 'all rights reserved'), a one-line \
         attribution, which unit keys it covers, and a short note on what to read. Mark free=false for anything paid \
         or copyrighted (only include those when nothing free covers a topic). Aim for 3-10 sources that together \
         cover every unit. Only list URLs you actually found.",
        plan.title,
        plan.summary,
        units.join("\n")
    );
    req(Tier::Sonnet, HOUSE_RULES, prompt, sources_schema(), &["WebSearch", "WebFetch"])
}

// ---------- 4. Unit content ----------

pub fn unit_schema() -> Value {
    obj(&[
        ("lesson", obj(&[("title", s()), ("markdown", s()), ("est_minutes", int()), ("sources_used", arr(s()))])),
        (
            "items",
            arr(obj(&[
                ("key", s()),
                ("kind", one_of(&["homework", "assignment", "quiz", "test", "exam", "project"])),
                ("title", s()),
                ("module", one_of(&["written", "upload", "quiz"])),
                ("instructions", s()),
                (
                    "questions",
                    arr(obj(&[
                        ("id", s()),
                        ("prompt", s()),
                        ("type", one_of(&["mcq", "numeric", "short", "long"])),
                        ("options", arr(s())),
                        ("points", n()),
                    ])),
                ),
                ("rubric", arr(obj(&[("criterion", s()), ("points", n()), ("description", s())]))),
                (
                    "answer_key",
                    arr(obj(&[("question_id", s()), ("answer", s()), ("explanation", s()), ("tolerance", n())])),
                ),
                ("due_days", int()),
            ])),
        ),
    ])
}

/// Shared course context: identical for every unit of a course, so it leads the prompt.
fn course_context(plan: &Plan, sources: &[SourceEntry]) -> String {
    let units: Vec<String> = plan
        .units
        .iter()
        .map(|u| format!("- {} (week {}) {}: {}{}", u.key, u.week, u.title, u.topics.join(", "),
            if u.exam == "none" { String::new() } else { format!(" [{} exam]", u.exam) }))
        .collect();
    let srcs: Vec<String> = sources
        .iter()
        .map(|s| format!("- {} <{}> ({}{}) units: {}", s.title, s.url, s.licence, if s.free { "" } else { ", get this yourself" }, s.unit_keys.join(",")))
        .collect();
    format!(
        "COURSE: {}\n{}\nPacing: {} weeks, about {} hours per week.\nGrade weights: homework {}, assignment {}, quiz {}, \
         test {}, exam {}, project {}.\nSyllabus:\n{}\nReading list:\n{}\n",
        plan.title, plan.summary, plan.pacing.total_weeks, plan.pacing.hours_per_week,
        plan.weights.homework, plan.weights.assignment, plan.weights.quiz, plan.weights.test, plan.weights.exam,
        plan.weights.project, units.join("\n"), srcs.join("\n")
    )
}

pub fn unit(plan: &Plan, sources: &[SourceEntry], unit_key: &str, feedback: Option<&str>) -> StageRequest {
    let u = plan.units.iter().find(|u| u.key == unit_key).expect("unit in plan");
    let exam_tier = if u.exam == "none" { Tier::Sonnet } else { Tier::Opus };
    let exam_line = match u.exam.as_str() {
        "midterm" => "This unit also includes the MIDTERM exam (kind 'exam') covering every unit so far.",
        "final" => "This unit also includes the FINAL exam (kind 'exam') covering the whole course.",
        _ => "No exam in this unit.",
    };
    let fix = feedback.map(|f| format!("\nA reviewer found these problems in your previous draft; fix them all:\n{f}\n")).unwrap_or_default();
    let prompt = format!(
        "{ctx}\nNOW WRITE UNIT {key}: {title} (topics: {topics}).\n{exam_line}\n{fix}\n\
         - lesson.markdown: a complete self-study lesson (roughly {mins} minutes of reading): explanations, worked \
           examples with full working, common mistakes, a short summary, and which reading-list sources to read \
           (cite by title). Put lesson.sources_used = the URLs you relied on.\n\
         - items: the graded work released with this unit, using only kinds with a non-zero weight. Usually one \
           homework or assignment plus a short quiz; add the exam if this unit has one. Keys like '{key}-hw', \
           '{key}-quiz', '{key}-exam'.\n\
         - module: 'quiz' for auto-marked items (every question mcq or numeric, with an exact answer_key entry); \
           'written' for typed answers; 'upload' when working is best done by hand and photographed.\n\
         - questions: clear and solvable from the lesson and readings. For mcq give 3-5 options and put the exact \
           correct option text in answer_key.answer. For numeric give the number and a sensible tolerance.\n\
         - rubric: modelled on real university marking schemes (method marks, final answer, communication). Rubric \
           points must sum to the item's total question points.\n\
         - answer_key: a full worked answer for EVERY question (this stays sealed and is used for marking).\n\
         - due_days: 2-14 days after release, matching the workload; quizzes 2-4, exams 1-3.\n\
         - The app can capture: {SUPPORTED_MODULES} Don't set work it can't capture.",
        ctx = course_context(plan, sources),
        key = u.key,
        title = u.title,
        topics = u.topics.join(", "),
        mins = (plan.pacing.hours_per_week * 60.0 * 0.4).round().max(20.0),
    );
    req(exam_tier, HOUSE_RULES, prompt, unit_schema(), &[])
}

// ---------- 5. Validation ----------

pub fn validation_schema() -> Value {
    obj(&[
        ("ok", b()),
        (
            "issues",
            arr(obj(&[("item_key", s()), ("question_id", s()), ("problem", s()), ("severity", one_of(&["minor", "major"]))])),
        ),
    ])
}

pub fn validate(plan: &Plan, unit_key: &str, content: &UnitContent) -> StageRequest {
    let prompt = format!(
        "Course: {}. Unit {unit_key}.\nCheck this generated unit as a careful second marker:\n{}\n\n\
         For every question: is it solvable from the lesson, is the answer key correct (re-derive numeric answers), \
         does the mcq answer exactly match one option, and do rubric points add up to the question points? Is the \
         lesson accurate? Report each problem; severity 'major' for wrong answers, unsolvable questions or factual \
         errors, 'minor' for wording. ok=true only if there are no major issues.",
        plan.title,
        serde_json::to_string(content).unwrap_or_default()
    );
    req(Tier::Haiku, HOUSE_RULES, prompt, validation_schema(), &[])
}

// ---------- 6. Marking ----------

pub fn mark_schema() -> Value {
    obj(&[
        ("lines", arr(obj(&[("criterion", s()), ("awarded", n()), ("possible", n()), ("comment", s())]))),
        ("overall_comment", s()),
        ("revise", arr(s())),
    ])
}

pub fn mark(
    course_title: &str,
    item: &ItemPublic,
    key: &ItemKey,
    text_answer: &str,
    answers: &Value,
    files: &[String],
    final_exam: bool,
) -> StageRequest {
    let file_part = if files.is_empty() {
        String::new()
    } else {
        format!(
            "\nThe learner also uploaded these files; open each with the Read tool before marking:\n{}\n",
            files.iter().map(|f| format!("- {f}")).collect::<Vec<_>>().join("\n")
        )
    };
    let prompt = format!(
        "Course: {course_title}\nMark this submission fairly and strictly, like a university marker, using the rubric \
         and the sealed answer key. Give one line per rubric criterion (awarded <= possible), specific comments \
         pointing at what was right or wrong, an overall comment, and concrete things to revise. Award method marks \
         for correct working even when the final answer is wrong. Don't reveal more of the answer key than needed \
         to explain mistakes.\n\nITEM: {}\n\nRUBRIC + QUESTIONS (JSON): {}\n\nANSWER KEY (JSON): {}\n\n\
         LEARNER'S TYPED ANSWER:\n{}\n\nLEARNER'S PER-QUESTION ANSWERS (JSON): {}\n{file_part}",
        item.title,
        json!({"instructions": item.instructions, "questions": item.questions, "rubric": item.rubric}),
        serde_json::to_string(&key.answer_key).unwrap_or_default(),
        if text_answer.trim().is_empty() { "(none)" } else { text_answer },
        answers,
    );
    let tools: &[&str] = if files.is_empty() { &[] } else { &["Read"] };
    let mut r = req(if final_exam { Tier::Opus } else { Tier::Sonnet }, HOUSE_RULES, prompt, mark_schema(), tools);
    let mut dirs: Vec<String> = files
        .iter()
        .filter_map(|f| std::path::Path::new(f).parent().map(|p| p.display().to_string()))
        .collect();
    dirs.dedup();
    r.add_dirs = dirs;
    r
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Every schema must be strict, or Claude's structured output may add or skip fields.
    fn assert_strict(v: &Value, path: &str) {
        if v.get("type") == Some(&json!("object")) {
            assert_eq!(v["additionalProperties"], json!(false), "{path} allows extra properties");
            let props = v["properties"].as_object().unwrap();
            let req: Vec<&str> = v["required"].as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
            for (k, sub) in props {
                assert!(req.contains(&k.as_str()), "{path}.{k} not required");
                assert_strict(sub, &format!("{path}.{k}"));
            }
        }
        if let Some(items) = v.get("items") {
            assert_strict(items, &format!("{path}[]"));
        }
    }

    #[test]
    fn schemas_are_strict() {
        for (name, s) in [
            ("intake", intake_schema()),
            ("plan", plan_schema()),
            ("sources", sources_schema()),
            ("unit", unit_schema()),
            ("validation", validation_schema()),
            ("mark", mark_schema()),
        ] {
            assert_strict(&s, name);
        }
    }

    #[test]
    fn schemas_fit_command_line() {
        for s in [plan_schema(), unit_schema(), sources_schema()] {
            assert!(s.to_string().len() + HOUSE_RULES.len() < 24_000);
        }
    }

    #[test]
    fn schema_matches_types() {
        // A value shaped by the schema must parse into the Rust type.
        let unit = json!({"lesson":{"title":"t","markdown":"m","est_minutes":30,"sources_used":[]},
            "items":[{"key":"u01-quiz","kind":"quiz","title":"Q","module":"quiz","instructions":"i",
            "questions":[{"id":"q1","prompt":"2+2?","type":"numeric","options":[],"points":1}],
            "rubric":[{"criterion":"correct","points":1,"description":""}],
            "answer_key":[{"question_id":"q1","answer":"4","explanation":"","tolerance":0}],"due_days":3}]});
        let u: UnitContent = serde_json::from_value(unit).unwrap();
        assert_eq!(u.items[0].questions[0].kind, "numeric");
    }

    #[test]
    fn tools_only_where_needed() {
        assert!(intake("x", "").tools.is_empty());
        let p: Plan = serde_json::from_value(sample_plan()).unwrap();
        assert_eq!(sources(&p).tools, vec!["WebSearch", "WebFetch"]);
        assert!(unit(&p, &[], "u01", None).tools.is_empty());
        assert_eq!(unit(&p, &[], "u02", None).tier, Tier::Opus); // final exam unit
    }

    pub fn sample_plan() -> Value {
        json!({"title":"QM 1","summary":"Intro","bundle":{"detected":false,"parts":[],"recommendation":""},
          "mode":"offline","dependencies":[],"pacing":{"total_weeks":2,"hours_per_week":6,"fits_user_hours":true,"note":""},
          "units":[{"key":"u01","title":"Waves","week":1,"topics":["wave function"],"exam":"none"},
                   {"key":"u02","title":"Operators","week":2,"topics":["operators"],"exam":"final"}],
          "weights":{"homework":0.4,"assignment":0,"quiz":0.2,"test":0,"exam":0.4,"project":0},
          "readiness":{"delivery":[],"marking":[],"overall_delivery":"High","overall_marking":"High","weak_spots":[],"basis":""},
          "domain_family":"theory-maths","unsupported_inputs":[]})
    }
}
