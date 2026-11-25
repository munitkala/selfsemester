pub mod backup;
pub mod claude;
pub mod course;
pub mod db;
pub mod demo;
pub mod paths;
pub mod tools;
pub mod widget;
pub mod worker;

use claude::endpoint::{AiChoice, Check, Endpoint};
use claude::{Backend, ClaudeRunner, ClaudeStatus, CliRunner, StageRequest, Tier};
use course::build::BuildSettings;
use course::{builder, marking, materials, pipeline, review, schedule::LatePolicy, schedule::Strictness, seal::Sealer};
use paths::AppPaths;
use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

pub struct AppState {
    pub startup: StartupInfo,
    pub paths: AppPaths,
    pub db: Arc<Mutex<Connection>>,
    pub runner: Arc<dyn ClaudeRunner>,
    /// The same thing as `runner`, for the settings that pick Claude Code or another AI service.
    pub backend: Arc<Backend>,
    pub sealer: Sealer,
}

type Res<T> = Result<T, String>;

/// What happened to your data when the app started (shown in Settings, and once as a note if it was restored).
#[derive(Clone, Serialize)]
pub struct StartupInfo {
    pub version: String,
    pub result: Option<backup::Startup>,
    /// Set when the safety backup couldn't be made; the app still starts.
    pub problem: Option<String>,
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Tell every window that data changed, so open screens reload.
fn changed(app: &AppHandle) {
    let _ = app.emit("data-changed", ());
}

struct TauriShell(AppHandle);
impl worker::Shell for TauriShell {
    fn notify(&self, title: &str, body: &str) {
        use tauri_plugin_notification::NotificationExt;
        let _ = self.0.notification().builder().title(title).body(body).show();
    }
    fn changed(&self) {
        changed(&self.0);
    }
}

// ---------- status ----------

#[derive(Serialize)]
struct AppStatus {
    startup: StartupInfo,
    claude: ClaudeStatus,
    data_dir: String,
    user_dir: String,
    schema_version: u32,
    jobs: course::JobSummary,
}

#[tauri::command]
async fn app_status(state: tauri::State<'_, AppState>) -> Res<AppStatus> {
    let runner = state.runner.clone();
    // `claude --version` / `auth status` spawn processes; keep them off the UI thread.
    let claude = tauri::async_runtime::spawn_blocking(move || runner.status()).await.map_err(err)?;
    let conn = state.db.lock().unwrap();
    Ok(AppStatus {
        startup: state.startup.clone(),
        claude,
        data_dir: state.paths.data_dir.display().to_string(),
        user_dir: state.paths.user_dir.display().to_string(),
        schema_version: db::schema_version(&conn).map_err(err)?,
        jobs: course::job_summary(&conn, None).map_err(err)?,
    })
}

#[derive(Serialize)]
struct TestResult {
    job: db::jobs::Job,
    result: Option<Value>,
}

/// Queue one tiny Haiku call and wait for the worker to run it, to prove Claude Code works with the subscription.
#[tauri::command]
async fn run_test_stage(state: tauri::State<'_, AppState>) -> Res<TestResult> {
    let req = StageRequest {
        tier: Tier::Haiku,
        system: Some("You check that SelfSemester can reach Claude. Answer only with the requested JSON.".into()),
        prompt: "Reply with ok=true and one short, encouraging sentence for someone starting a new course.".into(),
        schema: serde_json::json!({
            "type": "object",
            "properties": { "ok": {"type": "boolean"}, "message": {"type": "string"} },
            "required": ["ok", "message"],
            "additionalProperties": false
        }),
        tools: vec![],
        add_dirs: vec![],
        effort: None,
    };
    let id = db::jobs::enqueue(&state.db.lock().unwrap(), None, "test", &req).map_err(err)?;
    for _ in 0..(10 * 60 * 2) {
        {
            let conn = state.db.lock().unwrap();
            let job = db::jobs::get(&conn, id).map_err(err)?.ok_or("job vanished")?;
            if matches!(job.status.as_str(), "done" | "failed" | "waiting_usage") {
                let result = db::jobs::result(&conn, id).map_err(err)?;
                return Ok(TestResult { job, result });
            }
        }
        tokio_sleep(500).await;
    }
    Err("The test call is taking too long. Check Claude Code in a terminal.".into())
}

async fn tokio_sleep(ms: u64) {
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(std::time::Duration::from_millis(ms))).await;
}

