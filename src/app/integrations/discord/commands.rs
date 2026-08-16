//! Slash command definitions and interaction-response helpers for the Discord bot.
//!
//! Command *handling* (LLM wiring) lives on [`crate::integrations::discord::bot::DiscordBot`];
//! this module owns the static command definitions and the response shapes
//! used to talk to Discord's interaction callback endpoint.

use serde::{Deserialize, Serialize};

/// Interaction type for an application command (slash command).
/// See <https://docs.discord.com/developers/docs/interactions/receiving-and-responding#interaction-object>.
pub const APPLICATION_COMMAND: u8 = 2;

/// Slash command definition.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SlashCommand {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub options: Vec<CommandOption>,
}

/// Slash command option.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CommandOption {
    pub name: String,
    pub description: String,
    #[serde(rename = "type")]
    pub option_type: u8,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub choices: Vec<CommandChoice>,
}

/// Command choice for autocomplete/options.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CommandChoice {
    pub name: String,
    pub value: serde_json::Value,
}

/// Get the default slash commands for the bot.
pub fn get_default_commands() -> Vec<SlashCommand> {
    vec![
        SlashCommand {
            name: "chat".to_string(),
            description: "Chat with the LLM".to_string(),
            options: vec![CommandOption {
                name: "message".to_string(),
                description: "Your message to the LLM".to_string(),
                option_type: 3, // STRING
                required: true,
                choices: vec![],
            }],
        },
        SlashCommand {
            name: "summarize".to_string(),
            description: "Summarize text".to_string(),
            options: vec![CommandOption {
                name: "text".to_string(),
                description: "Text to summarize".to_string(),
                option_type: 3, // STRING
                required: true,
                choices: vec![],
            }],
        },
        SlashCommand {
            name: "code".to_string(),
            description: "Generate code".to_string(),
            options: vec![
                CommandOption {
                    name: "prompt".to_string(),
                    description: "What code to generate".to_string(),
                    option_type: 3, // STRING
                    required: true,
                    choices: vec![],
                },
                CommandOption {
                    name: "language".to_string(),
                    description: "Programming language".to_string(),
                    option_type: 3, // STRING
                    required: false,
                    choices: vec![
                        CommandChoice {
                            name: "Rust".to_string(),
                            value: serde_json::Value::String("rust".to_string()),
                        },
                        CommandChoice {
                            name: "Python".to_string(),
                            value: serde_json::Value::String("python".to_string()),
                        },
                        CommandChoice {
                            name: "JavaScript".to_string(),
                            value: serde_json::Value::String("javascript".to_string()),
                        },
                        CommandChoice {
                            name: "TypeScript".to_string(),
                            value: serde_json::Value::String("typescript".to_string()),
                        },
                        CommandChoice {
                            name: "Go".to_string(),
                            value: serde_json::Value::String("go".to_string()),
                        },
                    ],
                },
            ],
        },
        SlashCommand {
            name: "analyze".to_string(),
            description: "Analyze text/code".to_string(),
            options: vec![CommandOption {
                name: "content".to_string(),
                description: "Content to analyze".to_string(),
                option_type: 3, // STRING
                required: true,
                choices: vec![],
            }],
        },
    ]
}

/// Interaction response types.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum InteractionResponse {
    #[serde(rename = "4")]
    ChannelMessageWithSource { data: InteractionResponseData },
    #[serde(rename = "5")]
    DeferredChannelMessageWithSource {
        data: Option<InteractionResponseData>,
    },
}

/// Interaction response data.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InteractionResponseData {
    pub content: Option<String>,
    #[serde(default)]
    pub flags: u64,
}

/// EPHEMERAL flag for user-only messages.
pub const EPHEMERAL_FLAG: u64 = 1 << 6;

/// Create an ephemeral response.
pub fn ephemeral_response(content: String) -> InteractionResponse {
    InteractionResponse::ChannelMessageWithSource {
        data: InteractionResponseData {
            content: Some(content),
            flags: EPHEMERAL_FLAG,
        },
    }
}

/// Create a deferred response.
pub fn deferred_response() -> InteractionResponse {
    InteractionResponse::DeferredChannelMessageWithSource { data: None }
}

#[cfg(test)]
mod tests {
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
}
