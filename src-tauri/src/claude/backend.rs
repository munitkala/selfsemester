//! Sends each step to whoever does that job: the learner's Claude Code (the default) or one of their services.

use super::endpoint::{self, AiChoice, Check, Endpoint, EndpointRunner};
use super::{ClaudeRunner, ClaudeStatus, CliRunner, RunError, StageOutput, StageRequest};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Backend {
    cli: CliRunner,
    endpoint: EndpointRunner,
    file: PathBuf,
    choice: Mutex<AiChoice>,
}

impl Backend {
    /// `file` holds the saved choice (see `AiChoice`); `tmp_dir` is scratch space for requests.
    pub fn new(cli: CliRunner, file: PathBuf, tmp_dir: PathBuf) -> Self {
        let choice = AiChoice::load(&file);
        Self { cli, endpoint: EndpointRunner::new(tmp_dir), file, choice: Mutex::new(choice) }
    }

    pub fn choice(&self) -> AiChoice {
        self.choice.lock().unwrap().clone()
    }

    /// Save a new choice. Steps that start after this use it; one already running finishes where it started.
    pub fn set_choice(&self, choice: AiChoice) -> std::io::Result<()> {
        choice.save(&self.file)?;
        self.endpoint.forget_formats();
        *self.choice.lock().unwrap() = choice;
        Ok(())
    }

    /// Try one service as it's saved now, and keep the result with it.
    pub fn check_service(&self, id: &str, model: &str) -> Result<Check, String> {
        let service = self.choice().service(id).cloned().ok_or("That service isn't there any more.")?;
        let c = self.endpoint.check(&service, model);
        let mut choice = self.choice();
        if let Some(s) = choice.services.iter_mut().find(|s| s.id == id) {
            s.check = Some(c.clone());
        }
        self.set_choice(choice).map_err(|e| e.to_string())?;
        Ok(c)
    }

    /// A service's model list, from settings that may not be saved yet (so the picker works while filling them in).
    pub fn service_models(&self, draft: &Endpoint) -> Result<Vec<String>, String> {
        self.endpoint.models(draft)
    }
}

impl ClaudeRunner for Backend {
    fn status(&self) -> ClaudeStatus {
        let choice = self.choice();
        let used = choice.services_in_use();
        let service_problem = used.iter().map(|s| endpoint::status_of(s)).find(|s| s.problem.is_some());
        if used.is_empty() {
            return ClaudeStatus { provider: "claude_code".into(), ..self.cli.status() };
        }
        if !choice.uses_claude_code() {
            return service_problem.unwrap_or_else(|| endpoint::status_of(used[0]));
        }
        // Some jobs on Claude Code, some on services: Claude Code's details, and whichever problem comes first.
        let mut st = ClaudeStatus { provider: "mixed".into(), ..self.cli.status() };
        if st.problem.is_none() {
            st.problem = service_problem.and_then(|s| s.problem);
        }
        st
    }

    fn run(&self, req: &StageRequest) -> Result<StageOutput, RunError> {
        let choice = self.choice();
        let target = choice.roles.for_request(req);
        match choice.resolve(target, req.tier) {
            Some((service, model)) => self.endpoint.run_on(service, &model, req),
            None => self.cli.run(req),
        }
    }

    fn stop_current(&self) {
        // Only one of them is ever running a step, so telling both is harmless.
        self.cli.stop_current();
        self.endpoint.stop_current();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claude::endpoint::{Roles, Target};

    fn backend(dir: &std::path::Path) -> Backend {
        Backend::new(CliRunner::new(dir.join("work")), dir.join("ai.json"), dir.join("tmp"))
    }

    fn service(id: &str, model: &str) -> Endpoint {
        Endpoint { id: id.into(), base_url: "http://localhost:9/v1".into(), model: model.into(), ..Default::default() }
    }

    #[test]
    fn switches_and_remembers() {
        let dir = tempfile::tempdir().unwrap();
        let b = backend(dir.path());
        assert_eq!(b.status().provider, "claude_code");

        let all = Roles::every(Target { service: "s1".into(), model: String::new() });
        b.set_choice(AiChoice { services: vec![service("s1", "m")], roles: all, ..Default::default() }).unwrap();
        let st = b.status();
        assert_eq!((st.provider.as_str(), st.logged_in, st.on_subscription), ("endpoint", true, false));

        let again = backend(dir.path());
        assert!(again.choice().uses_endpoint());
        assert_eq!(again.status().version.as_deref(), Some("m"));

        let some = Roles { check: Target { service: "s1".into(), model: String::new() }, ..Default::default() };
        again.set_choice(AiChoice { services: vec![service("s1", "m")], roles: some, ..Default::default() }).unwrap();
        assert_eq!(again.status().provider, "mixed");
    }

    #[test]
    fn half_set_up_service_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let b = backend(dir.path());
        let broken = Endpoint { id: "s1".into(), ..Default::default() };
        b.set_choice(AiChoice { services: vec![broken], roles: Roles::every(Target { service: "s1".into(), model: String::new() }), ..Default::default() })
            .unwrap();
        let st = b.status();
        assert!(!st.logged_in && st.problem.is_some());
        let req = StageRequest {
            tier: super::super::Tier::Haiku,
            system: None,
            prompt: "hi".into(),
            schema: serde_json::json!({"type": "object"}),
            tools: vec![],
            add_dirs: vec![],
            effort: None,
        };
        assert!(matches!(b.run(&req), Err(RunError::Endpoint(_))));
    }
}