// ---------- which AI builds courses ----------

/// One saved service as the Settings page sees it. The API key itself never goes back to the page.
#[derive(Serialize)]
struct ServiceView {
    #[serde(flatten)]
    service: Endpoint,
    key_set: bool,
    /// The report if this service did every job.
    fit: claude::fit::Fit,
}

/// The AI settings as the Settings page sees them.
#[derive(Serialize)]
struct AiView {
    services: Vec<ServiceView>,
    roles: claude::endpoint::Roles,
    /// For what's set up now.
    fit: claude::fit::Fit,
    /// For Claude Code alone, to compare with.
    claude_code_fit: claude::fit::Fit,
}

fn ai_view(choice: &AiChoice) -> AiView {
    let services = choice
        .services
        .iter()
        .map(|s| {
            let mut shown = s.clone();
            shown.api_key.clear();
            ServiceView { key_set: !s.api_key.trim().is_empty(), fit: claude::fit::for_endpoint(s), service: shown }
        })
        .collect();
    AiView { services, roles: choice.roles.clone(), fit: claude::fit::assess(choice), claude_code_fit: claude::fit::for_claude_code() }
}

#[tauri::command]
fn ai_settings(state: tauri::State<'_, AppState>) -> AiView {
    ai_view(&state.backend.choice())
}

/// Add or update a service. An empty id adds a new one. `key` None keeps the saved key; Some("") removes it.
#[tauri::command]
fn save_service(app: AppHandle, state: tauri::State<'_, AppState>, service: Endpoint, key: Option<String>) -> Res<AiView> {
    let mut choice = state.backend.choice();
    let old = choice.service(&service.id).cloned();
    let mut s = service;
    if old.is_none() {
        s.id = choice.new_id();
    }
    for f in [&mut s.base_url, &mut s.model, &mut s.strong_model, &mut s.quick_model, &mut s.auth_header, &mut s.auth_query, &mut s.name] {
        *f = f.trim().to_string();
    }
    s.base_url = s.base_url.trim_end_matches('/').to_string();
    s.api_key = key.map(|k| k.trim().to_string()).unwrap_or_else(|| old.as_ref().map(|o| o.api_key.clone()).unwrap_or_default());
    if claude::endpoint::bad_key(&s.api_key) {
        return Err("The API key has a line break in it, which can't be right.".into());
    }
    // A check only counts for the settings it was made with.
    s.check = old.as_ref().and_then(|o| {
        let mut before = o.clone();
        let mut now = s.clone();
        before.check = None;
        now.check = None;
        before.name = String::new();
        now.name = String::new();
        (before == now).then(|| o.check.clone()).flatten()
    });
    match choice.services.iter_mut().find(|x| x.id == s.id) {
        Some(slot) => *slot = s,
        None => choice.services.push(s),
    }
    state.backend.set_choice(choice.clone()).map_err(|e| format!("Couldn't save the AI settings: {e}"))?;
    changed(&app);
    Ok(ai_view(&choice))
}

/// Remove a service. Jobs it did go back to Claude Code.
#[tauri::command]
fn delete_service(app: AppHandle, state: tauri::State<'_, AppState>, id: String) -> Res<AiView> {
    let mut choice = state.backend.choice();
    choice.services.retain(|s| s.id != id);
    for t in [&mut choice.roles.plan, &mut choice.roles.write, &mut choice.roles.check, &mut choice.roles.research] {
        if t.service == id {
            *t = claude::endpoint::Target::default();
        }
    }
    state.backend.set_choice(choice.clone()).map_err(|e| format!("Couldn't save the AI settings: {e}"))?;
    changed(&app);
    Ok(ai_view(&choice))
}

