pub mod backup;
pub mod claude;
pub mod course;
pub mod db;
pub mod paths;
pub mod worker;

use claude::{ClaudeRunner, ClaudeStatus, CliRunner, StageRequest, Tier};
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
const NOTICE_KEY: &str = "notice_v2_accepted_at";

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
    pipeline::answer_intake(&state.db.lock().unwrap(), &id, &answers)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
fn choose_bundle(app: AppHandle, state: tauri::State<'_, AppState>, id: String, choice: String) -> Res<()> {
    pipeline::choose_bundle(&state.db.lock().unwrap(), &id, &choice)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
fn approve_course(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    id: String,
    start_date: String,
    late_policy: LatePolicy,
    strictness: Strictness,
    on_transcript: bool,
) -> Res<()> {
    let start = chrono::NaiveDate::parse_from_str(&start_date, "%Y-%m-%d").map_err(|_| "Pick a valid start date.")?;
    pipeline::approve(&state.db.lock().unwrap(), &id, start, &late_policy, strictness, on_transcript)?;
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
    materials::list(&state.db.lock().unwrap(), &course_id)
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
    let id = builder::import(&state.db.lock().unwrap(), std::path::Path::new(&folder))?;
    changed(&app);
    Ok(id)
}

#[tauri::command]
fn get_transcript(state: tauri::State<'_, AppState>) -> Res<course::TranscriptView> {
    course::transcript(&state.db.lock().unwrap())
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
    let runner: Arc<dyn ClaudeRunner> = Arc::new(CliRunner::new(paths.claude_workdir()));
    let db = Arc::new(Mutex::new(conn));

    let worker = Arc::new(worker::Worker { paths: paths.clone(), db: db.clone(), runner: runner.clone(), sealer: sealer.clone() });

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { startup, paths, db, runner, sealer })
        .setup(move |app| {
            worker.clone().spawn(Arc::new(TauriShell(app.handle().clone())));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_status,
            run_test_stage,
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
            get_transcript,
            open_folder,
            list_backups,
            backup_now
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
