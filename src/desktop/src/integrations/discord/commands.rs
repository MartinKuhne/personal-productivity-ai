//! Slash command handling for Discord bot.

use crate::config::DiscordConfig;
use anyhow::Result;
use serde::{Deserialize, Serialize};

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

/// Handle a slash command interaction.
pub async fn handle_slash_command(
    interaction: &crate::integrations::discord::gateway::InteractionCreate,
    _config: &DiscordConfig,
) -> Result<InteractionResponse> {
    let data = interaction
        .data
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No interaction data"))?;

    match data.name.as_str() {
        "chat" => {
            let message = data
                .options
                .as_ref()
                .and_then(|opts| opts.iter().find(|o| o.name == "message"))
                .and_then(|o| o.value.as_ref())
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // This would forward to the LLM - for now return a placeholder
            Ok(ephemeral_response(format!("Chat: {}", message)))
        }
        "summarize" => {
            let text = data
                .options
                .as_ref()
                .and_then(|opts| opts.iter().find(|o| o.name == "text"))
                .and_then(|o| o.value.as_ref())
                .and_then(|v| v.as_str())
                .unwrap_or("");

            Ok(ephemeral_response(format!("Summary: {}", text)))
        }
        "code" => {
            let prompt = data
                .options
                .as_ref()
                .and_then(|opts| opts.iter().find(|o| o.name == "prompt"))
                .and_then(|o| o.value.as_ref())
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let language = data
                .options
                .as_ref()
                .and_then(|opts| opts.iter().find(|o| o.name == "language"))
                .and_then(|o| o.value.as_ref())
                .and_then(|v| v.as_str())
                .unwrap_or("rust");

            Ok(ephemeral_response(format!(
                "```{}\n// Code for: {}\n```",
                language, prompt
            )))
        }
        "analyze" => {
            let content = data
                .options
                .as_ref()
                .and_then(|opts| opts.iter().find(|o| o.name == "content"))
                .and_then(|o| o.value.as_ref())
                .and_then(|v| v.as_str())
                .unwrap_or("");

            Ok(ephemeral_response(format!("Analysis: {}", content)))
        }
        _ => Ok(ephemeral_response("Unknown command".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::discord::gateway::{
        InteractionCreate, InteractionData, InteractionOption,
    };

    fn create_mock_interaction(
        command_name: &str,
        options: Vec<(&str, &str)>,
    ) -> InteractionCreate {
        InteractionCreate {
            id: "interaction-123".to_string(),
            application_id: "app-123".to_string(),
            interaction_type: 2, // APPLICATION_COMMAND
            data: Some(InteractionData {
                id: "cmd-123".to_string(),
                name: command_name.to_string(),
                options: Some(
                    options
                        .into_iter()
                        .map(|(name, value)| InteractionOption {
                            name: name.to_string(),
                            option_type: 3, // STRING
                            value: Some(serde_json::Value::String(value.to_string())),
                        })
                        .collect(),
                ),
            }),
            guild_id: None,
            channel_id: "channel-123".to_string(),
            token: "token-123".to_string(),
            version: 1,
        }
    }

    #[tokio::test]
    async fn test_handle_chat_command() {
        let interaction = create_mock_interaction("chat", vec![("message", "Hello, world!")]);
        let config = DiscordConfig::default();

        let response = handle_slash_command(&interaction, &config).await.unwrap();

        match response {
            InteractionResponse::ChannelMessageWithSource { data } => {
                assert_eq!(data.flags, EPHEMERAL_FLAG);
                assert!(data.content.unwrap().contains("Chat: Hello, world!"));
            }
            _ => panic!("Expected ChannelMessageWithSource"),
        }
    }

    #[tokio::test]
    async fn test_handle_summarize_command() {
        let interaction =
            create_mock_interaction("summarize", vec![("text", "Long text to summarize")]);
        let config = DiscordConfig::default();

        let response = handle_slash_command(&interaction, &config).await.unwrap();

        match response {
            InteractionResponse::ChannelMessageWithSource { data } => {
                assert_eq!(data.flags, EPHEMERAL_FLAG);
                assert!(
                    data.content
                        .unwrap()
                        .contains("Summary: Long text to summarize")
                );
            }
            _ => panic!("Expected ChannelMessageWithSource"),
        }
    }

    #[tokio::test]
    async fn test_handle_code_command_with_language() {
        let interaction = create_mock_interaction(
            "code",
            vec![("prompt", "Create a hello world"), ("language", "python")],
        );
        let config = DiscordConfig::default();

        let response = handle_slash_command(&interaction, &config).await.unwrap();

        match response {
            InteractionResponse::ChannelMessageWithSource { data } => {
                assert_eq!(data.flags, EPHEMERAL_FLAG);
                let content = data.content.unwrap();
                assert!(content.contains("python"));
                assert!(content.contains("Create a hello world"));
            }
            _ => panic!("Expected ChannelMessageWithSource"),
        }
    }

    #[tokio::test]
    async fn test_handle_code_command_defaults_to_rust() {
        let interaction = create_mock_interaction("code", vec![("prompt", "Create a hello world")]);
        let config = DiscordConfig::default();

        let response = handle_slash_command(&interaction, &config).await.unwrap();

        match response {
            InteractionResponse::ChannelMessageWithSource { data } => {
                let content = data.content.unwrap();
                assert!(content.contains("rust"));
            }
            _ => panic!("Expected ChannelMessageWithSource"),
        }
    }

    #[tokio::test]
    async fn test_handle_analyze_command() {
        let interaction = create_mock_interaction("analyze", vec![("content", "Code to analyze")]);
        let config = DiscordConfig::default();

        let response = handle_slash_command(&interaction, &config).await.unwrap();

        match response {
            InteractionResponse::ChannelMessageWithSource { data } => {
                assert_eq!(data.flags, EPHEMERAL_FLAG);
                assert!(data.content.unwrap().contains("Analysis: Code to analyze"));
            }
            _ => panic!("Expected ChannelMessageWithSource"),
        }
    }

    #[tokio::test]
    async fn test_handle_unknown_command() {
        let interaction = create_mock_interaction("unknown", vec![]);
        let config = DiscordConfig::default();

        let response = handle_slash_command(&interaction, &config).await.unwrap();

        match response {
            InteractionResponse::ChannelMessageWithSource { data } => {
                assert_eq!(data.flags, EPHEMERAL_FLAG);
                assert_eq!(data.content.unwrap(), "Unknown command");
            }
            _ => panic!("Expected ChannelMessageWithSource"),
        }
    }

    #[tokio::test]
    async fn test_ephemeral_response_has_correct_flag() {
        let response = ephemeral_response("Test".to_string());

        match response {
            InteractionResponse::ChannelMessageWithSource { data } => {
                assert_eq!(data.flags, EPHEMERAL_FLAG);
                assert_eq!(data.content, Some("Test".to_string()));
            }
            _ => panic!("Expected ChannelMessageWithSource"),
        }
    }

    #[tokio::test]
    async fn test_get_default_commands_returns_all_commands() {
        let commands = get_default_commands();

        let names: Vec<&str> = commands.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["chat", "summarize", "code", "analyze"]);
    }

    #[tokio::test]
    async fn test_code_command_has_language_choices() {
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
