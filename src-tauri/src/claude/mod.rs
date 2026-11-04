//! The Claude layer.
//!
//! SelfSemester never calls the Claude API itself and never handles Claude credentials. It runs the user's own,
//! unmodified Claude Code CLI (`claude -p`), which the user signed in to through Claude Code's own flow, so usage
//! comes out of the user's own Claude subscription. See CLAUDE.md "Claude access" for the rules this follows.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod cli;
pub mod output;

pub use cli::CliRunner;

/// Model tier by job (passed to `--model` as an alias, so it follows the latest model of each tier).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    /// Scope check, syllabus, exams, final-exam marking.
    Opus,
    /// Intake, source discovery, lessons, most marking.
    Sonnet,
    /// Tagging chunks, summaries, validation checks.
    Haiku,
}

impl Tier {
    pub fn alias(self) -> &'static str {
        match self {
            Tier::Opus => "opus",
            Tier::Sonnet => "sonnet",
            Tier::Haiku => "haiku",
        }
    }
}

/// One stage call: a prompt in, JSON matching `schema` out.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageRequest {
    pub tier: Tier,
    /// Short role/rules text. Replaces Claude Code's default system prompt, which keeps calls small.
    pub system: Option<String>,
    /// Sent on stdin, so it can be long.
    pub prompt: String,
    /// JSON Schema the answer must match.
    pub schema: Value,
    /// Built-in Claude Code tools the stage may use, e.g. `["WebSearch", "WebFetch"]`. Empty = no tools.
    #[serde(default)]
    pub tools: Vec<String>,
    /// Extra folders Claude Code may read (e.g. a submission's uploads). Passed as `--add-dir`.
    #[serde(default)]
    pub add_dirs: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_tokens: u64,
    pub web_searches: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageOutput {
    pub value: Value,
    pub usage: Usage,
    pub model: String,
}

/// What the app knows about the local Claude Code install. Deliberately holds no account details or tokens.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ClaudeStatus {
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub logged_in: bool,
    /// As reported by `claude auth status`, e.g. "claude.ai" (subscription) or an API-key method.
    pub auth_method: Option<String>,
    /// True when Claude Code is signed in with a Claude subscription, i.e. no per-use API billing.
    pub on_subscription: bool,
    pub problem: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("Claude Code isn't installed or couldn't be found")]
    NotInstalled,
    #[error("Claude Code isn't signed in. Open a terminal, run `claude`, and sign in with your Claude account")]
    NotSignedIn,
    /// The plan's usage limit was hit. The job should wait, not retry straight away.
    #[error("Claude usage limit reached{}", .retry_at.map(|t| format!(", resets at {t}")).unwrap_or_default())]
    UsageLimited {
        retry_at: Option<chrono::DateTime<chrono::Utc>>,
        message: String,
    },
    #[error("Claude Code timed out after {0} s")]
    Timeout(u64),
    #[error("Claude Code returned an error: {0}")]
    Claude(String),
    #[error("couldn't read Claude Code's answer: {0}")]
    BadOutput(String),
    #[error("request too large for the command line: {0}")]
    TooLarge(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl RunError {
    /// Worth retrying later without changing the request.
    pub fn is_transient(&self) -> bool {
        matches!(self, RunError::UsageLimited { .. } | RunError::Timeout(_) | RunError::Io(_))
    }
}

/// Anything that can run a stage. The CLI runner is the only real one; tests use fakes.
pub trait ClaudeRunner: Send + Sync {
    fn status(&self) -> ClaudeStatus;
    fn run(&self, req: &StageRequest) -> Result<StageOutput, RunError>;
}
