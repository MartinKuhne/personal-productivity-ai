//! LLM HTTP client — builds requests, streams responses, parses tool calls, and extracts token-usage blocks from OpenAI/Anthropic APIs.
//!
//! Transport uses `async-openai` (with the `byot` feature for raw-JSON request/response
//! round-trips, preserving non-standard fields such as `reasoning_content`).
//! The agent loop remains synchronous: each `chat_completion` call drives the async
//! client on a process-wide tokio runtime via `std::sync::OnceLock<Runtime>`.

use crate::config::AgentConfig;
use crate::error::AgentError;
use crate::events::TokenUsageInfo;
use async_openai::{config::OpenAIConfig, error::OpenAIError, Client};

pub fn parse_usage_block(usage: &serde_json::Value) -> Option<TokenUsageInfo> {
    let prompt_tokens = usage
        .get("prompt_tokens")
        .and_then(|v| v.as_u64())
        .or_else(|| usage.get("input_tokens").and_then(|v| v.as_u64()));
    let completion_tokens = usage
        .get("completion_tokens")
        .and_then(|v| v.as_u64())
        .or_else(|| usage.get("output_tokens").and_then(|v| v.as_u64()));
    let total_tokens = usage.get("total_tokens").and_then(|v| v.as_u64());

    if prompt_tokens.is_none() && completion_tokens.is_none() && total_tokens.is_none() {
        return None;
    }

    let cached_tokens = usage
        .get("prompt_tokens_details")
        .and_then(|d| d.get("cached_tokens"))
        .and_then(|v| v.as_u64());
    let reasoning_tokens = usage
        .get("completion_tokens_details")
        .and_then(|d| d.get("reasoning_tokens"))
        .and_then(|v| v.as_u64());

    Some(TokenUsageInfo {
        prompt_tokens: prompt_tokens.unwrap_or(0),
        completion_tokens: completion_tokens.unwrap_or(0),
        total_tokens: total_tokens.unwrap_or_else(|| {
            prompt_tokens
                .unwrap_or(0)
                .saturating_add(completion_tokens.unwrap_or(0))
        }),
        cached_tokens,
        reasoning_tokens,
    })
}

static RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();

fn get_runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Runtime::new().expect("Failed to create tokio runtime for LLMClient")
    })
}

#[derive(Clone, Debug)]
pub struct LLMClient {
    api_url: String,
    api_key: String,
    model_name: String,
    max_tokens: u32,
}

impl LLMClient {
    pub fn from_agent_config(config: &AgentConfig, model_name: Option<&str>) -> Option<Self> {
        let model_cfg = if let Some(name) = model_name {
            config.models().get(name)?.clone()
        } else if let Some((_key, cfg)) = config
            .models()
            .iter()
            .filter(|(_, cfg)| cfg.has_use_case("chat"))
            .min_by_key(|(_, cfg)| cfg.get_cost())
        {
            cfg.clone()
        } else {
            config.models().values().next()?.clone()
        };
        Some(Self {
            api_url: model_cfg.api_url,
            api_key: model_cfg.api_key,
            model_name: model_cfg.model,
            max_tokens: config.max_tokens(),
        })
    }

    pub fn api_key_valid(&self) -> bool {
        self.api_key != "your-api-key-here" && !self.api_key.is_empty()
    }

    pub fn model_name(&self) -> &str {
        &self.model_name
    }

    pub fn max_tokens(&self) -> u32 {
        self.max_tokens
    }

    pub fn chat_completion(
        &self,
        messages: &[serde_json::Value],
        tools: &serde_json::Value,
    ) -> Result<serde_json::Value, AgentError> {
        let config = OpenAIConfig::new()
            .with_api_key(&self.api_key)
            .with_api_base(self.api_url.trim_matches('"').trim_end_matches('/'));

        let http_client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(60))
            .timeout(std::time::Duration::from_secs(1800))
            .build()
            .map_err(|e| AgentError::NetworkError(e.to_string()))?;

        let client = Client::build(http_client, config);

        let tools_value = if tools.as_array().map(|a| a.is_empty()).unwrap_or(true) {
            serde_json::Value::Null
        } else {
            tools.clone()
        };

        let body = serde_json::json!({
            "model": self.model_name,
            "messages": messages,
            "tools": tools_value,
            "tool_choice": "auto",
            "max_tokens": self.max_tokens
        });

        let response: serde_json::Value = retry_with_backoff(|| {
            get_runtime()
                .block_on(client.chat().create_byot(body.clone()))
                .map_err(map_openai_error)
        })?;

