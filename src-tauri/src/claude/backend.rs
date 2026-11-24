//! Sends each step to whichever is set up: the learner's Claude Code (the default) or another AI service.

use super::endpoint::{AiChoice, Check, Endpoint, EndpointRunner};
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
        Self { cli, endpoint: EndpointRunner::new(choice.endpoint.clone(), tmp_dir), file, choice: Mutex::new(choice) }
    }

    pub fn choice(&self) -> AiChoice {
        self.choice.lock().unwrap().clone()
    }

    /// Save a new choice. Steps that start after this use it; one already running finishes where it started.
    pub fn set_choice(&self, choice: AiChoice) -> std::io::Result<()> {
        choice.save(&self.file)?;
        self.endpoint.set_endpoint(choice.endpoint.clone());
        *self.choice.lock().unwrap() = choice;
        Ok(())
    }

    /// Try the service as it's set up now, and keep the result with the settings.
    pub fn check_endpoint(&self) -> std::io::Result<Check> {
        let c = self.endpoint.check();
        let mut choice = self.choice();
        choice.endpoint.check = Some(c.clone());
        self.set_choice(choice)?;
        Ok(c)
    }

    /// The service's model list, using settings that aren't saved yet (so the picker works while filling them in).
    pub fn endpoint_models(&self, draft: &Endpoint, tmp_dir: PathBuf) -> Result<Vec<String>, String> {
        EndpointRunner::new(draft.clone(), tmp_dir).models()
    }
}

impl ClaudeRunner for Backend {
    fn status(&self) -> ClaudeStatus {
        if self.choice().uses_endpoint() {
            self.endpoint.status()
        } else {
            ClaudeStatus { provider: "claude_code".into(), ..self.cli.status() }
        }
    }

    fn run(&self, req: &StageRequest) -> Result<StageOutput, RunError> {
        if self.choice().uses_endpoint() { self.endpoint.run(req) } else { self.cli.run(req) }
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

    #[test]
    fn switches_and_remembers() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ai.json");
        let b = Backend::new(CliRunner::new(dir.path().join("work")), file.clone(), dir.path().join("tmp"));
        assert_eq!(b.status().provider, "claude_code");

        let ep = Endpoint { base_url: "http://localhost:9/v1".into(), model: "m".into(), ..Default::default() };
        b.set_choice(AiChoice { provider: "endpoint".into(), endpoint: ep }).unwrap();
        let st = b.status();
        assert_eq!((st.provider.as_str(), st.logged_in, st.on_subscription), ("endpoint", true, false));

        let again = Backend::new(CliRunner::new(dir.path().join("work")), file, dir.path().join("tmp"));
        assert!(again.choice().uses_endpoint());
        assert_eq!(again.status().version.as_deref(), Some("m"));
    }

    #[test]
    fn half_set_up_endpoint_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let b = Backend::new(CliRunner::new(dir.path().join("work")), dir.path().join("ai.json"), dir.path().join("tmp"));
        b.set_choice(AiChoice { provider: "endpoint".into(), endpoint: Endpoint::default() }).unwrap();
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
