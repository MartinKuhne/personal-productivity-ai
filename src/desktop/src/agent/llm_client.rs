//! LLM HTTP client — builds requests, streams responses, parses tool calls, and extracts token-usage blocks from OpenAI/Anthropic APIs.

use crate::config::AppConfig;
use crate::error::AgentError;
use crate::messages::TokenUsageInfo;

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

pub struct LLMClient {
    api_url: String,
    api_key: String,
    model_name: String,
}

impl LLMClient {
    pub fn from_config(config: &AppConfig) -> Option<Self> {
        let model_cfg = if let Some((_key, cfg)) = config.model_for_use_case("chat") {
            cfg.clone()
        } else {
            config.models.values().next()?.clone()
        };
        Some(Self {
            api_url: model_cfg.api_url,
            api_key: model_cfg.api_key,
            model_name: model_cfg.model,
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
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(60))
            .timeout_read(std::time::Duration::from_secs(1800))
            .timeout(std::time::Duration::from_secs(1800))
            .build();

        let url = format!(
            "{}/chat/completions",
            self.api_url.trim_matches('"').trim_end_matches('/')
        );
        let body = serde_json::json!({
            "model": self.model_name,
            "messages": messages,
            "tools": tools,
            "tool_choice": "auto"
        });

        let max_retries = 3u32;
        let mut retry_attempt = 0u32;

        let response = loop {
            let result = agent
                .post(&url)
                .set("Authorization", &format!("Bearer {}", self.api_key))
                .set("Content-Type", "application/json")
                .send_json(body.clone());

            match result {
                Ok(resp) => break Ok(resp),
                Err(ureq::Error::Status(code, resp)) if code >= 500 || code == 429 => {
                    if retry_attempt < max_retries {
                        let delay = 1u64 << retry_attempt;
                        tracing::warn!(
                            name = "agent.api.retry",
                            status = code,
                            attempt = retry_attempt + 1,
                            delay_secs = delay,
                            "Retryable HTTP error, will retry"
                        );
                        std::thread::sleep(std::time::Duration::from_secs(delay));
                        retry_attempt += 1;
                        continue;
                    }
                    let body_str = resp
                        .into_string()
                        .unwrap_or_else(|_| "[Could not read body]".to_string());
                    tracing::error!(
                        name = "agent.api.failed",
                        status = code,
                        response = %body_str,
                        "Failed to get chat completion after all retries."
                    );
                    break Err(AgentError::HttpError {
                        status: code,
                        body: body_str,
                    });
                }
                Err(ureq::Error::Status(code, resp)) => {
                    let body_str = resp
                        .into_string()
                        .unwrap_or_else(|_| "[Could not read body]".to_string());
                    tracing::error!(
                        name = "agent.api.failed",
                        status = code,
                        response = %body_str,
                        "Failed to get chat completion."
                    );
                    break Err(AgentError::HttpError {
                        status: code,
                        body: body_str,
                    });
                }
                Err(ref e) => {
                    let err_str = e.to_string();
                    let is_timeout = err_str.contains("timed out")
                        || err_str.contains("Timeout")
                        || err_str.contains("Network is unreachable");
                    if is_timeout && retry_attempt < max_retries {
                        let delay = 1u64 << retry_attempt;
                        tracing::warn!(
                            name = "agent.api.retry",
                            error = %e,
                            attempt = retry_attempt + 1,
                            delay_secs = delay,
                            "Timeout, will retry"
                        );
                        std::thread::sleep(std::time::Duration::from_secs(delay));
                        retry_attempt += 1;
                        continue;
                    }
                    if is_timeout {
                        break Err(AgentError::Timeout);
                    }
                    break Err(AgentError::NetworkError(err_str));
                }
            }
        };

        response?.into_json().map_err(|e| {
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
        };
        assert!(client.api_key_valid());
    }

    #[test]
    fn test_llm_client_api_key_invalid_empty() {
        let client = LLMClient {
            api_url: "http://localhost".to_string(),
            api_key: "".to_string(),
            model_name: "test".to_string(),
        };
        assert!(!client.api_key_valid());
    }

    #[test]
    fn test_llm_client_api_key_invalid_default() {
        let client = LLMClient {
            api_url: "http://localhost".to_string(),
            api_key: "your-api-key-here".to_string(),
            model_name: "test".to_string(),
        };
        assert!(!client.api_key_valid());
    }
}
