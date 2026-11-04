//! Parsing what `claude -p --output-format json` prints, including usage-limit detection.

use super::{RunError, StageOutput, Usage};
use chrono::{DateTime, TimeZone, Utc};
use serde_json::Value;

/// Turn the CLI's JSON envelope into a stage result or an error.
pub fn parse_envelope(stdout: &str, stderr: &str, requested_model: &str) -> Result<StageOutput, RunError> {
    // The envelope is the last JSON object printed; anything before it is ignored.
    let env: Value = last_json_object(stdout).ok_or_else(|| {
        let text = if stdout.trim().is_empty() { stderr } else { stdout };
        if let Some(e) = limit_from_text(text) {
            return e;
        }
        RunError::BadOutput(snippet(text))
    })?;

    let is_error = env.get("is_error").and_then(Value::as_bool).unwrap_or(false);
    let subtype = env.get("subtype").and_then(Value::as_str).unwrap_or("");
    let result_text = env.get("result").and_then(Value::as_str).unwrap_or("");
    let api_status = env.get("api_error_status").and_then(Value::as_u64);

    if is_error || (subtype != "success" && !subtype.is_empty()) {
        if api_status == Some(429) {
            return Err(limit_from_text(result_text).unwrap_or(RunError::UsageLimited {
                retry_at: None,
                message: snippet(result_text),
            }));
        }
        if let Some(e) = limit_from_text(result_text) {
            return Err(e);
        }
        if api_status == Some(401) || mentions_login(result_text) {
            return Err(RunError::NotSignedIn);
        }
        let msg = if result_text.is_empty() { subtype.to_string() } else { snippet(result_text) };
        return Err(RunError::Claude(msg));
    }

    let value = match env.get("structured_output") {
        Some(v) if !v.is_null() => v.clone(),
        _ => serde_json::from_str::<Value>(result_text.trim())
            .map_err(|_| RunError::BadOutput(format!("no structured output; result was: {}", snippet(result_text))))?,
    };

    let usage = env.get("usage");
    let u = |k: &str| usage.and_then(|x| x.get(k)).and_then(Value::as_u64).unwrap_or(0);
    let model = env
        .get("modelUsage")
        .and_then(Value::as_object)
        .and_then(|m| m.keys().next().cloned())
        .unwrap_or_else(|| requested_model.to_string());

    Ok(StageOutput {
        value,
        model,
        usage: Usage {
            input_tokens: u("input_tokens"),
            output_tokens: u("output_tokens"),
            cache_tokens: u("cache_read_input_tokens") + u("cache_creation_input_tokens"),
            web_searches: usage
                .and_then(|x| x.pointer("/server_tool_use/web_search_requests"))
                .and_then(Value::as_u64)
                .unwrap_or(0),
            duration_ms: env.get("duration_ms").and_then(Value::as_u64).unwrap_or(0),
        },
    })
}

/// Recognise a usage-limit message. Handles the `...limit reached|<unix seconds>` form and plain-text forms
/// like "You've hit your limit · resets 3pm".
pub fn limit_from_text(text: &str) -> Option<RunError> {
    let lower = text.to_lowercase();
    let is_limit = (lower.contains("limit") && (lower.contains("usage") || lower.contains("resets") || lower.contains("hit your")))
        || lower.contains("rate limit")
        || lower.contains("rate_limit");
    if !is_limit {
        return None;
    }
    let retry_at = text
        .split('|')
        .nth(1)
        .and_then(|s| s.trim().split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|d| d.parse::<i64>().ok())
        .and_then(|secs| Utc.timestamp_opt(secs, 0).single());
    Some(RunError::UsageLimited { retry_at, message: snippet(text) })
}

fn mentions_login(text: &str) -> bool {
    let l = text.to_lowercase();
    l.contains("/login") || l.contains("not logged in") || l.contains("please log in") || l.contains("invalid api key")
}

fn last_json_object(s: &str) -> Option<Value> {
    let t = s.trim();
    if let Ok(v) = serde_json::from_str::<Value>(t) {
        if v.is_object() {
            return Some(v);
        }
    }
    t.lines().rev().find_map(|line| serde_json::from_str::<Value>(line.trim()).ok().filter(Value::is_object))
}

fn snippet(s: &str) -> String {
    let s = s.trim();
    if s.chars().count() > 400 { format!("{}…", s.chars().take(400).collect::<String>()) } else { s.to_string() }
}

