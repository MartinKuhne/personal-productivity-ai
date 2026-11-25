//! Unit tests for the agent crate's user-facing text catalogue
//! ([`crate::strings`]).
//!
//! The point of these tests is the *contract* the catalogue exists to keep: the
//! message a module raises is the message the catalogue declares, and page copy
//! that travels in an HTTP response cannot split one.

use super::*;
use crate::config::{AgentConfigBuilder, LlmConfig};
use std::collections::HashMap;

/// A chat model with a caller-chosen API key and nothing else interesting.
fn chat_model(api_key: &str) -> LlmConfig {
    LlmConfig {
        model: "test-model".to_string(),
        api_url: "http://localhost:0".to_string(),
        api_key: api_key.to_string(),
        cost: Some(0),
        use_case: vec!["chat".to_string()],
    }
}

/// Takes the `String` error out of a `Result` while keeping a bespoke failure
/// message. `unwrap_err` demands `Ok: Debug`, which `&LlmConfig` does not
/// implement.
#[track_caller]
fn error_of(result: Result<&LlmConfig, String>, what: &str) -> String {
    match result {
        Ok(_) => panic!("expected an error: {what}"),
        Err(err) => err,
    }
}

#[test]
fn an_empty_model_map_reports_the_catalogued_no_models_message() {
    let cfg = AgentConfigBuilder::new().build();
    assert_eq!(
        error_of(cfg.select_chat_model(), "no configured models"),
        ERR_NO_MODELS_CONFIGURED,
        "the no-models branch must surface the catalogued text verbatim"
    );
}

/// The second no-models branch: a selected model name that is not in the map,
/// with no fallback candidates at all.
#[test]
fn an_unresolvable_selected_model_reports_the_catalogued_no_models_message() {
    let cfg = AgentConfigBuilder::new()
        .with_selected_chat_model(Some("missing".to_string()))
        .build();
    assert_eq!(
        error_of(cfg.select_chat_model(), "selected model is absent"),
        ERR_NO_MODELS_CONFIGURED,
        "both no-models branches must raise the same catalogued text"
    );
}

#[test]
fn an_unset_api_key_reports_the_catalogued_key_message() {
    let mut models = HashMap::new();
    models.insert("blank".to_string(), chat_model(""));
    let cfg = AgentConfigBuilder::new().with_models(models).build();
    assert_eq!(
        error_of(cfg.select_chat_model(), "empty api key"),
        ERR_API_KEY_INVALID,
        "an empty key must surface the catalogued text"
    );
}

/// The shipped placeholder key is treated as unset, so it raises the same text.
#[test]
fn the_placeholder_api_key_reports_the_catalogued_key_message() {
    let mut models = HashMap::new();
    models.insert("placeholder".to_string(), chat_model("your-api-key-here"));
    let cfg = AgentConfigBuilder::new().with_models(models).build();
    assert_eq!(
        error_of(cfg.select_chat_model(), "placeholder api key"),
        ERR_API_KEY_INVALID,
        "the template placeholder must surface the catalogued text"
    );
}

/// Every status-line reason phrase must be a single header-safe token: a `\r` or
/// `\n` here would let the callback server emit a split response.
#[test]
fn status_reason_phrases_cannot_split_an_http_response() {
    for (name, reason) in [
        ("REASON_OK", REASON_OK),
        ("REASON_METHOD_NOT_ALLOWED", REASON_METHOD_NOT_ALLOWED),
        ("REASON_NOT_FOUND", REASON_NOT_FOUND),
        ("REASON_BAD_REQUEST", REASON_BAD_REQUEST),
    ] {
        assert!(
            !reason.contains('\r') && !reason.contains('\n'),
            "{name} is written into the status line and must not contain a line break: {reason:?}"
        );
        assert!(
            !reason.is_empty(),
            "{name} must carry a reason phrase, got an empty string"
        );
    }
}

/// The body copy is what the user actually reads in the browser, so none of it
/// may be blank.
#[test]
fn callback_page_copy_is_not_empty() {
    for (name, message) in [
        ("MESSAGE_SUCCESS", MESSAGE_SUCCESS),
        ("MESSAGE_METHOD_REQUIRED", MESSAGE_METHOD_REQUIRED),
        ("MESSAGE_WAITING_FOR_CALLBACK", MESSAGE_WAITING_FOR_CALLBACK),
        ("MESSAGE_MISSING_CODE", MESSAGE_MISSING_CODE),
        ("MESSAGE_MISSING_STATE", MESSAGE_MISSING_STATE),
    ] {
        assert!(
            !message.trim().is_empty(),
            "{name} is shown to the user and must not be blank"
        );
    }
}

#[test]
fn error_messages_are_non_empty_and_free_of_trailing_whitespace() {
    for (name, message) in [
        ("ERR_NO_MODELS_CONFIGURED", ERR_NO_MODELS_CONFIGURED),
        ("ERR_API_KEY_INVALID", ERR_API_KEY_INVALID),
        ("ERR_JMAP_JSON_PARSE_PREFIX", ERR_JMAP_JSON_PARSE_PREFIX),
    ] {
        assert!(!message.is_empty(), "{name} must not be an empty message");
        assert_eq!(
            message,
            message.trim_end(),
            "{name} must not carry trailing whitespace: {message:?}"
        );
    }
}
