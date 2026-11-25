//! The request and answer shapes of the AI APIs a service can speak. Pure functions: `endpoint.rs` does the
//! sending. Every style gets the same `Ask` and gives back the same `Answer`, so the rest of the app doesn't care
//! which one a service uses.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Which kind of API a service speaks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Style {
    /// POST {base}/chat/completions. OpenAI and most others, LM Studio and llama.cpp included.
    #[default]
    OpenaiChat,
    /// POST {base}/responses
    OpenaiResponses,
    /// POST {base}/messages
    Anthropic,
    /// POST {base}/models/{model}:generateContent
    Gemini,
    /// POST {base}/api/chat, where base has no /v1 (http://localhost:11434)
    Ollama,
}

/// How to ask for JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonMode {
    Schema,
    Json,
    Prompt,
}

/// One request, in a form every style can be built from.
pub struct Ask<'a> {
    pub model: &'a str,
    pub system: &'a str,
    /// The conversation: ("user" | "assistant", text). Always starts with a user turn.
    pub turns: &'a [(String, String)],
    /// Pictures for the first user turn: (mime type, base64 data).
    pub images: &'a [(String, String)],
    /// PDFs (base64) for the first user turn. Only Anthropic and Gemini take them; the others ignore them.
    pub pdfs: &'a [String],
    pub schema: &'a Value,
    pub json_mode: JsonMode,
    /// Let the service search the web itself, where the API has a tool for it.
    pub web_search: bool,
    pub max_output: Option<u64>,
    /// Context window to ask for. Only Ollama takes it.
    pub context: Option<u64>,
}

/// What came back.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Answer {
    pub text: String,
    /// The answer stopped at the output limit.
    pub cut: bool,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_tokens: u64,
    pub web_searches: u64,
    /// The model the service says answered, if it says.
    pub model: Option<String>,
}

impl Style {
    /// Name for the settings page.
    pub fn label(self) -> &'static str {
        match self {
            Style::OpenaiChat => "OpenAI-style chat completions",
            Style::OpenaiResponses => "OpenAI Responses API",
            Style::Anthropic => "Anthropic Messages API",
            Style::Gemini => "Google Gemini API",
            Style::Ollama => "Ollama's own API",
        }
    }

    /// Path after the base address for one request.
    pub fn path(self, model: &str) -> String {
        match self {
            Style::OpenaiChat => "chat/completions".into(),
            Style::OpenaiResponses => "responses".into(),
            Style::Anthropic => "messages".into(),
            Style::Gemini => format!("models/{}:generateContent", model.trim_start_matches("models/")),
            Style::Ollama => "api/chat".into(),
        }
    }

    /// Path that lists models.
    pub fn models_path(self) -> Option<&'static str> {
        Some(if self == Style::Ollama { "api/tags" } else { "models" })
    }

    /// The header the API key goes in, and what comes before the key.
    pub fn key_header(self) -> (&'static str, &'static str) {
        match self {
            Style::Anthropic => ("x-api-key", ""),
            Style::Gemini => ("x-goog-api-key", ""),
            _ => ("Authorization", "Bearer "),
        }
    }

    /// Headers this style always needs.
    pub fn fixed_headers(self) -> &'static [(&'static str, &'static str)] {
        if self == Style::Anthropic { &[("anthropic-version", "2023-06-01")] } else { &[] }
    }

    /// Whether PDFs can go along with the request.
    pub fn reads_pdfs(self) -> bool {
        matches!(self, Style::Anthropic | Style::Gemini)
    }
}

fn data_url(mime: &str, data: &str) -> String {
    format!("data:{mime};base64,{data}")
}

/// Later turns, after the first user message (which carries the pictures and files).
fn rest<'a>(ask: &'a Ask) -> impl Iterator<Item = (&'a str, &'a str)> {
    ask.turns.iter().skip(1).map(|(r, t)| (r.as_str(), t.as_str()))
}

fn first_text<'a>(ask: &'a Ask) -> &'a str {
    ask.turns.first().map(|(_, t)| t.as_str()).unwrap_or("")
}

pub fn body(style: Style, ask: &Ask) -> Value {
    match style {
        Style::OpenaiChat => openai_chat(ask),
        Style::OpenaiResponses => responses(ask),
        Style::Anthropic => anthropic(ask),
        Style::Gemini => gemini(ask),
        Style::Ollama => ollama(ask),
    }
}