/// Say who does each job.
#[tauri::command]
fn set_roles(app: AppHandle, state: tauri::State<'_, AppState>, roles: claude::endpoint::Roles) -> Res<AiView> {
    let mut choice = state.backend.choice();
    for (job, t) in roles.all() {
        if !t.is_claude_code() && choice.service(&t.service).is_none() {
            return Err(format!("The {job} job points at a service that isn't there any more."));
        }
    }
    choice.roles = roles;
    state.backend.set_choice(choice.clone()).map_err(|e| format!("Couldn't save the AI settings: {e}"))?;
    changed(&app);
    Ok(ai_view(&choice))
}

/// Try a saved service for real with a tiny question (and a tiny picture if it's meant to see them).
#[tauri::command]
async fn check_service(state: tauri::State<'_, AppState>, id: String, model: Option<String>) -> Res<(Check, AiView)> {
    let backend = state.backend.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let c = backend.check_service(&id, model.as_deref().unwrap_or(""))?;
        Ok((c, ai_view(&backend.choice())))
    })
    .await
    .map_err(err)?
}

/// The models a service offers, for the picker. Uses the saved key when `key` is None.
#[tauri::command]
async fn service_models(state: tauri::State<'_, AppState>, service: Endpoint, key: Option<String>) -> Res<Vec<String>> {
    let backend = state.backend.clone();
    let mut draft = service;
    draft.api_key = key.unwrap_or_else(|| backend.choice().service(&draft.id).map(|s| s.api_key.clone()).unwrap_or_default());
    tauri::async_runtime::spawn_blocking(move || backend.service_models(&draft)).await.map_err(err)?
}

/// Queue state only (cheap: no Claude Code process is started).
#[tauri::command]
fn jobs_status(state: tauri::State<'_, AppState>) -> Res<course::JobSummary> {
    course::job_summary(&state.db.lock().unwrap(), None).map_err(err)
}

/// Try paused work again now instead of waiting for the scheduled retry (all courses, or one).
#[tauri::command]
fn resume_now(app: AppHandle, state: tauri::State<'_, AppState>, course_id: Option<String>) -> Res<usize> {
    let n = db::jobs::resume_waiting(&state.db.lock().unwrap(), course_id.as_deref(), chrono::Utc::now()).map_err(err)?;
    changed(&app);
    Ok(n)
}

// ---------- backups ----------

#[tauri::command]
fn list_backups(state: tauri::State<'_, AppState>) -> Vec<backup::BackupInfo> {
    backup::list(&state.paths)
}

#[tauri::command]
async fn backup_now(state: tauri::State<'_, AppState>) -> Res<String> {
    let paths = state.paths.clone();
    let version = state.startup.version.clone();
    // copying course files can take a moment; keep it off the UI thread
    let dir = tauri::async_runtime::spawn_blocking(move || backup::create(&paths, "manual", Some(&version), &version))
        .await
        .map_err(err)?
        .map_err(|e| format!("Couldn't make the backup: {e}"))?;
    Ok(dir.display().to_string())
}

// ---------- terms notice ----------

/// Bump when the first-launch notice changes, so everyone sees the new version once.
const NOTICE_KEY: &str = "notice_v4_accepted_at";

#[tauri::command]
fn notice_accepted(state: tauri::State<'_, AppState>) -> Res<bool> {
    Ok(db::get_setting(&state.db.lock().unwrap(), NOTICE_KEY).map_err(err)?.is_some())
}

#[tauri::command]
fn accept_notice(state: tauri::State<'_, AppState>) -> Res<()> {
    db::set_setting(&state.db.lock().unwrap(), NOTICE_KEY, &db::now_utc()).map_err(err)
}

// ---------- courses ----------

#[tauri::command]
fn list_courses(state: tauri::State<'_, AppState>) -> Res<Vec<course::CourseCard>> {
    course::list(&state.db.lock().unwrap())
}

#[tauri::command]
fn get_course(state: tauri::State<'_, AppState>, id: String) -> Res<course::CourseView> {
    course::view(&state.db.lock().unwrap(), &state.paths, &id)
}

