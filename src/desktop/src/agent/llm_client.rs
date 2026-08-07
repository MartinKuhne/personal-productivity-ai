//! LLM HTTP client — builds requests, streams responses, parses tool calls, and extracts token-usage blocks from OpenAI/Anthropic APIs.

use crate::agent::error::AgentError;
use crate::bus::events::messages::TokenUsageInfo;
use crate::config::AppConfig;
use backon::BlockingRetryable;
use backon::ExponentialBuilder;

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

#[derive(Clone)]
pub struct LLMClient {
    api_url: String,
    api_key: String,
    model_name: String,
    max_tokens: u32,
}

impl LLMClient {
    pub fn from_config(config: &AppConfig, model_name: Option<&str>) -> Option<Self> {
        let model_cfg = if let Some(name) = model_name {
            config.models.get(name)?.clone()
        } else if let Some((_key, cfg)) = config.model_for_use_case("chat") {
            cfg.clone()
        } else {
            config.models.values().next()?.clone()
        };
        Some(Self {
            api_url: model_cfg.api_url,
            api_key: model_cfg.api_key,
            model_name: model_cfg.model,
            max_tokens: config.max_tokens,
        })
    }

    pub fn api_key_valid(&self) -> bool {
        self.api_key != "your-api-key-here" && !self.api_key.is_empty()
    }

    pub fn chat_completion(
        &self,
        messages: &[serde_json::Value],
        tools: &serde_json::Value,
    ) -> Result<serde_json::Value, AgentError> {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(60))
            .timeout(std::time::Duration::from_secs(1800))
            .build()
            .map_err(|e| AgentError::NetworkError(e.to_string()))?;

        let url = format!(
            "{}/chat/completions",
            self.api_url.trim_matches('"').trim_end_matches('/')
        );
        let body = serde_json::json!({
            "model": self.model_name,
            "messages": messages,
            "tools": tools,
            "tool_choice": "auto",
            "max_tokens": self.max_tokens
        });

        // reqwest's blocking client does not raise on 4xx/5xx by
        // default, so 2xx/4xx/5xx all land in `Ok(_)` and we branch on
        // the status code. This is *better* than the previous ureq
        // setup because the body is now available for diagnostic
        // logging on 5xx/429 retries (ureq 3.x's `StatusCode` error
        // dropped the body).
        let response = (|| {
            let result = client
                .post(&url)
                .bearer_auth(&self.api_key)
                .json(&body)
                .send();

            match result {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        Ok(resp)
                    } else {
                        let code = status.as_u16();
                        let body_str = resp
                            .text()
                            .unwrap_or_else(|_| "[Could not read body]".to_string());
                        if code >= 500 || code == 429 {
                            tracing::warn!(
                                name = "agent.api.retry",
                                status = code,
                                "Retryable HTTP error, will retry"
                            );
                        } else {
                            tracing::error!(
                                name = "agent.api.failed",
                                status = code,
                                response = %body_str,
                                "Non-retryable HTTP error."
                            );
                        }
                        Err(AgentError::HttpError {
                            status: code,
                            body: body_str,
                        })
                    }
                }
                Err(e) => {
                    let err_str = e.to_string();
                    // `reqwest::Error::is_timeout()` is the canonical
                    // check; we also match on substrings of the display
                    // form to keep behaviour aligned with the previous
                    // ureq-based classifier for cases that surface as
                    // connection-level errors (connect, read).
                    let is_timeout = e.is_timeout()
                        || e.is_connect()
                        || err_str.contains("timed out")
                        || err_str.contains("Timeout")
                        || err_str.contains("Network is unreachable");
                    if is_timeout {
                        tracing::warn!(
                            name = "agent.api.retry",
                            error = %err_str,
                            "Timeout, will retry"
                        );
                        Err(AgentError::Timeout)
                    } else {
                        Err(AgentError::NetworkError(err_str))
                    }
                }
            }
        })
        .retry(
            ExponentialBuilder::default()
                .with_factor(2.0)
                .with_min_delay(std::time::Duration::from_secs(1))
                .with_max_delay(std::time::Duration::from_secs(8))
                .with_total_delay(Some(std::time::Duration::from_secs(10))),
        )
        .when(|e: &AgentError| match e {
            AgentError::Timeout => true,
            AgentError::HttpError { status, .. } => *status >= 500 || *status == 429,
            _ => false,
        })
        .call()?;

        response.json().map_err(|e| {
            tracing::error!(
                name = "agent.api.invalid_json",
                error = %e,
                "Failed to parse JSON response."
            );
            AgentError::JsonParseError(e.to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_llm_client_from_config_includes_max_tokens() {
        let mut config = AppConfig::default();
        config.models.insert(
            "test_model".to_string(),
            crate::config::LlmConfig {
                model: "gpt-4".to_string(),
                api_url: "http://api.example.com".to_string(),
                api_key: "test-key".to_string(),
                cost: None,
                use_case: vec!["chat".to_string()],
            },
        );
        config.max_tokens = 16384;
        let client = LLMClient::from_config(&config, None).unwrap();
        assert_eq!(client.max_tokens, 16384);
    }
}
