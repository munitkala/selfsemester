//! Runs the user's installed Claude Code CLI as a child process.

use super::output::parse_envelope;
use super::{ClaudeRunner, ClaudeStatus, RunError, StageOutput, StageRequest};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Windows' command line tops out near 32,767 characters; keep a margin for the other arguments.
const MAX_ARG_CHARS: usize = 24_000;

pub struct CliRunner {
    exe: Option<PathBuf>,
    /// Empty folder the CLI runs in, so no project CLAUDE.md or files are picked up.
    workdir: PathBuf,
    timeout: Duration,
}

impl CliRunner {
    pub fn new(workdir: PathBuf) -> Self {
        Self { exe: find_claude(), workdir, timeout: Duration::from_secs(20 * 60) }
    }

    pub fn with_exe(exe: PathBuf, workdir: PathBuf, timeout: Duration) -> Self {
        Self { exe: Some(exe), workdir, timeout }
    }

    fn command(&self, exe: &Path) -> Command {
        let mut cmd = Command::new(exe);
        cmd.current_dir(&self.workdir).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        cmd
    }

    /// Arguments for one stage. Kept minimal so every call is small and isolated:
    /// no user/project settings (so no hooks or plugins), no MCP servers, no slash commands, no saved session.
    /// (`--bare` would do this too, but it only allows API-key sign-in, so it can't use a subscription.)
    pub fn stage_args(req: &StageRequest) -> Result<Vec<String>, RunError> {
        let schema = serde_json::to_string(&req.schema).map_err(|e| RunError::BadOutput(e.to_string()))?;
        let system = req.system.clone().unwrap_or_default();
        if schema.len() + system.len() > MAX_ARG_CHARS {
            return Err(RunError::TooLarge(format!(
                "schema ({} chars) + system prompt ({} chars) exceed {MAX_ARG_CHARS}; move text into the prompt",
                schema.len(),
                system.len()
            )));
        }
        let mut args: Vec<String> = [
            "-p",
            "--output-format",
            "json",
            "--model",
            req.tier.alias(),
            "--json-schema",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        args.push(schema);
        args.push("--tools".into());
        args.push(req.tools.join(","));
        if !req.tools.is_empty() {
            // Non-interactive: pre-approve exactly the tools the stage was given, nothing else.
            args.push("--allowedTools".into());
            args.push(req.tools.join(","));
        }
        args.extend(
            ["--strict-mcp-config", "--setting-sources", "", "--no-session-persistence", "--disable-slash-commands"]
                .iter()
                .map(|s| s.to_string()),
        );
        for d in &req.add_dirs {
            args.push("--add-dir".into());
            args.push(d.clone());
        }
        if !system.is_empty() {
            args.push("--system-prompt".into());
            args.push(system);
        }
        Ok(args)
    }

    /// Run with stdin, enforcing the timeout. Returns (exit ok, stdout, stderr).
    fn exec(&self, exe: &Path, args: &[String], stdin: &str) -> Result<(bool, String, String), RunError> {
        let mut child = self.command(exe).args(args).spawn()?;
        {
            let mut input = child.stdin.take().expect("stdin piped");
            input.write_all(stdin.as_bytes())?;
        } // dropping closes stdin so the CLI starts
        let mut out = child.stdout.take().expect("stdout piped");
        let mut err = child.stderr.take().expect("stderr piped");
        let out_t = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = out.read_to_string(&mut s);
            s
        });
        let err_t = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = err.read_to_string(&mut s);
            s
        });
        let start = Instant::now();
        let status = loop {
            if let Some(st) = child.try_wait()? {
                break st;
            }
            if start.elapsed() > self.timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err(RunError::Timeout(self.timeout.as_secs()));
            }
            std::thread::sleep(Duration::from_millis(150));
        };
        Ok((status.success(), out_t.join().unwrap_or_default(), err_t.join().unwrap_or_default()))
    }

    fn simple(&self, exe: &Path, args: &[&str]) -> Option<String> {
        let out = self.command(exe).args(args).stdin(Stdio::null()).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    }
}