        Ok(response)
    }
}

fn map_openai_error(e: OpenAIError) -> AgentError {
    match e {
        OpenAIError::Reqwest(e) => {
            let err_str = e.to_string();
            let is_timeout = e.is_timeout()
                || e.is_connect()
                || err_str.contains("timed out")
                || err_str.contains("Timeout")
                || err_str.contains("Network is unreachable");
            if is_timeout {
                AgentError::Timeout
            } else {
                AgentError::NetworkError(err_str)
            }
        }
        OpenAIError::ApiError(api_err) => {
            let status = api_err.status_code.as_u16();
            let body = api_err.api_error.message.clone();
            if status >= 500 || status == 429 {
                tracing::warn!(
                    name = "agent.api.retry",
                    status = status,
                    "Retryable HTTP error from async-openai"
                );
            } else {
                tracing::error!(
                    name = "agent.api.failed",
                    status = status,
                    response = %body,
                    "Non-retryable HTTP error from async-openai"
                );
            }
            AgentError::HttpError { status, body }
        }
        OpenAIError::JSONDeserialize(_err, content) => {
            tracing::error!(
                name = "agent.api.invalid_json",
                content = %content,
                "Failed to parse API response."
            );
            AgentError::JsonParseError(content)
        }
        OpenAIError::InvalidArgument(msg) => {
            AgentError::RuntimeError(format!("Invalid argument: {msg}"))
        }
        e => AgentError::RuntimeError(e.to_string()),
    }
}

fn retry_with_backoff<F, T>(f: F) -> Result<T, AgentError>
where
    F: FnMut() -> Result<T, AgentError>,
{
    retry_with_backoff_and_sleep(f, std::time::Duration::from_secs(10), |ms| {
        std::thread::sleep(std::time::Duration::from_millis(ms))
    })
}

