//! Tests for [`DiscordBot`] (`integrations::discord::bot`).

use super::*;

/// Verify that Role values map to the correct OpenAI role strings.
#[test]
fn test_role_to_openai_string() {
    assert_eq!(role_to_openai_string(Role::System), "system");
    assert_eq!(role_to_openai_string(Role::User), "user");
    assert_eq!(role_to_openai_string(Role::Assistant), "assistant");
}

/// Verify that a Vec<(Role, String)> is serialized to the OpenAI
/// message format with correct role mapping and content passthrough.
#[test]
fn test_messages_to_openai_format() {
    let messages = vec![
        (Role::System, "You are a helpful assistant.".to_string()),
        (Role::User, "Hello, bot!".to_string()),
        (Role::Assistant, "Hi there!".to_string()),
    ];
    let json = messages_to_openai(&messages);
    assert_eq!(json.len(), 3);
    assert_eq!(json[0]["role"], "system");
    assert_eq!(json[0]["content"], "You are a helpful assistant.");
    assert_eq!(json[1]["role"], "user");
    assert_eq!(json[1]["content"], "Hello, bot!");
    assert_eq!(json[2]["role"], "assistant");
    assert_eq!(json[2]["content"], "Hi there!");
}

/// Verify extract_llm_content pulls content from a standard OpenAI response.
#[test]
fn test_extract_llm_content_normal() {
    let resp = serde_json::json!({
        "choices": [{ "message": { "content": "Hello from LLM!" } }]
    });
    assert_eq!(extract_llm_content(&resp), "Hello from LLM!");
}

/// Verify extract_llm_content returns a fallback when choices is empty.
#[test]
fn test_extract_llm_content_empty_choices() {
    let resp = serde_json::json!({ "choices": [] });
    assert_eq!(
        extract_llm_content(&resp),
        "I received an empty response from the LLM."
    );
}

/// Verify extract_llm_content returns a fallback when content is null.
#[test]
fn test_extract_llm_content_null_content() {
    let resp = serde_json::json!({
        "choices": [{ "message": { "content": null } }]
    });
    assert_eq!(
        extract_llm_content(&resp),
        "I received an empty response from the LLM."
    );
}

/// Short content is returned as a single chunk.
#[test]
fn test_split_for_discord_short() {
    let chunks = split_for_discord("hello", 1900);
    assert_eq!(chunks, vec!["hello".to_string()]);
}

/// ASCII content is split on byte boundaries that align with char boundaries.
#[test]
fn test_split_for_discord_ascii_boundary() {
    let content = "0".repeat(5000);
    let chunks = split_for_discord(&content, 1900);
    assert_eq!(chunks.len(), 3);
    for c in &chunks {
        assert!(c.len() <= 1900);
    }
    assert_eq!(chunks.concat(), content);
}

/// UTF-8 content is never split in the middle of a multibyte codepoint:
/// concatenating the chunks reproduces the original string exactly.
#[test]
fn test_split_for_discord_multibyte_preserved() {
    // Each emoji is 4 bytes; force splits inside them.
    let content = "😀".repeat(1000); // 4000 bytes
    let chunks = split_for_discord(&content, 1900);
    assert!(chunks.len() > 1);
    for c in &chunks {
        assert!(c.len() <= 1900);
        // Every chunk must be valid UTF-8 (from_utf8 panics otherwise).
        std::str::from_utf8(c.as_bytes()).expect("chunk is valid UTF-8");
    }
    assert_eq!(chunks.concat(), content);
}

/// A split point that lands exactly on a char boundary needs no back-up.
#[test]
fn test_split_for_discord_boundary_exact() {
    let content = "abcDEF".to_string();
    let chunks = split_for_discord(&content, 3);
    assert_eq!(chunks, vec!["abc".to_string(), "DEF".to_string()]);
}

/// `build_command_prompt` reflects the documented slash-command shapes.
#[test]
fn test_build_command_prompt() {
    let opts = |name: &str, value: &str| {
        serde_json::json!([{
            "name": name,
            "type": 3,
            "value": value,
        }])
    };
    let mk = |name: &str, options: serde_json::Value| InteractionData {
        id: "x".to_string(),
        name: name.to_string(),
        options: Some(
            options
                .as_array()
                .unwrap()
                .iter()
                .map(|o| serde_json::from_value(o.clone()).unwrap())
                .collect(),
        ),
    };

    let chat = mk("chat", opts("message", "hi there"));
    assert_eq!(build_command_prompt(&chat), "hi there");

    let code = mk(
        "code",
        serde_json::json!([
            { "name": "prompt", "type": 3, "value": "hello world" },
            { "name": "language", "type": 3, "value": "python" },
        ]),
    );
    assert_eq!(
        build_command_prompt(&code),
        "Generate python code for: hello world"
    );
}

/// `new` fails fast without a bot token.
#[test]
fn test_new_requires_bot_token() {
    let config = DiscordConfig::default(); // bot_token None
    let app = AppConfig::default();
    assert!(DiscordBot::new(config, app).is_err());
}