impl ClaudeRunner for CliRunner {
    fn status(&self) -> ClaudeStatus {
        let Some(exe) = &self.exe else {
            return ClaudeStatus {
                problem: Some("Claude Code isn't installed. Install it from https://claude.com/claude-code, then sign in.".into()),
                ..Default::default()
            };
        };
        let mut st = ClaudeStatus {
            installed: true,
            path: Some(exe.display().to_string()),
            version: self.simple(exe, &["--version"]),
            ..Default::default()
        };
        // Only `loggedIn` and `authMethod` are read; account details in this output are ignored and never stored.
        match self.simple(exe, &["auth", "status", "--json"]).and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()) {
            Some(v) => {
                st.logged_in = v.get("loggedIn").and_then(|x| x.as_bool()).unwrap_or(false);
                st.auth_method = v.get("authMethod").and_then(|x| x.as_str()).map(str::to_string);
                st.on_subscription = st.logged_in && st.auth_method.as_deref() == Some("claude.ai");
                if !st.logged_in {
                    st.problem = Some("Claude Code isn't signed in. Open a terminal, run `claude`, and sign in with your Claude account.".into());
                } else if !st.on_subscription {
                    st.problem = Some(
                        "Claude Code is signed in with an API key or cloud provider, which is billed per use. \
                         To use your Claude subscription instead, run `claude auth login` and pick your Claude account."
                            .into(),
                    );
                }
            }
            None => st.problem = Some("Couldn't read `claude auth status`. Update Claude Code and try again.".into()),
        }
        st
    }

    fn run(&self, req: &StageRequest) -> Result<StageOutput, RunError> {
        let exe = self.exe.as_ref().ok_or(RunError::NotInstalled)?;
        std::fs::create_dir_all(&self.workdir)?;
        let args = Self::stage_args(req)?;
        let (_ok, stdout, stderr) = self.exec(exe, &args, &req.prompt)?;
        parse_envelope(&stdout, &stderr, req.tier.alias())
    }
}

/// Find `claude` on PATH, then in the installer's default places (a GUI app's PATH can be shorter than a terminal's).
pub fn find_claude() -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) { &["claude.exe", "claude.cmd"] } else { &["claude"] };
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local").join("bin"));
        dirs.push(home.join(".claude").join("local"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(appdata).join("npm"));
    }
    dirs.iter().flat_map(|d| names.iter().map(move |n| d.join(n))).find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claude::Tier;
    use serde_json::json;

    fn req(tools: &[&str]) -> StageRequest {
        StageRequest {
            tier: Tier::Haiku,
            system: Some("You are a test.".into()),
            prompt: "hi".into(),
            schema: json!({"type":"object"}),
            tools: tools.iter().map(|s| s.to_string()).collect(),
            add_dirs: vec![],
        }
    }

    #[test]
    fn args_are_isolated_and_toolless_by_default() {
        let a = CliRunner::stage_args(&req(&[])).unwrap();
        let pos = |f: &str| a.iter().position(|x| x == f).unwrap();
        assert_eq!(a[pos("--model") + 1], "haiku");
        assert_eq!(a[pos("--tools") + 1], "");
        assert_eq!(a[pos("--setting-sources") + 1], "");
        assert!(a.contains(&"--strict-mcp-config".to_string()));
        assert!(a.contains(&"--no-session-persistence".to_string()));
        assert!(!a.contains(&"--allowedTools".to_string()));
        assert!(!a.contains(&"--bare".to_string())); // --bare would force API-key auth
    }

    #[test]
    fn args_allow_only_given_tools() {
        let a = CliRunner::stage_args(&req(&["WebSearch", "WebFetch"])).unwrap();
        let pos = |f: &str| a.iter().position(|x| x == f).unwrap();
        assert_eq!(a[pos("--tools") + 1], "WebSearch,WebFetch");
        assert_eq!(a[pos("--allowedTools") + 1], "WebSearch,WebFetch");
    }

    #[test]
    fn rejects_oversized_arguments() {
        let mut r = req(&[]);
        r.system = Some("x".repeat(MAX_ARG_CHARS + 1));
        assert!(matches!(CliRunner::stage_args(&r), Err(RunError::TooLarge(_))));
    }

    #[test]
    fn missing_exe_reports_not_installed() {
        let r = CliRunner { exe: None, workdir: std::env::temp_dir(), timeout: Duration::from_secs(1) };
        assert!(!r.status().installed);
        assert!(matches!(r.run(&req(&[])), Err(RunError::NotInstalled)));
    }

    /// Real call against the local Claude Code install. Uses a little of the plan's usage, so it's opt-in:
    /// `cargo test -- --ignored live_`
    #[test]
    #[ignore]
    fn live_haiku_structured_call() {
        let dir = tempfile::tempdir().unwrap();
        let r = CliRunner::new(dir.path().to_path_buf());
        let st = r.status();
        assert!(st.installed && st.logged_in, "{st:?}");
        let out = r
            .run(&StageRequest {
                tier: Tier::Haiku,
                system: Some("Answer only with the requested JSON.".into()),
                prompt: "What is 2 + 3?".into(),
                schema: json!({"type":"object","properties":{"answer":{"type":"integer"}},"required":["answer"],"additionalProperties":false}),
                tools: vec![],
                add_dirs: vec![],
            })
            .unwrap();
        assert_eq!(out.value["answer"], 5);
    }
}
