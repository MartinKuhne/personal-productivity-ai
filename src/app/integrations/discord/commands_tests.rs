//! Tests for the Discord slash-command definitions and response helpers (`integrations::discord::commands`).

use super::*;

#[test]
fn test_ephemeral_response_has_correct_flag() {
    let response = ephemeral_response("Test".to_string());

    match response {
        InteractionResponse::ChannelMessageWithSource { data } => {
            assert_eq!(data.flags, EPHEMERAL_FLAG);
            assert_eq!(data.content, Some("Test".to_string()));
        }
        _ => panic!("Expected ChannelMessageWithSource"),
    }
}

#[test]
fn test_get_default_commands_returns_all_commands() {
    let commands = get_default_commands();

    let names: Vec<&str> = commands.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["chat", "summarize", "code", "analyze"]);
}

#[test]
fn test_code_command_has_language_choices() {
    let commands = get_default_commands();
    let code_cmd = commands.iter().find(|c| c.name == "code").unwrap();

    let lang_option = code_cmd
        .options
        .iter()
        .find(|o| o.name == "language")
        .unwrap();
    assert!(!lang_option.required);

    let choice_names: Vec<&str> = lang_option
        .choices
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(
        choice_names,
        vec!["Rust", "Python", "JavaScript", "TypeScript", "Go"]
    );
}
