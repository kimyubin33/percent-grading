//! Two fixed-origin API adapters. Models produce structured judgments, never grades.
//!
//! Protocol references (checked 2026-11-12):
//! https://developers.openai.com/api/docs/guides/structured-outputs
//! https://platform.claude.com/docs/en/build-with-claude/structured-outputs
//! https://platform.claude.com/docs/en/api/versioning

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{env, fmt, io::Read, str::FromStr, thread, time::Duration};

const CLAUDE_API_VERSION: &str = "2026-10-21";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Openai,
    Claude,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Claude => "claude",
        }
    }

    pub fn key_env(self) -> &'static str {
        match self {
            Self::Openai => "OPENAI_API_KEY",
            Self::Claude => "ANTHROPIC_API_KEY",
        }
    }

    pub fn endpoint(self) -> &'static str {
        match self {
            Self::Openai => "https://api.openai.com/v1/responses",
            Self::Claude => "https://api.anthropic.com/v1/messages",
        }
    }
}

impl fmt::Display for Provider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Provider {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "openai" => Ok(Self::Openai),
            "claude" => Ok(Self::Claude),
            _ => Err("provider must be exactly 'openai' or 'claude'".into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub timeout_seconds: u64,
    pub max_output_tokens: u32,
    pub max_response_bytes: usize,
    /// Includes the first attempt. A timeout can occur after a billable request.
    pub max_attempts: u8,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout_seconds: 90,
            max_output_tokens: 8192,
            max_response_bytes: 2 * 1024 * 1024,
            max_attempts: 1,
        }
    }
}