/// When to try a usage-limited job again: just after the reported reset, or an increasing back-off.
pub fn next_attempt(retry_at: Option<DateTime<Utc>>, attempts: u32, now: DateTime<Utc>) -> DateTime<Utc> {
    match retry_at {
        Some(t) if t > now => t + chrono::Duration::minutes(1),
        _ => {
            let mins = 15i64 * 2i64.pow(attempts.saturating_sub(1).min(4)); // 15, 30, 60, 120, 240
            now + chrono::Duration::minutes(mins)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Trimmed from a real `claude -p --output-format json --json-schema ...` run (Claude Code 2.1.318).
    const OK: &str = r#"{"type":"result","subtype":"success","is_error":false,"api_error_status":null,"duration_ms":2105,
      "result":"{\"capital\":\"Paris\",\"confidence\":1}","structured_output":{"capital":"Paris","confidence":1},
      "usage":{"input_tokens":2,"cache_creation_input_tokens":3893,"cache_read_input_tokens":0,"output_tokens":79,
      "server_tool_use":{"web_search_requests":0,"web_fetch_requests":0}},
      "modelUsage":{"claude-haiku-5-5":{"inputTokens":2}}}"#;

    #[test]
    fn parses_success() {
        let out = parse_envelope(OK, "", "haiku").unwrap();
        assert_eq!(out.value["capital"], "Paris");
        assert_eq!(out.model, "claude-haiku-5-5");
        assert_eq!(out.usage.output_tokens, 79);
        assert_eq!(out.usage.cache_tokens, 3893);
        assert_eq!(out.usage.duration_ms, 2105);
    }

    #[test]
    fn falls_back_to_result_text() {
        let s = r#"{"type":"result","subtype":"success","is_error":false,"result":"{\"a\":1}"}"#;
        assert_eq!(parse_envelope(s, "", "haiku").unwrap().value["a"], 1);
    }

    #[test]
    fn detects_limit_with_timestamp() {
        let s = r#"{"type":"result","subtype":"success","is_error":true,"result":"Claude AI usage limit reached|1762300800"}"#;
        match parse_envelope(s, "", "opus") {
            Err(RunError::UsageLimited { retry_at: Some(t), .. }) => assert_eq!(t.timestamp(), 1762300800),
            other => panic!("expected usage limit, got {other:?}"),
        }
    }

    #[test]
    fn detects_limit_plain_text_and_429() {
        let s = r#"{"type":"result","is_error":true,"result":"You've hit your limit · resets 3pm"}"#;
        assert!(matches!(parse_envelope(s, "", "opus"), Err(RunError::UsageLimited { retry_at: None, .. })));
        let s = r#"{"type":"result","is_error":true,"api_error_status":429,"result":"Too many requests"}"#;
        assert!(matches!(parse_envelope(s, "", "opus"), Err(RunError::UsageLimited { .. })));
        // Non-JSON output on stderr only.
        assert!(matches!(parse_envelope("", "Claude usage limit reached", "opus"), Err(RunError::UsageLimited { .. })));
    }

    #[test]
    fn detects_signed_out_and_other_errors() {
        let s = r#"{"type":"result","is_error":true,"api_error_status":401,"result":"Invalid API key · Please run /login"}"#;
        assert!(matches!(parse_envelope(s, "", "opus"), Err(RunError::NotSignedIn)));
        let s = r#"{"type":"result","subtype":"error_max_turns","is_error":true,"result":""}"#;
        assert!(matches!(parse_envelope(s, "", "opus"), Err(RunError::Claude(m)) if m == "error_max_turns"));
        assert!(matches!(parse_envelope("garbage", "", "opus"), Err(RunError::BadOutput(_))));
    }

    #[test]
    fn backoff() {
        let now = Utc.timestamp_opt(1_000_000, 0).unwrap();
        assert_eq!(next_attempt(None, 1, now) - now, chrono::Duration::minutes(15));
        assert_eq!(next_attempt(None, 3, now) - now, chrono::Duration::minutes(60));
        assert_eq!(next_attempt(None, 99, now) - now, chrono::Duration::minutes(240));
        let reset = now + chrono::Duration::hours(2);
        assert_eq!(next_attempt(Some(reset), 1, now), reset + chrono::Duration::minutes(1));
    }
}