fn openai_chat(ask: &Ask) -> Value {
    let first = if ask.images.is_empty() {
        json!(first_text(ask))
    } else {
        let mut parts = vec![json!({ "type": "text", "text": first_text(ask) })];
        parts.extend(ask.images.iter().map(|(m, d)| json!({ "type": "image_url", "image_url": { "url": data_url(m, d) } })));
        Value::Array(parts)
    };
    let mut messages = vec![json!({ "role": "system", "content": ask.system }), json!({ "role": "user", "content": first })];
    messages.extend(rest(ask).map(|(r, t)| json!({ "role": r, "content": t })));
    let mut body = json!({ "model": ask.model, "stream": false, "messages": messages });
    if let Some(n) = ask.max_output {
        body["max_tokens"] = json!(n);
    }
    match ask.json_mode {
        JsonMode::Schema => {
            body["response_format"] = json!({ "type": "json_schema", "json_schema": { "name": "answer", "schema": ask.schema, "strict": false } })
        }
        JsonMode::Json => body["response_format"] = json!({ "type": "json_object" }),
        JsonMode::Prompt => {}
    }
    body
}

fn responses(ask: &Ask) -> Value {
    let mut first = vec![json!({ "type": "input_text", "text": first_text(ask) })];
    first.extend(ask.images.iter().map(|(m, d)| json!({ "type": "input_image", "image_url": data_url(m, d) })));
    let mut input = vec![json!({ "role": "user", "content": first })];
    input.extend(rest(ask).map(|(r, t)| {
        let kind = if r == "assistant" { "output_text" } else { "input_text" };
        json!({ "role": r, "content": [{ "type": kind, "text": t }] })
    }));
    let mut body = json!({ "model": ask.model, "instructions": ask.system, "input": input });
    if let Some(n) = ask.max_output {
        body["max_output_tokens"] = json!(n);
    }
    match ask.json_mode {
        JsonMode::Schema => body["text"] = json!({ "format": { "type": "json_schema", "name": "answer", "schema": ask.schema, "strict": false } }),
        JsonMode::Json => body["text"] = json!({ "format": { "type": "json_object" } }),
        JsonMode::Prompt => {}
    }
    if ask.web_search {
        body["tools"] = json!([{ "type": "web_search" }]);
    }
    body
}

fn anthropic(ask: &Ask) -> Value {
    // Documents and pictures go before the text, as Anthropic suggests.
    let mut first: Vec<Value> = ask
        .pdfs
        .iter()
        .map(|d| json!({ "type": "document", "source": { "type": "base64", "media_type": "application/pdf", "data": d } }))
        .collect();
    first.extend(ask.images.iter().map(|(m, d)| json!({ "type": "image", "source": { "type": "base64", "media_type": m, "data": d } })));
    first.push(json!({ "type": "text", "text": first_text(ask) }));
    let mut messages = vec![json!({ "role": "user", "content": first })];
    messages.extend(rest(ask).map(|(r, t)| json!({ "role": r, "content": t })));
    // The schema is in the system prompt; this API has no JSON setting to try first.
    let mut body = json!({ "model": ask.model, "system": ask.system, "max_tokens": ask.max_output.unwrap_or(16000), "messages": messages });
    if ask.web_search {
        body["tools"] = json!([{ "type": "web_search_20250305", "name": "web_search", "max_uses": 5 }]);
    }
    body
}

fn gemini(ask: &Ask) -> Value {
    let mut first = vec![json!({ "text": first_text(ask) })];
    first.extend(ask.images.iter().map(|(m, d)| json!({ "inline_data": { "mime_type": m, "data": d } })));
    first.extend(ask.pdfs.iter().map(|d| json!({ "inline_data": { "mime_type": "application/pdf", "data": d } })));
    let mut contents = vec![json!({ "role": "user", "parts": first })];
    contents.extend(rest(ask).map(|(r, t)| json!({ "role": if r == "assistant" { "model" } else { "user" }, "parts": [{ "text": t }] })));
    let mut body = json!({ "systemInstruction": { "parts": [{ "text": ask.system }] }, "contents": contents });
    let mut config = serde_json::Map::new();
    // Gemini's schema dialect differs from JSON Schema, so it only gets "answer in JSON". The search tool doesn't
    // allow even that.
    if !ask.web_search && ask.json_mode != JsonMode::Prompt {
        config.insert("responseMimeType".into(), json!("application/json"));
    }
    if let Some(n) = ask.max_output {
        config.insert("maxOutputTokens".into(), json!(n));
    }
    if !config.is_empty() {
        body["generationConfig"] = Value::Object(config);
    }
    if ask.web_search {
        body["tools"] = json!([{ "google_search": {} }]);
    }
    body
}