/// Backoff loop shared by [`retry_with_backoff`] with injectable
/// `total_timeout` and `sleep` so the retry/backoff/cap logic is
/// testable without waiting on real wall-clock sleeps.
fn retry_with_backoff_and_sleep<F, S, T>(
    mut f: F,
    total_timeout: std::time::Duration,
    mut sleep: S,
) -> Result<T, AgentError>
where
    F: FnMut() -> Result<T, AgentError>,
    S: FnMut(u64),
{
    let start = std::time::Instant::now();
    let mut delay_ms = 1000u64;
    let max_delay_ms = 8000u64;

    loop {
        match f() {
            Ok(val) => return Ok(val),
            Err(e) if e.is_retryable() && start.elapsed() < total_timeout => {
                tracing::warn!(
                    name = "agent.api.retry",
                    error = %e,
                    "Retrying after backoff"
                );
                sleep(delay_ms);
                delay_ms = std::cmp::min(delay_ms * 2, max_delay_ms);
            }
            Err(e) => return Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AgentConfigBuilder;

    #[test]
    fn test_parse_usage_openai() {
        let usage = serde_json::json!({
            "prompt_tokens": 123,
            "completion_tokens": 45,
            "total_tokens": 168,
            "prompt_tokens_details": { "cached_tokens": 10 },
            "completion_tokens_details": { "reasoning_tokens": 7 }
        });
        let info = parse_usage_block(&usage).unwrap();
        assert_eq!(info.prompt_tokens, 123);
        assert_eq!(info.completion_tokens, 45);
        assert_eq!(info.total_tokens, 168);
        assert_eq!(info.cached_tokens, Some(10));
        assert_eq!(info.reasoning_tokens, Some(7));
    }

    #[test]
    fn test_parse_usage_anthropic() {
        let usage = serde_json::json!({ "input_tokens": 200, "output_tokens": 50 });
        let info = parse_usage_block(&usage).unwrap();
        assert_eq!(info.prompt_tokens, 200);
        assert_eq!(info.completion_tokens, 50);
        assert_eq!(info.total_tokens, 250);
    }

    #[test]
    fn test_parse_usage_missing() {
        assert!(parse_usage_block(&serde_json::json!({})).is_none());
    }

    #[test]
    fn test_parse_usage_partial() {
        let usage = serde_json::json!({ "prompt_tokens": 1, "completion_tokens": 2 });
        let info = parse_usage_block(&usage).unwrap();
        assert_eq!(info.total_tokens, 3);
    }

    #[test]
    fn test_llm_client_api_key_valid() {
        let client = LLMClient {
            api_url: "http://localhost".to_string(),
            api_key: "real-key".to_string(),
            model_name: "test".to_string(),
            max_tokens: 32768,
        };
        assert!(client.api_key_valid());
    }

    #[test]
    fn test_llm_client_api_key_invalid_empty() {
        let client = LLMClient {
            api_url: "http://localhost".to_string(),
            api_key: "".to_string(),
            model_name: "test".to_string(),
            max_tokens: 32768,
        };
        assert!(!client.api_key_valid());
    }

    #[test]
    fn test_llm_client_api_key_invalid_default() {
        let client = LLMClient {
            api_url: "http://localhost".to_string(),
            api_key: "your-api-key-here".to_string(),
            model_name: "test".to_string(),
            max_tokens: 32768,
        };
        assert!(!client.api_key_valid());
    }

    #[test]
    fn test_llm_client_from_agent_config_includes_max_tokens() {
        let config = AgentConfigBuilder::new()
            .with_max_tokens(16384)
            .with_models(std::collections::HashMap::from([(
                "test_model".to_string(),
                crate::config::LlmConfig {
                    model: "gpt-4".to_string(),
                    api_url: "http://api.example.com".to_string(),
                    api_key: "test-key".to_string(),
                    cost: None,
                    use_case: vec!["chat".to_string()],
                },
            )]))
            .build();
        let client = LLMClient::from_agent_config(&config, None).unwrap();
        assert_eq!(client.max_tokens, 16384);
    }

    // ---- from_agent_config corner cases ----

    #[test]
    fn test_from_agent_config_model_name_not_present_returns_none() {
        let config = AgentConfigBuilder::new()
            .with_models(std::collections::HashMap::from([(
                "existing".to_string(),
                crate::config::LlmConfig {
                    model: "m1".to_string(),
                    api_url: "http://a".to_string(),
                    api_key: "k".to_string(),
                    cost: None,
                    use_case: vec!["chat".to_string()],
                },
            )]))
            .build();
        assert!(LLMClient::from_agent_config(&config, Some("missing")).is_none());
    }

    #[test]
    fn test_from_agent_config_empty_models_returns_none() {
        let config = AgentConfigBuilder::new()
            .with_models(std::collections::HashMap::new())
            .build();
        assert!(LLMClient::from_agent_config(&config, None).is_none());
        assert!(LLMClient::from_agent_config(&config, Some("x")).is_none());
    }

    #[test]
    fn test_from_agent_config_no_chat_use_case_falls_back_to_first() {
        let config = AgentConfigBuilder::new()
            .with_models(std::collections::HashMap::from([(
                "vision_only".to_string(),
                crate::config::LlmConfig {
                    model: "vision-model".to_string(),
                    api_url: "http://a".to_string(),
                    api_key: "k".to_string(),
                    cost: None,
                    use_case: vec!["vision".to_string()],
                },
            )]))
            .build();
        let client = LLMClient::from_agent_config(&config, None).unwrap();
        assert_eq!(client.model_name(), "vision-model");
    }

    #[test]
    fn test_from_agent_config_chooses_cheapest_chat_model() {
        let config = AgentConfigBuilder::new()
            .with_models(std::collections::HashMap::from([
                (
                    "cheap".to_string(),
                    crate::config::LlmConfig {
                        model: "cheap-model".to_string(),
                        api_url: "http://a".to_string(),
                        api_key: "k".to_string(),
                        cost: Some(1),
                        use_case: vec!["chat".to_string()],
                    },
                ),
                (
                    "expensive".to_string(),
                    crate::config::LlmConfig {
                        model: "expensive-model".to_string(),
                        api_url: "http://a".to_string(),
                        api_key: "k".to_string(),
                        cost: Some(9),
                        use_case: vec!["chat".to_string()],
                    },
                ),
            ]))
            .build();
        let client = LLMClient::from_agent_config(&config, None).unwrap();
        assert_eq!(client.model_name(), "cheap-model");
    }

    // ---- map_openai_error ----

    fn api_err(status: u16, message: &str) -> OpenAIError {
        OpenAIError::ApiError(async_openai::error::ApiErrorResponse {
            status_code: reqwest::StatusCode::from_u16(status).unwrap(),
            api_error: async_openai::error::ApiError {
                message: message.to_string(),
                r#type: None,
                param: None,
                code: None,
            },
        })
    }

    #[test]
    fn test_map_api_error_retryable_5xx() {
        let err = map_openai_error(api_err(500, "boom"));
        assert!(matches!(err, AgentError::HttpError { status: 500, .. }));
        assert!(err.is_retryable());
    }

    #[test]
    fn test_map_api_error_429_retryable() {
        let err = map_openai_error(api_err(429, "rate limited"));
        assert!(matches!(err, AgentError::HttpError { status: 429, .. }));
        assert!(err.is_retryable());
    }

    #[test]
    fn test_map_api_error_400_not_retryable() {
        let err = map_openai_error(api_err(400, "bad request"));
        assert!(matches!(err, AgentError::HttpError { status: 400, .. }));
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_map_json_deserialize_error() {
        let serde_err = serde_json::from_str::<serde_json::Value>("{not json}").unwrap_err();
        let err = map_openai_error(OpenAIError::JSONDeserialize(serde_err, "{not json}".into()));
        assert!(matches!(err, AgentError::JsonParseError(_)));
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_map_invalid_argument_is_runtime_error() {
        let err = map_openai_error(OpenAIError::InvalidArgument("bad model".into()));
        assert!(matches!(err, AgentError::RuntimeError(ref m) if m.contains("Invalid argument")));
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_map_catch_all_runtime_error() {
        let err = map_openai_error(OpenAIError::FileSaveError("disk full".into()));
        assert!(matches!(err, AgentError::RuntimeError(_)));
        assert!(!err.is_retryable());
    }

    // ---- retry_with_backoff_and_sleep ----

    fn retryable(msg: &str) -> AgentError {
        AgentError::NetworkError(msg.to_string())
    }

    #[test]
    fn test_backoff_first_success_no_retry() {
        let mut calls = 0;
        let mut sleeps = Vec::new();
        let result = retry_with_backoff_and_sleep(
            || {
                calls += 1;
                Ok(42)
            },
            std::time::Duration::from_secs(10),
            |ms| sleeps.push(ms),
        );
        assert_eq!(result.unwrap(), 42);
        assert_eq!(calls, 1);
        assert!(sleeps.is_empty(), "no sleep on first success");
    }

    #[test]
    fn test_backoff_non_retryable_error_returns_immediately() {
        let mut calls = 0;
        let result: Result<(), AgentError> = retry_with_backoff_and_sleep(
            || {
                calls += 1;
                Err(AgentError::MissingApiKey)
            },
            std::time::Duration::from_secs(10),
            |_| panic!("non-retryable error must not sleep"),
        );
        assert!(matches!(result, Err(AgentError::MissingApiKey)));
        assert_eq!(calls, 1);
    }

    #[test]
    fn test_backoff_retries_then_succeeds_with_backoff() {
        let mut calls = 0;
        let mut sleeps = Vec::new();
        let result = retry_with_backoff_and_sleep(
            || {
                calls += 1;
                if calls <= 3 {
                    Err(retryable("conn reset"))
                } else {
                    Ok("done")
                }
            },
            std::time::Duration::from_secs(10),
            |ms| sleeps.push(ms),
        );
        assert_eq!(result.unwrap(), "done");
        assert_eq!(calls, 4);
        assert_eq!(
            sleeps,
            vec![1000, 2000, 4000],
            "backoff doubles then succeeds"
        );
    }

    #[test]
    fn test_backoff_caps_at_max_delay() {
        let mut calls = 0;
        let mut sleeps = Vec::new();
        let result = retry_with_backoff_and_sleep(
            || {
                calls += 1;
                if calls < 8 {
                    Err(retryable("flaky"))
                } else {
                    Ok(())
                }
            },
            std::time::Duration::from_secs(60),
            |ms| sleeps.push(ms),
        );
        assert!(result.is_ok());
        assert_eq!(sleeps[2], 4000);
        assert_eq!(sleeps[3], 8000);
        assert_eq!(sleeps[4], 8000, "delay must cap at max_delay_ms");
        assert_eq!(sleeps.len(), 7);
    }

    #[test]
    fn test_backoff_stops_after_timeout_returns_last_error() {
        let mut calls = 0;
        let result: Result<(), AgentError> = retry_with_backoff_and_sleep(
            || {
                calls += 1;
                Err(retryable("never recovers"))
            },
            std::time::Duration::from_millis(10),
            |_ms| std::thread::sleep(std::time::Duration::from_millis(20)),
        );
        assert!(matches!(result, Err(AgentError::NetworkError(_))));
        assert!(
            calls >= 2,
            "must have attempted more than once before timeout"
        );
    }
}