#[tauri::command]
fn create_course(app: AppHandle, state: tauri::State<'_, AppState>, subject: String, notes: String) -> Res<String> {
    let id = pipeline::create(&state.db.lock().unwrap(), &subject, &notes)?;
    changed(&app);
    Ok(id)
}

#[tauri::command]
fn answer_intake(app: AppHandle, state: tauri::State<'_, AppState>, id: String, answers: Value) -> Res<()> {
    pipeline::answer_intake(&state.db.lock().unwrap(), &state.paths, &id, &answers)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
fn choose_bundle(app: AppHandle, state: tauri::State<'_, AppState>, id: String, choice: String) -> Res<()> {
    pipeline::choose_bundle(&state.db.lock().unwrap(), &state.paths, &id, &choice)?;
    changed(&app);
    Ok(())
}

/// The choices on the review step's enrol form.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Enrol {
    start_date: String,
    late_policy: LatePolicy,
    strictness: Strictness,
    on_transcript: bool,
    build: BuildSettings,
}

#[tauri::command]
fn approve_course(app: AppHandle, state: tauri::State<'_, AppState>, id: String, enrol: Enrol) -> Res<()> {
    let start = chrono::NaiveDate::parse_from_str(&enrol.start_date, "%Y-%m-%d").map_err(|_| "Pick a valid start date.")?;
    pipeline::approve(&state.db.lock().unwrap(), &id, start, &enrol.late_policy, enrol.strictness, enrol.on_transcript, &enrol.build)?;
    changed(&app);
    Ok(())
}

/// The build levels, and the settings the learner built with last time (offered first).
#[derive(Serialize)]
struct BuildLevels {
    presets: Vec<BuildSettings>,
    last: BuildSettings,
}

#[tauri::command]
fn build_levels(state: tauri::State<'_, AppState>) -> BuildLevels {
    use course::build::{Level, preset};
    BuildLevels {
        presets: [Level::Quick, Level::Standard, Level::Thorough, Level::Max].into_iter().map(preset).collect(),
        last: course::build::last_used(&state.db.lock().unwrap()),
    }
}

#[tauri::command]
fn build_estimate(state: tauri::State<'_, AppState>, id: String, build: BuildSettings) -> Res<course::build::Estimate> {
    course::build::estimate(&state.db.lock().unwrap(), &id, &build)
}

/// Pause building a course. With `now`, a step that's running is stopped and goes back in the queue (what it had
/// used is lost); otherwise it finishes and is kept, and nothing after it starts until the course is continued.
#[tauri::command]
fn pause_course(app: AppHandle, state: tauri::State<'_, AppState>, id: String, now: Option<bool>) -> Res<()> {
    let running = pipeline::pause(&state.db.lock().unwrap(), &id)?;
    if running && now.unwrap_or(true) {
        state.runner.stop_current();
    }
    changed(&app);
    Ok(())
}

/// End building early and start the course with the weeks that are ready; the rest is dropped.
#[tauri::command]
fn finish_course_early(app: AppHandle, state: tauri::State<'_, AppState>, id: String) -> Res<()> {
    let (notice, running) = pipeline::finish_with_ready(&state.db.lock().unwrap(), &state.paths, &state.sealer, &id)?;
    if running {
        state.runner.stop_current();
    }
    let _ = notice;
    changed(&app);
    Ok(())
}

#[tauri::command]
fn build_remaining(state: tauri::State<'_, AppState>, id: String) -> Res<course::build::Remaining> {
    course::build::remaining(&state.db.lock().unwrap(), &id)
}