impl Limits {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=600).contains(&self.timeout_seconds) {
            return Err("timeout_seconds must be between 1 and 600".into());
        }
        if !(128..=32768).contains(&self.max_output_tokens) {
            return Err("max_output_tokens must be between 128 and 32768".into());
        }
        if !(1024..=8 * 1024 * 1024).contains(&self.max_response_bytes) {
            return Err("max_response_bytes must be between 1024 and 8388608".into());
        }
        if !(1..=3).contains(&self.max_attempts) {
            return Err("max_attempts must be between 1 and 3, including the first request".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub text: String,
    /// Provider response only; request headers and credentials are never included.
    pub raw: Value,
    pub usage: Value,
    pub attempts: u8,
}

/// Build the exact stateless request body, also used by offline protocol tests.
pub fn request_body(
    provider: Provider,
    model: &str,
    system: &str,
    user: &str,
    schema: &Value,
    limits: &Limits,
) -> Result<Value, String> {
    limits.validate()?;
    if model.is_empty()
        || model.len() > 200
        || !model
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:/".contains(&b))
    {
        return Err("model must be an explicit nonempty API model identifier".into());
    }
    if system.trim().is_empty() || user.trim().is_empty() {
        return Err("system and user messages must be nonempty".into());
    }
    if schema.get("type").and_then(Value::as_str) != Some("object") {
        return Err("judgment schema must have an object root".into());
    }
    // No tools, sampling overrides, conversation state, custom hosts, or model fallback.
    Ok(match provider {
        Provider::Openai => json!({
            "model": model,
            "instructions": system,
            "input": [{"role": "user", "content": [{"type": "input_text", "text": user}]}],
            "max_output_tokens": limits.max_output_tokens,
            "store": false,
            "stream": false,
            "text": {"format": {
                "type": "json_schema", "name": "percent_grading_judgment",
                "strict": true, "schema": schema
            }}
        }),
        Provider::Claude => json!({
            "model": model,
            "system": system,
            "messages": [{"role": "user", "content": user}],
            "max_tokens": limits.max_output_tokens,
            "stream": false,
            "output_config": {"format": {"type": "json_schema", "schema": schema}}
        }),
    })
}

/// Extract only completed, non-refusal assistant text. Validation of the grading
/// contract and evidence belongs to the deterministic engine, not this transport.
pub fn extract_text(provider: Provider, raw: &Value) -> Result<String, String> {
    if !raw.is_object() || raw.get("error").is_some_and(|e| !e.is_null()) {
        return Err(format!("{provider}: invalid or error response"));
    }
    let mut text = String::new();
    match provider {
        Provider::Openai => {
            if raw.get("status").and_then(Value::as_str) != Some("completed") {
                return Err("openai: response was not completed; no judgment accepted".into());
            }
            let output = raw
                .get("output")
                .and_then(Value::as_array)
                .ok_or("openai: missing output array")?;
            for item in output {
                match item.get("type").and_then(Value::as_str) {
                    Some("reasoning") => continue,
                    Some("message") => {
                        if item.get("role").and_then(Value::as_str) != Some("assistant")
                            || item.get("status").and_then(Value::as_str) != Some("completed")
                        {
                            return Err("openai: incomplete or non-assistant message".into());
                        }
                        let content = item
                            .get("content")
                            .and_then(Value::as_array)
                            .ok_or("openai: missing message content")?;
                        for block in content {
                            match block.get("type").and_then(Value::as_str) {
                                Some("output_text") => text.push_str(
                                    block
                                        .get("text")
                                        .and_then(Value::as_str)
                                        .ok_or("openai: invalid text block")?,
                                ),
                                Some("refusal") => {
                                    return Err(
                                        "openai: model refused; no judgment accepted".into()
                                    );
                                }
                                _ => {
                                    return Err(
                                        "openai: unexpected output content; no judgment accepted"
                                            .into(),
                                    );
                                }
                            }
                        }
                    }
                    _ => {
                        return Err("openai: unexpected output item; no tools are permitted".into());
                    }
                }
            }
        }
        Provider::Claude => {
            if raw.get("type").and_then(Value::as_str) != Some("message")
                || raw.get("role").and_then(Value::as_str) != Some("assistant")
                || raw.get("stop_reason").and_then(Value::as_str) != Some("end_turn")
                || raw.get("stop_details").is_some_and(|v| !v.is_null())
            {
                return Err(
                    "claude: incomplete, refused, or unexpected response; no judgment accepted"
                        .into(),
                );
            }
            let content = raw
                .get("content")
                .and_then(Value::as_array)
                .ok_or("claude: missing content array")?;
            for block in content {
                match block.get("type").and_then(Value::as_str) {
                    Some("text") => text.push_str(
                        block
                            .get("text")
                            .and_then(Value::as_str)
                            .ok_or("claude: invalid text block")?,
                    ),
                    // Some models emit reasoning even without an explicit thinking request.
                    Some("thinking" | "redacted_thinking") => continue,
                    _ => return Err("claude: unexpected content; no tools are permitted".into()),
                }
            }
        }
    }
    if text.trim().is_empty() {
        return Err(format!("{provider}: empty assistant text"));
    }
    let parsed: Value = serde_json::from_str(&text)
        .map_err(|_| format!("{provider}: assistant text is not valid JSON"))?;
    if !parsed.is_object() {
        return Err(format!("{provider}: assistant JSON must be an object"));
    }
    Ok(text)
}

fn retryable_status(status: u16) -> bool {
    status == 429 || (500..=599).contains(&status)
}

fn credential_header(provider: Provider) -> Result<reqwest::header::HeaderValue, String> {
    let key = env::var(provider.key_env())
        .map_err(|_| format!("{} is required for live evaluation", provider.key_env()))?;
    validate_key(provider, &key)
}

fn validate_key(provider: Provider, key: &str) -> Result<reqwest::header::HeaderValue, String> {
    if key.trim().is_empty() || key != key.trim() {
        return Err(format!(
            "{} must contain a nonempty API key without surrounding whitespace",
            provider.key_env()
        ));
    }
    let mut header = reqwest::header::HeaderValue::from_str(&format!("Bearer {key}"))
        .map_err(|_| "API key contains invalid header characters".to_owned())?;
    header.set_sensitive(true);
    Ok(header)
}

/// Validate all required credentials before any provider can incur usage.
pub fn validate_credentials(provider: Provider) -> Result<(), String> {
    credential_header(provider).map(|_| ())
}

/// Perform at most limits.max_attempts requests to one of the two official APIs.
/// Transient retries can incur additional charges; default is one attempt.
/// API keys are read only after all request options have been validated.
pub fn judge(
    provider: Provider,
    model: &str,
    system: &str,
    user: &str,
    schema: &Value,
    limits: &Limits,
) -> Result<Response, String> {
    let body = request_body(provider, model, system, user, schema, limits)?;
    let authorization = credential_header(provider)?;
    // The executable selects ring explicitly so builds do not need AWS-LC's CMake toolchain.
    // A concurrent call may have installed the same process-wide provider already.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let client = Client::builder()
        .timeout(Duration::from_secs(limits.timeout_seconds))
        .connect_timeout(Duration::from_secs(limits.timeout_seconds.min(15)))
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .https_only(true)
        .no_proxy()
        .build()
        .map_err(|_| "could not initialize HTTPS client".to_owned())?;

    for attempt in 1..=limits.max_attempts {
        let mut request = client
            .post(provider.endpoint())
            .header(reqwest::header::AUTHORIZATION, authorization.clone())
            .json(&body);
        if provider == Provider::Claude {
            request = request.header("anthropic-version", CLAUDE_API_VERSION);
        }
        let response = match request.send() {
            Ok(response) => response,
            Err(error) => {
                let transient = error.is_connect() || error.is_timeout();
                if transient && attempt < limits.max_attempts {
                    thread::sleep(Duration::from_millis(250 * u64::from(attempt)));
                    continue;
                }
                return Err(format!(
                    "{provider}: HTTPS request failed after {attempt} attempt(s); credentials and server details omitted"
                ));
            }
        };
        let status = response.status().as_u16();
        if !response.status().is_success() {
            if retryable_status(status) && attempt < limits.max_attempts {
                let wait = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok());
                // Do not retry earlier than a long server-requested delay or block indefinitely.
                if wait.is_some_and(|seconds| seconds > 10) {
                    return Err(format!(
                        "{provider}: HTTP {status}; retry later as requested by the service"
                    ));
                }
                thread::sleep(Duration::from_millis(
                    wait.map(|s| s * 1000).unwrap_or(250 * u64::from(attempt)),
                ));
                continue;
            }
            return Err(format!(
                "{provider}: HTTP {status} after {attempt} attempt(s); response body omitted"
            ));
        }
        if response
            .content_length()
            .is_some_and(|n| n > limits.max_response_bytes as u64)
        {
            return Err(format!("{provider}: response exceeded byte limit"));
        }
        let mut bytes = Vec::new();
        response
            .take(limits.max_response_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| {
                format!("{provider}: response body could not be read; no judgment accepted")
            })?;
        if bytes.len() > limits.max_response_bytes {
            return Err(format!("{provider}: response exceeded byte limit"));
        }
        let raw: Value = serde_json::from_slice(&bytes)
            .map_err(|_| format!("{provider}: API response was not valid JSON"))?;
        let text = extract_text(provider, &raw)?;
        let usage = raw.get("usage").cloned().unwrap_or(Value::Null);
        return Ok(Response {
            text,
            raw,
            usage,
            attempts: attempt,
        });
    }
    Err("request attempt limit exhausted".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> Value {
        json!({"type":"object","properties":{"result":{"type":"string"}},
            "required":["result"],"additionalProperties":false})
    }

    fn openai(text: &str) -> Value {
        json!({"status":"completed","output":[{"type":"reasoning","summary":[]},
            {"type":"message","role":"assistant","status":"completed",
            "content":[{"type":"output_text","text":text}]}]})
    }

    fn claude(text: &str) -> Value {
        json!({"type":"message","role":"assistant","stop_reason":"end_turn",
            "stop_details":null,"content":[{"type":"text","text":text}]})
    }

    #[test]
    fn only_two_providers_are_accepted() {
        assert_eq!("openai".parse::<Provider>().unwrap(), Provider::Openai);
        assert_eq!("claude".parse::<Provider>().unwrap(), Provider::Claude);
        for invalid in [
            "anthropic",
            "gemini",
            "openrouter",
            "OpenAI",
            "",
            "http://localhost",
        ] {
            assert!(invalid.parse::<Provider>().is_err());
            assert!(serde_json::from_value::<Provider>(json!(invalid)).is_err());
        }
    }

    #[test]
    fn both_requests_bind_the_schema_and_disable_unrequested_features() {
        let limits = Limits::default();
        let a = request_body(
            Provider::Openai,
            "gpt-example",
            "system",
            "answer",
            &schema(),
            &limits,
        )
        .unwrap();
        let b = request_body(
            Provider::Claude,
            "claude-example",
            "system",
            "answer",
            &schema(),
            &limits,
        )
        .unwrap();
        assert_eq!(a["text"]["format"]["schema"], schema());
        assert_eq!(a["text"]["format"]["strict"], true);
        assert_eq!(a["store"], false);
        assert_eq!(b["output_config"]["format"]["schema"], schema());
        for body in [a, b] {
            assert_eq!(body["stream"], false);
            for key in [
                "tools",
                "temperature",
                "top_p",
                "top_k",
                "previous_response_id",
            ] {
                assert!(body.get(key).is_none());
            }
        }
    }

    #[test]
    fn valid_responses_return_exact_json_text() {
        let text = "{\"result\":\"supported\"}";
        assert_eq!(extract_text(Provider::Openai, &openai(text)).unwrap(), text);
        assert_eq!(extract_text(Provider::Claude, &claude(text)).unwrap(), text);
    }

    #[test]
    fn openai_refusal_partial_and_tool_calls_are_rejected() {
        let mut refusal = openai("{}");
        refusal["output"][1]["content"][0] = json!({"type":"refusal","refusal":"No"});
        assert!(extract_text(Provider::Openai, &refusal).is_err());
        let mut partial = openai("{}");
        partial["status"] = json!("incomplete");
        assert!(extract_text(Provider::Openai, &partial).is_err());
        let mut tool = openai("{}");
        tool["output"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type":"function_call"}));
        assert!(extract_text(Provider::Openai, &tool).is_err());
    }

    #[test]
    fn claude_refusal_truncation_and_tools_are_rejected() {
        for stop in [
            "refusal",
            "max_tokens",
            "tool_use",
            "pause_turn",
            "stop_sequence",
        ] {
            let mut response = claude("{}");
            response["stop_reason"] = json!(stop);
            assert!(extract_text(Provider::Claude, &response).is_err());
        }
        let mut refusal = claude("{}");
        refusal["stop_details"] = json!({"type":"refusal"});
        assert!(extract_text(Provider::Claude, &refusal).is_err());
        let mut tool = claude("{}");
        tool["content"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type":"tool_use"}));
        assert!(extract_text(Provider::Claude, &tool).is_err());
    }

    #[test]
    fn malformed_empty_and_non_object_json_are_rejected() {
        for text in ["", "not JSON", "```json\n{}\n```", "[]", "null", "{}{}"] {
            assert!(extract_text(Provider::Openai, &openai(text)).is_err());
            assert!(extract_text(Provider::Claude, &claude(text)).is_err());
        }
        assert!(extract_text(Provider::Openai, &json!({})).is_err());
        assert!(extract_text(Provider::Claude, &json!({})).is_err());
    }

    #[test]
    fn invalid_configuration_fails_before_credentials_or_network() {
        let mut limits = Limits {
            max_attempts: 0,
            ..Limits::default()
        };
        assert!(
            judge(
                Provider::Openai,
                "example",
                "system",
                "user",
                &schema(),
                &limits
            )
            .unwrap_err()
            .contains("max_attempts")
        );
        limits.max_attempts = 4;
        assert!(limits.validate().is_err());
        let limits = Limits::default();
        assert!(request_body(Provider::Openai, "", "system", "user", &schema(), &limits).is_err());
        assert!(
            request_body(
                Provider::Openai,
                "example",
                "system",
                "user",
                &json!({"type":"array"}),
                &limits
            )
            .is_err()
        );
    }

    #[test]
    fn credential_validation_rejects_whitespace_and_header_injection() {
        for provider in [Provider::Openai, Provider::Claude] {
            for invalid in ["", " ", " key", "key ", "secret\nheader"] {
                let error = validate_key(provider, invalid).unwrap_err();
                assert!(!error.contains("secret"));
            }
            assert!(
                validate_key(provider, "example-secret")
                    .unwrap()
                    .is_sensitive()
            );
        }
    }

    #[test]
    fn only_transient_statuses_are_retryable() {
        for status in [429, 500, 502, 503, 529, 599] {
            assert!(retryable_status(status));
        }
        for status in [200, 301, 307, 400, 401, 403, 404, 408, 422] {
            assert!(!retryable_status(status));
        }
    }
}
