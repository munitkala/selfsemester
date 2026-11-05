//! One background thread: runs queued Claude jobs one at a time (gentle on plan usage limits), advances the course
//! pipeline after each, and ticks the schedule (releases, deadlines, reminders). Tells the UI when data changed.

use crate::claude::ClaudeRunner;
use crate::course::{marking, pipeline, schedule, seal::Sealer};
use crate::db::jobs;
use crate::paths::AppPaths;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// What the worker needs from the app shell (so tests can run it without Tauri).
pub trait Shell: Send + Sync + 'static {
    fn notify(&self, title: &str, body: &str);
    fn changed(&self);
}

pub struct Worker {
    pub paths: AppPaths,
    pub db: Arc<Mutex<Connection>>,
    pub runner: Arc<dyn ClaudeRunner>,
    pub sealer: Sealer,
}

const TICK_EVERY: Duration = Duration::from_secs(30);
const IDLE_SLEEP: Duration = Duration::from_secs(2);

impl Worker {
    /// Run one job if any is runnable. Returns true if it did something.
    pub fn step(&self, shell: &dyn Shell) -> bool {
        let ran = match jobs::run_one_with(&self.db, self.runner.as_ref(), |_| shell.changed()) {
            Ok(Some(id)) => id,
            Ok(None) => return false,
            Err(e) => {
                eprintln!("job queue error: {e}");
                return false;
            }
        };
        let conn = self.db.lock().unwrap();
        let job = jobs::get(&conn, ran).ok().flatten();
        let model: String = conn
            .query_row("SELECT model FROM claude_calls WHERE job_id=?1 ORDER BY id DESC LIMIT 1", [ran], |r| r.get(0))
            .unwrap_or_default();
        match job.as_ref().map(|j| j.status.as_str()) {
            Some("done") => match pipeline::on_job_done(&conn, &self.paths, &self.sealer, ran, &model) {
                Ok(Some(n)) => shell.notify(&n.title, &n.body),
                Ok(None) => {}
                Err(e) => {
                    // The answer didn't fit what the next step needs: mark the job failed so the user can retry.
                    let _ = conn.execute("UPDATE jobs SET status='failed', error=?2 WHERE id=?1", rusqlite::params![ran, e]);
                    let _ = pipeline::on_job_failed(&conn, ran);
                }
            },
            Some("failed") => {
                let _ = pipeline::on_job_failed(&conn, ran);
            }
            _ => {} // waiting_usage / queued again: nothing to do until it runs
        }
        if let Some(j) = job.and_then(|j| j.course_id) {
            if let Ok(Some(dir)) = marking::maybe_complete(&conn, &self.paths, &self.sealer, &j) {
                shell.notify("Course complete", &format!("Archive saved to {}", dir.display()));
            }
        }
        drop(conn);
        shell.changed();
        true
    }

    /// Advance the schedule and finish any course whose work is all settled.
    pub fn tick(&self, shell: &dyn Shell) {
        let conn = self.db.lock().unwrap();
        match schedule::tick(&conn, chrono::Utc::now()) {
            Ok(notes) => {
                let any = !notes.is_empty();
                for n in notes {
                    shell.notify(&n.title, &n.body);
                }
                let ids: Vec<String> = conn
                    .prepare("SELECT id FROM courses WHERE status='active'")
                    .and_then(|mut st| st.query_map([], |r| r.get(0))?.collect())
                    .unwrap_or_default();
                let mut done = false;
                for id in ids {
                    if let Ok(Some(dir)) = marking::maybe_complete(&conn, &self.paths, &self.sealer, &id) {
                        shell.notify("Course complete", &format!("Archive saved to {}", dir.display()));
                        done = true;
                    }
                }
                drop(conn);
                if any || done {
                    shell.changed();
                }
            }
            Err(e) => eprintln!("schedule tick error: {e}"),
        }
    }

    pub fn spawn(self: Arc<Self>, shell: Arc<dyn Shell>) {
        std::thread::Builder::new()
            .name("selfsemester-worker".into())
            .spawn(move || {
                // SELFSEMESTER_NO_CLAUDE=1 keeps queued jobs waiting (for a demo or test copy of the app).
                let run_jobs = std::env::var("SELFSEMESTER_NO_CLAUDE").map_or(true, |v| v.is_empty() || v == "0");
                let mut last_tick = Instant::now() - TICK_EVERY;
                loop {
                    if last_tick.elapsed() >= TICK_EVERY {
                        self.tick(shell.as_ref());
                        last_tick = Instant::now();
                    }
                    if !run_jobs || !self.step(shell.as_ref()) {
                        std::thread::sleep(IDLE_SLEEP);
                    }
                }
            })
            .expect("start worker thread");
    }
}