fn ollama(ask: &Ask) -> Value {
    let mut first = json!({ "role": "user", "content": first_text(ask) });
    if !ask.images.is_empty() {
        first["images"] = json!(ask.images.iter().map(|(_, d)| d).collect::<Vec<_>>());
    }
    let mut messages = vec![json!({ "role": "system", "content": ask.system }), first];
    messages.extend(rest(ask).map(|(r, t)| json!({ "role": r, "content": t })));
    let mut body = json!({ "model": ask.model, "stream": false, "messages": messages });
    match ask.json_mode {
        JsonMode::Schema => body["format"] = ask.schema.clone(),
        JsonMode::Json => body["format"] = json!("json"),
        JsonMode::Prompt => {}
    }
    let mut options = serde_json::Map::new();
    if let Some(n) = ask.max_output {
        options.insert("num_predict".into(), json!(n));
    }
    if let Some(n) = ask.context {
        options.insert("num_ctx".into(), json!(n));
    }
    if !options.is_empty() {
        body["options"] = Value::Object(options);
    }
    body
}

fn n(v: &Value, pointer: &str) -> u64 {
    v.pointer(pointer).and_then(Value::as_u64).unwrap_or(0)
}

fn s(v: &Value, pointer: &str) -> Option<String> {
    v.pointer(pointer).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string)
}

/// The "text" of each part in a list, joined.
fn join_text<'a>(parts: impl Iterator<Item = &'a Value>) -> String {
    parts.filter_map(|p| p["text"].as_str()).collect()
}

pub fn parse(style: Style, v: &Value) -> Result<Answer, String> {
    if let Some(error) = v.get("error").filter(|e| !e.is_null()) {
        let message = error["message"].as_str().or_else(|| error.as_str()).map(str::to_string);
        return Err(message.unwrap_or_else(|| error.to_string()));
    }
    let empty = vec![];
    let list = |p: &str| v.pointer(p).and_then(Value::as_array).unwrap_or(&empty);
    let a = match style {
        Style::OpenaiChat => Answer {
            text: match &v["choices"][0]["message"]["content"] {
                Value::String(t) => t.clone(),
                Value::Array(parts) => join_text(parts.iter()),
                _ => String::new(),
            },
            cut: v["choices"][0]["finish_reason"] == "length",
            input_tokens: n(v, "/usage/prompt_tokens"),
            output_tokens: n(v, "/usage/completion_tokens"),
            cache_tokens: n(v, "/usage/prompt_tokens_details/cached_tokens"),
            web_searches: 0,
            model: s(v, "/model"),
        },
        Style::OpenaiResponses => Answer {
            // `output_text` is only there when an SDK added it; the plain reply has the text inside `output`.
            text: v["output_text"].as_str().map(str::to_string).unwrap_or_else(|| {
                join_text(
                    list("/output")
                        .iter()
                        .filter(|o| o["type"] == "message")
                        .flat_map(|o| o["content"].as_array().into_iter().flatten())
                        .filter(|c| c["type"] == "output_text"),
                )
            }),
            cut: v["status"] == "incomplete",
            input_tokens: n(v, "/usage/input_tokens"),
            output_tokens: n(v, "/usage/output_tokens"),
            cache_tokens: n(v, "/usage/input_tokens_details/cached_tokens"),
            web_searches: list("/output").iter().filter(|o| o["type"] == "web_search_call").count() as u64,
            model: s(v, "/model"),
        },
        Style::Anthropic => Answer {
            text: join_text(list("/content").iter().filter(|c| c["type"] == "text")),
            cut: v["stop_reason"] == "max_tokens",
            input_tokens: n(v, "/usage/input_tokens"),
            output_tokens: n(v, "/usage/output_tokens"),
            cache_tokens: n(v, "/usage/cache_read_input_tokens") + n(v, "/usage/cache_creation_input_tokens"),
            web_searches: n(v, "/usage/server_tool_use/web_search_requests"),
            model: s(v, "/model"),
        },
        Style::Gemini => Answer {
            // Thinking models send their thoughts as parts too; skip those.
            text: join_text(list("/candidates/0/content/parts").iter().filter(|p| p["thought"] != true)),
            cut: v["candidates"][0]["finishReason"] == "MAX_TOKENS",
            input_tokens: n(v, "/usageMetadata/promptTokenCount"),
            output_tokens: n(v, "/usageMetadata/candidatesTokenCount"),
            cache_tokens: n(v, "/usageMetadata/cachedContentTokenCount"),
            web_searches: 0,
            model: s(v, "/modelVersion"),
        },
        Style::Ollama => Answer {
            text: v["message"]["content"].as_str().unwrap_or("").to_string(),
            cut: v["done_reason"] == "length",
            input_tokens: n(v, "/prompt_eval_count"),
            output_tokens: n(v, "/eval_count"),
            cache_tokens: 0,
            web_searches: 0,
            model: s(v, "/model"),
        },
    };
    if a.text.is_empty() && !a.cut {
        return Err("the service's answer had no text in it".into());
    }
    Ok(a)
}

