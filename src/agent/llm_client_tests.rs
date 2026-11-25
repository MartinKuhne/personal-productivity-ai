//! Tests for token-usage parsing, model selection, error mapping, system-message merging and retry backoff (`agent::llm_client`).

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

#[test]
fn test_from_agent_config_prefers_selected_chat_model_over_cheapest() {
    let config = AgentConfigBuilder::new()
        .with_selected_chat_model(Some("expensive".to_string()))
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
    assert_eq!(client.model_name(), "expensive-model");
}

#[test]
fn test_from_agent_config_explicit_model_name_overrides_selected_chat_model() {
    let config = AgentConfigBuilder::new()
        .with_selected_chat_model(Some("expensive".to_string()))
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
    let client = LLMClient::from_agent_config(&config, Some("cheap")).unwrap();
    assert_eq!(client.model_name(), "cheap-model");
}

#[test]
fn test_from_agent_config_invalid_selected_chat_model_falls_back_to_cheapest() {
    let config = AgentConfigBuilder::new()
        .with_selected_chat_model(Some("nonexistent".to_string()))
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

// ---- merge_leading_system_messages ----

fn sys(content: &str) -> serde_json::Value {
    serde_json::json!({"role": "system", "content": content})
}

#[test]
fn test_merge_single_system_untouched() {
    let msgs = vec![
        sys("a"),
        serde_json::json!({"role": "user", "content": "hi"}),
    ];
    let out = merge_leading_system_messages(&msgs);
    assert_eq!(out, msgs);
}

#[test]
fn test_merge_multiple_leading_systems_joined() {
    let msgs = vec![
        sys("static"),
        sys("dynamic"),
        sys("user-md"),
        serde_json::json!({"role": "user", "content": "hi"}),
    ];
    let out = merge_leading_system_messages(&msgs);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0]["role"], "system");
    assert_eq!(out[0]["content"], "static\n\ndynamic\n\nuser-md");
    assert_eq!(out[1]["role"], "user");
}

#[test]
fn test_merge_no_system_untouched() {
    let msgs = vec![serde_json::json!({"role": "user", "content": "hi"})];
    let out = merge_leading_system_messages(&msgs);
    assert_eq!(out, msgs);
}

#[test]
fn test_merge_empty_untouched() {
    let out = merge_leading_system_messages(&[]);
    assert!(out.is_empty());
}

#[test]
fn test_merge_non_leading_system_left_alone() {
    let msgs = vec![
        sys("first"),
        serde_json::json!({"role": "user", "content": "hi"}),
        sys("stray"),
    ];
    let out = merge_leading_system_messages(&msgs);
    assert_eq!(out, msgs, "only the leading run is merged");
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