#[tauri::command]
fn continue_course(app: AppHandle, state: tauri::State<'_, AppState>, id: String) -> Res<()> {
    pipeline::unpause(&state.db.lock().unwrap(), &id)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
fn retry_course(app: AppHandle, state: tauri::State<'_, AppState>, id: String) -> Res<usize> {
    let n = pipeline::retry(&state.db.lock().unwrap(), &id)?;
    changed(&app);
    Ok(n)
}

#[tauri::command]
fn delete_course(app: AppHandle, state: tauri::State<'_, AppState>, id: String) -> Res<()> {
    pipeline::delete(&state.db.lock().unwrap(), &state.paths, &id)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
fn today(state: tauri::State<'_, AppState>) -> Res<course::Today> {
    course::today(&state.db.lock().unwrap())
}

// ---------- the learner's own materials ----------

#[tauri::command]
fn list_materials(state: tauri::State<'_, AppState>, course_id: String) -> Res<Vec<materials::Material>> {
    materials::list(&state.db.lock().unwrap(), &state.paths, &course_id)
}

#[tauri::command]
fn add_materials(app: AppHandle, state: tauri::State<'_, AppState>, course_id: String, files: Vec<String>) -> Res<usize> {
    let files: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
    let n = materials::add_files(&state.db.lock().unwrap(), &state.paths, &course_id, &files)?;
    changed(&app);
    Ok(n)
}

#[tauri::command]
fn add_material_note(app: AppHandle, state: tauri::State<'_, AppState>, course_id: String, title: String, text: String) -> Res<()> {
    materials::add_note(&state.db.lock().unwrap(), &state.paths, &course_id, &title, &text)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
fn remove_material(app: AppHandle, state: tauri::State<'_, AppState>, id: String) -> Res<()> {
    materials::remove(&state.db.lock().unwrap(), &state.paths, &id)?;
    changed(&app);
    Ok(())
}

/// Open a material's copy in its default app (PDF viewer, image viewer, editor).
#[tauri::command]
fn open_material(app: AppHandle, state: tauri::State<'_, AppState>, id: String) -> Res<()> {
    use tauri_plugin_opener::OpenerExt;
    let path = materials::file_of(&state.db.lock().unwrap(), &state.paths, &id)?;
    app.opener().open_path(path.display().to_string(), None::<&str>).map_err(err)
}

// ---------- items ----------

#[tauri::command]
fn get_item(state: tauri::State<'_, AppState>, id: String) -> Res<course::ItemView> {
    course::item(&state.db.lock().unwrap(), &state.paths, &state.sealer, &id)
}

#[tauri::command]
fn mark_lesson_read(app: AppHandle, state: tauri::State<'_, AppState>, id: String) -> Res<()> {
    marking::mark_read(&state.db.lock().unwrap(), &id)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
fn submit_item(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    id: String,
    text_answer: String,
    answers: Value,
    files: Vec<String>,
) -> Res<marking::SubmitOutcome> {
    let files: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
    let out = marking::submit(&state.db.lock().unwrap(), &state.paths, &state.sealer, &id, &text_answer, &answers, &files, chrono::Utc::now())?;
    changed(&app);
    Ok(out)
}

/// Manual courses: the learner marks their own submitted work, one number per self-mark line.
#[tauri::command]
fn self_mark_item(app: AppHandle, state: tauri::State<'_, AppState>, id: String, awarded: Vec<f64>, comment: String) -> Res<()> {
    marking::self_mark(&state.db.lock().unwrap(), &state.paths, &state.sealer, &id, &awarded, &comment)?;
    changed(&app);
    Ok(())
}

/// Sandbox: show the answer key. Ends the attempts on that item.
#[tauri::command]
fn reveal_answer_key(app: AppHandle, state: tauri::State<'_, AppState>, id: String) -> Res<()> {
    marking::reveal_key(&state.db.lock().unwrap(), &id)?;
    changed(&app);
    Ok(())
}

/// Review mode: ask Claude about a marked item.
#[tauri::command]
fn send_review_message(app: AppHandle, state: tauri::State<'_, AppState>, id: String, text: String) -> Res<()> {
    review::send(&state.db.lock().unwrap(), &state.paths, &state.sealer, &id, &text)?;
    changed(&app);
    Ok(())
}

// ---------- course builder (no Claude) ----------

#[tauri::command]
fn builder_create(app: AppHandle, state: tauri::State<'_, AppState>, title: String) -> Res<String> {
    let id = builder::create(&state.db.lock().unwrap(), &title)?;
    changed(&app);
    Ok(id)
}

#[derive(Serialize)]
struct DraftView {
    draft: builder::Draft,
    problems: Vec<String>,
}

#[tauri::command]
fn builder_get(state: tauri::State<'_, AppState>, id: String) -> Res<DraftView> {
    let draft = builder::get(&state.db.lock().unwrap(), &id)?;
    let problems = builder::problems(&draft);
    Ok(DraftView { draft, problems })
}

/// Save the draft; returns what still stops it from being published.
#[tauri::command]
fn builder_save(state: tauri::State<'_, AppState>, id: String, draft: builder::Draft) -> Res<Vec<String>> {
    builder::save(&state.db.lock().unwrap(), &id, &draft)?;
    Ok(builder::problems(&draft))
}

#[tauri::command]
fn builder_publish(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    id: String,
    start_date: String,
    late_policy: LatePolicy,
    strictness: Strictness,
    on_transcript: bool,
) -> Res<()> {
    let start = chrono::NaiveDate::parse_from_str(&start_date, "%Y-%m-%d").map_err(|_| "Pick a valid start date.")?;
    builder::publish(&state.db.lock().unwrap(), &state.paths, &state.sealer, &id, start, &late_policy, strictness, on_transcript)?;
    changed(&app);
    Ok(())
}

/// Write the draft to a folder in Documents and return its path.
#[tauri::command]
fn builder_export(state: tauri::State<'_, AppState>, id: String) -> Res<String> {
    Ok(builder::export(&state.db.lock().unwrap(), &state.paths, &id)?.display().to_string())
}

#[tauri::command]
fn builder_import(app: AppHandle, state: tauri::State<'_, AppState>, folder: String) -> Res<String> {
    let id = builder::import(&state.db.lock().unwrap(), &state.paths, std::path::Path::new(&folder))?;
    changed(&app);
    Ok(id)
}

// ---------- documents in courses you write, and growing a published course ----------

#[tauri::command]
fn builder_attach(state: tauri::State<'_, AppState>, id: String, files: Vec<String>) -> Res<Vec<course::types::Attachment>> {
    let files: Vec<std::path::PathBuf> = files.into_iter().map(std::path::PathBuf::from).collect();
    course::attachments::add(&state.db.lock().unwrap(), &state.paths, &id, &files)
}

#[tauri::command]
fn builder_file_text(state: tauri::State<'_, AppState>, id: String, file: String) -> Res<String> {
    course::attachments::read_text(&state.paths, &id, &file)
}

#[tauri::command]
fn builder_remove_file(state: tauri::State<'_, AppState>, id: String, file: String) -> Res<()> {
    course::attachments::remove(&state.db.lock().unwrap(), &state.paths, &id, &file)
}

/// Open a lesson's or a piece of work's document in the learner's usual app.
#[tauri::command]
fn open_course_file(app: AppHandle, state: tauri::State<'_, AppState>, course_id: String, file: String) -> Res<()> {
    use tauri_plugin_opener::OpenerExt;
    let path = course::attachments::path_of(&state.paths, &course_id, &file)?;
    app.opener().open_path(path.display().to_string(), None::<&str>).map_err(err)
}

#[tauri::command]
fn course_audio(state: tauri::State<'_, AppState>, course_id: String, file: String) -> Res<tauri::ipc::Response> {
    let path = course::audio::path_of(&state.paths, &course_id, &file)?;
    Ok(tauri::ipc::Response::new(std::fs::read(path).map_err(err)?))
}

/// Keep an interactive figure's HTML for its sandboxed frame; returns the id the frame loads (see widget.rs).
#[tauri::command]
fn widget_put(store: tauri::State<'_, widget::Store>, html: String, tools: Option<Vec<String>>) -> Res<String> {
    store.put(&html, &tools.unwrap_or_default())
}

// ---------- tools (see tools.rs) ----------

#[tauri::command]
fn tools_view(state: tauri::State<'_, AppState>) -> tools::ToolsView {
    tools::view(&state.paths, &state.db.lock().unwrap())
}

#[tauri::command]
async fn tool_install(app: AppHandle, state: tauri::State<'_, AppState>, id: String) -> Res<()> {
    let guard = tools::begin(&state.db.lock().unwrap(), &id)?;
    changed(&app);
    let paths = state.paths.clone();
    // Downloads take a moment; keep them off the UI thread.
    let result = tauri::async_runtime::spawn_blocking(move || tools::install(&paths, &id, tools::default_source(&paths).as_ref()))
        .await
        .map_err(err)
        .and_then(|r| r);
    drop(guard);
    changed(&app);
    result
}

#[tauri::command]
fn tools_lookup(state: tauri::State<'_, AppState>, ids: Vec<String>) -> Vec<tools::ToolView> {
    tools::lookup(&state.paths, &ids)
}

#[tauri::command]
fn tool_remove(app: AppHandle, state: tauri::State<'_, AppState>, id: String) -> Res<()> {
    tools::remove(&state.paths, &state.db.lock().unwrap(), &id)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
fn tools_setup_done(state: tauri::State<'_, AppState>) -> Res<()> {
    db::set_setting(&state.db.lock().unwrap(), tools::SETUP_SETTING, "1").map_err(err)
}

#[tauri::command]
fn audio_online(state: tauri::State<'_, AppState>) -> Res<bool> {
    Ok(db::get_setting(&state.db.lock().unwrap(), course::audio::ONLINE_SETTING).map_err(err)?.as_deref() != Some("0"))
}

#[tauri::command]
fn set_audio_online(state: tauri::State<'_, AppState>, on: bool) -> Res<()> {
    db::set_setting(&state.db.lock().unwrap(), course::audio::ONLINE_SETTING, if on { "1" } else { "0" }).map_err(err)
}

#[tauri::command]
fn course_add_week(app: AppHandle, state: tauri::State<'_, AppState>, id: String, unit: builder::DraftUnit) -> Res<String> {
    let key = builder::add_week(&state.db.lock().unwrap(), &state.paths, &state.sealer, &id, unit)?;
    changed(&app);
    Ok(key)
}

#[tauri::command]
fn course_add_work(app: AppHandle, state: tauri::State<'_, AppState>, id: String, unit_key: String, item: course::types::GenItem) -> Res<()> {
    builder::add_work(&state.db.lock().unwrap(), &state.paths, &state.sealer, &id, &unit_key, item)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
fn get_transcript(state: tauri::State<'_, AppState>) -> Res<course::TranscriptView> {
    course::transcript(&state.db.lock().unwrap())
}

// ---------- study time and insights ----------

/// Called about once a minute while a lesson or item page is open, focused and in use.
/// Doesn't send `data-changed`: a heartbeat shouldn't make every open page reload.
#[tauri::command]
fn record_study(state: tauri::State<'_, AppState>, item_id: String, day: String, hour: u32, seconds: u32) -> Res<()> {
    course::insights::record_study(&state.db.lock().unwrap(), &item_id, &day, hour, seconds)
}

#[tauri::command]
fn get_insights(state: tauri::State<'_, AppState>) -> Res<course::insights::Insights> {
    course::insights::insights(&state.db.lock().unwrap())
}

#[tauri::command]
fn clear_study_time(app: AppHandle, state: tauri::State<'_, AppState>) -> Res<usize> {
    let n = course::insights::clear_study(&state.db.lock().unwrap())?;
    changed(&app);
    Ok(n)
}

/// Open one of the app's own folders (course archive, Documents folder) in File Explorer.
#[tauri::command]
fn open_folder(app: AppHandle, state: tauri::State<'_, AppState>, path: String) -> Res<()> {
    use tauri_plugin_opener::OpenerExt;
    let p = PathBuf::from(&path);
    if !(p.starts_with(&state.paths.user_dir) || p.starts_with(&state.paths.data_dir)) || !p.is_dir() {
        return Err("That folder isn't one of SelfSemester's.".into());
    }
    app.opener().open_path(path, None::<&str>).map_err(err)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let paths = AppPaths::standard().expect("standard folders");
    // A demo build brings a finished course; on its first launch on a computer with no data, copy it in.
    if let Some(demo) = demo::bundled()
        && let Err(e) = demo::seed(&paths, &demo, chrono::Utc::now())
    {
        eprintln!("couldn't set up the demo course: {e}");
    }
    paths.ensure().expect("create app folders");
    let version = env!("CARGO_PKG_VERSION").to_string();
    // back up (or restore) before this version opens and upgrades the database
    let startup = match backup::before_open(&paths, &version, db::latest_schema()) {
        Ok(r) => StartupInfo { version: version.clone(), result: Some(r), problem: None },
        Err(e) => {
            eprintln!("start-up backup failed: {e}");
            StartupInfo { version: version.clone(), result: None, problem: Some(e.to_string()) }
        }
    };
    let conn = db::open(&paths.db_file()).expect("open database");
    backup::mark_version(&conn, &version).expect("record app version");
    db::jobs::requeue_interrupted(&conn).expect("requeue jobs");
    let sealer = Sealer::load(&conn).expect("seal key");
    let backend = Arc::new(Backend::new(CliRunner::new(paths.claude_workdir()), paths.data_dir.join("ai.json"), paths.data_dir.join("tmp")));
    let runner: Arc<dyn ClaudeRunner> = backend.clone();
    let db = Arc::new(Mutex::new(conn));

    let worker = Arc::new(worker::Worker { paths: paths.clone(), db: db.clone(), runner: runner.clone(), sealer: sealer.clone() });

    let (audio_paths, audio_db) = (paths.clone(), db.clone());
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { startup, paths, db, runner, backend, sealer })
        .manage(widget::Store::default())
        .register_uri_scheme_protocol("widget", |ctx, request| {
            use tauri::Manager;
            let app = ctx.app_handle();
            widget::respond(&app.state::<widget::Store>(), &app.state::<AppState>().paths, &request.uri().to_string())
        })
        .setup(move |app| {
            worker.clone().spawn(Arc::new(TauriShell(app.handle().clone())));
            // Tools fixed since they were installed are downloaded again, quietly, in the background.
            let (tool_paths, tool_db, tool_app) = (audio_paths.clone(), audio_db.clone(), app.handle().clone());
            std::thread::spawn(move || {
                if !tools::update_outdated(&tool_paths, &tool_db, tools::default_source(&tool_paths).as_ref()).is_empty() {
                    changed(&tool_app);
                }
            });
            let (conn, handle) = (audio_db.clone(), app.handle().clone());
            course::audio::spawn(
                audio_paths.clone(),
                move || db::get_setting(&conn.lock().unwrap(), course::audio::ONLINE_SETTING).ok().flatten().as_deref() != Some("0"),
                move || changed(&handle),
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            course_audio,
            widget_put,
            tools_view,
            tools_lookup,
            tool_install,
            tool_remove,
            tools_setup_done,
            audio_online,
            set_audio_online,
            app_status,
            run_test_stage,
            ai_settings,
            save_service,
            delete_service,
            set_roles,
            check_service,
            service_models,
            jobs_status,
            resume_now,
            notice_accepted,
            accept_notice,
            list_courses,
            get_course,
            create_course,
            answer_intake,
            choose_bundle,
            approve_course,
            build_levels,
            pause_course,
            finish_course_early,
            build_remaining,
            continue_course,
            build_estimate,
            retry_course,
            delete_course,
            list_materials,
            add_materials,
            add_material_note,
            remove_material,
            open_material,
            today,
            get_item,
            mark_lesson_read,
            submit_item,
            self_mark_item,
            reveal_answer_key,
            send_review_message,
            builder_create,
            builder_get,
            builder_save,
            builder_publish,
            builder_export,
            builder_import,
            builder_attach,
            builder_file_text,
            builder_remove_file,
            open_course_file,
            course_add_week,
            course_add_work,
            get_transcript,
            record_study,
            get_insights,
            clear_study_time,
            open_folder,
            list_backups,
            backup_now
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