pub fn parse_models(style: Style, v: &Value) -> Vec<String> {
    let empty = vec![];
    let list = |key: &str| v[key].as_array().unwrap_or(&empty).clone();
    let mut models: Vec<String> = match style {
        Style::Gemini => list("models")
            .iter()
            // Keep models that can write text; embedding models can't.
            .filter(|m| m["supportedGenerationMethods"].as_array().is_none_or(|ms| ms.iter().any(|x| x == "generateContent")))
            .filter_map(|m| m["name"].as_str())
            .map(|n| n.trim_start_matches("models/").to_string())
            .collect(),
        Style::Ollama => list("models").iter().filter_map(|m| m["name"].as_str()).map(str::to_string).collect(),
        _ => list("data").iter().filter_map(|m| m["id"].as_str()).map(str::to_string).collect(),
    };
    models.sort();
    models.dedup();
    models
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_headers() {
        assert_eq!(Style::OpenaiChat.path("gpt-4"), "chat/completions");
        assert_eq!(Style::OpenaiResponses.path("gpt-4"), "responses");
        assert_eq!(Style::Anthropic.path("claude-3"), "messages");
        assert_eq!(Style::Gemini.path("gemini-pro"), "models/gemini-pro:generateContent");
        assert_eq!(Style::Gemini.path("models/gemini-pro"), "models/gemini-pro:generateContent");
        assert_eq!(Style::Ollama.path("llama3"), "api/chat");

        assert_eq!(Style::OpenaiChat.models_path(), Some("models"));
        assert_eq!(Style::OpenaiResponses.models_path(), Some("models"));
        assert_eq!(Style::Anthropic.models_path(), Some("models"));
        assert_eq!(Style::Gemini.models_path(), Some("models"));
        assert_eq!(Style::Ollama.models_path(), Some("api/tags"));

        assert_eq!(Style::Anthropic.key_header(), ("x-api-key", ""));
        assert_eq!(Style::Gemini.key_header(), ("x-goog-api-key", ""));

        assert_eq!(Style::OpenaiChat.fixed_headers(), &[]);
        assert_eq!(Style::Anthropic.fixed_headers(), &[("anthropic-version", "2023-06-01")]);
    }

    #[test]
    fn openai_chat_body_with_image_and_schema() {
        let ask = Ask {
            model: "gpt-4",
            system: "Answer briefly.",
            turns: &[("user".to_string(), "Hello".to_string())],
            images: &[("image/png".to_string(), "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==".to_string())],
            pdfs: &[],
            schema: &json!({"type": "object", "properties": {"answer": {"type": "string"}}}),
            json_mode: JsonMode::Schema,
            web_search: false,
            max_output: Some(100),
            context: None,
        };

        let body = body(Style::OpenaiChat, &ask);
        assert_eq!(body["model"], "gpt-4");
        assert_eq!(body["max_tokens"], 100);
        assert_eq!(body["response_format"]["type"], "json_schema");
        assert_eq!(body["messages"][1]["content"][0]["type"], "text");
        assert_eq!(body["messages"][1]["content"][1]["type"], "image_url");
    }

    #[test]
    fn responses_body_uses_input_text_and_web_search() {
        let ask = Ask {
            model: "gpt-4",
            system: "Answer briefly.",
            turns: &[("user".to_string(), "Hello".to_string())],
            images: &[],
            pdfs: &[],
            schema: &json!({}),
            json_mode: JsonMode::Prompt,
            web_search: true,
            max_output: Some(100),
            context: None,
        };

        let body = body(Style::OpenaiResponses, &ask);
        assert_eq!(body["model"], "gpt-4");
        assert_eq!(body["max_output_tokens"], 100);
        assert_eq!(body["tools"][0]["type"], "web_search");
    }

    #[test]
    fn anthropic_body_puts_pdf_and_image_before_text() {
        let ask = Ask {
            model: "claude-3",
            system: "Answer briefly.",
            turns: &[("user".to_string(), "Hello".to_string())],
            images: &[("image/png".to_string(), "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==".to_string())],
            pdfs: &["JVBERi0xLjQKJdPr6eEK...".to_string()],
            schema: &json!({}),
            json_mode: JsonMode::Prompt,
            web_search: true,
            max_output: Some(100),
            context: None,
        };

        let body = body(Style::Anthropic, &ask);
        assert_eq!(body["model"], "claude-3");
        assert_eq!(body["max_tokens"], 100);
        assert_eq!(body["tools"][0]["type"], "web_search_20250305");
        assert_eq!(body["messages"][0]["content"][0]["type"], "document");
        assert_eq!(body["messages"][0]["content"][1]["type"], "image");
        assert_eq!(body["messages"][0]["content"][2]["type"], "text");
    }

    #[test]
    fn gemini_body() {
        let ask = Ask {
            model: "gemini-pro",
            system: "Answer briefly.",
            turns: &[("user".to_string(), "Hello".to_string())],
            images: &[("image/png".to_string(), "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==".to_string())],
            pdfs: &[],
            schema: &json!({}),
            json_mode: JsonMode::Json,
            web_search: false,
            max_output: Some(100),
            context: None,
        };

        let body = body(Style::Gemini, &ask);
        assert_eq!(body["systemInstruction"]["parts"][0]["text"], "Answer briefly.");
        assert_eq!(body["contents"][0]["role"], "user");
        assert_eq!(body["contents"][0]["parts"][1]["inline_data"]["mime_type"], "image/png");
        assert_eq!(body["generationConfig"]["responseMimeType"], "application/json");
    }

    #[test]
    fn ollama_body_sets_format_and_options() {
        let ask = Ask {
            model: "llama3",
            system: "Answer briefly.",
            turns: &[("user".to_string(), "Hello".to_string())],
            images: &[("image/png".to_string(), "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==".to_string())],
            pdfs: &[],
            schema: &json!({"type": "object", "properties": {"answer": {"type": "string"}}}),
            json_mode: JsonMode::Schema,
            web_search: false,
            max_output: Some(100),
            context: Some(2048),
        };

        let body = body(Style::Ollama, &ask);
        assert_eq!(body["model"], "llama3");
        assert_eq!(&body["format"], ask.schema);
        assert_eq!(body["options"]["num_predict"], 100);
        assert_eq!(body["options"]["num_ctx"], 2048);
        assert_eq!(body["messages"][1]["images"][0], "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==");
    }

    #[test]
    fn parses_each_style() {
        let openai_chat = json!({
            "choices": [{
                "message": {
                    "content": "Hello, world!"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "prompt_tokens_details": {
                    "cached_tokens": 2
                }
            },
            "model": "gpt-4"
        });

        let answer = parse(Style::OpenaiChat, &openai_chat).unwrap();
        assert_eq!(answer.text, "Hello, world!");
        assert!(!answer.cut);
        assert_eq!(answer.input_tokens, 10);
        assert_eq!(answer.output_tokens, 5);
        assert_eq!(answer.cache_tokens, 2);
        assert_eq!(answer.model.unwrap(), "gpt-4");

        let anthropic = json!({
            "content": [{
                "type": "text",
                "text": "Hello, world!"
            }],
            "stop_reason": "max_tokens",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5,
                "cache_read_input_tokens": 2,
                "cache_creation_input_tokens": 3
            },
            "model": "claude-3"
        });

        let answer = parse(Style::Anthropic, &anthropic).unwrap();
        assert_eq!(answer.text, "Hello, world!");
        assert!(answer.cut);
        assert_eq!(answer.input_tokens, 10);
        assert_eq!(answer.output_tokens, 5);
        assert_eq!(answer.cache_tokens, 5);
        assert_eq!(answer.model.unwrap(), "claude-3");

        let gemini = json!({
            "candidates": [{
                "content": {
                    "parts": [
                        {"text": "Hello, world!"}
                    ]
                },
                "finishReason": "MAX_TOKENS"
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 5,
                "cachedContentTokenCount": 2
            },
            "modelVersion": "gemini-pro"
        });

        let answer = parse(Style::Gemini, &gemini).unwrap();
        assert_eq!(answer.text, "Hello, world!");
        assert!(answer.cut);
        assert_eq!(answer.input_tokens, 10);
        assert_eq!(answer.output_tokens, 5);
        assert_eq!(answer.cache_tokens, 2);
        assert_eq!(answer.model.unwrap(), "gemini-pro");

        let ollama = json!({
            "message": {
                "content": "Hello, world!"
            },
            "done_reason": "length",
            "prompt_eval_count": 10,
            "eval_count": 5,
            "model": "llama3"
        });

        let answer = parse(Style::Ollama, &ollama).unwrap();
        assert_eq!(answer.text, "Hello, world!");
        assert!(answer.cut);
        assert_eq!(answer.input_tokens, 10);
        assert_eq!(answer.output_tokens, 5);
        assert_eq!(answer.model.unwrap(), "llama3");
    }

    #[test]
    fn parse_reports_errors() {
        let error_response = json!({
            "error": {
                "message": "bad key"
            }
        });

        let result = parse(Style::OpenaiChat, &error_response);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "bad key");

        let error_response2 = json!({
            "error": "nope"
        });

        let result = parse(Style::OpenaiChat, &error_response2);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "nope");
    }

    #[test]
    fn lists_models() {
        let openai_data = json!({
            "data": [
                {"id": "gpt-4"},
                {"id": "gpt-3.5-turbo"}
            ]
        });

        let models = parse_models(Style::OpenaiChat, &openai_data);
        assert_eq!(models, vec!["gpt-3.5-turbo", "gpt-4"]);

        let gemini_data = json!({
            "models": [
                {
                    "name": "models/gemini-pro",
                    "supportedGenerationMethods": ["generateContent"]
                },
                {
                    "name": "models/gemini-1.5-pro",
                    "supportedGenerationMethods": ["generateContent"]
                },
                {
                    "name": "models/gemini-1.5-flash",
                    "supportedGenerationMethods": ["otherMethod"]
                }
            ]
        });

        let models = parse_models(Style::Gemini, &gemini_data);
        assert_eq!(models, vec!["gemini-1.5-pro", "gemini-pro"]);

        let ollama_data = json!({
            "models": [
                {"name": "llama3"},
                {"name": "mistral"}
            ]
        });

        let models = parse_models(Style::Ollama, &ollama_data);
        assert_eq!(models, vec!["llama3", "mistral"]);
    }

    /// The follow-up asking for JSON again has to reach every API, not just some.
    #[test]
    fn later_user_turns_are_kept() {
        let turns = vec![
            ("user".to_string(), "first".to_string()),
            ("assistant".to_string(), "not json".to_string()),
            ("user".to_string(), "again, as JSON".to_string()),
        ];
        let schema = json!({});
        let ask = Ask {
            model: "m",
            system: "s",
            turns: &turns,
            images: &[],
            pdfs: &[],
            schema: &schema,
            json_mode: JsonMode::Prompt,
            web_search: false,
            max_output: None,
            context: None,
        };
        for style in [Style::OpenaiChat, Style::OpenaiResponses, Style::Anthropic, Style::Gemini, Style::Ollama] {
            let b = body(style, &ask).to_string();
            assert!(b.contains("again, as JSON") && b.contains("not json"), "{style:?}: {b}");
        }
        let g = body(Style::Gemini, &ask);
        assert_eq!(g["contents"][1]["role"], "model");
        assert_eq!(g["contents"][2]["role"], "user");
    }

    /// The plain Responses reply has no `output_text`; that's something the SDKs add.
    #[test]
    fn reads_a_plain_responses_reply() {
        let v = json!({
            "status": "completed",
            "model": "gpt-5",
            "output": [
                {"type": "web_search_call", "status": "completed"},
                {"type": "message", "content": [{"type": "output_text", "text": "{\"ok\":"}, {"type": "output_text", "text": "true}"}]}
            ],
            "usage": {"input_tokens": 12, "output_tokens": 4}
        });
        let a = parse(Style::OpenaiResponses, &v).unwrap();
        assert_eq!(a.text, "{\"ok\":true}");
        assert_eq!((a.web_searches, a.input_tokens, a.output_tokens), (1, 12, 4));
    }
}
